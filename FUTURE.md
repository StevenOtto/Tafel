# Out of scope for now — proposed, not built

Anything not required by the current specs in `specs/` lands here instead of
in the codebase. Items that grew into a spec are listed there, not here.

- **Fiducial markers / desk calibration.** Zones are fixed fractions aligned
  once with a taped line. If that proves too rigid, ArUco-style corner
  markers would make zones follow the paper.
- **Sessions over MCP.** Sessions persist (`specs/2026-09-14-sessies.md`),
  but the agent sees only the current one. `list_sessions` and a
  `search_deliveries(query, session?)` would let it reach back into last
  week's work — but on-device OCR is gone (2026-09-14), so the index would
  have to come from elsewhere, e.g. the agent naming or tagging a delivery
  when it uses one. Worth it once there is a corpus.
- **Moving deliveries between sessions, retention.** Adoption on naming should
  make mis-filing rare; a move is the first thing to add if it is not. Storage
  is shown but never pruned — an automatic deleter of the user's drawings
  needs more trust than a first version has.
- **Threshold tuning UI.** Detector thresholds are constants surfaced in the
  instrument panel; making them adjustable live (with persistence) is the
  obvious next instrument feature.
- **True push into arbitrary apps.** The Wispr mechanic — trigger → simulated
  paste of the snapshot into the frontmost app — would work beyond Claude, but
  is a separate, opt-in feature with its own permission posture.
- **Bundled MCP App SDK build step.** The in-Claude UI vendors the official
  ext-apps browser bundle as a checked-in file; a vite single-file build
  would track upstream automatically at the cost of a second build pipeline.
- **Create ML gesture classifier.** The gesture classifier is heuristic
  geometry over Vision landmarks; a trained hand-pose classifier is the
  upgrade path if the heuristics prove brittle across hands and lighting.
- **Tafel running on Android.** Tauri 2 targets Android, but hand detection
  would move from Apple Vision to MediaPipe, and the loopback MCP endpoint on
  a phone is out of reach for Claude on the Mac. Android phones join as
  cameras instead (`specs/2026-09-22-android-als-camera.md`).
