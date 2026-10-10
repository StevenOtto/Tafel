# UI-feedback na de eerste sessies-build

`2026-09-14` · Steven Otto · branch `feature/sessions` · vervolg op `2026-09-14-sessies.md`

## Waarom

De eerste build met sessies werkt; in gebruik vallen vijf dingen op. Een
misser in een sessie kun je niet weghalen, dus blijft hij mee naar de agent
gaan. Het zijpaneel toont technische informatie (endpoint, opslag, netwerk-URL)
op de plek waar rust hoort. Een gebaar testen zonder camera kan niet. De
gebarenring springt en vult in harde stappen. En de leveringstegel verspilt
zijn ruimte aan de cameranaam.

## Scope

- **Wel:** één levering uit een sessie verwijderen (bestanden weg); agent-
  verbinding en netwerkcamera-veld naar _Developer_; gebaren in de gids
  aanklikbaar; vloeiende ring; tegel zonder cameranaam, cameranaam in de
  vergroting.
- **Niet:** leveringen verplaatsen tussen sessies; ongedaan maken van een
  verwijdering; de agent laten verwijderen.

## Acceptatiecriteria

1. In de vergroting van een levering staat _Remove_; na bevestiging zijn de
   JSON en JPEG's van die levering van schijf, de tegel is weg, de teller van
   de sessie klopt en `get_delivery(seq)` meldt dat er niets is (test op de
   store).
2. Het zijpaneel toont standaard alleen _Gestures_ en _Developer_; endpoint,
   sessie-opslag en het netwerkcamera-veld staan onder _Developer_. Een
   netwerkcamera toevoegen werkt daar zoals voorheen in de kop.
3. Elke rij in de gebarengids is een knop: duim omhoog stuurt een snapshot van
   de eerste kijkende camera, twee duimen alle camera's, ok-teken opent/sluit
   de verzameling, open hand pauzeert alles, vuist hervat. Zonder camera's zijn
   de knoppen uitgeschakeld.
4. De ring vult zonder zichtbare stappen (voortgang loopt lineair tussen twee
   detecties) en de ring volgt de hand zonder te springen (positie en straal
   gedempt).
5. De leveringstegel toont volgnummer, soort en aantal afbeeldingen, geen
   cameranaam; de vergroting noemt de camera('s) in de balk en per afbeelding.
