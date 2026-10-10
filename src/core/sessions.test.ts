import { describe, expect, it } from "vitest";
import {
  Session,
  byRecentActivity,
  formatBytes,
  sessionLabel,
} from "./sessions";

const session = (over: Partial<Session>): Session => ({
  id: "1",
  name: "Kiosk",
  provisional: false,
  createdMs: 1_000,
  lastDeliveryMs: null,
  deliveryCount: 0,
  ...over,
});

describe("sessionLabel", () => {
  it("uses the name of a named session", () => {
    expect(sessionLabel(session({ name: "Kiosk" }))).toBe("Kiosk");
  });

  it("labels a provisional session by its start", () => {
    const label = sessionLabel(
      session({ provisional: true, name: "", createdMs: Date.UTC(2026, 8, 9) }),
    );
    expect(label.startsWith("Unnamed · ")).toBe(true);
    expect(label).toMatch(/Sep/);
  });
});

describe("byRecentActivity", () => {
  it("puts the most recently active session first, falling back to creation", () => {
    const old = session({ id: "old", createdMs: 1, lastDeliveryMs: 50 });
    const fresh = session({
      id: "fresh",
      createdMs: 100,
      lastDeliveryMs: null,
    });
    const busy = session({ id: "busy", createdMs: 2, lastDeliveryMs: 500 });
    expect(byRecentActivity([old, fresh, busy]).map((s) => s.id)).toEqual([
      "busy",
      "fresh",
      "old",
    ]);
  });

  it("does not mutate its input", () => {
    const input = [session({ id: "a" }), session({ id: "b", createdMs: 5 })];
    byRecentActivity(input);
    expect(input.map((s) => s.id)).toEqual(["a", "b"]);
  });
});

describe("formatBytes", () => {
  it("picks a unit that reads well", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(20 * 1024)).toBe("20 KB");
    expect(formatBytes(1.5 * 1024 ** 2)).toBe("1.5 MB");
    expect(formatBytes(2.25 * 1024 ** 3)).toBe("2.25 GB");
  });
});
