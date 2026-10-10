# Higher-resolution deliveries

Status: requested by the user on 2026-10-10.

## Change

Snapshots were capped at 1280 px wide (`regionJpegB64` in
`src/app/cameraSource.ts`), although cameras are asked for 1920×1080. The cap
is now 1920 px on the longest side (`SNAPSHOT_MAX_EDGE` and `fitWithin` in
`src/core/cameraSet.ts`), so a full-frame snapshot keeps every captured
pixel. Portrait and rotated frames get the same limit; before, a rotated
1080p frame came out at 1080×1920 while a landscape one was cut to 1280×720.

Everything that uses `regionJpegB64` follows: the delivery snapshot, the
hand-free pre-capture, focus crops and the `get_desk` frame. Rust stores and
serves the bytes unchanged, so no backend change is needed. Hand detection
keeps its own 640 px image.

## Decisions

- 1920, not higher: it matches the requested camera resolution, and current
  Claude models use images up to 2576 px without downscaling. Asking cameras
  for more than 1080p would make the 100 ms frame loop draw much larger
  frames; not worth it without knowing what Desk View can deliver.
- JPEG quality stays 0.75.
- Older sessions are unaffected; their 1280 px images still load.

## Trade-offs

- Token cost per full-frame image roughly doubles (about 1,200 to about
  2,700 tokens on current Claude models). Claude Code limits an MCP tool
  result to `MAX_MCP_OUTPUT_TOKENS` (default 25,000). The collection limit
  (`MAX_COLLECTION` in `src/core/delivery.ts`) is therefore lowered from 12
  to 8 snapshots: eight full-frame images are about 21,500 tokens.
- Files grow from roughly 90–150 KB to about 2× that.

## Verification

- `npx tsc --noEmit`, `npm test` (75 passed, including the new `fitWithin`
  test; the collection tests use the constant), Prettier on changed files.
- Runtime, signed local build (2026-10-10): new deliveries from the desk
  camera are stored at 1920×1080 (265–355 KB); the user tested deliveries
  and reported that they work.

## Later

Full-resolution originals next to an agent-sized copy, a `full_resolution`
option on `get_delivery`, and an export button in the app, if higher-res
sources (4K) become relevant.
