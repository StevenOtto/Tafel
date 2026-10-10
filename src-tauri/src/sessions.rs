//! Sessions: named, durable sets of deliveries on disk. There is at most one
//! *current* session; every delivery lands in whichever session is current
//! when it is published. Nobody has to open a session: the first delivery
//! opens a provisional one, and the agent's `start_session` names it — so a
//! forgotten name is a recoverable case, never a silent mis-filing.
//!
//! Layout under `<app data>/sessions/`:
//!   index.json                 { currentId }
//!   <id>/session.json          the session's metadata
//!   <id>/000042.json           one delivery (the struct minus its bytes)
//!   <id>/000042-0.jpg          one JPEG per snapshot, in order
//! Directories without a `session.json` (the removed prototype pipeline left
//! some) are ignored, never touched.

use crate::bus::{Delivery, DeliveryKind, Snapshot};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A provisional session that has seen no delivery for this long is over;
/// the next delivery opens a fresh one. Named sessions never expire.
pub const PROVISIONAL_GAP_MS: u64 = 4 * 60 * 60 * 1000;

const SESSION_FILE: &str = "session.json";
const INDEX_FILE: &str = "index.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    /// Empty while provisional; the UI derives a label from `created_ms`.
    pub name: String,
    pub provisional: bool,
    pub created_ms: u64,
    pub last_delivery_ms: Option<u64>,
    /// Derived from the files on disk, not persisted.
    #[serde(skip_deserializing, default)]
    pub delivery_count: usize,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Index {
    current_id: Option<String>,
}

/// What `start_session` did, so the agent can be told exactly that.
#[derive(Debug, PartialEq, Eq)]
pub enum StartOutcome {
    /// The provisional current session took the name and kept its deliveries.
    Named { adopted: usize },
    /// A session with that name existed and is current again. Any provisional
    /// deliveries were folded into it.
    Reopened { deliveries: usize, adopted: usize },
    /// A new named session opened and is current.
    Opened,
}

impl StartOutcome {
    /// The one-word label the agent result and the window's announcement share.
    pub fn label(&self) -> &'static str {
        match self {
            StartOutcome::Named { .. } => "named",
            StartOutcome::Reopened { .. } => "reopened",
            StartOutcome::Opened => "opened",
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub path: String,
    pub bytes: u64,
}

pub struct SessionStore {
    root: PathBuf,
    /// Oldest first.
    sessions: Vec<Session>,
    current_id: Option<String>,
    /// Highest seq on disk; the next delivery gets `next_seq + 1`.
    last_seq: u64,
}

impl SessionStore {
    /// Opens (creating if needed) the store at `root`, scanning what is there.
    pub fn open(root: PathBuf) -> SessionStore {
        let _ = fs::create_dir_all(&root);
        let mut sessions: Vec<Session> = Vec::new();
        let mut last_seq = 0;
        for entry in fs::read_dir(&root).into_iter().flatten().flatten() {
            let dir = entry.path();
            let Some(mut session) = read_json::<Session>(&dir.join(SESSION_FILE)) else {
                continue;
            };
            let deliveries = read_deliveries(&dir, false);
            session.delivery_count = deliveries.len();
            last_seq = last_seq.max(deliveries.iter().map(|d| d.seq).max().unwrap_or(0));
            sessions.push(session);
        }
        sessions.sort_by_key(|s| s.created_ms);
        let current_id = read_json::<Index>(&root.join(INDEX_FILE))
            .and_then(|index| index.current_id)
            .filter(|id| sessions.iter().any(|s| &s.id == id));
        SessionStore {
            root,
            sessions,
            current_id,
            last_seq,
        }
    }

    pub fn sessions(&self) -> Vec<Session> {
        self.sessions.clone()
    }

    pub fn current(&self) -> Option<Session> {
        let id = self.current_id.as_ref()?;
        self.sessions.iter().find(|s| &s.id == id).cloned()
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// The current session's deliveries with their images, oldest first.
    pub fn current_deliveries(&self) -> Vec<Delivery> {
        match &self.current_id {
            Some(id) => self.deliveries_of(id),
            None => Vec::new(),
        }
    }

    pub fn deliveries_of(&self, id: &str) -> Vec<Delivery> {
        if !self.sessions.iter().any(|s| s.id == id) {
            return Vec::new();
        }
        read_deliveries(&self.root.join(id), true)
    }

    /// Stores a delivery in the current session, opening a provisional one
    /// first when there is none or the current one has expired.
    pub fn append(
        &mut self,
        now_ms: u64,
        kind: DeliveryKind,
        trigger: String,
        snapshots: Vec<Snapshot>,
        skipped_cameras: Vec<String>,
    ) -> Delivery {
        let expired = self.current().is_some_and(|s| {
            s.provisional
                && now_ms.saturating_sub(s.last_delivery_ms.unwrap_or(s.created_ms))
                    > PROVISIONAL_GAP_MS
        });
        if self.current_id.is_none() || expired {
            self.open_session(String::new(), true, now_ms);
        }
        let id = self.current_id.clone().expect("a current session was just ensured");
        self.last_seq += 1;
        let delivery = Delivery {
            seq: self.last_seq,
            kind,
            timestamp_ms: now_ms,
            trigger,
            snapshots,
            skipped_cameras,
        };
        write_delivery(&self.root.join(&id), &delivery);
        if let Some(session) = self.session_mut(&id) {
            session.last_delivery_ms = Some(now_ms);
            session.delivery_count += 1;
        }
        self.save_session(&id);
        delivery
    }

    /// The agent's one verb. See `StartOutcome` for the three cases.
    pub fn start(&mut self, name: &str, now_ms: u64) -> Result<StartOutcome, String> {
        let name = clean_name(name)?;
        let current = self.current();
        let existing = self.sessions.iter().find(|s| s.name == name).cloned();
        match (existing, current) {
            (Some(existing), Some(current)) if current.provisional => {
                let adopted = self.merge_into(&existing.id, &current.id);
                self.set_current(Some(existing.id.clone()));
                let deliveries = self.session_mut(&existing.id).map_or(0, |s| s.delivery_count);
                Ok(StartOutcome::Reopened { deliveries, adopted })
            }
            (Some(existing), _) => {
                self.set_current(Some(existing.id.clone()));
                Ok(StartOutcome::Reopened {
                    deliveries: existing.delivery_count,
                    adopted: 0,
                })
            }
            (None, Some(current)) if current.provisional => {
                if let Some(session) = self.session_mut(&current.id) {
                    session.name = name;
                    session.provisional = false;
                }
                self.save_session(&current.id);
                Ok(StartOutcome::Named {
                    adopted: current.delivery_count,
                })
            }
            (None, _) => {
                self.open_session(name, false, now_ms);
                Ok(StartOutcome::Opened)
            }
        }
    }

    /// The window's "New": an empty named session, current from now on.
    /// Unlike `start` this never adopts or reopens; a duplicate name is refused.
    pub fn open_new(&mut self, name: &str, now_ms: u64) -> Result<(), String> {
        let name = clean_name(name)?;
        if self.sessions.iter().any(|s| s.name == name) {
            return Err(format!("a session named \"{name}\" already exists"));
        }
        self.open_session(name, false, now_ms);
        Ok(())
    }

    pub fn switch(&mut self, id: &str) -> Result<(), String> {
        if !self.sessions.iter().any(|s| s.id == id) {
            return Err("unknown session".to_string());
        }
        self.set_current(Some(id.to_string()));
        Ok(())
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<(), String> {
        let name = clean_name(name)?;
        if self.sessions.iter().any(|s| s.id != id && s.name == name) {
            return Err(format!("a session named \"{name}\" already exists"));
        }
        let session = self.session_mut(id).ok_or("unknown session")?;
        session.name = name;
        session.provisional = false;
        self.save_session(id);
        Ok(())
    }

    /// Removes one delivery (its metadata and images) from a session.
    pub fn delete_delivery(&mut self, id: &str, seq: u64) -> Result<(), String> {
        let dir = self.root.join(id);
        let delivery = read_json::<Delivery>(&dir.join(json_name(seq)))
            .ok_or_else(|| format!("no delivery #{seq} in this session"))?;
        for i in 0..delivery.snapshots.len() {
            let _ = fs::remove_file(dir.join(jpg_name(seq, i)));
        }
        fs::remove_file(dir.join(json_name(seq))).map_err(|e| e.to_string())?;
        if let Some(session) = self.session_mut(id) {
            session.delivery_count = session.delivery_count.saturating_sub(1);
        }
        self.save_session(id);
        Ok(())
    }

    /// Removes the session and its files. Deleting the current session
    /// leaves none current; the next delivery opens a provisional one.
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let index = self
            .sessions
            .iter()
            .position(|s| s.id == id)
            .ok_or("unknown session")?;
        fs::remove_dir_all(self.root.join(id)).map_err(|e| e.to_string())?;
        self.sessions.remove(index);
        if self.current_id.as_deref() == Some(id) {
            self.set_current(None);
        }
        Ok(())
    }

    pub fn storage(&self) -> Storage {
        let bytes = self
            .sessions
            .iter()
            .map(|s| dir_size(&self.root.join(&s.id)))
            .sum();
        Storage {
            path: self.root.display().to_string(),
            bytes,
        }
    }

    // ----- internals -----

    fn open_session(&mut self, name: String, provisional: bool, now_ms: u64) {
        let mut id = now_ms.to_string();
        let mut n = 1;
        while self.root.join(&id).exists() {
            id = format!("{now_ms}-{n}");
            n += 1;
        }
        let _ = fs::create_dir_all(self.root.join(&id));
        self.sessions.push(Session {
            id: id.clone(),
            name,
            provisional,
            created_ms: now_ms,
            last_delivery_ms: None,
            delivery_count: 0,
        });
        self.save_session(&id);
        self.set_current(Some(id));
    }

    /// Moves every delivery of `from` into `into` and removes `from`.
    /// Sequence numbers are global, so files never collide.
    fn merge_into(&mut self, into: &str, from: &str) -> usize {
        let source = self.root.join(from);
        let target = self.root.join(into);
        let mut moved = 0;
        let mut last_ms = None;
        for delivery in read_deliveries(&source, false) {
            for i in 0..delivery.snapshots.len() {
                let jpg = jpg_name(delivery.seq, i);
                let _ = fs::rename(source.join(&jpg), target.join(&jpg));
            }
            let json = json_name(delivery.seq);
            let _ = fs::rename(source.join(&json), target.join(&json));
            last_ms = last_ms.max(Some(delivery.timestamp_ms));
            moved += 1;
        }
        let _ = fs::remove_dir_all(&source);
        self.sessions.retain(|s| s.id != from);
        if let Some(session) = self.session_mut(into) {
            session.delivery_count += moved;
            session.last_delivery_ms = session.last_delivery_ms.max(last_ms);
        }
        self.save_session(into);
        moved
    }

    fn session_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    fn set_current(&mut self, id: Option<String>) {
        self.current_id = id;
        write_json(
            &self.root.join(INDEX_FILE),
            &Index {
                current_id: self.current_id.clone(),
            },
        );
    }

    fn save_session(&self, id: &str) {
        if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
            write_json(&self.root.join(id).join(SESSION_FILE), session);
        }
    }
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a session name cannot be empty".to_string());
    }
    Ok(name.to_string())
}

fn json_name(seq: u64) -> String {
    format!("{seq:06}.json")
}

fn jpg_name(seq: u64, index: usize) -> String {
    format!("{seq:06}-{index}.jpg")
}

fn write_delivery(dir: &Path, delivery: &Delivery) {
    for (i, snapshot) in delivery.snapshots.iter().enumerate() {
        let _ = fs::write(dir.join(jpg_name(delivery.seq, i)), &snapshot.jpeg);
    }
    write_json(&dir.join(json_name(delivery.seq)), delivery);
}

/// Deliveries in `dir`, oldest first; `with_images` reads the JPEGs back in.
fn read_deliveries(dir: &Path, with_images: bool) -> Vec<Delivery> {
    let mut deliveries: Vec<Delivery> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "json")
                && path.file_name().is_some_and(|name| name != SESSION_FILE)
        })
        .filter_map(|path| read_json::<Delivery>(&path))
        .collect();
    deliveries.sort_by_key(|d| d.seq);
    if with_images {
        for delivery in &mut deliveries {
            for (i, snapshot) in delivery.snapshots.iter_mut().enumerate() {
                snapshot.jpeg =
                    fs::read(dir.join(jpg_name(delivery.seq, i))).unwrap_or_default();
            }
        }
    }
    deliveries
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Ok(raw) = serde_json::to_string_pretty(value) {
        let _ = fs::write(path, raw);
    }
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 60 * 60 * 1000;

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

    fn deliver(store: &mut SessionStore, now_ms: u64) -> Delivery {
        store.append(
            now_ms,
            DeliveryKind::Single,
            "thumbs-up".to_string(),
            vec![snap("Desk", vec![1, 2, 3])],
            vec![],
        )
    }

    fn store() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().join("sessions"));
        (dir, store)
    }

    /// AC1: the first delivery opens a provisional session and lands on disk.
    #[test]
    fn first_delivery_opens_a_provisional_session_and_writes_files() {
        let (_dir, mut store) = store();
        assert!(store.current().is_none());
        let delivery = deliver(&mut store, 10_000);
        let current = store.current().expect("a session opened");
        assert!(current.provisional);
        assert_eq!(current.delivery_count, 1);
        assert_eq!(delivery.seq, 1);
        let session_dir = store.root.join(&current.id);
        assert!(session_dir.join("session.json").exists());
        assert!(session_dir.join("000001.json").exists());
        assert_eq!(fs::read(session_dir.join("000001-0.jpg")).unwrap(), vec![1, 2, 3]);
    }

    /// AC2: reopening restores the current session's deliveries and seq.
    #[test]
    fn reopening_the_store_restores_deliveries_and_continues_seq() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sessions");
        {
            let mut store = SessionStore::open(root.clone());
            deliver(&mut store, 10_000);
            deliver(&mut store, 11_000);
        }
        let mut store = SessionStore::open(root);
        let restored = store.current_deliveries();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].seq, 1);
        assert_eq!(restored[1].snapshots[0].jpeg, vec![1, 2, 3]);
        assert_eq!(deliver(&mut store, 12_000).seq, 3);
    }

    /// AC3: naming a provisional session adopts its deliveries.
    #[test]
    fn naming_a_provisional_session_adopts_its_deliveries() {
        let (_dir, mut store) = store();
        deliver(&mut store, 10_000);
        deliver(&mut store, 11_000);
        let outcome = store.start("Kiosk", 12_000).unwrap();
        assert_eq!(outcome, StartOutcome::Named { adopted: 2 });
        let current = store.current().unwrap();
        assert_eq!(current.name, "Kiosk");
        assert!(!current.provisional);
        assert_eq!(store.current_deliveries().len(), 2);
        assert_eq!(store.sessions().len(), 1);
    }

    /// AC4: naming while a named session is current opens a new one.
    #[test]
    fn naming_when_current_is_named_opens_a_new_session() {
        let (_dir, mut store) = store();
        deliver(&mut store, 10_000);
        store.start("Kiosk", 11_000).unwrap();
        assert_eq!(store.start("Menu", 12_000).unwrap(), StartOutcome::Opened);
        let current = store.current().unwrap();
        assert_eq!(current.name, "Menu");
        assert_eq!(store.current_deliveries().len(), 0);
        assert_eq!(store.sessions().len(), 2);
    }

    /// AC5: a repeated name reopens that session rather than making a second.
    #[test]
    fn repeating_a_name_reopens_that_session() {
        let (_dir, mut store) = store();
        deliver(&mut store, 10_000);
        store.start("Kiosk", 11_000).unwrap();
        store.start("Menu", 12_000).unwrap();
        deliver(&mut store, 13_000);
        let outcome = store.start("Kiosk", 14_000).unwrap();
        assert_eq!(
            outcome,
            StartOutcome::Reopened {
                deliveries: 1,
                adopted: 0
            }
        );
        assert_eq!(store.current().unwrap().name, "Kiosk");
        assert_eq!(store.sessions().iter().filter(|s| s.name == "Kiosk").count(), 1);
    }

    /// AC5, sketch-first variant: provisional work is folded into the
    /// reopened session instead of being orphaned.
    #[test]
    fn reopening_folds_a_provisional_session_into_the_named_one() {
        let (_dir, mut store) = store();
        deliver(&mut store, 10_000);
        store.start("Kiosk", 11_000).unwrap();
        // A long gap and a new delivery: a fresh provisional session.
        store.start("Menu", 12_000).unwrap();
        // As after deleting the current session: none current, so the next
        // delivery opens a provisional one.
        store.set_current(None);
        deliver(&mut store, 20 * HOUR);
        assert!(store.current().unwrap().provisional);
        let outcome = store.start("Kiosk", 20 * HOUR + 1).unwrap();
        assert_eq!(
            outcome,
            StartOutcome::Reopened {
                deliveries: 2,
                adopted: 1
            }
        );
        let deliveries = store.current_deliveries();
        assert_eq!(deliveries.len(), 2);
        assert_eq!(deliveries[1].snapshots[0].jpeg, vec![1, 2, 3]);
        assert_eq!(store.sessions().len(), 2, "the provisional session is gone");
    }

    /// AC6: a provisional session expires after the gap; a named one does not.
    #[test]
    fn provisional_session_expires_after_the_gap_but_a_named_one_does_not() {
        let (_dir, mut store) = store();
        deliver(&mut store, HOUR);
        deliver(&mut store, HOUR + PROVISIONAL_GAP_MS + 1);
        assert_eq!(store.sessions().len(), 2);
        assert_eq!(store.current().unwrap().delivery_count, 1);

        store.start("Kiosk", 2 * HOUR).unwrap();
        deliver(&mut store, 48 * HOUR);
        assert_eq!(store.sessions().len(), 2);
        assert_eq!(store.current().unwrap().name, "Kiosk");
        assert_eq!(store.current().unwrap().delivery_count, 2);
    }

    /// AC9: switching and renaming from the app.
    #[test]
    fn switching_and_renaming_sessions() {
        let (_dir, mut store) = store();
        deliver(&mut store, 10_000);
        store.start("Kiosk", 11_000).unwrap();
        store.start("Menu", 12_000).unwrap();
        let kiosk = store.sessions()[0].id.clone();
        store.switch(&kiosk).unwrap();
        assert_eq!(store.current().unwrap().name, "Kiosk");
        deliver(&mut store, 13_000);
        assert_eq!(store.current_deliveries().len(), 2);

        assert!(store.rename(&kiosk, "Menu").is_err(), "names stay unique");
        assert!(store.rename(&kiosk, "  ").is_err());
        store.rename(&kiosk, "Kiosk v2").unwrap();
        assert_eq!(store.current().unwrap().name, "Kiosk v2");
        assert!(store.switch("nope").is_err());
    }

    /// New from the window: empty and current; the old session keeps its deliveries.
    #[test]
    fn a_new_session_from_the_window_starts_empty_and_keeps_the_old_one() {
        let (_dir, mut store) = store();
        deliver(&mut store, 10_000);
        deliver(&mut store, 11_000);
        let provisional = store.current().unwrap().id;

        store.open_new("Menu", 12_000).unwrap();
        let menu = store.current().unwrap();
        assert_eq!(menu.name, "Menu");
        assert!(!menu.provisional);
        assert_eq!(store.current_deliveries().len(), 0);
        assert_eq!(store.deliveries_of(&provisional).len(), 2);
        assert_eq!(store.sessions().len(), 2);

        assert!(store.open_new("Menu", 13_000).is_err(), "names stay unique");
        assert!(store.open_new(" ", 13_000).is_err());
        assert_eq!(store.current().unwrap().id, menu.id);
    }

    /// AC10: deleting the current session leaves none; files are gone.
    #[test]
    fn deleting_the_current_session_removes_files_and_leaves_none_current() {
        let (_dir, mut store) = store();
        deliver(&mut store, 10_000);
        let id = store.current().unwrap().id;
        store.delete(&id).unwrap();
        assert!(store.current().is_none());
        assert!(!store.root.join(&id).exists());
        assert!(store.sessions().is_empty());
        let next = deliver(&mut store, 11_000);
        assert_eq!(next.seq, 2, "seq keeps counting");
        assert!(store.current().unwrap().provisional);
    }

    /// UI feedback AC1: one delivery can be removed; its files go with it.
    #[test]
    fn deleting_one_delivery_removes_its_files_and_keeps_the_rest() {
        let (_dir, mut store) = store();
        let first = deliver(&mut store, 10_000);
        let second = deliver(&mut store, 11_000);
        let id = store.current().unwrap().id;
        store.delete_delivery(&id, first.seq).unwrap();
        let dir = store.root.join(&id);
        assert!(!dir.join("000001.json").exists());
        assert!(!dir.join("000001-0.jpg").exists());
        assert!(dir.join("000002.json").exists());
        let remaining = store.current_deliveries();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].seq, second.seq);
        assert_eq!(store.current().unwrap().delivery_count, 1);
        assert!(store.delete_delivery(&id, 999).is_err());
        assert_eq!(deliver(&mut store, 12_000).seq, 3, "seq never reuses a number");
    }

    /// AC11: storage reports the root and the bytes of known sessions.
    #[test]
    fn storage_sums_the_session_files() {
        let (_dir, mut store) = store();
        assert_eq!(store.storage().bytes, 0);
        deliver(&mut store, 10_000);
        let storage = store.storage();
        assert!(storage.bytes > 3, "jpeg plus metadata");
        assert!(storage.path.ends_with("sessions"));
    }

    /// AC12: prototype leftovers without a session.json are left alone.
    #[test]
    fn directories_without_session_json_are_ignored_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sessions");
        let legacy = root.join("1787060682794");
        fs::create_dir_all(legacy.join("frames")).unwrap();
        fs::write(legacy.join("ledger.jsonl"), "{}").unwrap();
        let mut store = SessionStore::open(root);
        assert!(store.sessions().is_empty());
        deliver(&mut store, 10_000);
        assert_eq!(store.sessions().len(), 1);
        assert!(legacy.join("ledger.jsonl").exists());
        let current = store.current().unwrap();
        assert_eq!(store.storage().bytes, dir_size(&store.root.join(&current.id)));
    }

    #[test]
    fn a_blank_name_is_refused() {
        let (_dir, mut store) = store();
        assert!(store.start("   ", 1).is_err());
        assert!(store.current().is_none());
    }
}
