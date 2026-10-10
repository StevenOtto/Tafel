import { describe, expect, it } from "vitest";
import { gestureVisibleWhen, statusChange } from "./actions";
import { GESTURES } from "./hand";

describe("status changes", () => {
  it("fist pauses while watching, open hand does nothing", () => {
    expect(statusChange("fist", "watching")).toBe("paused");
    expect(statusChange("open-palm", "watching")).toBeNull();
    expect(statusChange("thumbs-up", "watching")).toBeNull();
  });

  it("only an open hand resumes while paused", () => {
    for (const gesture of GESTURES) {
      expect(statusChange(gesture, "paused")).toBe(
        gesture === "open-palm" ? "watching" : null,
      );
    }
  });
});

describe("visibility gate", () => {
  it("hides every gesture but the open hand while paused", () => {
    for (const gesture of GESTURES) {
      expect(gestureVisibleWhen(gesture, "paused")).toBe(
        gesture === "open-palm" ? "open-palm" : "none",
      );
      expect(gestureVisibleWhen(gesture, "watching")).toBe(gesture);
    }
  });
});

it("filters disabled gestures before hold tracking while keeping opted-in gestures", () => {
  expect(
    gestureVisibleWhen("fist", "watching", ["thumbs-up", "open-palm"]),
  ).toBe("none");
  expect(gestureVisibleWhen("fist", "watching", ["fist"])).toBe("fist");
  expect(gestureVisibleWhen("open-palm", "paused", [])).toBe("none");
  expect(gestureVisibleWhen("open-palm", "paused", ["open-palm"])).toBe(
    "open-palm",
  );
  expect(gestureVisibleWhen("thumbs-up", "paused", ["thumbs-up"])).toBe("none");
});
