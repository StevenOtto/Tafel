import { describe, expect, it } from "vitest";
import {
  CLOSED_COLLECTION,
  COLLECTION_TIMEOUT_MS,
  MAX_COLLECTION,
  SnapshotInput,
  addToCollection,
  deliveryIntent,
  isExpired,
  openCollection,
} from "./delivery";

const snap = (name = "Desk"): SnapshotInput => ({
  cameraName: name,
  timestampMs: 0,
  handFree: true,
  cropped: false,
  jpegB64: "x",
});

describe("deliveryIntent (no open collection)", () => {
  it("thumbs-up captures a single snapshot", () => {
    expect(deliveryIntent("thumbs-up", CLOSED_COLLECTION)).toEqual({
      type: "capture-single",
    });
  });

  it("two-thumbs-up captures all cameras", () => {
    expect(deliveryIntent("two-thumbs-up", CLOSED_COLLECTION)).toEqual({
      type: "capture-all",
    });
  });

  it("ok-sign opens a collection", () => {
    expect(deliveryIntent("ok-sign", CLOSED_COLLECTION)).toEqual({
      type: "open-collection",
    });
  });

  it("pause/resume/none gestures make no delivery", () => {
    for (const g of ["open-palm", "peace-sign", "none"] as const) {
      expect(deliveryIntent(g, CLOSED_COLLECTION)).toEqual({ type: "none" });
    }
  });
});

describe("deliveryIntent (collection open)", () => {
  const open = openCollection(0);

  it("thumbs add to the collection instead of publishing", () => {
    expect(deliveryIntent("thumbs-up", open)).toEqual({
      type: "add-to-collection",
      all: false,
    });
    expect(deliveryIntent("two-thumbs-up", open)).toEqual({
      type: "add-to-collection",
      all: true,
    });
  });

  it("ok-sign on a non-empty collection publishes it", () => {
    const withOne = addToCollection(open, [snap()], 10);
    expect(deliveryIntent("ok-sign", withOne)).toEqual({
      type: "publish-collection",
    });
  });

  it("ok-sign on an empty collection reports empty, not publish", () => {
    expect(deliveryIntent("ok-sign", open)).toEqual({
      type: "collection-empty",
    });
  });

  it("refuses to add beyond the maximum", () => {
    let collection = open;
    for (let i = 0; i < MAX_COLLECTION; i += 1) {
      collection = addToCollection(collection, [snap()], i);
    }
    expect(collection.snapshots).toHaveLength(MAX_COLLECTION);
    expect(deliveryIntent("thumbs-up", collection)).toEqual({
      type: "collection-full",
    });
  });
});

describe("collection lifecycle", () => {
  it("adds snapshots and caps at the maximum", () => {
    const open = openCollection(0);
    const many = Array.from({ length: MAX_COLLECTION + 3 }, () => snap());
    const filled = addToCollection(open, many, 1);
    expect(filled.snapshots).toHaveLength(MAX_COLLECTION);
  });

  it("expires only after the timeout with the collection open", () => {
    const open = openCollection(1000);
    expect(isExpired(open, 1000 + COLLECTION_TIMEOUT_MS)).toBe(false);
    expect(isExpired(open, 1001 + COLLECTION_TIMEOUT_MS)).toBe(true);
    expect(isExpired(CLOSED_COLLECTION, 10 ** 12)).toBe(false);
  });
});
