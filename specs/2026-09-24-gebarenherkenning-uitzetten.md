# Gebarenherkenning aan/uit

Datum: 2026-09-24

## Aanleiding

Soms wil je aan je bureau werken zonder dat een hand in beeld een gebaar
kan afvuren, terwijl de camera's wel blijven kijken. Nu kan dat alleen door
te pauzeren, en dan ziet de agent het bureau ook niet meer.

## Wijziging

- Nieuwe instelling `gesturesEnabled` (standaard `true`), bovenaan in het
  instellingenmenu als schakelaar "Gesture recognition". Blijft bewaard over
  herstarts; een ouder instellingenbestand laadt met `true`.
- Uit: de tegels vragen Apple Vision niet meer om handen. Er is geen ring,
  geen handoverlay en geen gebaar dat afvuurt; een lopende hold vervalt.
- Uit verandert niets aan kijken en pauzeren: een kijkende camera blijft
  `get_desk` voeden, Pauze/Resume en _Pause all_ werken zoals nu.
- De gidsrijen blijven klikbaar en doen wat het gebaar zou doen. De hint
  onder de gids zegt dat herkenning uit staat en dat een klik op een rij
  hetzelfde doet. De pauzetekst op een tegel verwijst dan naar Resume in
  plaats van naar een open hand.

## Buiten scope

Per-camera aan/uit; per-gebaar aan/uit.

## Acceptatiecriteria

1. Schakelaar uit → geen `detect_hands`-aanroepen; hand in beeld doet niets.
2. `get_desk` levert bij uit nog steeds verse beelden van kijkende camera's.
3. Klik op een gidsrij bij uit → dezelfde actie als het gebaar.
4. Herstart behoudt de stand; een bestand zonder het veld laadt als aan.
