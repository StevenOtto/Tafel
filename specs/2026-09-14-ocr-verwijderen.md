# On-device OCR verwijderen

`2026-09-14` · Steven Otto · branch `feature/sessions` · draait `2026-08-26-vervolg-ocr-on-device.md` terug

## Waarom

De herkende tekst is in de praktijk rommel: Vision leest gedrukte titels
redelijk maar handschrift — het meeste op een bureau — slecht. De agent leest
de afbeelding zelf, en beter. De tekst vervuilde de MCP-kaart en het
toolresultaat, en de enige eigen waarde (lokaal zoeken in de historie) is
zwak op zulke tekst. Een half-vertrouwd pad achter een schakelaar laten staan
is dood gewicht: Swift-brug, tijdbudget, instelling, veld op elke snapshot,
tests.

## Scope

- **Wel:** de hele OCR-pijplijn weg — Swift `tafelRecognizeText`,
  `vision::recognize_text`, het `text`-veld op snapshots en leveringen, de
  instelling, het zoekveld, de tekst in de kaart en in toolresultaten, de
  `words.png`-fixture.
- **Niet:** opgeslagen leveringen migreren (het oude `text`-veld wordt
  genegeerd); een alternatieve zoekindex (zie `FUTURE.md`).

## Acceptatiecriteria

1. Geen enkele verwijzing naar OCR, `recognize_text`, `TextLine` of het
   `text`-veld in `src/`, `src-tauri/src/`, de Swift-brug en de kaart.
2. Toolresultaten bevatten alleen de lead-tekst en de afbeeldingen;
   `structuredContent.images[]` heeft geen `text`.
3. Bestaande sessies op schijf (met `text` in de leveringsmetadata) laden
   zonder fout; een oude `settings.json` met `ocr` laadt zonder fout.
4. Bestaande TS- en Rust-tests slagen; de twee OCR-tests zijn weg.
