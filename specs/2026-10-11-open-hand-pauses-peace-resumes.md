# Open hand pauses, peace sign resumes, the fist does nothing

Supersedes the fist-to-pause parts of `2026-09-21-vuist-pauzeert.md` and the
"fist off by default" decision in `2026-10-09-visual-prompting/`.

## Problem

A hand holding a pen classifies as a fist (four fingers curled, thumb
tucked), so fist-to-pause could pause a camera while the user was writing.
It was made opt-in, which meant pausing by gesture was off for almost
everyone, and every doc needed a caveat.

## Decision

- An **open hand** ✋ pauses, the universal stop sign. A **peace sign** ✌️
  resumes and is the only gesture a paused camera listens to. The first
  version of this change had them the other way round; the user swapped
  them so the stop gesture means stop.
- The peace sign: index and middle extended and spread into a V, ring and
  little curled. The thumb is not checked, because a thumb resting on the
  curled fingers sits at an unpredictable distance from the index knuckle.
- The spread rule (fingertips more than 0.3 × palm length apart) keeps it
  apart from two fingers side by side, which is how the index and middle
  rest on a pen. The existing side-by-side test shape still yields `none`.
- A fist is now `none`.
- Both are on by default like every other gesture, so no caveat remains in
  the README, the app's settings or the site.
- Trade-off: a hand flattening paper can read as an open hand. The two-second
  hold and the ring guard against that; if it happens in practice, the open
  hand can be switched off and the Pause button used instead.

## Settings

Settings now store `disabledGestures` (the gestures switched off) instead of
`enabledGestures`, so a gesture added later starts on. A 0.1 file is
migrated on load: each gesture that existed then and is missing from
`enabledGestures` becomes disabled, `fist` is ignored, and `peace-sign`
starts on. `enabledGestures` is never written back.

## Changed files

- `src/core/hand.ts`, `hand.test.ts`: `peace-sign` replaces `fist`.
- `src/core/actions.ts`, `actions.test.ts`: open hand pauses, peace sign
  resumes;
  `enabledGestures(disabled)` replaces `DEFAULT_ENABLED_GESTURES`.
- `src/core/hold.test.ts`, `delivery.test.ts`: fixtures renamed.
- `src/app/ControlApp.tsx`, `CameraTile.tsx`, `GestureOverlay.tsx`,
  `tauri.ts`: guide row, settings switches and labels.
- `src-tauri/src/settings.rs`: `disabled_gestures` with legacy migration.
- `README.md`, `DESIGN.md`, `site/index.html`: gesture tables and FAQ.

## Verification

- `npx tsc --noEmit`: clean.
- `npm test`: 77 passed (1 recorded-fixture file skipped as before).
- `cargo test --manifest-path src-tauri/Cargo.toml`: 66 passed.

## Pending manual checks

- A real ✋ pauses and a real ✌️ resumes, front-on and from the overhead desk
  angle; `Show landmarks` helps if one does not.
- Writing, drawing and flattening paper for a few minutes never pauses a
  camera.
- An existing 0.1 settings file keeps its switches after upgrading.
