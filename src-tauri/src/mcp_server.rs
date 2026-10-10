//! The MCP server: Tafel as a visual input device for agents. Serves the
//! delivery bus over Streamable HTTP on 127.0.0.1 so an MCP client (Claude
//! Desktop via mcp-remote, Claude Code natively) can look at the desk and
//! wait for deliveries. The server can reach ONLY the bus — never settings or
//! the filesystem — and rmcp's transport defaults already reject non-loopback
//! hosts.
//!
//! Every delivery tool links to one MCP App UI resource (ui://tafel/delivery.html) via
//! `_meta.ui.resourceUri` per the MCP Apps extension (io.modelcontextprotocol/ui,
//! spec 2026-01-26), so hosts that render apps show the delivery inline.

use crate::sessions::{Session, StartOutcome};
use crate::bus::{
    AgentRead, CaptureState, Delivery, DeskFrame, SketchBus, WaitOutcome, WorkState,
};
use base64::Engine;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, ExtensionCapabilities, GetPromptRequestParams, GetPromptResponse,
    GetPromptResult, Icon, JsonObject, ListPromptsResult, ListResourcesResult, MetaObject,
    PaginatedRequestParams, ProgressNotificationParam, Prompt, PromptMessage,
    ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
    ResourceContents, Role, ServerCapabilities, ServerInfo,
};
use rmcp::service::{NotificationContext, RequestContext, RoleServer};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

pub const UI_RESOURCE_URI: &str = "ui://tafel/delivery.html";
pub const UI_MIME_TYPE: &str = "text/html;profile=mcp-app";
/// The MCP App rendered inline by hosts that support the extension.
const UI_HTML: &str = include_str!("../mcp-app/delivery.html");

/// The Tafel mark, shown by hosts next to the server's name and prompts.
const ICON_PNG: &[u8] = include_bytes!("../../brand/web/apple-touch-icon.png");
const ICON_SVG: &str = include_str!("../../public/tafel.svg");

const VISUAL_WORKFLOW: &str =
    include_str!("../../skills/tafel-visual-prompting/references/workflow.md");

const DEFAULT_WAIT_SECONDS: u64 = 120;
/// Claude Desktop silently cancels tool calls around the 4-minute mark and
/// progress notifications do not extend that, so a single wait stays well
/// under it; the timeout result tells the agent to call again.
const MAX_WAIT_SECONDS: u64 = 180;
const PROGRESS_INTERVAL: Duration = Duration::from_secs(10);

fn json_object(value: serde_json::Value) -> JsonObject {
    serde_json::from_value(value).expect("static json literal is an object")
}

/// `_meta` for tool definitions: the MCP Apps linkage (current key plus the
/// legacy flat key some earlier hosts read).
fn ui_tool_meta() -> MetaObject {
    MetaObject(json_object(serde_json::json!({
        "ui": { "resourceUri": UI_RESOURCE_URI },
        "ui/resourceUri": UI_RESOURCE_URI,
    })))
}

fn app_only_meta() -> MetaObject {
    MetaObject(json_object(
        serde_json::json!({"ui": {"visibility": ["app"]}}),
    ))
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Embedded as data URIs: the server has no public URL to point at. PNG
/// first, the format every icon-rendering host must support.
fn server_icons() -> Vec<Icon> {
    vec![
        Icon::new(format!("data:image/png;base64,{}", b64(ICON_PNG)))
            .with_mime_type("image/png")
            .with_sizes(vec!["180x180".to_string()]),
        Icon::new(format!(
            "data:image/svg+xml;base64,{}",
            b64(ICON_SVG.as_bytes())
        ))
        .with_mime_type("image/svg+xml")
        .with_sizes(vec!["any".to_string()]),
    ]
}

fn age_seconds(now_ms: u64, then_ms: u64) -> u64 {
    now_ms.saturating_sub(then_ms) / 1000
}

/// Small machine-readable summary riding on every result. The MCP App uses
/// it to tell fresh deliveries from old ones when it fetches data itself.
fn with_structured(mut result: CallToolResult, structured: serde_json::Value) -> CallToolResult {
    result.structured_content = Some(structured);
    result
}

/// Frames refresh every ~2 s while a camera watches; anything older means
/// that camera stopped feeding (paused or gone) after this frame.
const FRESH_FRAME_MAX_AGE_S: u64 = 10;

/// get_desk: one current frame per camera, or an explanation of why there is
/// none. Each frame is named, so with several cameras the agent knows which
/// view it is reading.
fn desk_result(frames: Vec<DeskFrame>, now_ms: u64) -> CallToolResult {
    if frames.is_empty() {
        return with_structured(
            CallToolResult::success(vec![ContentBlock::text(
                "No desk view available. Tafel is not watching: no camera has \
                 started yet.",
            )]),
            serde_json::json!({ "kind": "none" }),
        );
    }
    let described = frames
        .iter()
        .map(|frame| {
            let age = age_seconds(now_ms, frame.timestamp_ms);
            if age <= FRESH_FRAME_MAX_AGE_S {
                format!("{} ({}s ago)", frame.camera_name, age)
            } else {
                format!(
                    "{} ({}s ago — Tafel stopped watching it since, so it may have changed)",
                    frame.camera_name, age
                )
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    let text = format!(
        "Desk right now, {} camera(s), images in this order: {}.",
        frames.len(),
        described
    );
    let images = frames
        .iter()
        .map(|frame| {
            let age = age_seconds(now_ms, frame.timestamp_ms);
            serde_json::json!({
                "cameraName": frame.camera_name,
                "timestampMs": frame.timestamp_ms,
                "ageSeconds": age,
                "stale": age > FRESH_FRAME_MAX_AGE_S,
            })
        })
        .collect::<Vec<_>>();
    let mut content = vec![ContentBlock::text(text)];
    content.extend(
        frames
            .iter()
            .map(|frame| ContentBlock::image(b64(&frame.jpeg), "image/jpeg")),
    );
    with_structured(
        CallToolResult::success(content),
        serde_json::json!({ "kind": "desk", "images": images }),
    )
}

fn kind_phrase(delivery: &Delivery) -> String {
    let n = delivery.snapshots.len();
    let cameras = delivery
        .snapshots
        .iter()
        .map(|s| s.camera_name.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    match delivery.kind {
        crate::bus::DeliveryKind::Single => "1 snapshot".to_string(),
        crate::bus::DeliveryKind::AllCameras => {
            format!("{} snapshot(s) from {} camera(s)", n, cameras)
        }
        crate::bus::DeliveryKind::Collection => format!("a collection of {}", n),
    }
}

/// The machine-readable per-image summary for structuredContent.
fn delivery_structured(delivery: &Delivery, now_ms: u64) -> serde_json::Value {
    serde_json::json!({
        "kind": "delivery",
        "seq": delivery.seq,
        "deliveryKind": delivery.kind,
        "trigger": delivery.trigger,
        "timestampMs": delivery.timestamp_ms,
        "ageSeconds": age_seconds(now_ms, delivery.timestamp_ms),
        "skippedCameras": delivery.skipped_cameras,
        "images": delivery.snapshots.iter().map(|s| serde_json::json!({
            "cameraName": s.camera_name,
            "timestampMs": s.timestamp_ms,
            "ageSeconds": age_seconds(now_ms, s.timestamp_ms),
            "handFree": s.hand_free,
            "cropped": s.cropped,
            "focus": s.focus,
        })).collect::<Vec<_>>(),
    })
}

/// "42m ago" style age for session starts, which are minutes or days old.
fn age_phrase(now_ms: u64, then_ms: u64) -> String {
    let secs = age_seconds(now_ms, then_ms);
    match secs {
        s if s < 60 => format!("{s}s ago"),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3600),
        s => format!("{}d ago", s / 86_400),
    }
}

/// How the current session is named to the agent.
fn session_phrase(session: Option<&Session>, now_ms: u64) -> String {
    match session {
        None => "no session yet".to_string(),
        Some(s) if s.provisional => {
            format!("unnamed session (started {})", age_phrase(now_ms, s.created_ms))
        }
        Some(s) => format!("session \"{}\"", s.name),
    }
}

fn session_structured(session: Option<&Session>) -> serde_json::Value {
    match session {
        None => serde_json::Value::Null,
        Some(s) => serde_json::json!({
            "id": s.id,
            "name": s.name,
            "provisional": s.provisional,
            "deliveries": s.delivery_count,
        }),
    }
}

/// The result of start_session: plain text saying which of the three cases
/// happened, so the agent can tell the user.
fn start_session_result(outcome: StartOutcome, session: &Session) -> CallToolResult {
    let name = &session.name;
    let text = match outcome {
        StartOutcome::Named { adopted: 0 } => {
            format!("Session \"{name}\" started. Deliveries the user sends now land in it.")
        }
        StartOutcome::Named { adopted } => format!(
            "Session \"{name}\" started; it adopts the {adopted} delivery/deliveries the \
             user already sent, so they are part of this session."
        ),
        StartOutcome::Reopened { deliveries, adopted: 0 } => format!(
            "Reopened session \"{name}\" ({deliveries} earlier delivery/deliveries). New \
             deliveries land in it; list_deliveries shows the earlier ones."
        ),
        StartOutcome::Reopened { deliveries, adopted } => format!(
            "Reopened session \"{name}\" ({deliveries} delivery/deliveries, of which \
             {adopted} the user sent just now were folded in)."
        ),
        StartOutcome::Opened => {
            format!("Started new session \"{name}\". Deliveries the user sends now land in it.")
        }
    };
    with_structured(
        CallToolResult::success(vec![ContentBlock::text(text)]),
        serde_json::json!({
            "kind": "session",
            "outcome": outcome.label(),
            "session": session_structured(Some(session)),
        }),
    )
}

/// get_latest_delivery, get_delivery, and the successful wait share this
/// shape: lead text, then one image block per snapshot.
fn delivery_result(
    delivery: Option<Delivery>,
    now_ms: u64,
    lead: Option<String>,
    session: &str,
) -> CallToolResult {
    let Some(delivery) = delivery else {
        return with_structured(
            CallToolResult::success(vec![ContentBlock::text(format!(
                "No delivery has been sent yet in {session}. Ask the user to give a \
                 thumbs-up to their camera, or call wait_for_delivery to wait for one."
            ))]),
            serde_json::json!({ "kind": "none" }),
        );
    };
    let cameras: Vec<&str> = delivery
        .snapshots
        .iter()
        .map(|s| s.camera_name.as_str())
        .collect();
    let mut text = lead.unwrap_or_else(|| {
        format!(
            "Latest delivery (#{}, {}s ago): {}.",
            delivery.seq,
            age_seconds(now_ms, delivery.timestamp_ms),
            kind_phrase(&delivery)
        )
    });
    text.push_str(&format!(" Cameras, in order: {}. In {}.", cameras.join(", "), session));
    if !delivery.skipped_cameras.is_empty() {
        text.push_str(&format!(
            " Skipped (paused or disconnected): {}.",
            delivery.skipped_cameras.join(", ")
        ));
    }
    let mut content = vec![ContentBlock::text(text)];
    content.extend(
        delivery
            .snapshots
            .iter()
            .map(|s| ContentBlock::image(b64(&s.jpeg), "image/jpeg")),
    );
    with_structured(
        CallToolResult::success(content),
        delivery_structured(&delivery, now_ms),
    )
}

/// The wait timed out. A success result, not an error: "not yet" is a normal
/// answer. Reporting `after_seq` back lets the follow-up call resume from the
/// same baseline, so a delivery sent between the calls is never skipped.
fn timeout_result(waited_seconds: u64, after_seq: u64) -> CallToolResult {
    with_structured(
        CallToolResult::success(vec![ContentBlock::text(format!(
            "No delivery within {}s. The user may still be drawing — call \
             wait_for_delivery again with after_seq={} to keep waiting without \
             missing a delivery sent between calls.",
            waited_seconds, after_seq
        ))]),
        serde_json::json!({
            "kind": "timeout",
            "afterSeq": after_seq,
            "waitedSeconds": waited_seconds
        }),
    )
}

fn stopped_result(state: CaptureState, after_seq: u64) -> CallToolResult {
    let text = match state {
        CaptureState::Paused => "Paused in Tafel. Stop listening; do not call wait_for_delivery again while paused. Resume cameras and ask the agent to listen again. Work already started is not cancelled.",
        CaptureState::Disconnected => "Cameras disconnected. Listening stopped. Reconnect a camera and ask the agent to listen again.",
        _ => "No active cameras. Listening stopped. Add a camera in Tafel and ask the agent to listen again.",
    };
    with_structured(
        CallToolResult::success(vec![ContentBlock::text(text)]),
        serde_json::json!({"kind": state, "afterSeq": after_seq}),
    )
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_millis() as u64
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WaitForDeliveryArgs {
    /// Seconds to wait before returning a "nothing yet" result.
    /// Default 120, clamped to 5..=180.
    pub timeout_seconds: Option<u64>,
    /// Return the first delivery newer than this sequence number. Defaults to
    /// the latest at call time; pass the value from a previous timeout result
    /// so a delivery sent between calls is not missed.
    pub after_seq: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StartSessionArgs {
    /// A short name for the piece of work the conversation is about, e.g.
    /// "Kiosk redesign". Reusing a known name reopens that session.
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDeliveryArgs {
    /// Sequence number of the delivery to return, as listed by
    /// list_deliveries.
    pub seq: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AcknowledgeArgs {
    pub session_id: String,
    pub seq: u64,
    pub state: WorkState,
    /// A short interpretation, result or question, 1–240 characters.
    pub summary: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CardArgs {
    pub tool: String,
    pub seq: Option<u64>,
    pub after_seq: Option<u64>,
    pub session_id: Option<String>,
    pub status_only: Option<bool>,
}

#[derive(Clone)]
pub struct TafelMcp {
    bus: Arc<SketchBus>,
    #[expect(dead_code, reason = "the tool_handler macro reads this router field")]
    tool_router: ToolRouter<Self>,
}

impl TafelMcp {
    pub fn new(bus: Arc<SketchBus>) -> Self {
        Self {
            bus,
            tool_router: Self::tool_router(),
        }
    }

    fn session(&self, now_ms: u64) -> String {
        session_phrase(self.bus.current_session().as_ref(), now_ms)
    }

    /// Builds a delivery result and, when there is a delivery to show, tells
    /// the window that the agent read it.
    fn serve_delivery(&self, delivery: Option<Delivery>, lead: Option<String>) -> CallToolResult {
        let now = now_ms();
        if let Some(delivery) = &delivery {
            self.bus.record_agent_read(AgentRead::Delivery {
                seq: delivery.seq,
                at_ms: now,
            });
        }
        self.card_metadata(
            delivery_result(delivery, now, lead, &self.session(now)),
            self.bus.current_session().as_ref(),
        )
    }
    fn card_metadata(
        &self,
        mut result: CallToolResult,
        session: Option<&Session>,
    ) -> CallToolResult {
        if let Some(data) = result.structured_content.as_mut() {
            let seq = data["seq"].as_u64();
            data["session"] = session_structured(session);
            data["status"] = self.bus.status(seq);
        }
        result
    }

    fn status_result(&self) -> CallToolResult {
        let latest = self.bus.latest_delivery();
        let mut data = self.bus.status(latest.as_ref().map(|d| d.seq));
        data["kind"] = "status".into();
        data["currentSeq"] = self.bus.current_seq().into();
        data["latestSeq"] = latest.map(|d| d.seq).into();
        data["session"] = session_structured(self.bus.current_session().as_ref());
        with_structured(
            CallToolResult::success(vec![ContentBlock::text(data.to_string())]),
            data,
        )
    }
}

#[tool_router]
impl TafelMcp {
    #[tool(
        description = "Report Tafel capture state, number of active waits, and currentSeq for the first wait in a newly authorized continuous visual-prompting session. Preserve your existing cursor when resuming.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_tafel_status(&self) -> Result<CallToolResult, McpError> {
        Ok(self.status_result())
    }

    #[tool(
        description = "Acknowledge an existing visual prompt: working with your interpretation before acting, needs-input for a question, completed only after finishing. Use session.id and seq from the delivery. Never claim work merely because you fetched an image.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn acknowledge_delivery(
        &self,
        params: Parameters<AcknowledgeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let p = params.0;
        self.bus
            .acknowledge(&p.session_id, p.seq, p.state, &p.summary, now_ms())
            .map_err(|message| McpError::invalid_params(message, None))?;
        Ok(with_structured(
            CallToolResult::success(vec![ContentBlock::text(
                "Delivery acknowledgement recorded.",
            )]),
            self.bus.status(Some(p.seq)),
        ))
    }

    #[tool(description = "Read-only card synchronization. Does not record an agent read.", annotations(read_only_hint = true, open_world_hint = false), meta = app_only_meta())]
    async fn get_card_state(
        &self,
        params: Parameters<CardArgs>,
    ) -> Result<CallToolResult, McpError> {
        let p = params.0;
        let session = match &p.session_id {
            Some(id) => self.bus.sessions().into_iter().find(|s| &s.id == id),
            None => self.bus.current_session(),
        };
        if p.session_id.is_some() && session.is_none() {
            return Ok(with_structured(
                CallToolResult::success(vec![ContentBlock::text("This session was deleted.")]),
                serde_json::json!({"kind":"missing"}),
            ));
        }
        let deliveries = session
            .as_ref()
            .map(|s| self.bus.session_deliveries(&s.id))
            .unwrap_or_default();
        let delivery = match p.tool.as_str() {
            "get_delivery" => deliveries.into_iter().find(|d| Some(d.seq) == p.seq),
            "get_latest_delivery" => deliveries.last().cloned(),
            "wait_for_delivery" => {
                if self.bus.capture_state() != CaptureState::Watching {
                    return Ok(stopped_result(
                        self.bus.capture_state(),
                        p.after_seq.unwrap_or(self.bus.current_seq()),
                    ));
                }
                // No age-based guessing when the original wait cursor is unknown.
                deliveries
                    .into_iter()
                    .find(|d| p.after_seq.is_some_and(|seq| d.seq > seq))
            }
            "get_desk" => {
                return Ok(self.card_metadata(
                    desk_result(self.bus.latest_frames(), now_ms()),
                    session.as_ref(),
                ))
            }
            "list_deliveries" => return self.list_deliveries().await,
            _ => return Err(McpError::invalid_params("unsupported card tool", None)),
        };
        if p.status_only == Some(true) {
            return Ok(self.card_metadata(
                with_structured(
                    CallToolResult::success(vec![]),
                    serde_json::json!({
                        "kind": if delivery.is_some() { "status" } else { "missing" }, "seq": p.seq,
                    }),
                ),
                session.as_ref(),
            ));
        }
        if delivery.is_none() && p.tool == "wait_for_delivery" {
            return Ok(self.card_metadata(
                with_structured(
                    CallToolResult::success(vec![]),
                    serde_json::json!({"kind":"waiting"}),
                ),
                session.as_ref(),
            ));
        }
        if delivery.is_none() && p.tool == "get_delivery" {
            return Ok(with_structured(
                CallToolResult::success(vec![ContentBlock::text(
                    "This delivery is no longer available.",
                )]),
                serde_json::json!({"kind":"missing"}),
            ));
        }
        Ok(self.card_metadata(
            delivery_result(
                delivery,
                now_ms(),
                None,
                &session_phrase(session.as_ref(), now_ms()),
            ),
            session.as_ref(),
        ))
    }

    #[tool(
        description = "Look at the user's physical desk right now: returns one current \
                       photo per camera (desk, wall, …), each named. Use for 'what's on \
                       my desk' or to check the drawing surface between deliveries. The \
                       user sees in Tafel every time this is called.",
        annotations(read_only_hint = true, open_world_hint = false),
        meta = ui_tool_meta()
    )]
    async fn get_desk(&self) -> Result<CallToolResult, McpError> {
        let now = now_ms();
        let frames = self.bus.latest_frames();
        if !frames.is_empty() {
            self.bus.record_agent_read(AgentRead::Desk { at_ms: now });
        }
        if frames.is_empty() && self.bus.capture_state() != CaptureState::Watching {
            return Ok(stopped_result(
                self.bus.capture_state(),
                self.bus.current_seq(),
            ));
        }
        Ok(self.card_metadata(
            desk_result(frames, now),
            self.bus.current_session().as_ref(),
        ))
    }

    #[tool(
        description = "Return the most recent delivery the user sent with a gesture: one \
                       or more snapshots from one or more cameras. Use when the user \
                       refers to what they already drew or showed.",
        annotations(read_only_hint = true, open_world_hint = false),
        meta = ui_tool_meta()
    )]
    async fn get_latest_delivery(&self) -> Result<CallToolResult, McpError> {
        Ok(self.serve_delivery(self.bus.latest_delivery(), None))
    }

    #[tool(
        description = "Wait for the user's next delivery from their desk or wall camera \
                       (a thumbs-up sends one snapshot, two thumbs all cameras, an ok-sign \
                       closes a collection). Call this when the user says they are going \
                       to draw or show something. Returns a nothing-yet message on \
                       timeout; call again with the after_seq it reports to keep waiting.",
        annotations(read_only_hint = true, open_world_hint = false),
        meta = ui_tool_meta()
    )]
    async fn wait_for_delivery(
        &self,
        params: Parameters<WaitForDeliveryArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let timeout_seconds = params
            .0
            .timeout_seconds
            .unwrap_or(DEFAULT_WAIT_SECONDS)
            .clamp(5, MAX_WAIT_SECONDS);
        let after_seq = params.0.after_seq.unwrap_or_else(|| self.bus.current_seq());
        let started = tokio::time::Instant::now();
        let progress_token = ctx.meta.get_progress_token();
        let mut wait = std::pin::pin!(self
            .bus
            .wait_for_delivery(after_seq, Duration::from_secs(timeout_seconds)));
        let mut progress = tokio::time::interval(PROGRESS_INTERVAL);
        loop {
            tokio::select! {
                outcome = &mut wait => return Ok(match outcome {
                    WaitOutcome::Delivery(delivery) => {
                        let lead = format!("The user sent visual prompt #{} after {}s. Interpret their directions together with the visuals and carry out the task; acknowledge working before starting.", delivery.seq, started.elapsed().as_secs());
                        self.serve_delivery(Some(delivery), Some(lead))
                    }
                    WaitOutcome::Timeout => timeout_result(timeout_seconds, after_seq),
                    WaitOutcome::Stopped(state) => stopped_result(state, after_seq),
                }),
                _ = progress.tick() => {
                    if let Some(token) = &progress_token {
                        let _ = ctx.peer.notify_progress(
                            ProgressNotificationParam::new(token.clone(), started.elapsed().as_secs_f64())
                                .with_total(timeout_seconds as f64).with_message("Waiting for a visual prompt…")
                        ).await;
                    }
                }
            }
        }
    }

    #[tool(
        description = "List every delivery in the current session (metadata only: \
                       sequence number, kind, cameras, image count, age). Use \
                       get_delivery to fetch the images of one.",
        annotations(read_only_hint = true, open_world_hint = false),
        meta = ui_tool_meta()
    )]
    async fn list_deliveries(&self) -> Result<CallToolResult, McpError> {
        let now = now_ms();
        let deliveries = self.bus.deliveries();
        let session = self.bus.current_session();
        if deliveries.is_empty() {
            return Ok(with_structured(
                CallToolResult::success(vec![ContentBlock::text(format!(
                    "No deliveries yet in {}.",
                    session_phrase(session.as_ref(), now)
                ))]),
                serde_json::json!({
                    "kind": "deliveryList",
                    "session": session_structured(session.as_ref()),
                    "deliveries": [],
                }),
            ));
        }
        let lines = deliveries
            .iter()
            .map(|d| format!("#{} — {} ({}s ago)", d.seq, kind_phrase(d), age_seconds(now, d.timestamp_ms)))
            .collect::<Vec<_>>()
            .join("\n");
        let list = deliveries
            .iter()
            .map(|d| delivery_structured(d, now))
            .collect::<Vec<_>>();
        Ok(with_structured(
            CallToolResult::success(vec![ContentBlock::text(format!(
                "{} delivery/deliveries in {}:\n{}",
                deliveries.len(),
                session_phrase(session.as_ref(), now),
                lines
            ))]),
            serde_json::json!({
                "kind": "deliveryList",
                "session": session_structured(session.as_ref()),
                "deliveries": list,
            }),
        ))
    }

    #[tool(
        description = "Name the piece of work the user is doing at their desk, so the \
                       deliveries belong together and survive a restart. Call it once \
                       the conversation has a topic (\"Kiosk redesign\"). If the current \
                       session is still unnamed it takes this name and keeps what the \
                       user already sent; a name used before reopens that session; \
                       otherwise a new session starts. Only the user can rename, switch \
                       or delete sessions, in the Tafel app.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn start_session(
        &self,
        params: Parameters<StartSessionArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (outcome, session) = self
            .bus
            .start_session(&params.0.name, now_ms())
            .map_err(|message| McpError::invalid_params(message, None))?;
        Ok(start_session_result(outcome, &session))
    }

    #[tool(
        description = "Return the images of one delivery by its sequence number (from \
                       list_deliveries).",
        annotations(read_only_hint = true, open_world_hint = false),
        meta = ui_tool_meta()
    )]
    async fn get_delivery(
        &self,
        params: Parameters<GetDeliveryArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.serve_delivery(self.bus.delivery(params.0.seq), None))
    }
}

#[tool_handler]
impl ServerHandler for TafelMcp {
    fn get_info(&self) -> ServerInfo {
        let mut extensions = ExtensionCapabilities::new();
        extensions.insert(
            "io.modelcontextprotocol/ui".to_string(),
            json_object(serde_json::json!({ "mimeTypes": [UI_MIME_TYPE] })),
        );
        let mut capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .enable_prompts()
            .build();
        capabilities.extensions = Some(extensions);

        let mut info = ServerInfo::default();
        info.capabilities = capabilities;
        info.server_info.name = "tafel".to_string();
        info.server_info.title = Some("Tafel".to_string());
        info.server_info.icons = Some(server_icons());
        info.server_info.version = env!("CARGO_PKG_VERSION").to_string();
        info.instructions = Some(
            "Tafel turns the user's physical desk into visual input: a camera watches \
             the desk and the user sends deliveries with hand gestures (thumbs-up = one \
             snapshot, two thumbs = all cameras, ok-sign = start/finish a collection). \
             Call wait_for_delivery when the user is about to draw or show something; \
             get_latest_delivery for the last one; list_deliveries / get_delivery to \
             revisit earlier ones; get_desk to look at the desk right now. Deliveries \
             are grouped in sessions that survive restarts: once the conversation has \
             a topic, call start_session with a short name — an unnamed session takes \
             the name and keeps what was already sent, a known name reopens that \
             session. The delivery tools see the current session only."
                .to_string(),
        );
        info.instructions
            .as_mut()
            .unwrap()
            .push_str(&format!("\n\n{VISUAL_WORKFLOW}"));
        info
    }

    /// Each connected client gets a task that turns bus "new delivery" signals
    /// into `notifications/resources/list_changed`, so app-capable hosts can
    /// refresh the card. Hosts that ignore it are unaffected.
    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        let peer = context.peer.clone();
        let mut rx = self.bus.subscribe_list_changed();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(()) => {
                        if peer.notify_resource_list_changed().await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let mut prompt = Prompt::new(
            "start_visual_prompting",
            Some("Start a continuous visual-prompting session with Tafel."),
            None,
        );
        prompt.title = Some("Start visual prompting".to_string());
        Ok(ListPromptsResult::with_all_items(vec![prompt]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        if request.name != "start_visual_prompting" {
            return Err(McpError::invalid_params("unknown prompt", None));
        }
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, format!("Start a continuous Tafel visual-prompting session. Treat the visual prompts I intentionally send as my requests and follow this workflow:\n\n{VISUAL_WORKFLOW}"))]).into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let mut resource = Resource::new(UI_RESOURCE_URI, "tafel-delivery")
            .with_title("Tafel delivery view")
            .with_description("Inline view of the snapshots delivered by Tafel's tools.");
        resource.mime_type = Some(UI_MIME_TYPE.to_string());
        Ok(ListResourcesResult::with_all_items(vec![resource]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if request.uri != UI_RESOURCE_URI {
            return Err(McpError::resource_not_found(
                format!("unknown resource: {}", request.uri),
                None,
            ));
        }
        let contents = ResourceContents::TextResourceContents {
            uri: UI_RESOURCE_URI.to_string(),
            mime_type: Some(UI_MIME_TYPE.to_string()),
            text: UI_HTML.to_string(),
            meta: Some(MetaObject(json_object(serde_json::json!({
                "ui": { "prefersBorder": true }
            })))),
        };
        Ok(ReadResourceResult::new(vec![contents]).into())
    }
}

/// Starts the MCP server on 127.0.0.1:`port`. A failed bind (port taken)
/// logs and gives up rather than crashing the app — the desk instrument
/// must keep working even if the agent endpoint cannot.
pub fn spawn(bus: Arc<SketchBus>, port: u16) {
    tauri::async_runtime::spawn(async move {
        let service = StreamableHttpService::new(
            move || Ok(TafelMcp::new(bus.clone())),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        );
        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("tafel: mcp server could not bind 127.0.0.1:{port}: {error}");
                return;
            }
        };
        if let Err(error) = axum::serve(listener, router).await {
            eprintln!("tafel: mcp server stopped: {error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{DeliveryKind, Snapshot};

    fn snap(name: &str, bytes: Vec<u8>) -> Snapshot {
        Snapshot {
            camera_name: name.to_string(),
            timestamp_ms: 0,
            hand_free: true,
            cropped: false,
            focus: None,
            jpeg: bytes,
        }
    }

    fn delivery(seq: u64, kind: DeliveryKind, snaps: Vec<Snapshot>, skipped: Vec<String>) -> Delivery {
        Delivery {
            seq,
            kind,
            timestamp_ms: 0,
            trigger: "thumbs-up".to_string(),
            snapshots: snaps,
            skipped_cameras: skipped,
        }
    }

    fn text_of(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn image_count(result: &CallToolResult) -> usize {
        result
            .content
            .iter()
            .filter(|block| matches!(block, ContentBlock::Image(_)))
            .count()
    }

    fn frame(name: &str, at_ms: u64) -> DeskFrame {
        DeskFrame {
            camera_id: name.to_lowercase(),
            camera_name: name.to_string(),
            timestamp_ms: at_ms,
            jpeg: vec![0xff, 0xd8],
        }
    }

    #[test]
    fn empty_bus_explains_that_tafel_is_not_watching() {
        let result = desk_result(Vec::new(), 0);
        assert_eq!(result.is_error, Some(false));
        assert!(text_of(&result).contains("not watching"));
        assert_eq!(image_count(&result), 0);
    }

    /// AC5: one named image per camera, stale ones flagged.
    #[test]
    fn desk_result_names_every_camera_and_flags_stale_frames() {
        let result = desk_result(vec![frame("Desk", 1_000), frame("Wall", 60_000)], 66_000);
        assert_eq!(image_count(&result), 2);
        let text = text_of(&result);
        assert!(text.contains("2 camera(s)"));
        assert!(text.contains("Wall (6s ago)"));
        assert!(text.contains("Desk (65s ago — Tafel stopped watching"));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["kind"], "desk");
        assert_eq!(structured["images"][0]["cameraName"], "Desk");
        assert_eq!(structured["images"][0]["stale"], true);
        assert_eq!(structured["images"][1]["cameraName"], "Wall");
        assert_eq!(structured["images"][1]["stale"], false);
    }

    #[test]
    fn server_info_carries_the_tafel_icon() {
        let dir = tempfile::tempdir().unwrap();
        let bus = Arc::new(SketchBus::new(dir.path().join("sessions")));
        let icons = TafelMcp::new(bus).get_info().server_info.icons.unwrap();
        assert_eq!(icons.len(), 2);
        let png = "data:image/png;base64,iVBORw0KGgo";
        assert!(icons[0].src.starts_with(png));
        assert_eq!(icons[0].mime_type.as_deref(), Some("image/png"));
        assert!(icons[1].src.starts_with("data:image/svg+xml;base64,"));
    }

    /// AC1: reading the desk or a delivery is reported to the window.
    #[tokio::test]
    async fn reading_material_records_an_agent_read() {
        let dir = tempfile::tempdir().unwrap();
        let bus = Arc::new(SketchBus::new(dir.path().join("sessions")));
        let mut reads = bus.subscribe_agent_reads();
        let mcp = TafelMcp::new(bus.clone());

        mcp.get_desk().await.unwrap();
        assert!(reads.try_recv().is_err(), "an empty desk is not a read");
        bus.set_camera_state("desk", crate::bus::CameraState::Watching);
        bus.publish_frame(frame("Desk", 0));
        mcp.get_desk().await.unwrap();
        assert!(matches!(reads.try_recv().unwrap(), AgentRead::Desk { .. }));

        mcp.get_latest_delivery().await.unwrap();
        assert!(reads.try_recv().is_err(), "no delivery yet, nothing read");
        let seq = bus.publish_delivery(
            0,
            DeliveryKind::Single,
            "thumbs-up".to_string(),
            vec![snap("Desk", vec![1])],
            vec![],
        );
        let is_read_of =
            |read: AgentRead| matches!(read, AgentRead::Delivery { seq: s, .. } if s == seq);
        mcp.get_latest_delivery().await.unwrap();
        assert!(is_read_of(reads.try_recv().unwrap()));
        mcp.get_delivery(Parameters(GetDeliveryArgs { seq })).await.unwrap();
        assert!(is_read_of(reads.try_recv().unwrap()));
    }

    #[test]
    fn single_delivery_carries_one_image_and_the_camera_name() {
        let d = delivery(1, DeliveryKind::Single, vec![snap("Desk", vec![1])], vec![]);
        let result = delivery_result(Some(d), 0, None, "session \"Test\"");
        assert_eq!(image_count(&result), 1);
        assert!(text_of(&result).contains("Desk"));
        assert!(text_of(&result).contains("1 snapshot"));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["images"][0]["cameraName"], "Desk");
        assert_eq!(structured["images"][0]["handFree"], true);
    }

    #[test]
    fn a_focus_point_appears_in_structured_content() {
        use crate::bus::FocusPoint;
        let mut s = snap("Wall", vec![1]);
        s.focus = Some(FocusPoint { x: 0.4, y: 0.6 });
        let d = delivery(1, DeliveryKind::Single, vec![s], vec![]);
        let structured = delivery_result(Some(d), 0, None, "session \"Test\"")
            .structured_content
            .unwrap();
        assert_eq!(structured["images"][0]["focus"]["x"], 0.4);
        assert_eq!(structured["images"][0]["focus"]["y"], 0.6);
    }

    #[test]
    fn all_cameras_delivery_names_skipped_cameras() {
        let d = delivery(
            2,
            DeliveryKind::AllCameras,
            vec![snap("Desk", vec![1]), snap("Wall", vec![2])],
            vec!["Whiteboard".to_string()],
        );
        let result = delivery_result(Some(d), 0, None, "session \"Test\"");
        assert_eq!(image_count(&result), 2);
        assert!(text_of(&result).contains("Whiteboard"));
        assert!(text_of(&result).contains("Skipped"));
    }

    #[test]
    fn collection_reports_its_size() {
        let snaps = (0..5).map(|i| snap("Desk", vec![i])).collect();
        let d = delivery(3, DeliveryKind::Collection, snaps, vec![]);
        let result = delivery_result(Some(d), 0, None, "session \"Test\"");
        assert_eq!(image_count(&result), 5);
        assert!(text_of(&result).contains("collection of 5"));
    }

    #[test]
    fn no_delivery_yet_is_a_success_that_explains_itself() {
        let result = delivery_result(None, 0, None, "session \"Test\"");
        assert_eq!(result.is_error, Some(false));
        assert!(text_of(&result).contains("No delivery"));
        assert_eq!(image_count(&result), 0);
    }

    #[test]
    fn timeout_is_a_success_that_invites_another_wait() {
        let result = timeout_result(120, 7);
        assert_eq!(result.is_error, Some(false));
        assert!(text_of(&result).contains("120s"));
        assert!(text_of(&result).contains("wait_for_delivery again"));
        // The baseline seq rides along so the follow-up call misses nothing.
        assert!(text_of(&result).contains("after_seq=7"));
    }

    #[test]
    fn a_delivery_result_names_the_session() {
        let d = delivery(1, DeliveryKind::Single, vec![snap("Desk", vec![1])], vec![]);
        let result = delivery_result(Some(d), 0, None, "session \"Kiosk\"");
        assert!(text_of(&result).contains("session \"Kiosk\""));
        let result = delivery_result(None, 0, None, "unnamed session (started 5m ago)");
        assert!(text_of(&result).contains("unnamed session (started 5m ago)"));
    }

    #[test]
    fn session_phrase_covers_none_provisional_and_named() {
        assert_eq!(session_phrase(None, 0), "no session yet");
        let mut s = Session {
            id: "1".to_string(),
            name: String::new(),
            provisional: true,
            created_ms: 0,
            last_delivery_ms: None,
            delivery_count: 0,
        };
        assert_eq!(session_phrase(Some(&s), 300_000), "unnamed session (started 5m ago)");
        s.name = "Kiosk".to_string();
        s.provisional = false;
        assert_eq!(session_phrase(Some(&s), 300_000), "session \"Kiosk\"");
    }

    /// AC8: start_session is the only tool that writes, and it is not
    /// destructive; every other tool stays read-only.
    #[test]
    fn start_session_is_the_only_mutating_tool_and_is_not_destructive() {
        let tools = TafelMcp::tool_router().list_all();
        assert!(tools.iter().any(|t| t.name == "start_session"));
        for tool in tools {
            let annotations = tool.annotations.expect("every tool is annotated");
            if tool.name == "start_session" || tool.name == "acknowledge_delivery" {
                assert_eq!(annotations.read_only_hint, Some(false));
                assert_eq!(annotations.destructive_hint, Some(false));
            } else {
                assert_eq!(annotations.read_only_hint, Some(true), "{} must be read-only", tool.name);
            }
        }
    }

    /// AC3 + AC7 through the tool surface: naming adopts, and the delivery
    /// tools then speak of that session.
    #[tokio::test]
    async fn start_session_names_the_provisional_session_and_lists_scope_to_it() {
        let dir = tempfile::tempdir().unwrap();
        let bus = Arc::new(SketchBus::new(dir.path().join("sessions")));
        bus.publish_delivery(1000, DeliveryKind::Single, "thumbs-up".to_string(), vec![snap("Desk", vec![1])], vec![]);
        let mcp = TafelMcp::new(bus.clone());
        let result = mcp
            .start_session(Parameters(StartSessionArgs { name: "Kiosk".to_string() }))
            .await
            .unwrap();
        assert!(text_of(&result).contains("adopts the 1 delivery"));
        assert_eq!(result.structured_content.unwrap()["outcome"], "named");
        let listed = mcp.list_deliveries().await.unwrap();
        assert!(text_of(&listed).contains("1 delivery/deliveries in session \"Kiosk\""));
        let blank = mcp
            .start_session(Parameters(StartSessionArgs { name: "  ".to_string() }))
            .await;
        assert!(blank.is_err(), "a blank name is invalid params");
    }

    #[test]
    fn ui_meta_links_tools_to_the_delivery_app() {
        let meta = ui_tool_meta();
        assert_eq!(
            meta.0["ui"]["resourceUri"],
            serde_json::json!(UI_RESOURCE_URI)
        );
        assert_eq!(meta.0["ui/resourceUri"], serde_json::json!(UI_RESOURCE_URI));
    }

    /// Wire-level smoke test over the real HTTP transport, speaking the
    /// legacy initialize handshake that mcp-remote (Claude Desktop's bridge)
    /// uses today.
    #[tokio::test]
    async fn http_transport_answers_a_legacy_initialize() {
        let dir = tempfile::tempdir().unwrap();
        let bus = Arc::new(SketchBus::new(dir.path().join("sessions")));
        let service = StreamableHttpService::new(
            move || Ok(TafelMcp::new(bus.clone())),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        );
        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

        let response = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/mcp"))
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "smoke-test", "version": "0" }
                }
            }))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        let body = response.text().await.unwrap();
        assert!(body.contains("tafel"), "unexpected initialize body: {body}");
        assert!(
            body.contains("io.modelcontextprotocol/ui"),
            "apps extension capability missing: {body}"
        );
    }
}
