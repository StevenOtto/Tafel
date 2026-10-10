# Prototype-pipeline verwijderen

`2026-08-26` · Steven Otto · branch `feature/gesture-capture`

## Waarom

Tafel is sinds de pivot van 2026-08-18 een visueel invoerapparaat voor agents. De
oorspronkelijke prototype-pipeline (Gemini fast path → UI-tree, Stitch promote,
tweede display-venster, kostenledger, Keychain-credentials) is dood gewicht: hij
verdubbelt elke codepad achter een modusschakelaar, houdt uitgaand netwerk en
API-sleutels in de app en maakt elke nieuwe feature twee keer zo duur. De
gebaren- en multi-camera-specs bouwen op één modus; deze opruiming gaat vooraf.

Correctie op de aanname "Google Live API houdt beweging bij": de
bewegingsdetectie is nu al lokaal (`src/core/detector.ts`, pixel-diff). Google
zit alleen in de prototype-pipeline. Die gaat er hier volledig uit.

## Scope

- **Wel:** verwijderen van `gemini.rs`, `stitch.rs`, `assets.rs`, `ledger.rs`,
  `history.rs`, `secrets.rs`; TS `document`, `editOps`, `diff`, `render`,
  `pipeline`, `prompt`, `promote`, `DisplayApp`, `display.html`; live tests,
  `scripts/stitch-probe.mjs`, `scripts/verify-identifiers.mjs`, `docs/probes`,
  `docs/stitch-tools-list…json`, `design-system.json`; de `mode`-setting en de
  Stitch-/zone-velden in `Settings`; de bijbehorende Tauri-commands en
  Cargo-dependencies (`reqwest` alleen nog als dev-dependency voor de
  HTTP-smoketest, `security-framework` weg); README/DESIGN.md/FUTURE.md.
- **Niet:** MCP-server, `SketchBus`, camera-acquisitie, settings-persistentie,
  MCP App-kaart (die verandert in de leveringen-spec), settle detector (wordt
  vervangen in de gebaren-spec, blijft hier intact).

## Acceptatiecriteria

1. `grep -rEi "gemini|stitch|googleapis|keychain" src src-tauri/src` levert 0
   treffers; de app doet geen enkele uitgaande netwerkverbinding meer (alleen
   de loopback MCP-listener).
2. De commands `fast_generate`, `stitch_call`, `fetch_stitch_asset`,
   `save_generation`, `ledger_state`, `credential_presence`, `load_history`,
   `load_frame` bestaan niet meer; `report_frame`/`report_sketch`,
   `get_settings`/`set_settings` werken ongewijzigd.
3. Eén venster (`control`); `vite build` heeft één entry; de
   capabilities-lijst noemt alleen dat venster.
4. Een `settings.json` van de vorige versie (met `mode`, `promoteRegion`,
   `stitch*`) laadt zonder fout; onbekende velden worden genegeerd
   (regressietest in `settings.rs`).
5. `npm test` en `cargo test` slagen; `cargo build` geeft geen
   dead-code-warnings; er blijft geen ongebruikte export achter.
6. README beschrijft alleen nog de agent-input-richting (installatie, MCP-
   koppeling, trust model); DESIGN.md §1–11 wordt vervangen door één korte
   sectie "Historie" met verwijzing naar de git-historie; FUTURE.md verliest de
   Stitch-items.
7. De macOS-camera-usage-string in `Info.plist` beschrijft de nieuwe
   functie (gebaren, snapshots naar een agent), niet "sketches into interfaces".
