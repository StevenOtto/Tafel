import { describe, expect, it } from "vitest";
import {
  Camera,
  MIN_CROP,
  addCamera,
  addNetworkCamera,
  availableToAdd,
  clampCrop,
  isLocalCameraUrl,
  isNetworkCamera,
  maskUrl,
  removeCamera,
  renameCamera,
  resetView,
  rotateCamera,
  setCrop,
  setMirror,
} from "./cameraSet";

const device = (deviceId: string, label = ""): MediaDeviceInfo =>
  ({ deviceId, label, kind: "videoinput", groupId: "" }) as MediaDeviceInfo;

describe("camera set", () => {
  it("adds a camera with a default name and keeps order", () => {
    let set: Camera[] = [];
    set = addCamera(set, "desk", "FaceTime HD");
    set = addCamera(set, "wall", "");
    expect(set.map((c) => c.deviceId)).toEqual(["desk", "wall"]);
    expect(set[0].name).toBe("FaceTime HD");
    expect(set[1].name).toBe("Camera 2");
  });

  it("does not add a duplicate or an empty device id", () => {
    const set = addCamera(addCamera([], "desk", "Desk"), "desk", "Desk");
    expect(set).toHaveLength(1);
    expect(addCamera(set, "", "Nothing")).toHaveLength(1);
  });

  it("removes by device id", () => {
    const set = addCamera(addCamera([], "desk", "Desk"), "wall", "Wall");
    expect(removeCamera(set, "desk").map((c) => c.deviceId)).toEqual(["wall"]);
  });

  it("renames, trimming and falling back when blank", () => {
    const set = addCamera([], "desk", "Desk");
    expect(renameCamera(set, "desk", "  Bureau  ")[0].name).toBe("Bureau");
    expect(renameCamera(set, "desk", "   ")[0].name).toBe("Camera");
  });

  it("toggles mirror per camera", () => {
    let set = addCamera(addCamera([], "desk", "Desk"), "wall", "Wall");
    set = setMirror(set, "wall", true);
    expect(set.find((c) => c.deviceId === "wall")?.mirror).toBe(true);
    expect(set.find((c) => c.deviceId === "desk")?.mirror).toBe(false);
  });

  it("offers only video inputs not already added", () => {
    const set = addCamera([], "desk", "Desk");
    const devices = [
      device("desk", "Desk"),
      device("wall", "Wall"),
      device("", "no permission yet"),
      {
        deviceId: "mic",
        label: "Mic",
        kind: "audioinput",
        groupId: "",
      } as MediaDeviceInfo,
    ];
    expect(availableToAdd(set, devices).map((d) => d.deviceId)).toEqual([
      "wall",
    ]);
  });

  it("starts a camera unrotated with a full-frame crop", () => {
    const camera = addCamera([], "desk", "Desk")[0];
    expect(camera.rotation).toBe(0);
    expect(camera.crop).toEqual({ x: 0, y: 0, width: 1, height: 1 });
  });

  it("rotates in 90° steps and resets the crop each turn", () => {
    let set = setCrop(addCamera([], "desk", "Desk"), "desk", {
      x: 0.3,
      y: 0.3,
      width: 0.4,
      height: 0.4,
    });
    set = rotateCamera(set, "desk");
    expect(set[0].rotation).toBe(90);
    expect(set[0].crop).toEqual({ x: 0, y: 0, width: 1, height: 1 });
    set = rotateCamera(rotateCamera(rotateCamera(set, "desk"), "desk"), "desk");
    expect(set[0].rotation).toBe(0);
  });

  it("clamps a crop to the minimum size and inside the frame", () => {
    expect(clampCrop({ x: 0, y: 0, width: 0.1, height: 0.1 })).toEqual({
      x: 0,
      y: 0,
      width: MIN_CROP,
      height: MIN_CROP,
    });
    // A crop pushed past the right edge slides back in rather than shrinking.
    const c = clampCrop({ x: 0.9, y: 0.9, width: 0.5, height: 0.5 });
    expect(c.x + c.width).toBeLessThanOrEqual(1.0001);
    expect(c.y + c.height).toBeLessThanOrEqual(1.0001);
  });

  it("resets rotation and crop", () => {
    let set = rotateCamera(addCamera([], "desk", "Desk"), "desk");
    set = setCrop(set, "desk", { x: 0.25, y: 0.25, width: 0.5, height: 0.5 });
    set = resetView(set, "desk");
    expect(set[0].rotation).toBe(0);
    expect(set[0].crop).toEqual({ x: 0, y: 0, width: 1, height: 1 });
  });
});

describe("network cameras", () => {
  it("accepts local URLs and refuses public ones", () => {
    for (const url of [
      "http://127.0.0.1:8080/stream",
      "http://localhost/snapshot.jpg",
      "http://192.168.1.50:81/video",
      "http://10.0.0.9/mjpeg",
      "http://172.16.5.4/cam",
      "http://phone.local/stream",
    ]) {
      expect(isLocalCameraUrl(url)).toBe(true);
    }
    for (const url of [
      "http://example.com/stream",
      "https://8.8.8.8/cam",
      "http://172.32.0.1/x",
      "ftp://192.168.1.1/x",
      "not a url",
    ]) {
      expect(isLocalCameraUrl(url)).toBe(false);
    }
  });

  it("adds a network camera keyed by its URL and refuses a public one", () => {
    let set = addNetworkCamera([], "http://192.168.1.50/video");
    expect(set).toHaveLength(1);
    expect(isNetworkCamera(set[0])).toBe(true);
    expect(set[0].deviceId).toBe("http://192.168.1.50/video");
    expect(set[0].name).toBe("192.168.1.50");
    // Duplicate URL and public host are both refused.
    expect(addNetworkCamera(set, "http://192.168.1.50/video")).toHaveLength(1);
    set = addNetworkCamera(set, "http://example.com/x");
    expect(set).toHaveLength(1);
  });

  it("masks credentials in a URL for display", () => {
    expect(maskUrl("http://user:pass@192.168.1.5/stream")).toContain("•••");
    expect(maskUrl("http://user:pass@192.168.1.5/stream")).not.toContain(
      "pass",
    );
  });
});
