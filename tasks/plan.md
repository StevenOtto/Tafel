# Implementation Plan: Android phones as Tafel cameras

Spec: `specs/2026-09-22-android-als-camera.md`. Tasks: `tasks/todo.md`.

## Overview

Make MJPEG network cameras work (today they never deliver a frame), close
the redirect hole in the only outbound network path, and tell Android users
how to connect. Everything outside `netcam.rs` stays thin.

## Architecture Decisions

- One long-lived reader per camera URL in Rust, holding only the newest
  frame; the webview reads memory, never the network. Frame rate stops
  depending on round-trip time.
- Redirects off (`Policy::none()`), not a custom policy that re-checks each
  hop: simpler, and no known camera app needs a redirect.
- `netcam.rs` stays free of `tauri` and `swift-rs` so its tests run outside
  macOS in a scratch crate that includes the file via `#[path]`.
- Webview poll interval 100 ms to 50 ms, deduplicated by sequence number.

## Task List

### Phase 1: Security and parsing (no behaviour change for users yet)
- [x] Task 1: Refuse redirects in `fetch_frame`
- [x] Task 2: Incremental multipart parser with a byte bound

### Checkpoint: Foundation
- [x] netcam tests pass in the scratch harness
- [x] `npm test` passes

### Phase 2: Streaming reader
- [x] Task 3: Reader and registry (stream, snapshot fallback, idle timeout, reconnect, stop when unrequested)
- [x] Task 4: `latest_network_frame` command and the tile poll loop

### Checkpoint: Core
- [x] netcam wire tests pass (connection count, frame rate, drop and recover, redirect, snapshot)
- [x] `npx tsc --noEmit` and `npm test` pass
- [ ] Human on a Mac: `cargo test` in `src-tauri`, and one real Android MJPEG URL shows a live tile

### Phase 3: Guidance and docs
- [x] Task 5: Android guidance in the Network camera panel
- [x] Task 6: `NSLocalNetworkUsageDescription` in `Info.plist`
- [x] Task 7: README and DESIGN.md

### Checkpoint: Complete
- [x] All spec acceptance criteria met or marked manual
- [ ] Ready for review

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| `src-tauri` does not compile outside macOS | Med | Done: the crate was compiled, linted and tested on Linux with only `vision.rs` stubbed and `compile_error!` removed (54 tests pass, clippy clean); Swift bridge and the real app still need a Mac |
| `tokio::spawn` needs the `rt` feature; `src-tauri` lists only `time, sync, macros` | Med | Tauri already pulls a multi-thread runtime, so features unify; spawn through `tauri::async_runtime` is not possible in `netcam.rs`, so add `rt` to the tokio features explicitly (no new dependency) |
| Timing tests (frame rate, reconnect) flaky under load | Med | Assert with margins from the spec (9 of 10, 5 s, 10 s), use a local server, no sleeps shorter than 50 ms |
| Local Network prompt behaviour unverified | Low | Only the plist key ships; behaviour is a manual check |

## Open Questions

- Name an IP-camera app in the panel text? Plan assumes no: URL shape only.
