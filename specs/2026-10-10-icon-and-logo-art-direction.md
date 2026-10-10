# Tafel icon and logo art direction — 2026-10-10

## Accepted scope

Explore many logo and app-icon directions from the user's landing-page screenshot
and hand-drawn sketches: combine a `< _` prompt with a table side view, with
secondary explorations of paper, framing, and camera shapes. This is an exploratory
review round. The user subsequently selected C1 for production; see the production
handoff below.

## Round 1

Created an interactive board with 24 numbered concepts across six families:
prompt/furniture, cursor/surface, folded strokes, paper/desk, frame/capture, and
table/letterform. Each mark can be previewed with a wordmark, landing-page header,
and 128/64/32/16px app-icon treatments. Shortlisting and follow-up controls support
iteration. Optional design controls adjust stroke, ends, case, and icon background.

Palette follows the supplied landing page: paper, dark ink, and sticky-note yellow.
The initial art-direction suggestions are A2, B1, and C3; these are suggestions,
not user selections. Final tiny-size optical corrections and native icon packaging
are outside this exploration round.

## Files and verification

- Board: `/Users/steven/.codex/visualizations/2026/10/10/01a1265d-40ec-7d80-877d-845768ce4ade/tafel-mark-exploration.html`
- Repository change: this scope/handoff document only. Existing app and site assets
  are untouched. Pre-existing working-tree changes were preserved.
- Node script syntax check passed.
- Rendered with the visualize skill's `scripts/render.py` into a temporary preview.
- Playwright browser check via `/tmp/check-tafel-marks.cjs`: 24 choices rendered,
  B1 selection and shortlist update passed, no browser script errors. At a 360px
  browser viewport the content width and scroll width both measured 328px.
- Visually inspected the full board and application previews. Corrected F4's
  negative-space geometry following inspection.
- Host-mediated follow-up submission and host design controls require manual use
  in the conversation; the standalone preview cannot verify actual chat delivery.
- No frontend or native build needed: no production code changed.

## Next step after round 1

User shortlists concepts, then refine the selected constructions and compare
wordmarks and icon treatments. Changes are uncommitted; no commit, push, or release
authorized.

## Round 2 — ideas on a table

User liked C1 (Continuous fold), but found the terminal interpretation too strong.
The accepted direction is to explore physical things on a table coming to life.
The prompt chevron is no longer required. C1 is a starting preference, not a final
selection.

Created a separate board, preserving round 1:
`/Users/steven/.codex/visualizations/2026/10/10/01a1265d-40ec-7d80-877d-845768ce4ade/tafel-ideas-on-a-table.html`.
It contains 16 concepts (G1–G4, H1–H4, J1–J4, K1–K4): folded desks with objects,
lifting paper, living drawings, and perspective tabletops. Wordmark, icon sizes,
shortlist, and optional design controls remain available. H1 is the initial
preview. H1 and K4 are recommended for comparing simplicity against explicit
physical context; G2 is a quieter alternative.

Verification: Node syntax check passed (11,571 bytes); rendered with the visualize
preview helper; `/tmp/check-tafel-alive.cjs` checked 16 choices, K4 selection and
shortlisting, and reported zero browser script errors. At 360px viewport, both
content and scroll widths were 328px. Full board screenshot visually inspected.
Host follow-up delivery and optional design controls still require in-conversation
manual verification. No production assets changed. Scope document and exploration
remain uncommitted. Next step: user selects which physical-object/metamorphosis
reading to refine, followed by optical simplification at small sizes.

## Production handoff — C1 selected

The user explicitly selected **C1 / Continuous fold** and requested all asset
creation. Preserve its original geometry and round 6-unit strokes. The production
app uses dark ink on sticky-note yellow; optional paper and dark icon exports are
provided. The website retains its established uppercase typography with the mark
added. No round-2 objects or motion cues are included.

Created `brand/` with editable SVG masters, transparent SVG/PNG marks in ink,
paper, black, and white; outlined lowercase/uppercase wordmark lockups; app icons
at 16, 32, 64, 128, 256, 512, and 1024px; native ICNS/ICO; web favicons; home-screen
bookmark; social avatar and sharing image; and usage/regeneration instructions.
The lockup glyphs were converted from local Helvetica Neue Bold using CoreText;
no font binaries are included. The social-card SVG retains live tagline text and
has a ready-to-use PNG export.

Changed files/groups:

- `brand/` — production assets and usage guide.
- `src-tauri/icons/` — replaced the existing icon family via Tauri's icon generator.
- `site/assets/` — website icon files.
- `site/index.html` — header mark and favicon/bookmark links.
- `public/tafel.svg`, `index.html` — frontend browser tab icon.
- This spec — accepted scope and handoff.

Verification performed on the current files:

- `npm run tauri -- icon brand/source/app-icon.svg --output /tmp/tafel-production-icons`
  succeeded; copied matching existing native filenames, leaving mobile output out.
- `npm run build` passed: TypeScript and Vite, 48 modules transformed.
- `iconutil --convert iconset --output /tmp/tafel-verified.iconset src-tauri/icons/icon.icns`
  succeeded, confirming macOS can decode the bundle.
- `/tmp/verify-tafel-assets.cjs` decoded 57 PNGs and 20 SVGs across the pack and
  installed copies, checked all seven yellow icon dimensions and alpha, confirmed
  outlined lockups, and checked three favicon ICO entries.
- `/tmp/check-tafel-brand-site.cjs` confirmed the exact C1 path, two favicon links,
  correct light/dark header colors, no script errors, and no horizontal overflow
  at 375px (content width 375px). Light/dark screenshots visually inspected.
- Production proof sheet visually inspected, including 16px and 32px specimens.
- Formatting check passed for changed HTML and Markdown files.

A downloadable ZIP and preview sheet live in this chat's visualization directory,
not in the repository. The ZIP contains the `brand/` asset set and its usage guide.
The app's Finder/Dock appearance requires a native rebuild and launch; this was not
performed. The website changes are local, not deployed. Existing unrelated working
tree changes remain preserved. All changes are uncommitted; no push or release was
requested. Next step: use the assets and verify the icon when next rebuilding the
native app.

## Landing-page wordmark spacing correction

User flagged the uppercase landing-page logotype's spacing. In `site/index.html`,
reduced `.wordmark` tracking from `0.18em` to `0.04em`, explicitly enabled the
font's native kerning, and reduced the symbol/text gap from 10px to 8px. The C1
geometry and exported outlined logos are unchanged; this corrects the live-text
header treatment shown in the user's screenshot.

Verification: `npx prettier --check site/index.html` passed. Browser verification
with Schibsted Grotesk confirmed loaded: 0.672px letter spacing, normal kerning,
8px symbol gap; no browser script errors and no overflow at 375px. Light/dark
screenshots captured and light preview visually inspected. No native build is
needed for this CSS-only correction. Changes remain uncommitted and unpublished.
Next step: review the corrected header in the local landing page.
