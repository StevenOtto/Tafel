# Handgebaren als trigger (Apple Vision)

`2026-08-26` · Steven Otto · branch `feature/gesture-capture`

## Waarom

De settle-detector stuurt een snapshot zodra het beeld een paar seconden stil is:
ongevraagd, op het verkeerde moment, en met een muurcamera is "stilte" geen
signaal meer. Expliciete handgebaren geven de gebruiker de regie, werken op elke
camera zonder de Mac aan te raken, en de 2-seconden-hold met zichtbare voortgang
voorkomt per-ongeluk-triggers. Apple Vision (`VNDetectHumanHandPoseRequest`)
levert 21 handlandmarks per hand on-device, offline en gratis.

## Scope

- **Wel:** handdetectie via Vision in een kleine Swift-brug (gecompileerd vanuit
  `build.rs`, bv. `swift-rs`); gebaarclassificatie op landmarks; hold-machine
  met voortgangsring; acties pauze/hervat; vervangen van settle- en
  promote-token-detector incl. synthetische fixtures; instrument panel voor
  gebaren.
- **Niet:** wat een snapshot/levering is en hoe die bij de agent komt
  (→ leveringen-spec); meerdere camera's (→ multi-camera-spec); getrainde
  Create ML-classifier (later, als heuristiek tekortkomt).

## Domeinmodel

- **Hand** — 21 landmarks + chiraliteit (links/rechts) uit één cameraframe.
- **Gebaar** — `thumbs-up`, `two-thumbs-up` (twee handen), `open-palm`, `fist`,
  `ok-sign`, `none`. Deterministische functie van landmarks, geen model.
- **Hold** — hetzelfde gebaar aaneengesloten vastgehouden; voortgang 0→1 over
  2,0 s; bij 1 vuurt de bijbehorende **actie** één keer; loslaten ervóór =
  annuleren; na vuren geen nieuwe hold tot het gebaar is losgelaten.
- **Camerastatus** — `watching` | `paused`, **per camera**: een gebaar geldt
  voor de camera die het ziet. Gepauzeerd: geen frames naar de bus, geen
  snapshot-acties; alleen `fist` (hervat) wordt herkend. Daarnaast een globale
  pauzeknop in de UI.

Frames blijven in de webview (`getUserMedia`); per camera gaat ~10 fps een
verkleinde JPEG (≤ 640 px) naar Rust → Swift → Vision; landmarks komen als JSON
terug. Classificatie en hold-machine zijn TypeScript zonder eigen tijdbron, dus
exact te testen met opgenomen landmark-sequenties — zoals de detector nu.

## Acceptatiecriteria

1. Handdetectie werkt met netwerk uit; geen enkele externe dienst.
2. Classifier: synthetische landmark-sets voor elk van de vijf gebaren worden
   correct benoemd, een neutrale hand geeft `none`; testcases zijn literals.
3. Hold: gebaar 2,0 s vastgehouden → precies 1 actie; losgelaten bij 1,9 s →
   0 acties; ≤ 2 opeenvolgende frames zonder detectie (~200 ms) breken de hold
   niet; na vuren volgt geen tweede actie zolang het gebaar niet ≥ 500 ms weg
   is geweest. Getest met opgenomen sequenties, zonder camera of Vision.
4. Feedback: binnen 100 ms na herkenning verschijnt een ring rond de hand
   (bbox-centrum) die lineair volloopt; bij vuren korte bevestiging (kleur +
   vinkje), bij annuleren verdwijnt de ring. Naast de ring staat de gebaarnaam
   als tekst; een `aria-live`-regio meldt elke gevuurde actie; optioneel
   geluid bij vuren (instelling, standaard aan — de gebruiker kijkt vaak niet
   naar het scherm).
5. `open-palm` → die camera `paused` (UI-status als tekst én kleur; `get_desk`
   slaat hem over en noemt hem gepauzeerd); `fist` → `watching`. In `paused`
   reageert alleen `fist`. De globale pauzeknop pauzeert/hervat alle camera's
   tegelijk.
6. Instrument panel: latency frame→landmarks (p50/p95), aantal handen, huidig
   gebaar + hold-voortgang, landmark-overlay aan/uit, knop **Record** die een
   JSONL-fixture met landmark-sequentie wegschrijft.
7. `SettleDetector`, `PromoteTokenDetector`, `src/fixtures/*`, de bijbehorende
   tests en `scripts/convert-fixture.sh` zijn verwijderd; `cargo test` bevat
   een smoketest die Vision op een meegeleverde hand-JPEG ≥ 1 hand laat vinden
   en op een lege afbeelding 0.
8. Build is macOS-only en zegt dat expliciet (`cfg(target_os = "macos")` met
   `compile_error!` elders); minimum macOS blijft 12.
9. De hold-duur is een setting (1,0–4,0 s, default 2,0) en de ring volgt de
   ingestelde duur.
10. Spiegelweergave is per camera instelbaar (default niet gespiegeld);
    spiegelen verandert alleen de weergave, niet de snapshot of de
    handdetectie.
