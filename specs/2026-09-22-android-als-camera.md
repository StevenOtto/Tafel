# Android-telefoon als Tafel-camera

`2026-09-22` · Steven Otto · branch `claude/android-device-support-cft7yu` ·
vervolg op `2026-08-26-vervolg-netwerkstream-camerabron.md`

## Waarom

Een Android-telefoon is de goedkoopste tweede camera die collega's al op zak
hebben, en het statief-op-de-muur-scenario vraagt er vaak om. Op papier kan
dat al: een IP-camera-app op de telefoon plus een URL onder _Developer →
Network camera_. In de praktijk werkt alleen de snapshot-URL. De MJPEG-URL,
de standaard-URL van elke Android-IP-camera-app, faalt altijd.

De oorzaak zit in `netcam.rs`: `fetch_frame` doet per frame een nieuwe GET en
leest met `response.bytes()` de hele body. Een MJPEG-stream eindigt nooit,
dus de lees loopt tegen de totale timeout van 4 s en geeft
`error decoding response body`. Nagebootst op 2026-09-22 met een lokale
multipart-server (20 kB-frames op 10 fps) en de ongewijzigde `netcam.rs`:
fout na 4,0 s, geen frame. De tile staat daardoor op `Disconnected` en
probeert elke 5 s opnieuw, zonder resultaat. README en DESIGN.md beloven
MJPEG; dat klopt nu niet.

Ook de snapshot-route is zwak: één HTTP-request per frame, elke 100 ms na
het vorige antwoord, dus de framerate is `1 / (100 ms + RTT)`. Op wifi met
50–150 ms per request is dat 4–7 fps, en de telefoon codeert elke keer een
losse JPEG.

## Drie routes, één aanbevolen

**A. Android als camera van de Mac (aanbevolen).** De telefoon levert beeld;
handdetectie, de MCP-server en de sessies blijven op de Mac. Past in de
bestaande architectuur en het vertrouwensmodel (alleen een
loopback-listener, alleen uitgaand naar het lokale netwerk).

**B. Android via USB als webcam.** Android 14 kan zich via USB als
UVC-webcam aanmelden (instelling _USB → Webcam_); macOS ziet dan een gewone
USB-camera, dus `getUserMedia` en het _Add camera_-menu werken zonder code.
Niet elk toestel biedt dit: het is bevestigd op Pixel, voor andere merken
moet het per toestel worden nagegaan (onzeker, niet zelf getest). Alleen
documentatie, geen code.

**C. Tafel zelf op Android draaien (afgeraden).** Tauri 2 bouwt voor
Android, maar drie dingen breken. Handdetectie is Apple Vision via Swift
(`lib.rs` weigert andere targets met `compile_error!`); op Android wordt dat
MediaPipe Hand Landmarker, met ook 21 punten maar andere namen en een andere
foutmarge, dus nieuwe fixtures. De MCP-server luistert op `127.0.0.1` van de
telefoon, waar Claude Desktop op de Mac niet bij kan; openstellen op het LAN
betekent een endpoint zonder authenticatie op het netwerk. De
Claude-app op Android kan geen lokale MCP-server aanspreken. Het resultaat
is een tweede product met een ander beveiligingsmodel, voor een gebruiker
die toch al achter een Mac zit.

Verworpen binnen A: een door Tafel geserveerde webpagina die de telefoon in
Chrome opent en frames terugstuurt. `getUserMedia` vereist een veilige
context; over `http://` op het LAN weigert Chrome de camera, dus dat vraagt
een zelfondertekend certificaat én een LAN-listener in Tafel. Beide botsen
met het huidige model.

## Scope

- **Wel:** MJPEG echt ondersteunen met één blijvende verbinding per
  netwerkcamera; framerate los van de RTT; uitleg in de app voor Android
  (welke URL, welke app-instellingen, USB-webcam); README en DESIGN.md
  kloppend maken.
- **Niet:** route C; een eigen Android-app; RTSP/H.264; discovery via mDNS;
  authenticatie anders dan in de URL (ongewijzigd uit de netwerkstream-spec).

## Ontwerp

`netcam.rs` krijgt een lezer per netwerkcamera: een tokio-taak die één GET
openhoudt, de body met `response.chunk()` in een begrensde buffer leest,
elke complete JPEG (FFD8…FFD9) eruit haalt en alleen de nieuwste bewaart
(`Arc<Mutex<Option<Frame>>>` met tijdstip en volgnummer). Geen totale
timeout meer maar een connect-timeout (4 s) en een idle-timeout (geen nieuwe
frame binnen 4 s = verbroken). Bij een fout wacht de lezer 5 s en verbindt
opnieuw. Is het antwoord geen multipart, dan valt de lezer terug op de
huidige snapshot-polling met de bestaande limiet van 8 MB per frame.

De hostcontrole (`host_is_local`) blijft vóór elke verbinding gelden,
inclusief elke herverbinding. Redirects gaan uit (`Policy::none()`): nu volgt
reqwest standaard tot tien redirects zonder de nieuwe host te controleren.
Nagebootst op 2026-09-23 met de ongewijzigde `netcam.rs`: een lokale URL die
met 302 doorverwijst naar een host die `host_is_local` weigert, levert
gewoon het frame van die host. Een lokale camera (of wie hem overneemt) kan
de app zo naar het internet sturen.

De webview houdt zijn pollende lus, maar `fetch_network_frame` wordt
`latest_network_frame(url)`: een geheugenlees zonder netwerk die ook het
volgnummer teruggeeft, zodat de tile een frame dat hij al tekende niet
opnieuw decodeert. De lus pollt elke 50 ms in plaats van 100 ms, anders
mist een lus van 10 Hz frames van een bron van 10 fps. Lezers starten bij
de eerste vraag naar een URL en stoppen als een URL 10 s niet gevraagd is
(camera verwijderd of tile weg).

`Info.plist` krijgt `NSLocalNetworkUsageDescription`. Sinds macOS 15 vraagt
macOS toestemming voordat een app het lokale netwerk op mag; de sleutel
bepaalt de tekst van die vraag. Of de vraag zonder sleutel verschijnt of de
verbinding stil faalt, is niet getest.

## Commando's

```sh
npm install
npm test                      # vitest, TypeScript-kern
npx tsc --noEmit              # typecheck webview (npm run build doet tsc + vite)
(cd src-tauri && cargo test)  # Rust, alleen op macOS
npm run tauri dev             # app draaien, alleen op macOS
```

`src-tauri` compileert alleen op macOS (`compile_error!` in `lib.rs`, Swift-brug
voor Vision). Buiten macOS draaien de tests van `netcam.rs` in een losse
testcrate die het bestand via `#[path]` insluit; daarvoor moet `netcam.rs`
vrij blijven van `tauri`- en `swift-rs`-imports.

## Projectstructuur

- `src-tauri/src/netcam.rs`: hostcontrole, multipart-parser, lezers en
  register. Alle netwerkcode van de app staat hier.
- `src-tauri/src/lib.rs`: alleen de Tauri-command `latest_network_frame` en
  het register in `AppState`.
- `src/app/tauri.ts`: getypte wrapper van de command.
- `src/app/CameraTile.tsx`: de pollende lus van de tile.
- `src/app/ControlApp.tsx`: uitleg in het paneel _Network camera_.
- `src-tauri/Info.plist`, `README.md`, `DESIGN.md`.

## Codestijl

Zoals de bestaande code: modulecommentaar met `//!` bovenaan, constanten met
eenheid in de naam, fouten als `Result<_, String>` richting webview,
commentaar alleen voor het niet-voor-de-hand-liggende waarom.

```rust
/// Largest frame we will read, so a hostile or wrong endpoint can't stream
/// unbounded data into memory.
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(4);
```

## Teststrategie

- **Eenheid (Rust):** de multipart-parser als pure functie over chunks die op
  willekeurige plekken zijn geknipt, ook midden in een FFD8- of FFD9-marker;
  de buffergrens.
- **Wire-level (Rust):** een axum-server in de test (axum is al een
  dependency) die multipart streamt met instelbare vertraging, verbindingen
  telt, halverwege afbreekt en doorverwijst. Zelfde patroon als de
  wire-level test in `mcp_server.rs`.
- **TypeScript:** er is geen test voor `CameraTile`; de lus blijft dun en
  wordt gedekt door typecheck en een handmatige controle.
- **Handmatig, op een Mac:** een echte Android-IP-camera-app, de
  USB-webcamroute, en de Local Network-vraag.

## Grenzen

- **Altijd:** `host_is_local` vóór elke verbinding; begrensd geheugen per
  lezer; tests draaien vóór elke commit.
- **Eerst vragen:** een nieuwe dependency; een listener op iets anders dan
  loopback; wijzigingen aan de MCP-server of de bus.
- **Nooit:** redirects volgen naar een niet-gecontroleerde host; frames via de
  webview ophalen (CSP blijft zoals hij is); credentials uit de URL tonen of
  loggen.

## Acceptatiecriteria

1. Een MJPEG-URL van een lokale camera verschijnt binnen 3 s als live tile;
   gebaren en snapshots werken als bij een lokale camera. _Handmatig, met een
   Android-IP-camera-app._
2. Per netwerkcamera staat er één HTTP-verbinding open, niet één per frame
   (test: de testserver telt verbindingen).
3. Bij een bron op 10 fps met 150 ms vertraging per frame levert de lezer
   ≥ 9 van de 10 frames per seconde af (test); de tile pollt elke 50 ms en
   decodeert alleen nieuwe volgnummers.
4. Valt de stream weg, dan geeft `latest_network_frame` binnen 5 s een fout
   en toont de tile `Disconnected`; komt de stream terug, dan hervat hij
   binnen 10 s zonder handeling (test op lezerniveau; tile handmatig).
5. Een frame groter dan 8 MB of een buffer zonder JPEG-grens na 8 MB sluit
   de verbinding met een foutmelding; het geheugen van een lezer blijft
   begrensd (test).
6. Een redirect wordt niet gevolgd; `host_is_local` wordt bij elke
   (her)verbinding gecontroleerd (test).
7. Een snapshot-URL (`image/jpeg`) werkt als voorheen (test).
8. Het _Network camera_-paneel noemt voor Android: installeer een
   IP-camera-app, gebruik de MJPEG-URL (voorbeeld
   `http://192.168.1.23:8080/video`), zet resolutie op 1080p, en geef de
   telefoon een vast IP-adres in de router, omdat Android geen
   `.local`-naam adverteert. Het noemt ook de USB-webcamroute (B).
9. `Info.plist` heeft `NSLocalNetworkUsageDescription`.
10. De bestaande Rust- en TS-tests blijven slagen; README en DESIGN.md
    beschrijven de blijvende verbinding en de Android-routes, en de claim
    "MJPEG" klopt weer.

## Open vragen

- Welke Android-IP-camera-app noemen we in de uitleg? Voorkeur: geen merk
  noemen en alleen de URL-vorm, want apps verdwijnen of veranderen van model.
- Werkt een Android-telefoon als UVC-webcam in WKWebView gelijk aan een
  gewone USB-camera? Verwacht ja (macOS ziet UVC als AVFoundation-apparaat),
  maar met de hand te controleren op een Pixel.
