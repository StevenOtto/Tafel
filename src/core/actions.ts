/**
 * Camera status and the gestures that change it. Snapshot and collection
 * gestures are resolved separately in delivery.ts; this module owns only the
 * pause/resume behaviour and the visibility gate, so each concern has one
 * small table and one small test.
 */

import { Gesture, GESTURES } from "./hand";

export const DEFAULT_ENABLED_GESTURES: readonly Gesture[] = [
  "thumbs-up",
  "two-thumbs-up",
  "open-palm",
  "ok-sign",
  "pointing",
];

export type CameraStatus = "watching" | "paused";

/**
 * Gestures a paused camera still listens to. Everything else is filtered out
 * before the hold tracker, so no ring appears for gestures that cannot act.
 */
export const gestureVisibleWhen = (
  gesture: Gesture,
  status: CameraStatus,
  enabled: readonly Gesture[] = GESTURES,
): Gesture => {
  if (!enabled.includes(gesture)) return "none";
  if (status === "watching") return gesture;
  return gesture === "open-palm" ? "open-palm" : "none";
};

/** The status change a fired gesture causes, if any. */
export const statusChange = (
  gesture: Gesture,
  status: CameraStatus,
): CameraStatus | null => {
  if (status === "paused") return gesture === "open-palm" ? "watching" : null;
  return gesture === "fist" ? "paused" : null;
};
