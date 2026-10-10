//! The delivery bus: shared state between the webview (which reports frames
//! and gesture-triggered deliveries) and the MCP server (which serves them to
//! a connected agent). This is the whole coupling surface between the two —
//! the MCP server never touches settings or the filesystem.
//!
//! The webview composes a whole delivery (it knows the cameras, the
//! collection state, and which frame was hand-free); this side only stores,
//! sequences, and hands them out.

use crate::sessions::{Session, SessionStore, StartOutcome, Storage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::{broadcast, Notify};

/// A marked attention point in frame fractions (origin top-left).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FocusPoint {
    pub x: f64,
    pub y: f64,
}

/// One image in a delivery, with the context an agent needs to read it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub camera_name: String,
    pub timestamp_ms: u64,
    /// True when the frame chosen had no hand in it (so the drawing is not
    /// covered by the user's own hand).
    pub hand_free: bool,
    /// True when the snapshot is a crop of the camera's frame.
    pub cropped: bool,
    /// A marked attention point (a pointing gesture), if any.
    pub focus: Option<FocusPoint>,
    #[serde(skip)]
    pub jpeg: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryKind {
    Single,
    AllCameras,
    Collection,
}

/// The unit of input to the agent: one or more snapshots, from one or more
/// cameras, sent by one gesture.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub seq: u64,
    pub kind: DeliveryKind,
    pub timestamp_ms: u64,
    /// The gesture that triggered the delivery ("thumbs-up", "two-thumbs-up",
    /// "ok-sign").
    pub trigger: String,
    pub snapshots: Vec<Snapshot>,
    /// Cameras that were skipped (paused or disconnected) at trigger time.
    pub skipped_cameras: Vec<String>,
}

/// The latest low-rate frame of one camera, for `get_desk`.
#[derive(Clone, Debug)]
pub struct DeskFrame {
    pub camera_id: String,
    pub camera_name: String,
    pub timestamp_ms: u64,
    pub jpeg: Vec<u8>,
}

/// One time the agent read camera material. Broadcast so the control window
/// can show the user what the agent looked at, and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all_fields = "camelCase", tag = "what")]
pub enum AgentRead {
    /// `get_desk`: a look at the live desk, without a gesture from the user.
    #[serde(rename = "desk")]
    Desk { at_ms: u64 },
    /// A delivery the user sent was (re)read.
    #[serde(rename = "delivery")]
    Delivery { seq: u64, at_ms: u64 },
}

/// Why the session set changed, so the window can say so when it was the
/// agent and stay quiet when the user did it themselves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all_fields = "camelCase", tag = "source")]
pub enum SessionChange {
    #[serde(rename = "app")]
    App,
    #[serde(rename = "agent")]
    Agent { outcome: &'static str, name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraState {
    Watching,
    Paused,
    Disconnected,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureState {
    Watching,
    Paused,
    Disconnected,
    Idle,
}

#[derive(Default)]
struct Capture {
    cameras: HashMap<String, CameraState>,
    frames: Vec<DeskFrame>,
    stop_generation: u64,
    last_stop: Option<CaptureState>,
}

impl Capture {
    fn state(&self) -> CaptureState {
        let states: Vec<_> = self
            .cameras
            .values()
            .filter(|s| **s != CameraState::Removed)
            .collect();
        if states.contains(&&CameraState::Watching) {
            CaptureState::Watching
        } else if states.is_empty() {
            CaptureState::Idle
        } else if states.iter().all(|s| **s == CameraState::Paused) {
            CaptureState::Paused
        } else {
            CaptureState::Disconnected
        }
    }
}

pub enum WaitOutcome {
    Delivery(Delivery),
    Timeout,
    Stopped(CaptureState),
}

struct WaitRegistration<'a>(&'a AtomicUsize);
impl Drop for WaitRegistration<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorkState {
    Working,
    Completed,
    NeedsInput,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkProgress {
    pub state: WorkState,
    pub summary: String,
    pub at_ms: u64,
}

pub struct SketchBus {
    /// Sessions on disk; every delivery is written through to it.
    store: Mutex<SessionStore>,
    /// The current session's deliveries, oldest first — a cache of what the
    /// store holds for it, so serving an agent never touches the disk.
    deliveries: Mutex<Vec<Delivery>>,
    /// The latest frame per camera, in the order cameras first reported.
    capture: Mutex<Capture>,
    active_waits: AtomicUsize,
    receipts: Mutex<HashMap<u64, u64>>,
    work: Mutex<HashMap<u64, WorkProgress>>,
    delivered: Notify,
    /// Fires once per new delivery so the MCP server can emit
    /// `notifications/resources/list_changed` to connected hosts.
    list_changed: broadcast::Sender<()>,
    /// Fires when the session set or the current session changes, so the
    /// control window can refresh its switcher and history.
    sessions_changed: broadcast::Sender<SessionChange>,
    /// Fires on every agent read of camera material.
    agent_reads: broadcast::Sender<AgentRead>,
}

impl SketchBus {
    /// Opens the bus over the session store rooted at `sessions_dir`.
    pub fn new(sessions_dir: PathBuf) -> Self {
        let store = SessionStore::open(sessions_dir);
        let deliveries = store.current_deliveries();
        let (list_changed, _) = broadcast::channel(16);
        let (sessions_changed, _) = broadcast::channel(16);
        let (agent_reads, _) = broadcast::channel(16);
        Self {
            store: Mutex::new(store),
            deliveries: Mutex::new(deliveries),
            capture: Mutex::new(Capture::default()),
            active_waits: AtomicUsize::new(0),
            receipts: Mutex::new(HashMap::new()),
            work: Mutex::new(HashMap::new()),
            delivered: Notify::new(),
            list_changed,
            sessions_changed,
            agent_reads,
        }
    }

    /// Stores a delivery composed by the webview in the current session
    /// (opening one if needed), assigning it a sequence number. Returns the
    /// assigned seq.
    pub fn publish_delivery(
        &self,
        timestamp_ms: u64,
        kind: DeliveryKind,
        trigger: String,
        snapshots: Vec<Snapshot>,
        skipped_cameras: Vec<String>,
    ) -> u64 {
        let (delivery, opened_session) = {
            let mut store = self.store.lock().expect("bus poisoned");
            let before = store.current().map(|s| s.id);
            let delivery = store.append(timestamp_ms, kind, trigger, snapshots, skipped_cameras);
            let opened = before != store.current().map(|s| s.id);
            (delivery, opened)
        };
        let seq = delivery.seq;
        {
            let mut deliveries = self.deliveries.lock().expect("bus poisoned");
            if opened_session {
                deliveries.clear();
            }
            deliveries.push(delivery);
        }
        self.delivered.notify_waiters();
        // A lagging or absent receiver is fine: the notification is a hint.
        let _ = self.list_changed.send(());
        if opened_session {
            let _ = self.sessions_changed.send(SessionChange::App);
        }
        seq
    }

    /// Replaces the camera's latest frame; a camera seen for the first time
    /// is appended, so `get_desk` lists cameras in a stable order.
    pub fn publish_frame(&self, frame: DeskFrame) {
        let mut capture = self.capture.lock().expect("bus poisoned");
        // Unknown/removed cameras cannot repopulate the desk after pause/removal.
        if capture.cameras.get(&frame.camera_id) != Some(&CameraState::Watching) {
            return;
        }
        match capture
            .frames
            .iter_mut()
            .find(|f| f.camera_id == frame.camera_id)
        {
            Some(existing) => *existing = frame,
            None => capture.frames.push(frame),
        }
    }

    pub fn set_camera_state(&self, id: &str, state: CameraState) {
        let mut capture = self.capture.lock().expect("bus poisoned");
        let previous = capture.state();
        capture.cameras.insert(id.to_string(), state);
        if state != CameraState::Watching {
            capture.frames.retain(|f| f.camera_id != id);
        }
        let next = capture.state();
        if next != CaptureState::Watching && next != previous {
            capture.stop_generation += 1;
            capture.last_stop = Some(next);
        }
        drop(capture);
        self.delivered.notify_waiters();
    }

    /// A camera was removed: it must stop appearing in `get_desk`.
    pub fn retire_frame(&self, camera_id: &str) {
        self.set_camera_state(camera_id, CameraState::Removed);
    }

    pub fn capture_state(&self) -> CaptureState {
        self.capture.lock().expect("bus poisoned").state()
    }

    pub fn active_waits(&self) -> usize {
        self.active_waits.load(Ordering::SeqCst)
    }

    /// Tells the control window what the agent just read.
    pub fn record_agent_read(&self, read: AgentRead) {
        if let AgentRead::Delivery { seq, at_ms } = &read {
            self.receipts
                .lock()
                .expect("bus poisoned")
                .insert(*seq, *at_ms);
        }
        let _ = self.agent_reads.send(read);
    }

    pub fn acknowledge(
        &self,
        session_id: &str,
        seq: u64,
        state: WorkState,
        summary: &str,
        at_ms: u64,
    ) -> Result<(), String> {
        // Validate identity against storage so naming/adopting a session does not
        // cause us to acknowledge a different delivery or invent a missing one.
        let store = self.store.lock().expect("bus poisoned");
        if !store.deliveries_of(session_id).iter().any(|d| d.seq == seq) {
            return Err("delivery does not belong to this session".to_string());
        }
        let summary = summary.trim();
        if summary.is_empty() || summary.chars().count() > 240 {
            return Err("summary must contain 1–240 characters".to_string());
        }
        let mut work = self.work.lock().expect("bus poisoned");
        if work
            .get(&seq)
            .is_some_and(|w| w.state == WorkState::Completed)
            && state != WorkState::Completed
        {
            return Err(
                "this delivery is already completed; submit a new prompt for new work".to_string(),
            );
        }
        work.insert(
            seq,
            WorkProgress {
                state,
                summary: summary.to_string(),
                at_ms,
            },
        );
        Ok(())
    }

    pub fn status(&self, seq: Option<u64>) -> serde_json::Value {
        let receipts = self.receipts.lock().expect("bus poisoned");
        let work = self.work.lock().expect("bus poisoned");
        serde_json::json!({
            "captureState": self.capture_state(),
            "activeWaits": self.active_waits(),
            "receivedAtMs": seq.and_then(|s| receipts.get(&s).copied()),
            "work": seq.and_then(|s| work.get(&s)),
        })
    }

    pub fn latest_delivery(&self) -> Option<Delivery> {
        self.deliveries.lock().expect("bus poisoned").last().cloned()
    }

    pub fn delivery(&self, seq: u64) -> Option<Delivery> {
        self.deliveries
            .lock()
            .expect("bus poisoned")
            .iter()
            .find(|d| d.seq == seq)
            .cloned()
    }

    /// All deliveries of the current session, oldest first.
    pub fn deliveries(&self) -> Vec<Delivery> {
        self.deliveries.lock().expect("bus poisoned").clone()
    }

    pub fn latest_frames(&self) -> Vec<DeskFrame> {
        self.capture.lock().expect("bus poisoned").frames.clone()
    }

    /// The highest sequence number handed out so far, across all sessions.
    pub fn current_seq(&self) -> u64 {
        self.store.lock().expect("bus poisoned").last_seq()
    }

    pub fn subscribe_list_changed(&self) -> broadcast::Receiver<()> {
        self.list_changed.subscribe()
    }

    pub fn subscribe_sessions_changed(&self) -> broadcast::Receiver<SessionChange> {
        self.sessions_changed.subscribe()
    }

    pub fn subscribe_agent_reads(&self) -> broadcast::Receiver<AgentRead> {
        self.agent_reads.subscribe()
    }

    // ----- sessions -----

    /// The agent names the work; see `StartOutcome` for what that did.
    /// Returns the session that is current afterwards.
    pub fn start_session(&self, name: &str, now_ms: u64) -> Result<(StartOutcome, Session), String> {
        let (outcome, session) = {
            let mut store = self.store.lock().expect("bus poisoned");
            let outcome = store.start(name, now_ms)?;
            let session = store.current().expect("start leaves a session current");
            (outcome, session)
        };
        self.reload_current_with(SessionChange::Agent {
            outcome: outcome.label(),
            name: session.name.clone(),
        });
        Ok((outcome, session))
    }

    pub fn sessions(&self) -> Vec<Session> {
        self.store.lock().expect("bus poisoned").sessions()
    }

    pub fn current_session(&self) -> Option<Session> {
        self.store.lock().expect("bus poisoned").current()
    }

    pub fn session_deliveries(&self, id: &str) -> Vec<Delivery> {
        self.store.lock().expect("bus poisoned").deliveries_of(id)
    }

    pub fn switch_session(&self, id: &str) -> Result<(), String> {
        self.store.lock().expect("bus poisoned").switch(id)?;
        self.reload_current();
        Ok(())
    }

    /// The window opens an empty named session; see `SessionStore::open_new`.
    pub fn new_session(&self, name: &str, now_ms: u64) -> Result<(), String> {
        self.store.lock().expect("bus poisoned").open_new(name, now_ms)?;
        self.reload_current();
        Ok(())
    }

    pub fn rename_session(&self, id: &str, name: &str) -> Result<(), String> {
        self.store.lock().expect("bus poisoned").rename(id, name)?;
        let _ = self.sessions_changed.send(SessionChange::App);
        Ok(())
    }

    pub fn delete_session(&self, id: &str) -> Result<(), String> {
        self.store.lock().expect("bus poisoned").delete(id)?;
        self.reload_current();
        Ok(())
    }

    /// Removes one delivery from the current session.
    pub fn delete_delivery(&self, seq: u64) -> Result<(), String> {
        {
            let mut store = self.store.lock().expect("bus poisoned");
            let current = store.current().ok_or("no current session")?;
            store.delete_delivery(&current.id, seq)?;
        }
        self.reload_current();
        Ok(())
    }

    pub fn storage(&self) -> Storage {
        self.store.lock().expect("bus poisoned").storage()
    }

    /// Re-reads the current session's deliveries after the window changed
    /// the current session, and tells the window.
    fn reload_current(&self) {
        self.reload_current_with(SessionChange::App);
    }

    fn reload_current_with(&self, change: SessionChange) {
        let deliveries = self.store.lock().expect("bus poisoned").current_deliveries();
        *self.deliveries.lock().expect("bus poisoned") = deliveries;
        let _ = self.sessions_changed.send(change);
    }

    /// Registers for the lifetime of the future; cancellation drops the count.
    /// A stop generation preserves a quick pause/resume even before this wakes.
    pub async fn wait_for_delivery(&self, after_seq: u64, timeout: Duration) -> WaitOutcome {
        self.active_waits.fetch_add(1, Ordering::SeqCst);
        let _registration = WaitRegistration(&self.active_waits);
        let generation = self.capture.lock().expect("bus poisoned").stop_generation;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            // `notified()` only registers the waiter once polled, so a
            // publish landing between the check below and the await would be
            // lost; `enable()` registers interest eagerly, closing that gap.
            let mut notified = std::pin::pin!(self.delivered.notified());
            notified.as_mut().enable();
            {
                let capture = self.capture.lock().expect("bus poisoned");
                if capture.stop_generation != generation {
                    return WaitOutcome::Stopped(capture.last_stop.unwrap_or(CaptureState::Idle));
                }
                if capture.state() != CaptureState::Watching {
                    return WaitOutcome::Stopped(capture.state());
                }
            }
            // Preserve ordering if several prompts arrived between calls.
            if let Some(delivery) = self.deliveries().into_iter().find(|d| d.seq > after_seq) {
                return WaitOutcome::Delivery(delivery);
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return WaitOutcome::Timeout;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn snap(name: &str, bytes: Vec<u8>) -> Snapshot {
        Snapshot {
            camera_name: name.to_string(),
            timestamp_ms: 1000,
            hand_free: true,
            cropped: false,
            focus: None,
            jpeg: bytes,
        }
    }

    fn bus() -> (tempfile::TempDir, SketchBus) {
        let dir = tempfile::tempdir().unwrap();
        let bus = SketchBus::new(dir.path().join("sessions"));
        (dir, bus)
    }

    fn publish(bus: &SketchBus, kind: DeliveryKind, snaps: Vec<Snapshot>) -> u64 {
        bus.publish_delivery(1000, kind, "thumbs-up".to_string(), snaps, vec![])
    }

    #[tokio::test]
    async fn cancelling_a_wait_releases_listener_count() {
        let (_dir, bus) = bus();
        let bus = Arc::new(bus);
        bus.set_camera_state("a", CameraState::Watching);
        let task = {
            let bus = bus.clone();
            tokio::spawn(async move { bus.wait_for_delivery(0, Duration::from_secs(60)).await })
        };
        tokio::task::yield_now().await;
        assert_eq!(bus.active_waits(), 1);
        task.abort();
        let _ = task.await;
        assert_eq!(bus.active_waits(), 0);
    }

    #[tokio::test]
    async fn queued_prompts_are_delivered_in_order() {
        let (_dir, bus) = bus();
        bus.set_camera_state("a", CameraState::Watching);
        publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![2])]);
        assert!(
            matches!(bus.wait_for_delivery(0, Duration::from_secs(1)).await, WaitOutcome::Delivery(d) if d.seq == 1)
        );
    }

    #[test]
    fn acknowledgements_validate_identity_and_preserve_completion() {
        let (_dir, bus) = bus();
        let seq = publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        let id = bus.current_session().unwrap().id;
        assert!(bus
            .acknowledge("wrong", seq, WorkState::Working, "Reading the diagram", 1)
            .is_err());
        assert!(bus
            .acknowledge(&id, seq + 1, WorkState::Working, "Reading", 1)
            .is_err());
        assert!(bus
            .acknowledge(&id, seq, WorkState::Working, " ", 1)
            .is_err());
        bus.acknowledge(&id, seq, WorkState::Working, "Building the explanation", 1)
            .unwrap();
        bus.acknowledge(&id, seq, WorkState::Completed, "Explanation ready", 2)
            .unwrap();
        assert!(bus
            .acknowledge(&id, seq, WorkState::Working, "Repeat", 3)
            .is_err());
        assert_eq!(bus.status(Some(seq))["work"]["state"], "completed");
    }

    #[tokio::test]
    async fn pause_interrupts_wait_without_losing_history() {
        let (_dir, bus) = bus();
        let bus = Arc::new(bus);
        bus.set_camera_state("desk", CameraState::Watching);
        publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        let waiter = {
            let bus = bus.clone();
            tokio::spawn(async move { bus.wait_for_delivery(1, Duration::from_secs(60)).await })
        };
        tokio::task::yield_now().await;
        bus.set_camera_state("desk", CameraState::Paused);
        bus.set_camera_state("desk", CameraState::Watching);
        assert!(matches!(
            tokio::time::timeout(Duration::from_millis(100), waiter)
                .await
                .unwrap()
                .unwrap(),
            WaitOutcome::Stopped(CaptureState::Paused)
        ));
        assert_eq!(bus.deliveries().len(), 1);
    }

    #[test]
    fn pause_rejects_late_frames_and_preserves_other_camera() {
        let (_dir, bus) = bus();
        bus.set_camera_state("a", CameraState::Watching);
        bus.set_camera_state("b", CameraState::Watching);
        bus.publish_frame(frame("a", "Desk", 1));
        bus.publish_frame(frame("b", "Wall", 1));
        bus.set_camera_state("a", CameraState::Paused);
        bus.publish_frame(frame("a", "Desk", 2));
        assert_eq!(bus.latest_frames().len(), 1);
        assert_eq!(bus.capture_state(), CaptureState::Watching);
        bus.set_camera_state("b", CameraState::Paused);
        assert_eq!(bus.capture_state(), CaptureState::Paused);
        assert!(bus.latest_frames().is_empty());
        bus.set_camera_state("b", CameraState::Disconnected);
        assert_eq!(bus.capture_state(), CaptureState::Disconnected);
    }

    #[tokio::test]
    async fn waiter_receives_a_delivery_published_after_it_started_waiting() {
        let (_dir, bus) = bus();
        let bus = Arc::new(bus);
        bus.set_camera_state("desk", CameraState::Watching);
        let waiter = {
            let bus = bus.clone();
            tokio::spawn(async move { bus.wait_for_delivery(0, Duration::from_secs(5)).await })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1, 2])]);
        let WaitOutcome::Delivery(delivery) = waiter.await.unwrap() else {
            panic!("expected delivery");
        };
        assert_eq!(delivery.seq, 1);
        assert_eq!(delivery.snapshots[0].jpeg, vec![1, 2]);
    }

    #[tokio::test]
    async fn waiter_ignores_deliveries_it_has_already_seen() {
        let (_dir, bus) = bus();
        publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        bus.set_camera_state("desk", CameraState::Watching);
        let result = bus.wait_for_delivery(1, Duration::from_millis(50)).await;
        assert!(
            matches!(result, WaitOutcome::Timeout),
            "seq 1 was already seen; must time out"
        );
    }

    #[tokio::test]
    async fn waiter_returns_immediately_when_an_unseen_delivery_exists() {
        let (_dir, bus) = bus();
        publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        bus.set_camera_state("desk", CameraState::Watching);
        let result = bus.wait_for_delivery(0, Duration::from_millis(50)).await;
        assert!(matches!(result, WaitOutcome::Delivery(d) if d.seq == 1));
    }

    #[test]
    fn history_keeps_order_and_lookup_by_seq() {
        let (_dir, bus) = bus();
        let a = publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        let b = publish(&bus, DeliveryKind::AllCameras, vec![snap("Wall", vec![2])]);
        let all = bus.deliveries();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].seq, a);
        assert_eq!(all[1].seq, b);
        assert_eq!(bus.delivery(b).unwrap().kind, DeliveryKind::AllCameras);
        assert!(bus.delivery(999).is_none());
        assert_eq!(bus.latest_delivery().unwrap().seq, b);
    }

    #[test]
    fn switching_sessions_changes_what_the_bus_serves() {
        let (_dir, bus) = bus();
        let a = publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        bus.start_session("Kiosk", 2000).unwrap();
        bus.start_session("Menu", 3000).unwrap();
        assert!(bus.deliveries().is_empty(), "a fresh session has no deliveries");
        let b = publish(&bus, DeliveryKind::Single, vec![snap("Wall", vec![2])]);
        assert_eq!(b, a + 1, "seq is global across sessions");
        let kiosk = bus.sessions()[0].id.clone();
        bus.switch_session(&kiosk).unwrap();
        assert_eq!(bus.deliveries().len(), 1);
        assert_eq!(bus.latest_delivery().unwrap().seq, a);
        assert_eq!(bus.current_seq(), b);
    }

    fn frame(id: &str, name: &str, at: u64) -> DeskFrame {
        DeskFrame {
            camera_id: id.to_string(),
            camera_name: name.to_string(),
            timestamp_ms: at,
            jpeg: vec![0xff],
        }
    }

    /// AC5: one frame per camera in first-seen order; a retired camera is gone.
    #[test]
    fn desk_frames_are_kept_per_camera_and_retired_on_removal() {
        let (_dir, bus) = bus();
        bus.set_camera_state("a", CameraState::Watching);
        bus.set_camera_state("b", CameraState::Watching);
        bus.publish_frame(frame("a", "Desk", 1));
        bus.publish_frame(frame("b", "Wall", 2));
        bus.publish_frame(frame("a", "Desk", 3));
        let frames = bus.latest_frames();
        assert_eq!(frames.len(), 2);
        assert_eq!((frames[0].camera_name.as_str(), frames[0].timestamp_ms), ("Desk", 3));
        assert_eq!(frames[1].camera_name, "Wall");
        bus.retire_frame("a");
        assert_eq!(bus.latest_frames().len(), 1);
        assert_eq!(bus.latest_frames()[0].camera_id, "b");
    }

    /// AC1: an agent read reaches whoever listens, as a typed event.
    #[test]
    fn an_agent_read_is_broadcast() {
        let (_dir, bus) = bus();
        let mut rx = bus.subscribe_agent_reads();
        bus.record_agent_read(AgentRead::Desk { at_ms: 5 });
        bus.record_agent_read(AgentRead::Delivery { seq: 3, at_ms: 6 });
        assert_eq!(rx.try_recv().unwrap(), AgentRead::Desk { at_ms: 5 });
        assert_eq!(rx.try_recv().unwrap(), AgentRead::Delivery { seq: 3, at_ms: 6 });
        let json = serde_json::to_value(AgentRead::Delivery { seq: 3, at_ms: 6 }).unwrap();
        assert_eq!(json, serde_json::json!({ "what": "delivery", "seq": 3, "atMs": 6 }));
    }

    /// AC4: the agent naming a session is distinguishable from the window
    /// changing one itself.
    #[test]
    fn session_changes_say_whether_the_agent_caused_them() {
        let (_dir, bus) = bus();
        let mut rx = bus.subscribe_sessions_changed();
        publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        assert_eq!(
            rx.try_recv().unwrap(),
            SessionChange::App,
            "a provisional open is not the agent's"
        );
        bus.start_session("Kiosk", 2000).unwrap();
        assert_eq!(
            rx.try_recv().unwrap(),
            SessionChange::Agent { outcome: "named", name: "Kiosk".to_string() }
        );
        let kiosk = bus.current_session().unwrap().id;
        bus.rename_session(&kiosk, "Kiosk v2").unwrap();
        assert_eq!(rx.try_recv().unwrap(), SessionChange::App);
        let change = SessionChange::Agent {
            outcome: "opened",
            name: "X".to_string(),
        };
        assert_eq!(
            serde_json::to_value(change).unwrap(),
            serde_json::json!({ "source": "agent", "outcome": "opened", "name": "X" })
        );
    }

    #[test]
    fn a_new_delivery_signals_list_changed() {
        let (_dir, bus) = bus();
        let mut rx = bus.subscribe_list_changed();
        publish(&bus, DeliveryKind::Single, vec![snap("Desk", vec![1])]);
        assert!(rx.try_recv().is_ok());
    }
}
