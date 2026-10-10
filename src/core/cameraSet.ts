/**
 * The camera set: the ordered list of cameras the user has added. Pure
 * operations over the list, so add/remove/rename/reorder/mirror are tested
 * without a camera. Order is the array order — it drives the overview and the
 * order snapshots appear in an all-cameras delivery.
 */

/** A rectangle in frame fractions (0..1), origin top-left. */
export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** Clockwise rotation applied to the frame, in degrees. */
export type Rotation = 0 | 90 | 180 | 270;

export const FULL_FRAME: Rect = { x: 0, y: 0, width: 1, height: 1 };
/** A crop may not be smaller than this fraction of the (rotated) frame. */
export const MIN_CROP = 0.25;

export interface Camera {
  /** Origin-scoped device id (a network camera uses its URL as the id). */
  readonly deviceId: string;
  /** The system label at add time — a fallback name and a re-match hint. */
  readonly label: string;
  /** The user's name for this viewpoint ("Desk", "Wall"). */
  readonly name: string;
  /** Mirror the live view horizontally (display only). */
  readonly mirror: boolean;
  /** Clockwise rotation applied to display, detection, and snapshots. */
  readonly rotation: Rotation;
  /** Snapshot crop within the rotated frame; full frame when omitted. */
  readonly crop: Rect;
  /** Set for a network camera: the MJPEG/snapshot URL frames are fetched from. */
  readonly url?: string;
}

/** A network camera's frames come from a URL rather than a local device. */
export const isNetworkCamera = (camera: Camera): boolean =>
  camera.url !== undefined && camera.url !== "";

/**
 * Whether a URL points at the local network (loopback, private, link-local,
 * or `.local`). Mirrors the Rust enforcement; the app must never reach the
 * public internet. Rust validates again before every fetch.
 */
export const isLocalCameraUrl = (raw: string): boolean => {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return false;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return false;
  const host = url.hostname;
  if (host === "localhost" || host.endsWith(".local")) return true;
  const v4 = host.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (v4) {
    const [a, b] = [Number(v4[1]), Number(v4[2])];
    if (a === 127 || a === 10) return true;
    if (a === 192 && b === 168) return true;
    if (a === 172 && b >= 16 && b <= 31) return true;
    if (a === 169 && b === 254) return true;
    return false;
  }
  // IPv6 loopback / unique-local / link-local.
  return host === "[::1]" || /^\[(fc|fd|fe80)/i.test(host);
};

/** Hides any credentials embedded in a URL, for display. */
export const maskUrl = (raw: string): string =>
  raw.replace(/\/\/[^/@]+@/, "//•••@");

export const addNetworkCamera = (
  set: readonly Camera[],
  url: string,
): Camera[] => {
  if (!isLocalCameraUrl(url) || hasCamera(set, url)) return [...set];
  const host = (() => {
    try {
      return new URL(url).hostname;
    } catch {
      return url;
    }
  })();
  return [
    ...set,
    {
      deviceId: url,
      label: host,
      name: defaultName(set, host),
      mirror: false,
      rotation: 0,
      crop: FULL_FRAME,
      url,
    },
  ];
};

/** Cameras render smoothly up to this many; beyond it is not guaranteed. */
export const COMFORTABLE_CAMERA_COUNT = 4;

const defaultName = (set: readonly Camera[], label: string): string =>
  label || `Camera ${set.length + 1}`;

export const hasCamera = (set: readonly Camera[], deviceId: string): boolean =>
  set.some((camera) => camera.deviceId === deviceId);

export const addCamera = (
  set: readonly Camera[],
  deviceId: string,
  label: string,
): Camera[] => {
  if (deviceId === "" || hasCamera(set, deviceId)) return [...set];
  return [
    ...set,
    {
      deviceId,
      label,
      name: defaultName(set, label),
      mirror: false,
      rotation: 0,
      crop: FULL_FRAME,
    },
  ];
};

export const removeCamera = (
  set: readonly Camera[],
  deviceId: string,
): Camera[] => set.filter((camera) => camera.deviceId !== deviceId);

const patch = (
  set: readonly Camera[],
  deviceId: string,
  change: Partial<Camera>,
): Camera[] =>
  set.map((camera) =>
    camera.deviceId === deviceId ? { ...camera, ...change } : camera,
  );

export const renameCamera = (
  set: readonly Camera[],
  deviceId: string,
  name: string,
): Camera[] => patch(set, deviceId, { name: name.trim() || "Camera" });

export const setMirror = (
  set: readonly Camera[],
  deviceId: string,
  mirror: boolean,
): Camera[] => patch(set, deviceId, { mirror });

export const rotateCamera = (
  set: readonly Camera[],
  deviceId: string,
): Camera[] =>
  set.map((camera) =>
    camera.deviceId === deviceId
      ? // Rotating resets the crop: the old rectangle means nothing in the
        // new orientation.
        {
          ...camera,
          rotation: ((camera.rotation + 90) % 360) as Rotation,
          crop: FULL_FRAME,
        }
      : camera,
  );

/** Clamps a crop to at least MIN_CROP on each side and within the frame. */
export const clampCrop = (rect: Rect): Rect => {
  const width = Math.min(1, Math.max(MIN_CROP, rect.width));
  const height = Math.min(1, Math.max(MIN_CROP, rect.height));
  const x = Math.min(Math.max(0, rect.x), 1 - width);
  const y = Math.min(Math.max(0, rect.y), 1 - height);
  return { x, y, width, height };
};

/**
 * Longest side of a snapshot. Matches the 1920×1080 the cameras are asked
 * for, so a full-frame snapshot keeps every captured pixel; portrait and
 * rotated frames get the same limit as landscape ones.
 */
export const SNAPSHOT_MAX_EDGE = 1920;

/** A region's output size: scaled down to fit `maxEdge`, never up. */
export const fitWithin = (
  width: number,
  height: number,
  maxEdge: number,
): { width: number; height: number } => {
  const scale = Math.min(1, maxEdge / Math.max(width, height));
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
};

export const setCrop = (
  set: readonly Camera[],
  deviceId: string,
  crop: Rect,
): Camera[] => patch(set, deviceId, { crop: clampCrop(crop) });

export const resetView = (set: readonly Camera[], deviceId: string): Camera[] =>
  patch(set, deviceId, { rotation: 0, crop: FULL_FRAME });

/** Devices offered in "add camera": video inputs not already in the set. */
export const availableToAdd = (
  set: readonly Camera[],
  devices: readonly MediaDeviceInfo[],
): MediaDeviceInfo[] =>
  devices.filter(
    (device) =>
      device.kind === "videoinput" &&
      device.deviceId !== "" &&
      !hasCamera(set, device.deviceId),
  );
