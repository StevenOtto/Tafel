import { describe, expect, it } from "vitest";
import { enabledGestures, gestureVisibleWhen, statusChange } from "./actions";
import { GESTURES } from "./hand";

describe("status changes", () => {
  it("open hand pauses while watching, peace sign does nothing", () => {
    expect(statusChange("open-palm", "watching")).toBe("paused");
    expect(statusChange("peace-sign", "watching")).toBeNull();
    expect(statusChange("thumbs-up", "watching")).toBeNull();
  });

  it("only a peace sign resumes while paused", () => {
    for (const gesture of GESTURES) {
      expect(statusChange(gesture, "paused")).toBe(
        gesture === "peace-sign" ? "watching" : null,
      );
    }
  });
});

describe("visibility gate", () => {
  it("hides every gesture but the peace sign while paused", () => {
    for (const gesture of GESTURES) {
      expect(gestureVisibleWhen(gesture, "paused")).toBe(
        gesture === "peace-sign" ? "peace-sign" : "none",
      );
      expect(gestureVisibleWhen(gesture, "watching")).toBe(gesture);
    }
  });
});

it("filters disabled gestures before hold tracking", () => {
  expect(
    gestureVisibleWhen("peace-sign", "watching", ["thumbs-up", "open-palm"]),
  ).toBe("none");
  expect(gestureVisibleWhen("peace-sign", "watching", ["peace-sign"])).toBe(
    "peace-sign",
  );
  expect(gestureVisibleWhen("peace-sign", "paused", [])).toBe("none");
  expect(gestureVisibleWhen("peace-sign", "paused", ["peace-sign"])).toBe(
    "peace-sign",
  );
  expect(gestureVisibleWhen("thumbs-up", "paused", ["thumbs-up"])).toBe("none");
});

it("turns the switched-off list into the gestures that may act", () => {
  expect(enabledGestures([])).toEqual(GESTURES);
  const enabled = enabledGestures(["peace-sign", "pointing"]);
  expect(enabled).not.toContain("peace-sign");
  expect(enabled).not.toContain("pointing");
  expect(enabled).toContain("thumbs-up");
});
