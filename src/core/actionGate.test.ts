import { describe, expect, it } from "vitest";
import { ActionGate } from "./actionGate";
import { HoldTracker } from "./hold";
import { Gesture } from "./hand";

describe("action gate", () => {
  it("fires the first completed gesture", () => {
    expect(new ActionGate().evaluate("desk", "thumbs-up", 1000)).toBe("fire");
  });

  it("suppresses the same gesture from a second camera within the window", () => {
    const gate = new ActionGate();
    expect(gate.evaluate("desk", "thumbs-up", 1000)).toBe("fire");
    expect(gate.evaluate("wall", "thumbs-up", 1300)).toBe("duplicate");
    // A different gesture from the second camera is not a duplicate.
    expect(gate.evaluate("wall", "open-palm", 1300)).toBe("fire");
  });

  it("lets the same gesture fire again once the dedupe window passes", () => {
    const gate = new ActionGate();
    expect(gate.evaluate("desk", "thumbs-up", 1000)).toBe("fire");
    expect(gate.evaluate("wall", "thumbs-up", 2100)).toBe("fire");
  });

  it("keeps one camera on cooldown for a second after it fires", () => {
    const gate = new ActionGate();
    expect(gate.evaluate("desk", "ok-sign", 1000)).toBe("fire");
    // Same camera, different gesture, still within 1s → cooldown.
    expect(gate.evaluate("desk", "open-palm", 1500)).toBe("cooldown");
    expect(gate.evaluate("desk", "open-palm", 2100)).toBe("fire");
  });
});

/**
 * Criterion 1: a hold on one camera does not affect another. Two independent
 * trackers driven by a shared gate — camera A completing a hold leaves camera
 * B's in-progress hold untouched.
 */
describe("independent per-camera holds", () => {
  const repeat = (gesture: Gesture, n: number): Gesture[] =>
    Array.from({ length: n }, () => gesture);

  it("A firing does not disturb B's progress, and the gate dedupes", () => {
    const gate = new ActionGate();
    const a = new HoldTracker();
    const b = new HoldTracker();
    // A holds thumbs-up to completion over 2s; B holds it but 300ms behind.
    const aSeq = repeat("thumbs-up", 21);
    const bSeq = [...repeat("none", 3), ...repeat("thumbs-up", 21)];

    const results: string[] = [];
    for (let frame = 0; frame < 24; frame += 1) {
      const t = frame * 100;
      const aOut = a.next(aSeq[frame] ?? "thumbs-up", t);
      const bOut = b.next(bSeq[frame] ?? "thumbs-up", t);
      if (aOut.fired) results.push(`A:${gate.evaluate("A", aOut.fired, t)}`);
      if (bOut.fired) results.push(`B:${gate.evaluate("B", bOut.fired, t)}`);
    }
    // A fires first (frame 20), B fires ~frame 23 → deduped as B is within 1s.
    expect(results).toEqual(["A:fire", "B:duplicate"]);
  });
});
