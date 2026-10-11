# Tafel — Continuous fold

Production asset set selected on 10 October 2026 and revised on 11 October
2026: the inset stroke now points right, so the mark reads as a prompt (`>_`).
The chevron is 24 units tall with exact 45° arms, which keeps small sizes
crisp, and sits 6 units from the top and left inner edges. The outline's open
lower edge starts at x 52. Path:
`M21 71V29H79V71H52M33 41L45 53L33 65`. Round 2's paper, objects, and motion
cues are not part of this identity.

## Choose an asset

| Use                            | File or folder                                               |
| ------------------------------ | ------------------------------------------------------------ |
| Editable mark master           | `source/mark.svg`                                            |
| Editable macOS app-icon master | `source/app-icon.svg`                                        |
| Transparent standalone marks   | `marks/` — SVG and 1024px PNG                                |
| Horizontal logos               | `lockups/` — lowercase and uppercase, SVG and 1600px PNG     |
| App icons                      | `app-icon/` — yellow, dark, and paper; SVG and 16–1024px PNG |
| Native app bundles             | `app-icon/tafel.icns` and `app-icon/tafel.ico`               |
| Website and browser tabs       | `web/` — SVG, multi-size ICO, 16/32/48px PNG                 |
| iOS home-screen bookmark       | `web/apple-touch-icon.png` — opaque 180px PNG                |
| Social avatar                  | `social/avatar-1024.png` — square, opaque                    |
| Social sharing image           | `social/social-card-1200x630.png`                            |

All marks and lockups have transparent backgrounds. `ink` is for light surfaces;
`paper` is for dark surfaces. `black` and `white` provide pure monochrome versions.
The app-icon SVGs and PNGs include a rounded tile with transparent outside padding.
The avatar and home-screen bookmark are full bleed so their host can apply its mask.

The standalone lockups use outlined Helvetica Neue Bold lettering and need no
font installation. The landing page keeps its existing uppercase Schibsted Grotesk
wordmark and adds the C1 mark beside it. The editable social-card SVG uses live text
for its tagline; use the supplied PNG when exact appearance matters.

## Color and spacing

- Ink: `#1b2130`
- Paper: `#f5f5f1`
- Sticky-note yellow: `#f1df72`
- Mark: 6-unit round stroke on a 100 × 100 coordinate system.
- Keep at least 12 units of clear space around the visible mark.
- Keep the angles, open lower edge, and rounded stroke ends intact. Do not
  flip the chevron back, stretch, close the gap, or add a cursor, paper, or
  other symbol.
- Default app treatment: ink mark on yellow. Dark and paper are optional exports;
  they do not automatically switch the installed macOS app's appearance.
- Prefer at least 24px for the standalone mark and 120px for a horizontal logo.
  16px files are supplied for constrained system uses and require contextual review.

## Installed assets

`src-tauri/icons/` contains the replacement native icon family, using the existing
Tauri configuration. `site/assets/` contains the web icons, and the landing-page
header renders the same mark in the current text color. `public/tafel.svg` supplies
the frontend browser tab icon. The site header therefore follows both light and
dark themes. No network resources or image credentials are embedded in the assets.

## Regeneration

From the repository root, regenerate the native family from the master:

```sh
npm run tauri -- icon brand/source/app-icon.svg --output /tmp/tafel-icons
```

Copy the matching filenames into `src-tauri/icons/`, then copy `icon.icns` and
`icon.ico` to this pack's `app-icon/` folder as `tafel.icns` and `tafel.ico`.
The Tauri command also produces mobile files; this macOS app does not use them.
Other PNG exports can be rasterized from their corresponding SVGs at the pixel
width in the filename. Keep alpha transparency when exporting marks and logos.
The social PNG is 1200 × 630; the bookmark is 180 × 180.

The ICNS file must be included in a rebuilt app for Finder and the Dock to use it.
macOS may cache the previous icon until the app is relaunched. Installed-app/Dock
appearance has not been verified by the asset-generation checks.
