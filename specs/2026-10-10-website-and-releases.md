# Website and signed releases

Status: approved by the user on 2026-10-10 (scope below). Repository stays
private for now; publishing is a later decision.

## Goal

Move the landing page from a private Claude artifact into this repository as
a static site that GitHub Pages can serve, and publish signed, notarized DMGs
from version tags so the site's download button points at a real build.

## Scope

- `site/`: the landing page as plain HTML, CSS and JS, no build step. Assets
  (desk photos, screen recording, example pages) are files next to it.
  - Music example: a real link laid over the Spotify card in the video opens
    the track in Spotify (keyboard-focusable, labelled, visible focus ring),
    plus a visible "Play on Spotify" link. No hidden YouTube player: YouTube's
    terms forbid hiding the player, and sound cannot start without a click.
  - Download button: `releases/latest/download/Tafel.dmg`, a stable asset
    name, so the link never changes between versions.
- `.github/workflows/pages.yml`: deploys `site/` to GitHub Pages. Manual
  trigger only while the repository is private (Pages on a private repository
  needs a paid plan). Switch to `push` on `main` when going public.
- `.github/workflows/release.yml`: on a `v*` tag, on a macOS runner:
  check the tag matches `tauri.conf.json`'s version, import the Developer ID
  certificate into a temporary keychain, build, sign and notarize the app,
  notarize and staple the DMG, and attach it to a GitHub release as
  `Tafel.dmg`.
- README: website and release sections, including the repository secrets.

Out of scope: making the repository public, a custom domain, auto-update,
Intel builds, the directory plugin.

## Decisions

- Releases on tags, not every push to `main`: a macOS build plus notarization
  takes minutes and costs macOS runner time, and half-finished work should not
  ship.
- Desk photos in `site/` are images the user chose for publication. They are
  the only desk images in the repository; the AGENTS.md rule against
  committing private desk images still holds for everything else.

## Secrets (set by the user)

`APPLE_CERTIFICATE` (base64 of the exported Developer ID `.p12`),
`APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD` (any random string),
`APPLE_ID`, `APPLE_PASSWORD` (app-specific password), `APPLE_TEAM_ID`.

## Verification

- Site: open `site/index.html` locally; all examples, enlarge, video loop,
  Spotify link (mouse and keyboard), light and dark, 400 px width.
- Workflows: syntax check locally. The release workflow can only be proven by
  pushing a tag once the secrets exist; that run is a pending manual check.

## Handoff (2026-10-10)

Built on `feature/website`, uncommitted: `site/` (page plus `examples/`),
both workflows, README "Releases" and "Website", AGENTS.md area row and
image note, `.prettierignore` entry for `site/examples/`.

Checked: the page loads from a local HTTP server, its scripts parse, and a
headless Chrome screenshot of the examples section matches the artifact.
Workflow files parse as YAML. Prettier passes on changed files.

Pending: Spotify link over the card (mouse and keyboard) in a real browser;
the
release workflow's first run after the secrets are added; Pages deploy once
the repository is public.
