# Visual prompting feedback

Approved scope: user requested implementation of the 2026-10-09 proposal.
This authorizes specification, implementation and verification without further
phase approval. Work remains uncommitted on `codex/mcp-feedback`.

| Module            | Scope                                                                                                                      | Dependencies      |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------- | ----------------- |
| capture-state     | Camera status propagated to Rust; paused frames removed; waits wake on pause; active waiter count released on cancellation | None              |
| delivery-feedback | Exact delivery identity, bounded fallback, receipt vs acknowledgement, timestamps and zoom                                 | capture-state     |
| visual-prompting  | Shared server guidance, user-selected MCP prompt, portable skill, explicit work acknowledgement                            | delivery-feedback |
| gesture-intent    | Persisted per-gesture switches; fist off by default; buttons always usable                                                 | None              |

Use existing React/TypeScript/CSS and Rust/Tauri patterns from AGENTS.md.
No dependencies, storage migration, classifier threshold changes, or plugin
installation. Gesture zones remain a future recording-based evaluation.

## Acceptance and implementation order

1. Capture state: pause one camera excludes its cached frame, including late
   frame reports. Other cameras keep waiting. Pause all interrupts all pending
   waits promptly, including a quick pause/resume, and never deletes history.
   Disconnection is a distinct outcome. Cancellation releases listener counts.
2. Card: loading/received/paused/timeout/error/cancelled states are truthful.
   Fallback preserves tool arguments and never marks its own fetch as an agent
   read. No age heuristic that substitutes an old delivery. All polling is
   bounded and stopped on teardown. Historical cards keep exact delivery IDs.
   Keyboard-accessible zoom and absolute image timestamps are provided.
3. Agent: shared guidance interprets intentional visual prompts in context,
   separates instructions from references, acts instead of merely describing,
   and preserves permissions and sequence cursors. Explicit acknowledgement
   reports working/completed/needs-input for an existing session and delivery;
   completed work cannot be accidentally reset by rereading it.
4. Gestures: each recognized gesture can be disabled independently; fist is
   off for both fresh and legacy settings. Disabled gestures never enter the
   hold tracker. Buttons continue to work and clearly show disabled detection.

## Verification

- `npm test`: pure gesture behavior and simulated MCP host/card lifecycle.
- `npm run build`: frontend type checking and build.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: camera/wait races,
  acknowledgement identity and transitions, settings migration, MCP tools/prompts.
- `npm run tauri build`: native bridge and embedded card build.
- Browser synthetic host checks if available; real host/physical gesture tests
  remain explicitly reported if unavailable. Do not use real desk photos for tests.

## Progress

- [x] Capture state
- [x] Delivery feedback
- [x] Visual prompting and skill
- [x] Gesture controls
- [x] Verification and docs (automated); manual checks below still pending

## Handoff (2026-10-09)

Decisions: guidance lives in one file,
`skills/tafel-visual-prompting/references/workflow.md`, embedded in the MCP
server instructions, the `start_visual_prompting` MCP prompt and the app's
copy button; the skill folder is portable but not installed as a plugin.
Work acknowledgements (`acknowledge_delivery`) are in memory for one app run.
The card's fallback uses an app-only `get_card_state` tool that never records
an agent read. Fist is off by default, also for legacy settings.

Changed: `src-tauri/src/{bus,mcp_server,lib,settings}.rs`,
`src-tauri/mcp-app/{delivery.html,README.md}`, `src/app/{CameraTile,ControlApp}.tsx`,
`src/app/tauri.ts`, `src/core/actions{,.test}.ts`, `src/core/mcpCard.test.ts`,
`src/styles.css`, `README.md`, `skills/tafel-visual-prompting/`. New Rust
code is rustfmt-formatted; pre-existing unformatted lines were left alone.

Verification run on this tree:

- `npm test`: 74 passed, 1 file skipped (fixture).
- `npm run build`: passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: 64 passed (after
  `cargo clean -p tafel`; the stale build pointed at Xcode's old clang 17).
- `cargo build --manifest-path src-tauri/Cargo.toml`: passed, no warnings.
- `npx prettier --check` on changed supported files and `git diff --check`: passed.

Pending manual checks: card states in a real MCP host (receipt → working →
completed; pause during an open wait; cancel); MCP prompt visibility in the
host; whether the host hides the app-only tool from the model; writing and
drawing with fist off and thumbs-up on; launching the built `Tafel.app`.

`npm run tauri build`: passed (app + DMG) after two toolchain fixes. Xcode 27
/ Swift 6.4 makes swift-rs's helpers in `SwiftRs.o` local in release static
libraries and swift-rs 1.0.8 only re-exports the package's own symbols, so
`src-tauri/build.rs` now promotes `data_from_bytes`, `retain_object`,
`release_object` and `string_from_bytes` with rustup's `llvm-objcopy`
(`rustup component add llvm-tools`). Remove once swift-rs fixes this. Stale
release output pointing at clang 17 needed `cargo clean -p tafel --release`.

Next: run the manual checks with `npm run tauri dev`, then decide on commit
and PR. Changes are uncommitted on `codex/mcp-feedback`.
