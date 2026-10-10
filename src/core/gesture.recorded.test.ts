/**
 * Replays real landmark recordings (see src/fixtures/gestures.ts) through
 * the classifier and the hold tracker and asserts the fires stated in each
 * recording's expected.json. Skips when no recordings exist yet.
 */

import { describe, expect, it } from "vitest";
import { loadRecordedGestureSessions } from "../fixtures/gestures";
import { classifyGesture } from "./hand";
import { HoldTracker } from "./hold";

const sessions = loadRecordedGestureSessions();

describe.skipIf(sessions.length === 0)("recorded gesture sessions", () => {
  for (const session of sessions) {
    it(`${session.name}: fires exactly ${session.expected.length} time(s) at the expected moments`, () => {
      const tracker = new HoldTracker();
      const fires = session.frames.flatMap((frame) => {
        const output = tracker.next(classifyGesture(frame), frame.t);
        return output.fired ? [{ gesture: output.fired, atMs: frame.t }] : [];
      });
      expect(fires.map((fire) => fire.gesture)).toEqual(
        session.expected.map((fire) => fire.gesture),
      );
      fires.forEach((fire, index) => {
        const expected = session.expected[index];
        expect(Math.abs(fire.atMs - expected.atMs)).toBeLessThanOrEqual(
          expected.toleranceMs,
        );
      });
    });
  }
});
