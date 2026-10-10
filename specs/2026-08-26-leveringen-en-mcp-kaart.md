# Leveringen, verzamelingen en de MCP App-kaart

`2026-08-26` · Steven Otto · branch `feature/gesture-capture`

## Waarom

De agent krijgt nu één "settled sketch" met twee vaste zone-crops. Met gebaren en
meerdere camera's wordt de eenheid van invoer een **levering**: één of meer
afbeeldingen, van één of meer camera's, eventueel in een reeks verzameld. De
gebruiker moet in Claude ondubbelzinnig zien wát er is meegestuurd — dat is
vertrouwen én privacy, want de camera kijkt op je bureau en je muur.

## Scope

- **Wel:** leveringsmodel op de bus; vervanging van de MCP-tools; gebaren →
  leveringen (enkel, alle camera's, verzameling); hand-vrije snapshot;
  leveringshistorie in de app; MCP App-kaart die elke meegestuurde afbeelding
  toont.
- **Niet:** vaste instructiezone/sketchzone-split (verdwijnt: een muurcamera
  heeft geen marge; een snapshot is het hele frame); persistentie van
  leveringen over herstarts; push naar andere apps dan MCP-hosts.

## Domeinmodel

- **Snapshot** — JPEG (≤ 1280 px breed) + cameranaam + tijdstip + `handFree`.
- **Levering** — `seq`, `kind` (`single` | `all-cameras` | `collection`),
  snapshots in cameravolgorde, tijdstip, trigger-gebaar, overgeslagen camera's.
- **Verzameling** — één globaal open venster (over alle camera's): geopend met
  `ok-sign`, gevuld door `thumbs-up` (die camera) of `two-thumbs-up` (alle
  camera's), gesloten met `ok-sign` → één levering `collection`. Zolang een
  verzameling open is, publiceren duimen géén losse levering. Maximaal 12
  snapshots; verloopt na 10 minuten zonder toevoeging.
- **Hand-vrije snapshot** — het meest recente frame (≤ 3 s oud) van die camera
  waarin geen hand werd gezien; anders het huidige frame. Zo staat je eigen
  hand niet over de schets.

MCP-tools (alle read-only): `get_desk` (huidig frame per `watching` camera;
gepauzeerde camera's leveren niets en worden als gepauzeerd genoemd),
`get_latest_delivery`, `wait_for_delivery(timeout_seconds, after_seq)` (zelfde
semantiek en 180 s-plafond als `wait_for_sketch` nu), `list_deliveries`
(metadata van deze sessie), `get_delivery(seq)`.

## Acceptatiecriteria

1. `thumbs-up` op camera "Bureau" → binnen 500 ms staat een levering
   `single` met precies 1 snapshot van "Bureau" op de bus; een lopende
   `wait_for_delivery` keert ermee terug; het resultaat bevat 1 image-block,
   tekst met cameranaam en tijd, en `structuredContent` met per afbeelding
   camera, tijdstip en `handFree`.
2. `two-thumbs-up` → levering `all-cameras` met één snapshot per `watching`
   camera in cameravolgorde; overgeslagen camera's worden in de tekst genoemd.
3. `ok-sign` opent een verzameling (UI: "Verzameling open · 0"); duimen voegen
   toe zonder te publiceren (teller loopt op, `wait_for_delivery` blijft
   wachten); tweede `ok-sign` publiceert één levering `collection` met alle
   snapshots op volgorde. Een lege verzameling sluiten publiceert niets en
   meldt dat in de UI.
4. Een open verzameling is annuleerbaar via een knop in de UI; geannuleerd =
   niets gepubliceerd. Bij de 12e snapshot meldt de UI "vol" en worden verdere
   duimen genegeerd (ring toont "vol", geen bevestiging). Na 10 minuten zonder
   toevoeging verloopt de verzameling: niets gepubliceerd, UI meldt het; de
   resterende tijd is zichtbaar zolang de verzameling open is.
5. Hand-vrije snapshot: in een testsequentie met een hand die 1 s vóór de
   trigger het beeld inkomt, is de gebruikte snapshot het laatste hand-vrije
   frame en `handFree: true`; zonder zo'n frame binnen 3 s: huidig frame,
   `handFree: false`.
6. De MCP App-kaart toont bij elke levering: soort en aantal ("3 afbeeldingen
   uit 2 camera's" / "Verzameling van 5"), per afbeelding een thumbnail met
   cameranaam en tijd, overgeslagen camera's, en bij een verzameling de
   volgorde. Bij `get_desk`: huidig frame per camera met status. Live
   geverifieerd in Claude Desktop, zoals op 2026-08-18.
7. De app toont de leveringshistorie van de sessie (lijst met thumbnails,
   soort, tijd, camera's); een item openen toont alle afbeeldingen groot.
8. `get_latest_sketch`, `wait_for_sketch`, `report_sketch` en de zone-crops
   bestaan niet meer; `instructions` van de server beschrijven de nieuwe
   tools en gebaren.
9. Nieuwe leveringen sturen `notifications/resources/list_changed` zodat hosts
   die dat ondersteunen de kaart kunnen verversen; hosts zonder blijven werken.
10. Bus- en resultaattests: één test per acceptatiecriterium 1–5 en 8, zonder
    camera of Vision.
11. Een `collection`-resultaat met meer dan 6 afbeeldingen gebruikt een lagere
    JPEG-kwaliteit (0,6 i.p.v. 0,75) zodat het totale resultaat onder
    hostlimieten blijft; de kaart toont dezelfde afbeeldingen.
