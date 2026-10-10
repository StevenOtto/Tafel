# Meerdere camera's

`2026-08-26` · Steven Otto · branch `feature/gesture-capture`

## Waarom

Eén overhead camera dekt het bureau; ideeën leven ook op de muur of het
whiteboard. Met twee of meer gezichtspunten kan de agent schets én context in
één levering krijgen ("de schets op tafel is versie 2 van het schema op de
muur"). Elke camera heeft zijn eigen gebarenpipeline, zodat je vanaf de muur
kunt triggeren zonder terug te lopen naar het bureau.

## Scope

- **Wel:** N gelijktijdige camerastreams, elk met eigen gebaardetectie,
  status en voortgangsring; overzichts-UI (grid met naam en status);
  koppelen, ontkoppelen, hernoemen; hot-plug en automatisch herverbinden;
  persistentie van de cameraset. Alles wat macOS zelf als videodevice ziet:
  ingebouwd, USB, iPhone via Continuity Camera.
- **Niet:** een eigen iOS/iPadOS-companion-app; netwerkstreams (RTSP/MJPEG)
  — vervolg-spec `2026-08-26-vervolg-netwerkstream-camerabron.md`, en dat is
  ook de route voor een iPad of tweede telefoon (Continuity Camera werkt alleen
  met iPhone); virtuele-camera-apps van derden worden niet ondersteund of
  getest.

## Domeinmodel

- **Camera** — een videodevice met een gebruikersnaam ("Bureau", "Muur"),
  `deviceId` + systeemlabel (fallback als het id na herstart niet meer klopt),
  status `watching` | `paused` | `disconnected`, spiegel-instelling.
- **Cameraset** — geordende lijst gekoppelde camera's; de volgorde bepaalt de
  volgorde in het overzicht en in leveringen. Vervangt `cameraDeviceId`.

## Acceptatiecriteria

1. Twee camera's staan tegelijk live in het overzicht met elk een eigen ring;
   een hold op camera A verandert de hold-state van camera B niet (test op de
   pipeline zonder camera's: twee onafhankelijke instanties).
2. De cameraset (ids, namen, volgorde, spiegeling) overleeft een herstart
   (test analoog aan `camera_selection_survives_restart`).
3. Een iPhone via Continuity Camera verschijnt in de kieslijst en kan worden
   gekoppeld. Valt hij weg, dan toont zijn tegel `disconnected`, de andere
   camera's lopen door en er is geen foutdialoog; komt hij terug, dan hervat
   de stream automatisch binnen 5 s (`devicechange`).
4. Nieuw aangesloten devices verschijnen zonder herstart in de kieslijst.
5. `two-thumbs-up` op één camera levert een snapshot van **alle** camera's in
   status `watching`; gepauzeerde of losgekoppelde camera's worden overgeslagen
   en bij naam genoemd in de levering (→ leveringen-spec).
6. De cameranaam reist mee in elke snapshot.
7. Toegankelijkheid (WCAG 2.2 AA): tegels vormen een benoemde lijst; status is
   tekst, niet alleen kleur; koppelen/ontkoppelen/hernoemen volledig met het
   toetsenbord; hernoemen is een gelabeld tekstveld; focus blijft na
   verwijderen op een zinnige plek.
8. Het instrument panel toont per camera analyse-fps en latency; met twee
   camera's op ~10 fps analyse blijft de UI vloeiend (geen zichtbare
   ring-hik; gemeten en getoond).
9. Het overzicht schaalt tot 4 camera's zonder horizontaal scrollen op een
   1440-px-venster; meer is niet gegarandeerd en wordt gemeld.
10. Dedupe: vuren twee camera's binnen 1 s dezelfde actie (zelfde gebaar), dan
    telt alleen de eerste; de tweede ring toont "al uitgevoerd" in plaats van
    een bevestiging. Na elke gevuurde actie geldt per camera een cooldown van
    1 s waarin geen nieuwe hold start. Getest zonder camera's.
11. README documenteert de Continuity Camera-voorwaarden (draadloos: iPhone
    vergrendeld, stilliggend, landscape; via USB niet) en verwijst voor tablets
    naar de netwerkstream-vervolgspec.
