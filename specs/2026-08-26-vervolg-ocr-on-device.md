# Vervolg: on-device OCR bij snapshots

`2026-08-26` · Steven Otto · vervolg op `2026-08-26-leveringen-en-mcp-kaart.md` — bouwen ná de vier basis-specs

## Waarom

Whiteboards en marges staan vol handschrift. Vision zit met de gebarenspec al
aan boord; `VNRecognizeTextRequest` herkent dat handschrift on-device, offline,
in tientallen milliseconden. Tekst naast de afbeelding maakt leveringen
doorzoekbaar in de historie en geeft de agent een tweede lezing van
onduidelijk handschrift — zonder extra dienst of kosten.

## Scope

- **Wel:** OCR per snapshot in de Swift-brug; herkende regels met positie en
  betrouwbaarheid in `structuredContent` en als tekstblok in het toolresultaat;
  tonen in de leveringshistorie; aan/uit-instelling.
- **Niet:** taalkeuze-UI (systeemtalen volstaan), correctie of interpretatie van
  de tekst, OCR op livebeeld.

## Acceptatiecriteria

1. Elke snapshot in een levering bevat `text: [{ line, confidence, box }]`;
   het toolresultaat bevat na de afbeeldingen één tekstblok "Herkende tekst
   (<camera>)" met de regels op leesvolgorde.
2. Een fixture-JPEG met drie duidelijk geschreven woorden levert die drie
   woorden (case-insensitief) als regels; een leeg vel levert `[]`.
3. OCR verlengt de tijd tot levering met ≤ 200 ms per snapshot (gemeten in het
   instrument panel); bij overschrijding wordt de levering zonder tekst
   gepubliceerd en dat gemeld.
4. Setting `ocr: true|false` (default aan); uit → geen `text`-veld, geen
   tekstblok.
5. De leveringshistorie in de app toont de herkende tekst naast de thumbnail en
   is erop doorzoekbaar.
