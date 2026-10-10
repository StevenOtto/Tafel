# Vuist pauzeert, open hand hervat

Datum: 2026-09-21

## Aanleiding

Sinds `2026-08-26-handgebaren-apple-vision.md` pauzeert een open hand een
camera en hervat een vuist. In gebruik voelt dat omgekeerd: een vuist leest
als "stop", een open hand als "ga door" — en een open hand is bovendien het
gebaar dat je vanzelf maakt als je weer aan het werk gaat.

## Wijziging

- `fist` → camera `paused`; `open-palm` → camera `watching`. In `paused`
  reageert alleen `open-palm`.
- De gebarengids, de statusregel, de tegeltekst en de README volgen mee.
  Herkenning, hold-tijden en de globale pauzeknop veranderen niet.

## Acceptatiecriteria

1. Vuist bij een kijkende camera → `paused`; open hand doet dan niets.
2. Open hand bij een gepauzeerde camera → `watching`; elk ander gebaar wordt
   niet eens getoond.
3. Klik op de gidsrij Vuist → alles gepauzeerd; klik op Open hand → hervat.
