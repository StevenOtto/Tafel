/**
 * When several cameras can see the same hand, two of them can complete the
 * same gesture at almost the same moment. The action gate decides, at fire
 * time, whether a completed gesture actually acts:
 *
 *  - "fire"      — perform it.
 *  - "duplicate" — another camera just fired the same gesture (within
 *                  dedupeMs); this one is suppressed and its ring says so.
 *  - "cooldown"  — this camera fired anything within cooldownMs; it must rest
 *                  before it can fire again.
 *
 * Pure and time-injected, so it is tested without any camera.
 */

export type GateResult = "fire" | "duplicate" | "cooldown";

export class ActionGate {
  private readonly cooldownMs: number;
  private readonly dedupeMs: number;
  private lastFirePerCamera = new Map<string, number>();
  private lastAction: { gesture: string; atMs: number } | null = null;

  constructor(cooldownMs = 1000, dedupeMs = 1000) {
    this.cooldownMs = cooldownMs;
    this.dedupeMs = dedupeMs;
  }

  evaluate(cameraId: string, gesture: string, nowMs: number): GateResult {
    const lastForCamera = this.lastFirePerCamera.get(cameraId);
    if (
      lastForCamera !== undefined &&
      nowMs - lastForCamera < this.cooldownMs
    ) {
      return "cooldown";
    }
    if (
      this.lastAction !== null &&
      this.lastAction.gesture === gesture &&
      nowMs - this.lastAction.atMs < this.dedupeMs
    ) {
      return "duplicate";
    }
    this.lastFirePerCamera.set(cameraId, nowMs);
    this.lastAction = { gesture, atMs: nowMs };
    return "fire";
  }
}
