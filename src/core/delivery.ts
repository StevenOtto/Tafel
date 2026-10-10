/**
 * Deliveries: what a fired gesture turns into. The collection state machine
 * and the "which gesture makes which delivery" decision live here as pure
 * functions, so they are tested without a camera, Vision, or the bus.
 */

import { Gesture } from "./hand";

export type DeliveryKind = "single" | "all-cameras" | "collection";

/** One snapshot, as reported to Rust (JPEG base64 + context). */
export interface SnapshotInput {
  readonly cameraName: string;
  readonly timestampMs: number;
  readonly handFree: boolean;
  readonly jpegB64: string;
  /** True when the snapshot is a crop of the camera's frame, not the whole. */
  readonly cropped: boolean;
  /** A marked attention point (frame fractions), set by a pointing gesture. */
  readonly focus?: { x: number; y: number };
}

/**
 * A collection accumulates snapshots between two ok-signs. Eight full-frame
 * 1920×1080 snapshots stay under Claude Code's default 25,000-token limit
 * for one MCP tool result.
 */
export const MAX_COLLECTION = 8;
/** A collection expires after this long without an addition. */
export const COLLECTION_TIMEOUT_MS = 10 * 60 * 1000;

export interface CollectionState {
  readonly open: boolean;
  readonly snapshots: readonly SnapshotInput[];
  /** performance.now() of the last addition or of opening. */
  readonly lastActivityMs: number;
}

export const CLOSED_COLLECTION: CollectionState = {
  open: false,
  snapshots: [],
  lastActivityMs: 0,
};

/**
 * What a fired gesture does to the collection, and what (if anything) to
 * publish. Kept separate from capture so capture (which needs the camera) is
 * the only impure step.
 */
export type DeliveryIntent =
  | { readonly type: "capture-single" }
  | { readonly type: "capture-all" }
  | { readonly type: "open-collection" }
  | { readonly type: "add-to-collection"; readonly all: boolean }
  | { readonly type: "publish-collection" }
  | { readonly type: "collection-empty" }
  | { readonly type: "collection-full" }
  | { readonly type: "none" };

/**
 * Resolves a fired snapshot/collection gesture against the current collection
 * state. Pause/resume are handled upstream (they change camera status, not
 * deliveries) and map to "none" here.
 */
export const deliveryIntent = (
  gesture: Gesture,
  collection: CollectionState,
): DeliveryIntent => {
  if (gesture === "ok-sign") {
    if (!collection.open) return { type: "open-collection" };
    return collection.snapshots.length === 0
      ? { type: "collection-empty" }
      : { type: "publish-collection" };
  }
  if (gesture === "thumbs-up" || gesture === "two-thumbs-up") {
    const all = gesture === "two-thumbs-up";
    if (collection.open) {
      return collection.snapshots.length >= MAX_COLLECTION
        ? { type: "collection-full" }
        : { type: "add-to-collection", all };
    }
    return all ? { type: "capture-all" } : { type: "capture-single" };
  }
  return { type: "none" };
};

export const openCollection = (nowMs: number): CollectionState => ({
  open: true,
  snapshots: [],
  lastActivityMs: nowMs,
});

export const addToCollection = (
  collection: CollectionState,
  snapshots: readonly SnapshotInput[],
  nowMs: number,
): CollectionState => ({
  open: true,
  snapshots: [...collection.snapshots, ...snapshots].slice(0, MAX_COLLECTION),
  lastActivityMs: nowMs,
});

export const isExpired = (
  collection: CollectionState,
  nowMs: number,
): boolean =>
  collection.open && nowMs - collection.lastActivityMs > COLLECTION_TIMEOUT_MS;
