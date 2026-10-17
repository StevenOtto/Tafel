# Linux and Windows: research for feature parity

`2026-10-17` · research only, no product code changed · status: awaiting
decisions (see "Open decisions")

## Question

What does it take to run Tafel on Linux and Windows with the same features
as on macOS? The working assumption was that hand gestures are the main
challenge and that Tauri makes the rest easy.

## Short answer

The assumption is half right. Getting hand landmarks on Linux and Windows is
solved and measured below: Google's MediaPipe hand models produce the same 21
joints as Apple Vision, and on the two photos tested the unchanged
`src/core/hand.ts` classified them correctly. The open problem is behavioral parity: the
classifier's thresholds were tuned on Vision landmarks, MediaPipe reports no
per-joint confidence, and no recorded gesture fixtures exist to compare
against. That is calibration work, and it needs real desk recordings.

The rest is mostly as easy as hoped, with one exception. The Rust core
compiles on Linux after about 15 changed lines, and all 63 non-Vision Rust
tests and all 77 frontend tests pass there. Cameras on Linux do not work as
the code stands: WebKitGTK refuses `getUserMedia` with `NotAllowedError`.
Tauri 2.12 adds a permission handler that fixes this (verified with
WebKitGTK's mock camera), but behavior with real cameras through GStreamer is
unverified. Windows uses WebView2, which is Chromium, and is the easier
platform.

## What is macOS-specific today

| Area                     | Where                                                                                                                                 | Linux / Windows                                                                                                    |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Hand landmarks           | `src-tauri/src/vision.rs`, `swift/TafelVision`, `build.rs`                                                                            | New backend needed (below)                                                                                         |
| Build gate               | `src-tauri/src/lib.rs:1` (`compile_error!`), `swift-rs` in `Cargo.toml`, Swift linking in `build.rs`                                  | Gate on target OS; done in the experiment below                                                                    |
| Camera capture           | `src/app/cameraSource.ts` (`getUserMedia` in the webview)                                                                             | Works in WebView2; WebKitGTK needs a permission handler and GStreamer plugins                                      |
| Window chrome            | `tauri.conf.json`: `macOSPrivateApi`, `titleBarStyle: Overlay`, `transparent`, `windowEffects: sidebar`                               | Move to `tauri.macos.conf.json`; opaque window with native decorations elsewhere                                   |
| Styling                  | `src/styles.css:1-5,53` (transparent body over vibrancy), `:77` (toolbar under the traffic lights)                                    | Opaque background and no traffic-light inset on other platforms                                                    |
| Wording                  | "this Mac" in `src/app/ControlApp.tsx:1131,1326,1445`, `README.md`, `site/`                                                           | "this computer"                                                                                                    |
| Session storage location | `app_data_dir()` in `src-tauri/src/lib.rs` (setup)                                                                                    | On Windows this is Roaming AppData; session images belong in `app_local_data_dir()` (same path on macOS and Linux) |
| Release                  | `.github/workflows/release.yml` (one macOS job, Developer ID signing, notarization, DMG); site download button `site/index.html:1401` | Per-OS jobs, packages and signing; per-OS download links                                                           |
| Not portable by nature   | Continuity Camera (`Info.plist`), vibrancy                                                                                            | Accepted gaps; Windows 11 Phone Link can expose an Android phone as a webcam; network cameras work everywhere      |

Portable as is: the MCP server on `127.0.0.1`, the delivery bus, sessions,
settings, network-camera readers and their local-network checks, the gesture
classifier, hold tracker and action gate, Web Audio tones, and IPC.

## Hand landmarks: options

All three options use MediaPipe's hand models (Apache 2.0): a palm detector
(192 x 192 input) and a landmark model (224 x 224 input, 21 joints). Their
joint order matches `JOINT_NAMES` in `src/core/hand.ts` one to one, so index
`i` maps to `JOINT_NAMES[i]`. Coordinates are frame fractions with the origin
top-left, as Tafel already expects.

**A. MediaPipe's native library through Rust FFI (recommended).** MediaPipe
1.x (1.1.1, 2026-10-14) ships a C library, `libmediapipe`, with a versioned
C API (`MpHandLandmarkerCreate`, `MpHandLandmarkerDetectImage`,
`MpHandLandmarkerDetectForVideo`, symbol version `VERS_1.0`). The official
Python package is now a thin `ctypes` wrapper over it. Prebuilt binaries exist
as wheels for Linux x86_64 and aarch64, Windows x64 and arm64, and macOS
arm64 (the library inside was checked for Linux x86_64 and Windows x64). The
header is upstream at `mediapipe/tasks/c/vision/hand_landmarker/hand_landmarker.h`.
It gives the full official pipeline (detection, tracking, XNNPACK) for about
200 lines of FFI and wrapper. Costs: the prebuilt library holds every
MediaPipe task and is large (Windows DLL 62 MB; Linux `.so` 94 MB stripped,
124 MB as shipped). The Linux build links `libEGL.so.1` and `libGLESv2.so.2`
even on CPU. I found no documentation presenting this C API as a supported
desktop SDK, so its stability is an inference from the symbol versioning.
Upstream Bazel builds `libmediapipe.so` from source and already has
vision-only targets for its Apple frameworks, so a smaller vision-only build
looks possible, at the cost of a Bazel build in CI.

**B. Own two-stage pipeline in Rust over ONNX.** The `.tflite` models convert
to ONNX with `tf2onnx`; outputs match the originals to a relative difference
of 7e-6. Pure-Rust runtimes (`rten` 0.27, `tract` 0.23.9) load and run both
models with no native dependency, and their outputs match ONNX Runtime to 5
significant digits. Costs: Tafel would reimplement anchor decoding (2016
anchors), non-max suppression, the rotated hand crop, projection back to the
frame and tracking, likely 600 to 900 lines, with the risk of subtly worse
landmarks than the official pipeline. Pure Rust is also 2 to 3 times slower
than XNNPACK (table below). The `ort` crate (ONNX Runtime) is faster but is
still a release candidate (2.0.0-rc.14) and ships a native library.

**C. MediaPipe's WebAssembly build in the webview (not recommended).** It
runs under a strict CSP with only `'wasm-unsafe-eval'` added (verified, no
violations). On CPU it takes 76 to 125 ms per frame in Chromium, so one
camera at 10 fps would saturate the webview's main thread. The GPU delegate
would be needed, and WebGL on WebKitGTK is the least reliable part of the
Linux stack. It also widens the webview CSP, a boundary in `AGENTS.md`, and
moves landmark work out of Rust, against the current layering.

**macOS: keep Apple Vision (recommended).** MediaPipe ships only arm64 for
macOS, so Intel Macs would lose gestures. Vision adds nothing to the bundle,
and its behavior is the reference the classifier was tuned on. The cost is
two backends that must stay in parity, which the harness under "Plan" step 4
addresses.

## Measurements

Environment: this cloud container, Linux x86_64, 4 vCPU Intel Xeon at 2.1
GHz. A current laptop is likely faster per core; treat absolute numbers as an
upper bound, not a prediction. Model files: `hand_landmarker.task`, float16,
7.8 MB.

Per model, median (p50) over 100 to 200 runs on a fixed input:

| Runtime                      | Threads | Palm detector | Landmark model |
| ---------------------------- | ------- | ------------- | -------------- |
| LiteRT (XNNPACK), `.tflite`  | 1       | 9.2 ms        | 7.4 ms         |
| LiteRT (XNNPACK), `.tflite`  | 4       | 3.8 ms        | 3.0 ms         |
| ONNX Runtime 1.31, converted | 1       | 16.3 ms       | 8.4 ms         |
| rten 0.27, converted         | 1       | 28.7 ms       | 21.2 ms        |
| rten 0.27, converted         | 4       | 26.2 ms       | 15.8 ms        |
| tract 0.23.9, converted      | 1       | 32.8 ms       | 25.9 ms        |

End to end, through `libmediapipe` (HandLandmarker, two hands max):

| Input                          | Image mode | Video mode (tracking) | Webview WASM, CPU (image / video) |
| ------------------------------ | ---------- | --------------------- | --------------------------------- |
| One hand, thumbs up, 382 x 406 | 25.3 ms    | 24.5 ms               | 76.2 / 79.2 ms                    |
| Two relaxed hands, 640 x 960   | 33.1 ms    | 22.8 ms               | 124.9 / 79.7 ms                   |

For comparison, `DESIGN.md` gives Apple Vision 20 to 40 ms on Apple silicon.
At 10 fps, 25 ms per frame is 25% of one core per camera on this machine.
Video mode only saves time when both hands are tracked, because MediaPipe
reruns the palm detector while fewer than `num_hands` hands are found.

Classifier check: the landmarks from MediaPipe's own test photos, mapped to
Tafel's joint names and passed through the unmodified `classifyHand` and
`classifyGesture`, gave `thumbs-up` for the thumbs-up photo and `none` for a
photo of two relaxed, half-curled hands. Both are correct. Two images prove
the data path, not parity.

## Parity risks in the gesture path

1. **No per-joint confidence.** Vision scores each joint, and `hand.ts`
   drops joints below 0.3, so an occluded finger yields `none`. MediaPipe
   scores the hand, not the joints, and always returns all 21 joints,
   guessing the hidden ones. Mapping the hand score onto every joint keeps
   the code unchanged but removes that safety net. The likely effect is more
   false positives from pen grips and hands flattening paper.
2. **Thresholds tuned on Vision.** The finger ratios (1.15 and 0.95) and the
   palm fractions (0.6, 0.35, 0.3) came from Vision landmarks. MediaPipe
   places some joints differently (the thumb base in particular), so a
   gesture near a threshold may read differently per OS.
3. **Desk viewpoints.** Cameras look down at the backs of hands. Neither
   model's behavior from that angle is measured here.
4. **No ground truth in the repository.** `fixtures/gestures/` does not
   exist, so `src/core/gesture.recorded.test.ts` is skipped. Parity cannot be
   measured until recordings exist.
5. Chirality is not used by the classifier, so MediaPipe's handedness
   convention (it assumes a mirrored selfie image) does not matter.

## Cameras and permissions

**Linux (WebKitGTK).** Reproduced with the current code under Tauri 2.12.4 and
WebKitGTK 2.52.6: `getUserMedia` fails with `NotAllowedError`, because wry
answers WebKitGTK's `permission-request` signal only when a permission handler
is set, and unanswered requests are denied. `enable-media-stream` is already
on by default in 2.52, and the `tauri://localhost` origin is a secure
context. With Tauri 2.12's `WebviewWindowBuilder::on_permission_request`
returning `Allow` for `PermissionKind::Camera` (and `Default` for everything
else), the same build opened WebKitGTK's mock camera at 2560 x 1440 and
`enumerateDevices` listed both mock devices with labels. Requirements that
follow:

- Upgrade Tauri from 2.11.5 (current `Cargo.lock`) to 2.12.x. The handler
  exists only on the window builder, so the control window moves from
  `tauri.conf.json` to code: keep its config, set `"create": false`, and
  build it in `setup` with `WebviewWindowBuilder::from_config`.
- Real cameras need a GStreamer source in the WebKit process: `v4l2src` from
  gst-plugins-good, or the PipeWire plugin. Package dependencies must declare
  them.
- Unverified until tested on hardware: 1080p MJPEG from V4L2, several cameras
  at once, hot-plug (`devicechange`), and cameras that only one process can
  open at a time under plain V4L2.
- Fallback if WebKitGTK capture proves unreliable: capture in Rust and hand
  frames to the webview, as network cameras already do through
  `latest_network_frame`.
- Known WebKitGTK rendering issues on NVIDIA (blank or grey windows) are
  usually worked around with `WEBKIT_DISABLE_DMABUF_RENDERER=1` on X11. Not
  reproduced here.

**Windows (WebView2).** `getUserMedia` is Chromium's. Without a handler
WebView2 shows its own permission prompt; the same Tauri handler grants it
silently. The origin is `http://tauri.localhost`; `*.localhost` counts as a
secure context in Chromium (not verified on Windows). Windows also has a
system privacy switch for desktop camera access. Before the multi-app camera
option (Windows 11, March 2025 update, opt-in per camera), only one app can
hold a camera, so Tafel and a video call would conflict.

**macOS.** No change needed; whether to apply the same handler there is a
consistency choice, not a fix.

## MCP clients

Claude Desktop runs on Windows and, since mid-2026, on Linux as a beta for
Ubuntu and Debian. Claude Code runs on all three. Whether the Linux desktop
beta renders the MCP App card (`src-tauri/mcp-app/delivery.html`) is
unverified.

## Verification done for this research

All experiments ran outside the working tree, in a scratch copy of `HEAD`
(`f07df21`), so nothing in the repository changed except this file.

- Build split: gated the Swift linking in `build.rs` on
  `CARGO_CFG_TARGET_OS`, moved `swift-rs` under
  `[target.'cfg(target_os = "macos")'.dependencies]`, removed the
  `compile_error!`, and stubbed `vision::detect_hands` on other platforms.
  `cargo test` on Linux: 63 passed, 0 failed (the 3 Vision tests are
  macOS-only). `npm test`: 77 passed, 1 file skipped (no recordings).
  `npx tsc --noEmit`: clean.
- Camera probe: the scratch copy upgraded to Tauri 2.12.4, the window built in
  `setup`, WebKitGTK mock capture devices on, run under Xvfb. Results as
  above, with and without the handler.
- Models: `hand_landmarker.task` from Google's model storage, converted with
  `tf2onnx` 1.17 (opset 17), timed with LiteRT, ONNX Runtime 1.31, `rten`
  0.27.0 and `tract-onnx` 0.23.9. End-to-end runs used `mediapipe` 1.1.1
  from PyPI on MediaPipe's own test images (`thumb_up.jpg`,
  `woman_hands.jpg`); the WASM run used `@mediapipe/tasks-vision` 1.1.1 in
  headless Chromium with the strict CSP plus `'wasm-unsafe-eval'`.
- Not done: any build or run on Windows; any real camera on Linux; any
  native (non-mock) GStreamer capture; Rust FFI to `libmediapipe` (exercised
  through the official Python wrapper over the same C API instead); packaging.

## Plan (if option A and Vision-on-macOS are accepted)

1. **Build split and CI.** Land the gating from the experiment, with
   `vision.rs` split into an Apple backend and a MediaPipe backend behind the
   unchanged `detect_hands` command and `Detection` type. Add a CI workflow
   running `cargo test` and `npm test` on Linux and Windows; none exists
   today.
2. **MediaPipe backend.** FFI wrapper over `MpHandLandmarker` in image mode,
   which keeps `detect_hands` stateless (video mode would need a camera id and
   timestamp per call and saves little, per the table). Map the hand score
   onto every joint's `confidence`. Rust tests mirror the existing Vision
   tests: a blank image yields no hands, garbage bytes yield an error, and an
   optional local hand photo yields 21 joints.
3. **Window, camera and copy.** Tauri 2.12, window built in code with the
   camera permission handler, platform config files, opaque styling and
   native decorations off macOS, "this computer" wording, session storage in
   `app_local_data_dir()`.
4. **Parity harness on a Mac.** MediaPipe also runs on Apple silicon, so one
   Mac can run both backends over the same frames. Record short JPEG
   sequences of each gesture, plus pen grips and paper flattening as
   negatives, from real desk and wall cameras. Keep them local and untracked
   like `fixtures/hand.jpg`, since they are private desk images. Compare
   gesture output per frame between Vision and MediaPipe, then tune or split
   thresholds until the fire sequences match. Commit only the derived
   landmark JSONL fixtures, if the user agrees they contain nothing private.
5. **Packaging and release.** Linux `.deb` and AppImage first (WebKitGTK,
   GStreamer camera plugins, EGL; Tauri's AppImage option to bundle the media
   framework), Windows NSIS installer with code signing, a release matrix,
   and per-OS download links on the site.

Rough sizing, a guess rather than a measurement: steps 1 to 3 take 5 to 8
working days; step 4 takes 3 to 5 days and is open-ended, because its outcome
depends on what the recordings show; step 5 takes 3 to 5 days plus waiting
time for a Windows signing identity.

## Open decisions

1. Landmark backend for Linux and Windows: A (MediaPipe library through FFI,
   recommended), B (own Rust pipeline over ONNX), or C (WASM in the webview,
   which also requires widening the CSP).
2. macOS backend: keep Apple Vision (recommended) or switch to MediaPipe
   everywhere (one backend, but drops gestures on Intel Macs).
3. For A: ship the prebuilt library from the official release (62 to 94 MB
   per platform) or build a vision-only library with Bazel in CI (smaller,
   more CI work). Prebuilt is enough for a first working build either way.
4. Window chrome off macOS: native decorations with an opaque window
   (recommended, least code) or custom chrome with Mica on Windows 11.
5. Order: Windows first (lower camera risk, more users) or Linux first
   (testable in CI and in cloud sessions).
6. Distribution: which Linux formats, and which Windows signing route.

## Sources

- Tauri 2.12 changelog, permission handler exposed (#14865):
  https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/CHANGELOG.md
- wry changelog, expanded permission API for WebView2, WKWebView and
  WebKitGTK (#1654): https://github.com/tauri-apps/wry/blob/dev/CHANGELOG.md
- Camera failing on Linux under Tauri, analysis of the unanswered
  permission request (October 2026):
  https://github.com/jfolcini/agaric/issues/5386
- WebKitGTK media setup and GStreamer plugin requirements (September 2026):
  https://github.com/zackees/kernal-api/issues/154
- MediaPipe C API header:
  https://github.com/google-ai-edge/mediapipe/blob/master/mediapipe/tasks/c/vision/hand_landmarker/hand_landmarker.h
- MediaPipe on PyPI (1.1.1, Apache 2.0): https://pypi.org/project/mediapipe/
- MediaPipe hand landmarker guide:
  https://developers.google.com/edge/mediapipe/solutions/vision/hand_landmarker
- WebView2 permission prompt default and handling:
  https://v3.wails.io/features/windows/permissions
- Windows 11 multi-app camera:
  https://pureinfotech.com/allow-multiple-apps-use-camera-windows-11/
- Claude Desktop for Linux beta: https://claude.com/download
