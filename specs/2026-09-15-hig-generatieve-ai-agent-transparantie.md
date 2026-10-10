# HIG generatieve AI: wat de agent doet, zichtbaar in de app

`2026-09-15` · Steven Otto · branch `feature/agent-transparency` · toetsing aan
[Apple HIG · Generative AI](https://developer.apple.com/design/human-interface-guidelines/generative-ai)

## Waarom

Tafel voldoet al aan het meeste uit de richtlijn: alles on-device, alleen een
loopback-endpoint, de agent leest maar mag niet verwijderen, destructieve acties
vragen bevestiging, elk gebaar heeft een knop als niet-AI-alternatief. Wat
ontbreekt is één ding: de app laat niet zien wat de agent dóet. Een verbonden
agent kan elke ~2 s naar het bureau kijken zonder dat er een gebaar aan
voorafgaat, en niets in het venster verraadt dat. Benoemt de agent een sessie,
dan verspringt alleen stil een dropdown. `get_desk` belooft "de bureaucamera"
maar levert het frame van de camera die het laatst rapporteerde. En de
privacy-uitleg staat in de README, niet in de app die collega's nu installeren.
De richtlijn vraagt precies dit: maak zichtbaar waar AI werkt, bevestig wat een
actie deed, en wees eerlijk over wat waar wordt opgeslagen.

## Scope

- **Wel:** elke agent-lees van beeld zichtbaar in de app; benoeming door de
  agent gemeld in de statusregel; `get_desk` per camera met naam; eerlijke
  tekst in de verwijder-dialogen; korte uitleg over wat de agent ziet buiten
  _Developer_; cameratoestemmingstekst die het kijken door de agent noemt.
- **Niet:** authenticatie op het endpoint; tonen hoeveel of welke hosts
  verbonden zijn; een feedbackknop; nieuwe tools of gebaren; gebaren testen op
  meer handen (proces, geen code — fixtures van de eerste collega's).

## Acceptatiecriteria

1. Elke tool die beeld teruggeeft (`get_desk`, `get_latest_delivery`,
   `wait_for_delivery` met resultaat, `get_delivery`) legt een _agent-lees_
   vast op de bus (bureau of levering met `seq`, tijdstip); het venster hoort
   dit binnen 1 s.
2. Het paneel heeft een sectie _Agent_ boven _Developer_ met twee regels:
   wanneer de agent het laatst naar het bureau keek ("12 s ago" / "Not yet")
   en welke levering hij het laatst las; de leeftijd loopt live mee.
3. Een `get_desk` meldt zich bovendien kort in de statusregel
   ("Agent looked at the desk"). Leveringsreads niet: die volgen op een
   gebaar van de gebruiker en de kaart pollt ze.
4. Een `start_session` door de agent meldt uitkomst en naam in de statusregel
   ("The agent named this session 'Kiosk'", "… reopened …", "… started …").
   Wijzigingen vanuit het venster zelf melden niets extra.
5. `get_desk` levert één frame per camera die heeft gerapporteerd, in
   rapportagevolgorde, met per frame cameranaam en leeftijd in tekst én in
   `structuredContent.images[]`; een frame ouder dan 10 s wordt als
   "stopped watching" gemarkeerd; een verwijderde camera verdwijnt uit het
   resultaat.
6. De kaart toont een `get_desk`-resultaat als "Desk right now" met per
   afbeelding de cameranaam en, indien van toepassing, dat de camera pauzeert.
7. De bevestigingen voor _Remove_ (levering) en _Delete_ (sessie) zeggen dat
   wat de agent al ontving in dat gesprek blijft.
8. De sectie _Agent_ legt in maximaal drie zinnen uit: een verbonden agent kan
   naar het bureau kijken zolang een camera kijkt en ontvangt wat je stuurt;
   pauzeren stopt beide; verzonden snapshots blijven op deze Mac tot je ze
   verwijdert.
9. `NSCameraUsageDescription` noemt dat de verbonden agent naar het bureau kan
   kijken en de gestuurde snapshots ontvangt.
10. Criteria 1, 4 en 5 hebben een test zonder camera of Vision (bus en
    `desk_result`); de bestaande Rust- en TS-tests blijven slagen; README en
    DESIGN.md beschrijven `get_desk` per camera en de agent-sectie.
