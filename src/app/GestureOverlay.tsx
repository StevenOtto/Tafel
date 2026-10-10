/**
 * The ring around the hand: fills over the hold, confirms on fire, vanishes
 * on release. Drawn as SVG in the video's content box (the <video> uses
 * object-fit: contain, so the box is computed from the element and the
 * stream size). Text is never mirrored; when the view is mirrored the
 * coordinates are, so the ring follows the hand the user sees.
 */

import React, { useRef } from "react";
import {
  Gesture,
  Hand,
  JOINT_NAMES,
  classifyHand,
  handBounds,
} from "../core/hand";
import { HoldOutput } from "../core/hold";

export interface ContentBox {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

const contentBox = (
  clientWidth: number,
  clientHeight: number,
  intrinsicWidth: number,
  intrinsicHeight: number,
): ContentBox | null => {
  if (!clientWidth || !clientHeight || !intrinsicWidth || !intrinsicHeight)
    return null;
  const scale = Math.min(
    clientWidth / intrinsicWidth,
    clientHeight / intrinsicHeight,
  );
  const width = intrinsicWidth * scale;
  const height = intrinsicHeight * scale;
  return {
    left: (clientWidth - width) / 2,
    top: (clientHeight - height) / 2,
    width,
    height,
  };
};

/** Where the video pixels actually land inside the element (object-fit: contain). */
export const videoContentBox = (video: HTMLVideoElement): ContentBox | null =>
  contentBox(
    video.clientWidth,
    video.clientHeight,
    video.videoWidth,
    video.videoHeight,
  );

/** Same, for a canvas whose intrinsic size is the drawn (upright) frame. */
export const canvasContentBox = (
  canvas: HTMLCanvasElement,
): ContentBox | null =>
  contentBox(
    canvas.clientWidth,
    canvas.clientHeight,
    canvas.width,
    canvas.height,
  );

const GESTURE_LABELS: Record<Gesture, string> = {
  "thumbs-up": "Thumbs up",
  "two-thumbs-up": "Two thumbs up",
  "open-palm": "Open hand",
  fist: "Fist",
  "ok-sign": "Ok sign",
  pointing: "Pointing",
  none: "",
};

interface Ring {
  readonly cx: number;
  readonly cy: number;
  readonly r: number;
}

/** Hands that show the held gesture; falls back to every hand in view. */
const handsForGesture = (hands: readonly Hand[], gesture: Gesture): Hand[] => {
  const wanted = gesture === "two-thumbs-up" ? "thumbs-up" : gesture;
  const matching = hands.filter((hand) => classifyHand(hand) === wanted);
  return matching.length > 0 ? matching : [...hands];
};

const ringAround = (
  hands: readonly Hand[],
  box: ContentBox,
  mirror: boolean,
): Ring | null => {
  const bounds = hands
    .map(handBounds)
    .filter((b): b is NonNullable<typeof b> => b !== null);
  if (bounds.length === 0) return null;
  const x0 = Math.min(...bounds.map((b) => b.x));
  const y0 = Math.min(...bounds.map((b) => b.y));
  const x1 = Math.max(...bounds.map((b) => b.x + b.width));
  const y1 = Math.max(...bounds.map((b) => b.y + b.height));
  const cxFraction = mirror ? 1 - (x0 + x1) / 2 : (x0 + x1) / 2;
  const w = (x1 - x0) * box.width;
  const h = (y1 - y0) * box.height;
  return {
    cx: cxFraction * box.width,
    cy: ((y0 + y1) / 2) * box.height,
    r: Math.max(w, h) * 0.65 + 14,
  };
};

/**
 * Per-sample smoothing of the ring's centre and radius. Hand bounds jitter
 * from one detection to the next; easing a third of the way each sample
 * (~100 ms apart) settles within a few frames without lagging the hand.
 */
const RING_SMOOTHING = 0.35;

const smoothRing = (previous: Ring | null, next: Ring): Ring =>
  previous === null
    ? next
    : {
        cx: previous.cx + (next.cx - previous.cx) * RING_SMOOTHING,
        cy: previous.cy + (next.cy - previous.cy) * RING_SMOOTHING,
        r: previous.r + (next.r - previous.r) * RING_SMOOTHING,
      };

/** How a completed hold resolved, so the ring can confirm or explain. */
export type FireOutcome = "fire" | "duplicate" | "cooldown";

const OUTCOME_SUFFIX: Record<FireOutcome, string> = {
  fire: "✓",
  duplicate: "· already sent",
  cooldown: "· too soon",
};

export const GestureOverlay = ({
  hands,
  hold,
  holdMs,
  box,
  mirror,
  showLandmarks,
  outcome,
}: {
  hands: readonly Hand[];
  hold: HoldOutput | null;
  holdMs: number;
  box: ContentBox | null;
  mirror: boolean;
  showLandmarks: boolean;
  outcome?: FireOutcome | null;
}): React.JSX.Element | null => {
  // The last ring survives a detection dropout, so the ring does not flicker
  // while the hold tracker bridges the gap.
  const lastRingRef = useRef<Ring | null>(null);
  if (!box) return null;

  const gesture = hold?.gesture ?? null;
  const measured =
    gesture !== null
      ? ringAround(handsForGesture(hands, gesture), box, mirror)
      : null;
  const ring =
    gesture === null
      ? null
      : measured
        ? smoothRing(lastRingRef.current, measured)
        : lastRingRef.current;
  lastRingRef.current = ring;

  const progress = hold?.progress ?? 0;
  const fired = hold?.phase === "fired";
  const suppressed =
    fired && (outcome === "duplicate" || outcome === "cooldown");
  const remaining = Math.max(0, ((1 - progress) * holdMs) / 1000);
  const circumference = ring ? 2 * Math.PI * ring.r : 0;

  return (
    <svg
      className="gesture-overlay"
      style={{ left: box.left, top: box.top }}
      width={box.width}
      height={box.height}
      viewBox={`0 0 ${box.width} ${box.height}`}
      aria-hidden="true"
    >
      {showLandmarks &&
        hands.map((hand, handIndex) =>
          JOINT_NAMES.map((name) => {
            const point = hand.points[name];
            if (!point) return null;
            const x = (mirror ? 1 - point.x : point.x) * box.width;
            return (
              <circle
                key={`${handIndex}-${name}`}
                className="gesture-landmark"
                cx={x}
                cy={point.y * box.height}
                r={3}
                opacity={Math.max(0.2, point.confidence)}
              />
            );
          }),
        )}
      {ring && gesture && (
        <g
          className={`gesture-ring${fired ? " is-fired" : ""}${suppressed ? " is-suppressed" : ""}`}
        >
          <circle
            className="gesture-ring__track"
            cx={ring.cx}
            cy={ring.cy}
            r={ring.r}
          />
          <circle
            className="gesture-ring__progress"
            cx={ring.cx}
            cy={ring.cy}
            r={ring.r}
            strokeDasharray={circumference}
            strokeDashoffset={circumference * (1 - progress)}
            transform={`rotate(-90 ${ring.cx} ${ring.cy})`}
          />
          <text
            className="gesture-ring__label"
            x={ring.cx}
            y={ring.cy + ring.r + 22}
            textAnchor="middle"
          >
            {fired
              ? `${GESTURE_LABELS[gesture]} ${OUTCOME_SUFFIX[outcome ?? "fire"]}`
              : `${GESTURE_LABELS[gesture]} · ${remaining.toFixed(1)} s`}
          </text>
        </g>
      )}
    </svg>
  );
};
