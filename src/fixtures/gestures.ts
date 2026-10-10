/**
 * Recorded gesture sessions: the instrument panel's Record button writes one
 * JSONL line per analysed frame (`{ "t": ms, "hands": [...] }`), and a
 * sidecar `expected.json` states which gestures fired and roughly when.
 * Node/test-side only — the app never imports this.
 */

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { Detection, Gesture } from "../core/hand";

export interface RecordedFrame {
  readonly t: number;
  readonly hands: Detection["hands"];
}

export interface ExpectedFire {
  readonly gesture: Gesture;
  /** Approximate time of the fire in ms since the recording started. */
  readonly atMs: number;
  /** Allowed deviation from `atMs`. */
  readonly toleranceMs: number;
}

export interface RecordedGestureSession {
  readonly name: string;
  readonly frames: readonly RecordedFrame[];
  readonly expected: readonly ExpectedFire[];
}

export const GESTURE_FIXTURES_DIR = join(process.cwd(), "fixtures", "gestures");

/** Lines of the recording as written by the app. */
export const parseRecording = (jsonl: string): RecordedFrame[] =>
  jsonl
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line) => JSON.parse(line) as RecordedFrame);

/**
 * Every `fixtures/gestures/<name>/` directory holding a `frames.jsonl` and an
 * `expected.json`. Empty when nothing has been recorded yet.
 */
export const loadRecordedGestureSessions = (
  root = GESTURE_FIXTURES_DIR,
): RecordedGestureSession[] => {
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => join(root, entry.name))
    .filter(
      (dir) =>
        existsSync(join(dir, "frames.jsonl")) &&
        existsSync(join(dir, "expected.json")),
    )
    .map((dir) => ({
      name: dir.slice(root.length + 1),
      frames: parseRecording(readFileSync(join(dir, "frames.jsonl"), "utf8")),
      expected: (
        JSON.parse(readFileSync(join(dir, "expected.json"), "utf8")) as {
          fires: ExpectedFire[];
        }
      ).fires,
    }));
};
