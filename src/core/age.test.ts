import { describe, expect, it } from "vitest";
import { agePhrase } from "./age";

describe("agePhrase", () => {
  it("calls anything under five seconds 'just now'", () => {
    expect(agePhrase(1000, 1000)).toBe("just now");
    expect(agePhrase(1000, 5999)).toBe("just now");
  });

  it("counts seconds, then minutes, then hours", () => {
    expect(agePhrase(0, 12_000)).toBe("12 s ago");
    expect(agePhrase(0, 59_999)).toBe("59 s ago");
    expect(agePhrase(0, 60_000)).toBe("1 min ago");
    expect(agePhrase(0, 59 * 60_000)).toBe("59 min ago");
    expect(agePhrase(0, 3_600_000)).toBe("1 h ago");
  });

  it("never goes negative when clocks disagree", () => {
    expect(agePhrase(5000, 1000)).toBe("just now");
  });
});
