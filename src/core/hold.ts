/**
 * The hold: a gesture must be kept up for `holdMs` before it counts. Like the
 * old settle detector this has no clock of its own — callers pass
 * timestamps — so recorded gesture sequences replay exactly.
 *
 * idle --gesture--> holding --held for holdMs--> fire (once) --gesture gone
 * for rearmMs--> idle. Releasing before the window cancels; short detection
 * dropouts (≤ graceMs) are bridged; a different gesture restarts the hold.
 */

import { Gesture } from "./hand";

export interface HoldConfig {
  /** How long a gesture must be held before it fires. */
  readonly holdMs: number;
  /** Detection dropout tolerated without cancelling the hold. */
  readonly graceMs: number;
  /** How long the gesture must be gone after firing before it can fire again. */
  readonly rearmMs: number;
}

export const DEFAULT_HOLD_CONFIG: HoldConfig = {
  holdMs: 2000,
  graceMs: 250,
  rearmMs: 500,
};

export type HoldPhase = "idle" | "holding" | "fired";

export interface HoldOutput {
  readonly phase: HoldPhase;
  /** The gesture being held (or just fired); null when idle. */
  readonly gesture: Gesture | null;
  /** 0..1 fill of the ring; 1 once fired. */
  readonly progress: number;
  /** Set on exactly the frame the hold completes. */
  readonly fired: Gesture | null;
}

const IDLE: HoldOutput = {
  phase: "idle",
  gesture: null,
  progress: 0,
  fired: null,
};

export class HoldTracker {
  private readonly config: HoldConfig;
  private phase: HoldPhase = "idle";
  private held: Gesture | null = null;
  private sinceMs = 0;
  private lastSeenMs = 0;

  constructor(config: HoldConfig = DEFAULT_HOLD_CONFIG) {
    this.config = config;
  }

  next(gesture: Gesture, timestampMs: number): HoldOutput {
    if (this.phase === "idle") return this.startIfPresent(gesture, timestampMs);
    if (this.phase === "holding") return this.hold(gesture, timestampMs);
    return this.rearm(gesture, timestampMs);
  }

  private startIfPresent(gesture: Gesture, timestampMs: number): HoldOutput {
    if (gesture === "none") {
      this.phase = "idle";
      this.held = null;
      return IDLE;
    }
    this.phase = "holding";
    this.held = gesture;
    this.sinceMs = timestampMs;
    this.lastSeenMs = timestampMs;
    return { phase: "holding", gesture, progress: 0, fired: null };
  }

  private hold(gesture: Gesture, timestampMs: number): HoldOutput {
    const held = this.held as Gesture;
    if (gesture !== held && gesture !== "none") {
      return this.startIfPresent(gesture, timestampMs);
    }
    const progress = Math.min(
      1,
      (timestampMs - this.sinceMs) / this.config.holdMs,
    );
    if (gesture === "none") {
      if (timestampMs - this.lastSeenMs > this.config.graceMs) {
        return this.startIfPresent("none", timestampMs);
      }
      // Bridging a dropout: the ring keeps filling, but only a frame that
      // actually shows the gesture may fire — a release at 1.9 s is a
      // release, not a hold.
      return { phase: "holding", gesture: held, progress, fired: null };
    }
    this.lastSeenMs = timestampMs;
    if (progress < 1) {
      return { phase: "holding", gesture: held, progress, fired: null };
    }
    this.phase = "fired";
    return { phase: "fired", gesture: held, progress: 1, fired: held };
  }

  private rearm(gesture: Gesture, timestampMs: number): HoldOutput {
    const held = this.held as Gesture;
    if (gesture === held) {
      this.lastSeenMs = timestampMs;
      return { phase: "fired", gesture: held, progress: 1, fired: null };
    }
    if (timestampMs - this.lastSeenMs < this.config.rearmMs) {
      return { phase: "fired", gesture: held, progress: 1, fired: null };
    }
    return this.startIfPresent(gesture, timestampMs);
  }
}
