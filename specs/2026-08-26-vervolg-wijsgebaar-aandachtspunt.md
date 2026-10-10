# Vervolg: wijsgebaar als aandachtspunt

`2026-08-26` · Steven Otto · vervolg op `2026-08-26-handgebaren-apple-vision.md` — bouwen ná de vier basis-specs

## Waarom

Een snapshot van een vol whiteboard laat de agent raden waar het om gaat. Met
een wijsvinger die 2 s op een plek blijft, markeer je een aandachtspunt dat als
coördinaat en uitsnede meereist met de volgende snapshot van die camera: "dit
stuk bedoel ik". Geen extra hardware, zelfde hold-mechaniek als de andere
gebaren.

## Scope

- **Wel:** gebaar `pointing` (alleen wijsvinger gestrekt); hold markeert een
  punt (vingertop, in frame-fracties) op die camera; markering blijft tot de
  volgende snapshot van die camera of tot ze verloopt; snapshot bevat punt +
  uitsnede rond het punt; kaart toont de markering.
- **Niet:** meerdere punten per snapshot, tekenen/slepen van gebieden, wijzen
  als trigger voor een snapshot (dat blijft de duim).

## Acceptatiecriteria

1. Classifier herkent `pointing` op een synthetische landmark-set en geeft
   `none` bij wijsvinger + duim gestrekt (om `thumbs-up`/`ok` niet te raken).
2. Hold van 2 s → markering op de vingertop-positie; UI toont een blijvende
   marker met label "Aandachtspunt" op die cameratile; `aria-live` meldt het.
3. De eerstvolgende snapshot van die camera bevat `focus: { x, y }` en een
   extra image-block met een uitsnede (25 % van de framebreedte rond het
   punt); daarna is de markering weg.
4. Zonder snapshot verloopt de markering na 60 s (zichtbaar aflopend) en wordt
   niets meegestuurd.
5. De kaart tekent de marker op de thumbnail en toont de uitsnede ernaast.
