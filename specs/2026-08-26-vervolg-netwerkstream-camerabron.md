# Vervolg: netwerkstream als camerabron

`2026-08-26` · Steven Otto · vervolg op `2026-08-26-meerdere-cameras.md` — bouwen ná de vier basis-specs

## Waarom

Continuity Camera ondersteunt alleen iPhone; een iPad of tweede telefoon kan
in v1 niet meedoen. Een MJPEG-stream over het lokale netwerk (talloze gratis
IP-camera-apps op iOS/Android) maakt elk apparaat een Tafel-camera zonder
extra software op de Mac en zonder afhankelijkheid van één leverancier.

## Scope

- **Wel:** camerabron van type `network` met een URL (`http://…`, MJPEG
  multipart); zelfde pipeline (gebaren, snapshots, status) als een lokale
  camera; verbindingsstatus en herverbinden; URL in de cameraset.
- **Niet:** RTSP/H.264 (vereist decoder-dependency), authenticatie anders dan
  in de URL, discovery (mDNS), eigen companion-app.

## Acceptatiecriteria

1. "Camera toevoegen" biedt naast systeemcamera's een URL-veld; een geldige
   MJPEG-URL verschijnt binnen 3 s als live tile met naam en status.
2. Gebaren en snapshots werken identiek aan een lokale camera (gedeelde
   tests draaien tegen beide brontypen via de `FrameSource`-seam).
3. Valt de stream weg, dan toont de tile `disconnected` en probeert de app
   elke 5 s opnieuw; terugkomst hervat zonder handeling.
4. Alleen URL's op loopback, RFC 1918-adressen of `.local`-hosts worden
   geaccepteerd; andere hosts worden geweigerd met uitleg (de app hoort nooit
   het internet op te gaan).
5. De URL wordt in de cameraset bewaard; eventuele credentials in de URL
   worden in de UI gemaskeerd getoond.
