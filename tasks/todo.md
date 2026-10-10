# Tasks: Android phones as Tafel cameras

Verify commands used below:
- Harness: `cargo test` in the scratch crate that includes `src-tauri/src/netcam.rs` via `#[path]`
- TS: `npm test` and `npx tsc --noEmit`
- Mac only: `(cd src-tauri && cargo test)`

## Task 1: Refuse redirects in fetch_frame
**Description:** Build the reqwest client with `redirect::Policy::none()` and treat a 3xx as an error that names the refusal.
**Acceptance criteria:**
- [x] A local URL answering 302 to another host returns an error, and the target server sees no request
- [x] Existing netcam tests still pass
**Verification:** Harness tests pass (new wire test with an axum server that redirects)
**Dependencies:** None
**Files:** `src-tauri/src/netcam.rs`
**Scope:** XS

## Task 2: Incremental multipart parser with a byte bound
**Description:** Replace `first_jpeg` with a small `JpegExtractor` that takes chunks, yields each complete JPEG, keeps at most 8 MB buffered, and errors when the bound is hit without a frame boundary.
**Acceptance criteria:**
- [x] Frames split at every byte offset, including inside FFD8 and FFD9, come out whole and in order
- [x] A buffer over 8 MB without a frame boundary is an error
**Verification:** Harness unit tests pass
**Dependencies:** None
**Files:** `src-tauri/src/netcam.rs`
**Scope:** S

## Task 3: Reader and registry
**Description:** One tokio task per URL keeps a GET open, feeds chunks to the extractor, and stores the newest frame with a sequence number and timestamp. Non-multipart responses fall back to snapshot polling. Connect timeout 4 s, idle timeout 4 s, retry after 5 s, `host_is_local` before every connection, stop when the URL has not been requested for 10 s.
**Acceptance criteria:**
- [x] One connection per URL for a live stream (server counts connections)
- [x] 10 fps source with 150 ms latency: at least 9 frames per second arrive
- [x] Stream cut: latest read errors within 5 s; stream back: frames resume within 10 s
**Verification:** Harness wire tests pass
**Dependencies:** Tasks 1, 2
**Files:** `src-tauri/src/netcam.rs`, `src-tauri/Cargo.toml` (tokio `rt` feature)
**Scope:** M

## Task 4: latest_network_frame command and tile poll loop
**Description:** Replace `fetch_network_frame` with `latest_network_frame(url) -> { jpegB64, seq }` backed by the registry in `AppState`; the tile polls every 50 ms and decodes only a new `seq`.
**Acceptance criteria:**
- [x] No other caller of `fetch_network_frame` remains
- [x] Tile shows `Disconnected` when the command errors, as today
**Verification:** `npx tsc --noEmit`, `npm test`; Mac: `cargo test` and a live Android URL
**Dependencies:** Task 3
**Files:** `src-tauri/src/lib.rs`, `src/app/tauri.ts`, `src/app/CameraTile.tsx`
**Scope:** S

## Checkpoint: Core
- [x] All harness and TS checks pass
- [ ] Human runs `cargo test` on a Mac and one real Android MJPEG URL

## Task 5: Android guidance in the Network camera panel
**Description:** Short hint under the Stream URL field per spec criterion 8, no app brand named.
**Acceptance criteria:** [ ] Text covers MJPEG URL example, 1080p, fixed IP, USB webcam route
**Verification:** `npx tsc --noEmit`, `npm test`
**Dependencies:** None
**Files:** `src/app/ControlApp.tsx`
**Scope:** XS

## Task 6: NSLocalNetworkUsageDescription
**Acceptance criteria:** [ ] Key present with a one-sentence reason naming network cameras
**Verification:** `plutil -lint` on a Mac; XML well-formed check here
**Dependencies:** None
**Files:** `src-tauri/Info.plist`
**Scope:** XS

## Task 7: README and DESIGN.md
**Acceptance criteria:** [ ] Both describe the persistent reader, redirect refusal, and Android routes
**Verification:** Read-through
**Dependencies:** Tasks 1 to 6
**Files:** `README.md`, `DESIGN.md`
**Scope:** S
