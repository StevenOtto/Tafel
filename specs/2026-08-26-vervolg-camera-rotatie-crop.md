# Vervolg: per-camera rotatie en uitsnede

`2026-08-26` · Steven Otto · vervolg op `2026-08-26-meerdere-cameras.md` — bouwen ná de vier basis-specs

## Waarom

Een muurcamera hangt vaak gekanteld of op zijn kant, een overhead camera ziet
meer bureau dan papier. Zonder correctie krijgt de agent gedraaide beelden met
irrelevante randen, en OCR en handdetectie presteren slechter. Rotatie en een
vaste uitsnede per camera lossen dat één keer op bij het inrichten.

## Scope

- **Wel:** per camera rotatie in stappen van 90° en een rechthoekige uitsnede
  (frame-fracties), ingesteld via een overlay op het livebeeld; toegepast op
  weergave, handdetectie én snapshot; persistent in de cameraset.
- **Niet:** vrije hoeken, perspectiefcorrectie, automatische
  papierdetectie/fiducials.

## Acceptatiecriteria

1. Rotatie 0/90/180/270 per camera; de tile, de ring en de snapshot staan alle
   drie rechtop na instellen.
2. Uitsnede via slepen van hoeken op het livebeeld én via numerieke velden
   (toetsenbord); minimale uitsnede 25 % van breedte en hoogte.
3. Handdetectie werkt op het volledige frame (een hand buiten de uitsnede
   telt), de snapshot bevat alleen de uitsnede; de kaart meldt "uitsnede" bij
   de afbeelding.
4. Instellingen overleven herstart en volgen de camera bij hernoemen en
   herverbinden.
5. Resetknop per camera zet rotatie en uitsnede terug naar volledig frame.
