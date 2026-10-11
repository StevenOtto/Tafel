import { describe, expect, it } from "vitest";
import {
  Hand,
  HandPoint,
  JointName,
  classifyGesture,
  classifyHand,
  handBounds,
} from "./hand";

type FingerShape = "extended" | "curled";
type ThumbShape = "extended" | "tucked" | "ok";

interface HandShape {
  readonly thumb: ThumbShape;
  readonly index: FingerShape;
  readonly middle: FingerShape;
  readonly ring: FingerShape;
  readonly little: FingerShape;
}

const point = (x: number, y: number, confidence = 0.9): HandPoint => ({
  x,
  y,
  confidence,
});

/**
 * Synthetic right hand seen palm-on, wrist at the bottom, fingers pointing
 * up. Palm (wrist → middle MCP) is 0.20 tall; finger joints are laid out
 * along the finger's axis so extension and curl are unambiguous.
 */
const syntheticHand = (shape: HandShape): Hand => {
  const wrist = point(0.5, 0.8);
  const points: Partial<Record<JointName, HandPoint>> = { wrist };

  const finger = (
    prefix: "index" | "middle" | "ring" | "little",
    x: number,
    state: FingerShape,
  ) => {
    const mcpY = 0.6;
    points[`${prefix}Mcp`] = point(x, mcpY);
    if (state === "extended") {
      points[`${prefix}Pip`] = point(x, mcpY - 0.07);
      points[`${prefix}Dip`] = point(x, mcpY - 0.12);
      points[`${prefix}Tip`] = point(x, mcpY - 0.16);
      return;
    }
    // Curled: the tip folds back toward the palm, below the PIP joint.
    points[`${prefix}Pip`] = point(x, mcpY - 0.05);
    points[`${prefix}Dip`] = point(x + 0.01, mcpY - 0.01);
    points[`${prefix}Tip`] = point(x + 0.01, mcpY + 0.03);
  };
  finger("index", 0.44, shape.index);
  finger("middle", 0.49, shape.middle);
  finger("ring", 0.54, shape.ring);
  finger("little", 0.59, shape.little);

  points.thumbCmc = point(0.42, 0.74);
  points.thumbMp = point(0.37, 0.68);
  if (shape.thumb === "extended") {
    points.thumbIp = point(0.33, 0.62);
    points.thumbTip = point(0.29, 0.56);
  } else if (shape.thumb === "tucked") {
    points.thumbIp = point(0.4, 0.64);
    points.thumbTip = point(0.45, 0.62);
  } else {
    // OK sign: thumb tip meets the index tip.
    const indexTip = points.indexTip as HandPoint;
    points.thumbIp = point(indexTip.x - 0.05, indexTip.y + 0.06);
    points.thumbTip = point(indexTip.x - 0.01, indexTip.y + 0.01);
  }
  return { chirality: "right", points };
};

const THUMBS_UP: HandShape = {
  thumb: "extended",
  index: "curled",
  middle: "curled",
  ring: "curled",
  little: "curled",
};
const OPEN_PALM: HandShape = {
  thumb: "extended",
  index: "extended",
  middle: "extended",
  ring: "extended",
  little: "extended",
};
/** Every finger curled, thumb tucked: also how a hand holds a pen. */
const FIST: HandShape = {
  thumb: "tucked",
  index: "curled",
  middle: "curled",
  ring: "curled",
  little: "curled",
};
const OK_SIGN: HandShape = {
  thumb: "ok",
  index: "curled",
  middle: "extended",
  ring: "extended",
  little: "extended",
};
/** Index and middle up side by side, others down — maps to nothing. */
const NEUTRAL: HandShape = {
  thumb: "tucked",
  index: "extended",
  middle: "extended",
  ring: "curled",
  little: "curled",
};
/** Only the index up, thumb tucked — pointing. */
const POINTING: HandShape = {
  thumb: "tucked",
  index: "extended",
  middle: "curled",
  ring: "curled",
  little: "curled",
};
/** Index up with the thumb also out — an "L"/gun; deliberately nothing. */
const GUN: HandShape = {
  thumb: "extended",
  index: "extended",
  middle: "curled",
  ring: "curled",
  little: "curled",
};

/** NEUTRAL with the index and middle fanned out from their knuckles: a V. */
const peaceSign = (): Hand => {
  const hand = syntheticHand(NEUTRAL);
  return {
    ...hand,
    points: {
      ...hand.points,
      indexPip: point(0.42, 0.53),
      indexDip: point(0.405, 0.48),
      indexTip: point(0.39, 0.44),
      middlePip: point(0.51, 0.53),
      middleDip: point(0.525, 0.48),
      middleTip: point(0.54, 0.44),
    },
  };
};

describe("classifyHand", () => {
  it("recognises thumbs-up", () => {
    expect(classifyHand(syntheticHand(THUMBS_UP))).toBe("thumbs-up");
  });

  it("recognises an open palm", () => {
    expect(classifyHand(syntheticHand(OPEN_PALM))).toBe("open-palm");
  });

  it("recognises a peace sign", () => {
    expect(classifyHand(peaceSign())).toBe("peace-sign");
  });

  it("returns none for a fist, which a pen grip resembles", () => {
    expect(classifyHand(syntheticHand(FIST))).toBe("none");
  });

  it("recognises the ok sign", () => {
    expect(classifyHand(syntheticHand(OK_SIGN))).toBe("ok-sign");
  });

  it("returns none for two fingers up side by side", () => {
    expect(classifyHand(syntheticHand(NEUTRAL))).toBe("none");
  });

  it("recognises pointing and keeps it clear of thumbs-up and the gun shape", () => {
    expect(classifyHand(syntheticHand(POINTING))).toBe("pointing");
    expect(classifyHand(syntheticHand(THUMBS_UP))).toBe("thumbs-up");
    expect(classifyHand(syntheticHand(GUN))).toBe("none");
  });

  it("returns none when a joint is missing or unconfident", () => {
    const hand = syntheticHand(THUMBS_UP);
    const { indexTip: _indexTip, ...withoutIndexTip } = hand.points;
    expect(classifyHand({ ...hand, points: withoutIndexTip })).toBe("none");
    const faintWrist = {
      ...hand,
      points: { ...hand.points, wrist: point(0.5, 0.8, 0.1) },
    };
    expect(classifyHand(faintWrist)).toBe("none");
  });

  it("is scale-invariant", () => {
    const hand = syntheticHand(THUMBS_UP);
    const scaled: Hand = {
      ...hand,
      points: Object.fromEntries(
        Object.entries(hand.points).map(([name, p]) => [
          name,
          point(0.5 + (p.x - 0.5) * 0.3, 0.5 + (p.y - 0.5) * 0.3, p.confidence),
        ]),
      ),
    };
    expect(classifyHand(scaled)).toBe("thumbs-up");
  });
});

describe("classifyGesture", () => {
  it("is none with no hands", () => {
    expect(classifyGesture({ hands: [] })).toBe("none");
  });

  it("passes a single hand's pose through", () => {
    expect(classifyGesture({ hands: [peaceSign()] })).toBe("peace-sign");
  });

  it("turns two thumbs-up into two-thumbs-up", () => {
    expect(
      classifyGesture({
        hands: [syntheticHand(THUMBS_UP), syntheticHand(THUMBS_UP)],
      }),
    ).toBe("two-thumbs-up");
  });

  it("ignores a second hand that shows nothing", () => {
    expect(
      classifyGesture({
        hands: [syntheticHand(THUMBS_UP), syntheticHand(NEUTRAL)],
      }),
    ).toBe("thumbs-up");
  });

  it("is none when two hands disagree", () => {
    expect(
      classifyGesture({
        hands: [syntheticHand(THUMBS_UP), syntheticHand(OPEN_PALM)],
      }),
    ).toBe("none");
  });
});

describe("handBounds", () => {
  it("bounds the confident joints", () => {
    const bounds = handBounds(syntheticHand(OPEN_PALM));
    expect(bounds).not.toBeNull();
    expect(bounds?.x).toBeCloseTo(0.29);
    expect(bounds?.y).toBeCloseTo(0.44);
    expect(bounds?.width).toBeCloseTo(0.3);
    expect(bounds?.height).toBeCloseTo(0.36);
  });

  it("is null without confident joints", () => {
    expect(handBounds({ chirality: "unknown", points: {} })).toBeNull();
  });
});
