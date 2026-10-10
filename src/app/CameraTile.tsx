/**
 * One camera, end to end: its own stream, its own detection loop, its own
 * hold tracker and ring. Tiles are independent — a hold on one never touches
 * another. The frame is rotated once, into a display canvas, so display,
 * detection, ring, and snapshot all share one upright coordinate space; the
 * crop rectangle bounds only the snapshot.
 */

import React, { useCallback, useEffect, useRef, useState } from "react";
import { gestureVisibleWhen } from "../core/actions";
import { Camera, Rect, clampCrop, isNetworkCamera } from "../core/cameraSet";
import { SnapshotInput } from "../core/delivery";
import { Gesture, Hand, classifyGesture, pointingTip } from "../core/hand";
import { DEFAULT_HOLD_CONFIG, HoldOutput, HoldTracker } from "../core/hold";
import {
  ContentBox,
  FireOutcome,
  GestureOverlay,
  canvasContentBox,
} from "./GestureOverlay";
import {
  FrameCropper,
  drawRotated,
  openCamera,
  rotatedSize,
  sampleFrame,
} from "./cameraSource";
import {
  detectHands,
  latestNetworkFrame,
  reportFrame,
  reportCameraState,
  retireFrame,
} from "./tauri";

export type TileStatus = "watching" | "paused" | "disconnected";

export interface TileStats {
  readonly fps: number;
  readonly latencyMs: number;
}

const ANALYSIS_INTERVAL_MS = 100;
const FRAME_REPORT_INTERVAL_MS = 2000;
const HAND_FREE_CAPTURE_MS = 500;
const HAND_FREE_MAX_AGE_MS = 3000;
const STATS_REPORT_MS = 500;
/** Twice the usual 10 fps camera rate, so no frame is skipped. */
const NETWORK_POLL_MS = 50;
/** Rust reconnects on its own; the tile only checks back. */
const NETWORK_RETRY_MS = 1000;
/** A marked attention point expires this long after it is set. */
const FOCUS_TTL_MS = 60000;
/** Focus crop side, as a fraction of the frame width. */
const FOCUS_SIDE = 0.25;

interface Focus {
  readonly x: number;
  readonly y: number;
  readonly expiresAt: number;
}

interface HandFreeFrame {
  readonly jpegB64: string;
  readonly atMs: number;
}

const STATUS_LABEL: Record<TileStatus, string> = {
  watching: "Watching",
  paused: "Paused",
  disconnected: "Disconnected",
};

const isFullFrame = (crop: Rect): boolean =>
  crop.x === 0 && crop.y === 0 && crop.width === 1 && crop.height === 1;

export interface CameraTileProps {
  readonly camera: Camera;
  readonly present: boolean;
  readonly forcePaused: boolean;
  readonly holdMs: number;
  readonly gesturesEnabled: boolean;
  readonly enabledGestures: readonly Gesture[];
  readonly showLandmarks: boolean;
  readonly onFire: (deviceId: string, gesture: Gesture) => FireOutcome;
  readonly onStatus: (deviceId: string, status: TileStatus) => void;
  readonly onStats: (deviceId: string, stats: TileStats) => void;
  readonly onSample: (deviceId: string, hands: readonly Hand[]) => void;
  readonly registerCapture: (
    deviceId: string,
    capture: (() => SnapshotInput[]) | null,
  ) => void;
  readonly onRename: (deviceId: string, name: string) => void;
  readonly onRemove: (deviceId: string) => void;
  readonly onMirror: (deviceId: string, mirror: boolean) => void;
  readonly onRotate: (deviceId: string) => void;
  readonly onCrop: (deviceId: string, crop: Rect) => void;
  readonly onResetView: (deviceId: string) => void;
}

type Corner = "nw" | "ne" | "sw" | "se";

export const CameraTile = ({
  camera,
  present,
  forcePaused,
  holdMs,
  gesturesEnabled,
  enabledGestures,
  showLandmarks,
  onFire,
  onStatus,
  onStats,
  onSample,
  registerCapture,
  onRename,
  onRemove,
  onMirror,
  onRotate,
  onCrop,
  onResetView,
}: CameraTileProps): React.JSX.Element => {
  const isNetwork = isNetworkCamera(camera);
  const videoRef = useRef<HTMLVideoElement>(null);
  const netImageRef = useRef<HTMLImageElement | null>(null);
  const displayRef = useRef<HTMLCanvasElement>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const trackerRef = useRef(new HoldTracker());
  const busyRef = useRef(false);
  const cropperRef = useRef<FrameCropper | null>(null);
  const handFreeRef = useRef<HandFreeFrame | null>(null);
  const lastFrameReportRef = useRef(0);
  const lastHandFreeRef = useRef(0);
  const lastStatsRef = useRef(0);
  const latenciesRef = useRef<number[]>([]);
  const lastTickRef = useRef(0);
  const fpsRef = useRef(0);
  const localPausedRef = useRef(false);
  const outcomeRef = useRef<FireOutcome | null>(null);
  const viewRef = useRef({ rotation: camera.rotation, crop: camera.crop });
  const dragRef = useRef<Corner | null>(null);
  const focusRef = useRef<Focus | null>(null);

  const [disconnected, setDisconnected] = useState(!present);
  const [localPaused, setLocalPaused] = useState(false);
  const [hands, setHands] = useState<readonly Hand[]>([]);
  const [hold, setHold] = useState<HoldOutput | null>(null);
  const [outcome, setOutcome] = useState<FireOutcome | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [draftName, setDraftName] = useState(camera.name);
  const [editingCrop, setEditingCrop] = useState(false);
  const [box, setBox] = useState<ContentBox | null>(null);
  const [focus, setFocus] = useState<Focus | null>(null);

  const paused = localPaused || forcePaused;
  const status: TileStatus = disconnected
    ? "disconnected"
    : paused
      ? "paused"
      : "watching";

  useEffect(() => {
    localPausedRef.current = localPaused;
  }, [localPaused]);
  useEffect(() => {
    viewRef.current = { rotation: camera.rotation, crop: camera.crop };
  }, [camera.rotation, camera.crop]);

  useEffect(() => {
    onStatus(camera.deviceId, status);
    void reportCameraState(camera.deviceId, status);
  }, [camera.deviceId, status, onStatus]);

  // When the tile goes, so does the frame the agent could still read.
  useEffect(() => () => void retireFrame(camera.deviceId), [camera.deviceId]);

  useEffect(() => {
    trackerRef.current = new HoldTracker({ ...DEFAULT_HOLD_CONFIG, holdMs });
  }, [holdMs, enabledGestures]);

  // Register a snapshot accessor for deliveries. Returns the main snapshot
  // and, when a focus point is set, a crop around it — then clears the focus.
  useEffect(() => {
    const capture = (): SnapshotInput[] => {
      const cropper = cropperRef.current;
      if (!cropper) return [];
      const now = Date.now();
      const handFree = handFreeRef.current;
      const cropped = !isFullFrame(camera.crop);
      const useHandFree =
        handFree !== null && now - handFree.atMs <= HAND_FREE_MAX_AGE_MS;
      const focusPoint = focusRef.current;

      const main: SnapshotInput = {
        cameraName: camera.name,
        timestampMs: useHandFree ? (handFree as { atMs: number }).atMs : now,
        handFree: useHandFree,
        cropped,
        jpegB64: useHandFree
          ? (handFree as { jpegB64: string }).jpegB64
          : cropper.deskJpegB64(),
        ...(focusPoint ? { focus: { x: focusPoint.x, y: focusPoint.y } } : {}),
      };
      const snapshots = [main];

      if (focusPoint) {
        // A square (in pixels) around the point, clamped to the frame.
        const sideX = FOCUS_SIDE;
        const sideY = (FOCUS_SIDE * cropper.width) / cropper.height;
        const region = clampCrop({
          x: focusPoint.x - sideX / 2,
          y: focusPoint.y - sideY / 2,
          width: sideX,
          height: sideY,
        });
        snapshots.push({
          cameraName: `${camera.name} (focus)`,
          timestampMs: now,
          handFree: useHandFree,
          cropped: true,
          jpegB64: cropper.regionJpegB64(region),
        });
        focusRef.current = null;
        setFocus(null);
      }
      return snapshots;
    };
    registerCapture(camera.deviceId, capture);
    return () => registerCapture(camera.deviceId, null);
  }, [camera.deviceId, camera.name, camera.crop, registerCapture]);

  // A focus point counts down and clears itself if no snapshot consumes it.
  useEffect(() => {
    if (!focus) return;
    const interval = window.setInterval(() => {
      if (performance.now() >= focus.expiresAt) {
        focusRef.current = null;
        setFocus(null);
      } else {
        setFocus((current) => (current ? { ...current } : null));
      }
    }, 500);
    return () => window.clearInterval(interval);
  }, [focus]);

  // The current drawable frame and its native size — a live <video> for a
  // local camera, the latest decoded frame for a network one.
  const readSource = useCallback((): {
    source: CanvasImageSource;
    width: number;
    height: number;
  } | null => {
    if (isNetwork) {
      const img = netImageRef.current;
      if (!img || img.naturalWidth === 0) return null;
      return {
        source: img,
        width: img.naturalWidth,
        height: img.naturalHeight,
      };
    }
    const video = videoRef.current;
    if (!video || video.videoWidth === 0) return null;
    return {
      source: video,
      width: video.videoWidth,
      height: video.videoHeight,
    };
  }, [isNetwork]);

  // Local stream lifecycle (getUserMedia). Network cameras use the poller.
  useEffect(() => {
    if (isNetwork) return;
    if (!present) {
      setDisconnected(true);
      return;
    }
    let stream: MediaStream | null = null;
    let cancelled = false;
    void (async () => {
      try {
        const opened = await openCamera(camera.deviceId);
        if (cancelled) {
          opened.stream.getTracks().forEach((track) => track.stop());
          return;
        }
        stream = opened.stream;
        const video = videoRef.current;
        if (video) {
          video.srcObject = stream;
          await video.play();
        }
        setDisconnected(false);
      } catch {
        if (!cancelled) setDisconnected(true);
      }
    })();
    return () => {
      cancelled = true;
      stream?.getTracks().forEach((track) => track.stop());
      const video = videoRef.current;
      if (video) video.srcObject = null;
    };
  }, [camera.deviceId, present, isNetwork]);

  // Network camera: Rust holds the connection and keeps the newest frame;
  // poll it and decode only frames not drawn yet.
  useEffect(() => {
    if (!isNetwork || !camera.url) return;
    let active = true;
    let timer = 0;
    let lastSeq: number | null = null;
    const poll = async () => {
      if (!active) return;
      let delay = NETWORK_POLL_MS;
      try {
        const frame = await latestNetworkFrame(camera.url as string, lastSeq);
        if (frame?.jpegB64) {
          lastSeq = frame.seq;
          const img = new Image();
          img.onload = () => {
            if (!active) return;
            netImageRef.current = img;
            setDisconnected(false);
          };
          img.src = `data:image/jpeg;base64,${frame.jpegB64}`;
        }
      } catch {
        if (active) setDisconnected(true);
        delay = NETWORK_RETRY_MS;
      } finally {
        if (active) timer = window.setTimeout(() => void poll(), delay);
      }
    };
    void poll();
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [isNetwork, camera.url]);

  // Display loop: draw the current frame upright into the visible canvas at
  // the display's own frame rate (detection runs separately and slower).
  useEffect(() => {
    if (disconnected) return;
    let raf = 0;
    const draw = () => {
      raf = requestAnimationFrame(draw);
      const src = readSource();
      const canvas = displayRef.current;
      if (!src || !canvas) return;
      const { rotation } = viewRef.current;
      const size = rotatedSize(src.width, src.height, rotation);
      if (canvas.width !== size.width || canvas.height !== size.height) {
        canvas.width = size.width;
        canvas.height = size.height;
      }
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      drawRotated(ctx, src.source, src.width, src.height, rotation);
      setBox(canvasContentBox(canvas));
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [disconnected, readSource]);

  // Recognition switched off: drop any hands, ring, and half-done hold.
  useEffect(() => {
    if (gesturesEnabled) return;
    trackerRef.current = new HoldTracker({ ...DEFAULT_HOLD_CONFIG, holdMs });
    outcomeRef.current = null;
    setHands([]);
    setHold(null);
    setOutcome(null);
  }, [gesturesEnabled, holdMs]);

  // Detection loop. With recognition off it only keeps the desk frame fresh.
  useEffect(() => {
    if (disconnected) return;
    if (!canvasRef.current)
      canvasRef.current = document.createElement("canvas");

    let disposed = false;
    const tick = async (): Promise<void> => {
      if (busyRef.current) return;
      const src = readSource();
      const canvas = canvasRef.current;
      if (!src || !canvas) return;
      const { rotation, crop } = viewRef.current;
      const cropper = sampleFrame(
        src.source,
        src.width,
        src.height,
        canvas,
        rotation,
        crop,
      );
      if (!cropper) return;
      cropperRef.current = cropper;

      const sampledAt = performance.now();
      const watching = !localPausedRef.current && !forcePaused;
      if (
        watching &&
        sampledAt - lastFrameReportRef.current > FRAME_REPORT_INTERVAL_MS
      ) {
        lastFrameReportRef.current = sampledAt;
        void reportFrame(camera.deviceId, camera.name, cropper.deskJpegB64());
      }
      if (!gesturesEnabled) return;

      busyRef.current = true;
      try {
        const detection = await detectHands(cropper.detectionJpegB64());
        if (disposed) return;
        const now = performance.now();

        if (lastTickRef.current > 0) {
          const instantaneous = 1000 / (now - lastTickRef.current);
          fpsRef.current = fpsRef.current
            ? fpsRef.current * 0.8 + instantaneous * 0.2
            : instantaneous;
        }
        lastTickRef.current = now;
        const latencies = latenciesRef.current;
        latencies.push(now - sampledAt);
        if (latencies.length > 30) latencies.shift();
        if (now - lastStatsRef.current > STATS_REPORT_MS) {
          lastStatsRef.current = now;
          const sorted = [...latencies].sort((a, b) => a - b);
          onStats(camera.deviceId, {
            fps: fpsRef.current,
            latencyMs: sorted[Math.floor(sorted.length / 2)] ?? 0,
          });
        }

        if (
          watching &&
          detection.hands.length === 0 &&
          now - lastHandFreeRef.current > HAND_FREE_CAPTURE_MS
        ) {
          lastHandFreeRef.current = now;
          handFreeRef.current = {
            jpegB64: cropper.deskJpegB64(),
            atMs: Date.now(),
          };
        }
        const gesture = gestureVisibleWhen(
          classifyGesture(detection),
          !localPausedRef.current && !forcePaused ? "watching" : "paused",
          enabledGestures,
        );
        onSample(camera.deviceId, detection.hands);

        const output = trackerRef.current.next(gesture, sampledAt);
        setHands(detection.hands);
        setHold(output);

        if (output.fired) {
          const result = onFire(camera.deviceId, output.fired);
          outcomeRef.current = result;
          setOutcome(result);
          if (result === "fire") {
            if (output.fired === "fist") setLocalPaused(true);
            if (output.fired === "open-palm") setLocalPaused(false);
            if (output.fired === "pointing") {
              const tip = detection.hands
                .map((hand) => pointingTip(hand))
                .find((point) => point !== null);
              if (tip) {
                const next = {
                  x: tip.x,
                  y: tip.y,
                  expiresAt: performance.now() + FOCUS_TTL_MS,
                };
                focusRef.current = next;
                setFocus(next);
              }
            }
          }
        } else if (output.phase !== "fired" && outcomeRef.current !== null) {
          outcomeRef.current = null;
          setOutcome(null);
        }
      } finally {
        busyRef.current = false;
      }
    };

    const interval = window.setInterval(
      () => void tick(),
      ANALYSIS_INTERVAL_MS,
    );
    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [
    camera.deviceId,
    disconnected,
    forcePaused,
    gesturesEnabled,
    enabledGestures,
    onFire,
    onStats,
    onSample,
    readSource,
  ]);

  const commitRename = useCallback(() => {
    setRenaming(false);
    onRename(camera.deviceId, draftName);
  }, [camera.deviceId, draftName, onRename]);

  // ----- crop editing -----
  // A pointer position on the display → a fraction of the (unmirrored) frame.
  const pointerFraction = (
    event: React.PointerEvent,
  ): { x: number; y: number } | null => {
    const canvas = displayRef.current;
    if (!canvas || !box) return null;
    const rect = canvas.getBoundingClientRect();
    const px = (event.clientX - rect.left - box.left) / box.width;
    const py = (event.clientY - rect.top - box.top) / box.height;
    const x = Math.min(1, Math.max(0, camera.mirror ? 1 - px : px));
    const y = Math.min(1, Math.max(0, py));
    return { x, y };
  };

  const onHandleDown = (corner: Corner) => (event: React.PointerEvent) => {
    event.preventDefault();
    event.stopPropagation();
    (event.target as Element).setPointerCapture(event.pointerId);
    dragRef.current = corner;
  };

  const onHandleMove = (event: React.PointerEvent) => {
    if (!dragRef.current) return;
    const point = pointerFraction(event);
    if (!point) return;
    const c = camera.crop;
    // The corner opposite the dragged one stays put.
    const left = dragRef.current === "nw" || dragRef.current === "sw";
    const top = dragRef.current === "nw" || dragRef.current === "ne";
    const fixedX = left ? c.x + c.width : c.x;
    const fixedY = top ? c.y + c.height : c.y;
    const x = Math.min(point.x, fixedX);
    const y = Math.min(point.y, fixedY);
    onCrop(camera.deviceId, {
      x,
      y,
      width: Math.abs(fixedX - point.x),
      height: Math.abs(fixedY - point.y),
    });
  };

  const onHandleUp = (event: React.PointerEvent) => {
    dragRef.current = null;
    (event.target as Element).releasePointerCapture?.(event.pointerId);
  };

  // Crop rectangle in display pixels, accounting for mirror.
  const cropStyle = (): React.CSSProperties | undefined => {
    if (!box) return undefined;
    const c = camera.crop;
    const x = camera.mirror ? 1 - c.x - c.width : c.x;
    return {
      left: box.left + x * box.width,
      top: box.top + c.y * box.height,
      width: c.width * box.width,
      height: c.height * box.height,
    };
  };

  const showCropRect = editingCrop || !isFullFrame(camera.crop);
  const cropPercent = (value: number) => Math.round(value * 100);

  return (
    <li
      className="tile"
      aria-label={`${camera.name} — ${STATUS_LABEL[status]}`}
    >
      <div className="tile__video-wrap">
        <video ref={videoRef} className="tile__source" muted playsInline />
        <canvas ref={displayRef} className="tile__video" />
        {!disconnected && (
          <GestureOverlay
            hands={hands}
            hold={hold}
            holdMs={holdMs}
            box={box}
            mirror={camera.mirror}
            showLandmarks={showLandmarks}
            outcome={outcome}
          />
        )}
        {!disconnected && showCropRect && box && (
          <div className="tile__crop" style={cropStyle()}>
            {editingCrop &&
              (["nw", "ne", "sw", "se"] as Corner[]).map((corner) => (
                <span
                  key={corner}
                  className={`tile__crop-handle tile__crop-handle--${corner}`}
                  onPointerDown={onHandleDown(corner)}
                  onPointerMove={onHandleMove}
                  onPointerUp={onHandleUp}
                />
              ))}
          </div>
        )}
        {!disconnected && focus && box && (
          <div
            className="tile__focus"
            style={{
              left:
                box.left + (camera.mirror ? 1 - focus.x : focus.x) * box.width,
              top: box.top + focus.y * box.height,
            }}
          >
            <span className="tile__focus-dot" />
            <span className="tile__focus-label">
              Attention point ·{" "}
              {Math.max(
                0,
                Math.ceil((focus.expiresAt - performance.now()) / 1000),
              )}
              s
            </span>
          </div>
        )}
        <span className="tile__sr" role="status" aria-live="polite">
          {focus ? "Attention point set" : ""}
        </span>
        {disconnected && (
          <p className="tile__disconnected">
            Disconnected — waiting for {camera.name} to come back.
          </p>
        )}
        {status === "paused" && (
          <p className="tile__paused">
            {gesturesEnabled && enabledGestures.includes("open-palm")
              ? "Paused — show an open hand to resume."
              : "Paused — click Resume to continue."}
          </p>
        )}
      </div>

      <div className="tile__bar">
        <span
          className={`tile__status tile__status--${status}`}
          title={STATUS_LABEL[status]}
        >
          <span className="tile__status-dot" aria-hidden="true" />
        </span>
        {renaming ? (
          <input
            className="tile__name-input"
            aria-label={`Rename ${camera.name}`}
            value={draftName}
            autoFocus
            onChange={(event) => setDraftName(event.target.value)}
            onBlur={commitRename}
            onKeyDown={(event) => {
              if (event.key === "Enter") commitRename();
              if (event.key === "Escape") {
                setDraftName(camera.name);
                setRenaming(false);
              }
            }}
          />
        ) : (
          <button
            type="button"
            className="tile__name"
            onClick={() => {
              setDraftName(camera.name);
              setRenaming(true);
            }}
            aria-label={`${camera.name}. Rename`}
          >
            {camera.name}
            <span className="tile__status-text"> · {STATUS_LABEL[status]}</span>
          </button>
        )}
        <div className="tile__actions">
          <button
            type="button"
            className="tile__action"
            aria-pressed={localPaused}
            onClick={() => setLocalPaused((value) => !value)}
            disabled={disconnected || forcePaused}
          >
            {localPaused ? "Resume" : "Pause"}
          </button>
          <button
            type="button"
            className="tile__action"
            onClick={() => onRotate(camera.deviceId)}
            aria-label={`Rotate ${camera.name}`}
          >
            Rotate
          </button>
          <button
            type="button"
            className="tile__action"
            aria-pressed={editingCrop}
            onClick={() => setEditingCrop((value) => !value)}
          >
            Crop
          </button>
          <button
            type="button"
            className="tile__action"
            aria-pressed={camera.mirror}
            onClick={() => onMirror(camera.deviceId, !camera.mirror)}
          >
            Mirror
          </button>
          <button
            type="button"
            className="tile__action tile__action--remove"
            onClick={() => onRemove(camera.deviceId)}
            aria-label={`Remove ${camera.name}`}
          >
            Remove
          </button>
        </div>
      </div>

      {editingCrop && (
        <div className="tile__crop-editor">
          {(
            [
              ["x", camera.crop.x],
              ["y", camera.crop.y],
              ["width", camera.crop.width],
              ["height", camera.crop.height],
            ] as const
          ).map(([key, value]) => (
            <label key={key} className="tile__crop-field">
              {key}
              <input
                type="number"
                min={0}
                max={100}
                value={cropPercent(value)}
                onChange={(event) =>
                  onCrop(
                    camera.deviceId,
                    clampCrop({
                      ...camera.crop,
                      [key]: Number(event.target.value) / 100,
                    }),
                  )
                }
              />
              %
            </label>
          ))}
          <button
            type="button"
            className="tile__action"
            onClick={() => onResetView(camera.deviceId)}
          >
            Reset
          </button>
        </div>
      )}
    </li>
  );
};
