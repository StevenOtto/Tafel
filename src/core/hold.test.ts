import { describe, expect, it } from "vitest";
import { Gesture } from "./hand";
import { HoldTracker } from "./hold";

const FRAME_MS = 100;

/**
 * Feeds a gesture sequence at 100 ms per frame starting at `startMs`;
 * returns the fired gestures with their frame index within the sequence.
 */
const run = (
  sequence: readonly Gesture[],
  tracker = new HoldTracker(),
  startMs = 0,
): {
  fired: { gesture: Gesture; frame: number }[];
  last: ReturnType<HoldTracker["next"]>;
  endMs: number;
} => {
  const fired: { gesture: Gesture; frame: number }[] = [];
  let last = tracker.next("none", startMs - FRAME_MS);
  sequence.forEach((gesture, frame) => {
    last = tracker.next(gesture, startMs + frame * FRAME_MS);
    if (last.fired) fired.push({ gesture: last.fired, frame });
  });
  return { fired, last, endMs: startMs + sequence.length * FRAME_MS };
};

const repeat = (gesture: Gesture, frames: number): Gesture[] =>
  Array.from({ length: frames }, () => gesture);

describe("hold tracker", () => {
  it("fires exactly once after the gesture is held for 2.0 s", () => {
    // Frames at 0, 100, …, 2000 ms: the frame at 2000 completes the hold.
    const { fired } = run(repeat("thumbs-up", 21));
    expect(fired).toEqual([{ gesture: "thumbs-up", frame: 20 }]);
  });

  it("does not fire when released at 1.9 s", () => {
    const { fired } = run([...repeat("thumbs-up", 20), ...repeat("none", 10)]);
    expect(fired).toEqual([]);
  });

  it("bridges a two-frame detection dropout", () => {
    const sequence = [
      ...repeat("thumbs-up", 10),
      ...repeat("none", 2),
      ...repeat("thumbs-up", 9),
    ];
    const { fired } = run(sequence);
    expect(fired).toEqual([{ gesture: "thumbs-up", frame: 20 }]);
  });

  it("cancels on a dropout longer than the grace period and restarts fresh", () => {
    const sequence = [
      ...repeat("thumbs-up", 10),
      ...repeat("none", 3),
      ...repeat("thumbs-up", 21),
    ];
    const { fired } = run(sequence);
    // The second hold starts at frame 13 and completes 20 frames later.
    expect(fired).toEqual([{ gesture: "thumbs-up", frame: 33 }]);
  });

  it("does not fire again while the gesture stays held after firing", () => {
    const { fired } = run(repeat("open-palm", 60));
    expect(fired).toEqual([{ gesture: "open-palm", frame: 20 }]);
  });

  it("re-arms only after the gesture has been gone for 500 ms", () => {
    const tracker = new HoldTracker();
    const first = run(repeat("fist", 21), tracker);
    expect(first.fired).toHaveLength(1);
    // Gone for 400 ms, then back: no re-arm, no second fire.
    const tooShort = run(
      [...repeat("none", 4), ...repeat("fist", 25)],
      tracker,
      first.endMs,
    );
    expect(tooShort.fired).toEqual([]);
    // Gone for 500 ms, then held for 2 s: fires again.
    const longEnough = run(
      [...repeat("none", 5), ...repeat("fist", 21)],
      tracker,
      tooShort.endMs,
    );
    expect(longEnough.fired).toHaveLength(1);
  });

  it("restarts the hold when the gesture changes midway", () => {
    const sequence = [...repeat("thumbs-up", 10), ...repeat("ok-sign", 21)];
    const { fired } = run(sequence);
    expect(fired).toEqual([{ gesture: "ok-sign", frame: 30 }]);
  });

  it("reports progress that grows linearly toward 1", () => {
    const tracker = new HoldTracker();
    tracker.next("thumbs-up", 0);
    expect(tracker.next("thumbs-up", 500).progress).toBeCloseTo(0.25);
    expect(tracker.next("thumbs-up", 1000).progress).toBeCloseTo(0.5);
    expect(tracker.next("thumbs-up", 2000).progress).toBe(1);
  });

  it("stays idle on none", () => {
    const { last } = run(repeat("none", 5));
    expect(last).toEqual({
      phase: "idle",
      gesture: null,
      progress: 0,
      fired: null,
    });
  });
});
