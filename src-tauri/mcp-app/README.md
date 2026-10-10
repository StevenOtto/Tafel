# Tafel MCP App UI

`delivery.html` is the single-file MCP App rendered inside the Claude
conversation when Tafel's tools run (`get_desk`, `get_latest_delivery`,
`wait_for_delivery`, `list_deliveries`, `get_delivery`). It is embedded into
the Rust binary at compile time and served as the `ui://tafel/delivery.html`
resource (mimeType `text/html;profile=mcp-app`).

## States

- Tool call starts (`ontoolinput`): a quiet waiting card —
  "Waiting for your next visual prompt — thumbs up when ready…" for
  `wait_for_delivery` (the tool name comes from the host context's
  `toolInfo`), otherwise "Fetching from Tafel…". The original tool arguments
  are kept for the fallback below.
- Tool result (`ontoolresult`): a summary line (kind, `#seq` and image
  count), an optional "Skipped: …" line, and a gallery with one figure per
  image, captioned with camera name and absolute capture time. Each image is
  a button that opens an enlarged view in a `<dialog>`. The status line says
  "Delivered to agent"; it changes to "Working on your prompt", "Needs your
  input" or "Completed" only after the agent calls `acknowledge_delivery`.
  Desk views say "context only".
- Terminal states render as a message without old images: paused, cameras
  disconnected, no cameras, timeout (no protocol cursor text), missing
  delivery, cancelled (`ontoolcancelled`, late results ignored) and errors.
- Fallback and status polling go through the app-only `get_card_state` tool
  (`_meta.ui.visibility: ["app"]`), which never records an agent read. It
  replays the original tool with its exact `seq` / `after_seq` (no
  age-based guessing). A card-only fetch says "agent receipt unconfirmed".
  Polling is bounded, stops on a terminal state or completion, and is
  cleared at `onteardown`. `src/core/mcpCard.test.ts` drives this script
  against a simulated host.
- Theme follows the host: `applyDocumentTheme` / `applyHostStyleVariables` /
  `applyHostFonts` plus safe-area insets via `onhostcontextchanged`; CSS uses
  host variables (`--color-*`, `--font-*`, `--border-radius-*`,
  `--shadow-*`) with Tafel-flavored fallbacks.

## Vendored SDK

The second `<script>` in `delivery.html` is the official iframe SDK:

- Package: `@modelcontextprotocol/ext-apps` **1.7.5** (MIT)
- Source: `https://unpkg.com/@modelcontextprotocol/ext-apps@1.7.5/dist/src/app-with-deps.js`
  (fetched 2026-08-18)
- Rebundled from ESM to a minified IIFE exposing the global `ExtApps`
  (esbuild `--bundle --format=iife --global-name=ExtApps --minify`), because
  the sandboxed iframe's default CSP only allows inline scripts — no module
  imports from external origins.

To upgrade: fetch the new `app-with-deps.js`, rerun the esbuild command,
splice it into the first vendored `<script>` block, and re-verify the
handler names used by the app code (`ontoolinput`, `ontoolresult`,
`ontoolcancelled`, `onhostcontextchanged`, `onteardown`, `getHostContext`).
