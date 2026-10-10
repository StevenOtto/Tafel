# Nieuwe sessie starten vanuit de app

Datum: 2026-09-21

## Aanleiding

Sessies (zie `2026-09-14-sessies.md`) ontstaan nu op twee manieren: de
eerste levering opent een voorlopige sessie, en de agent benoemt werk met
`start_session(naam)`. Het venster kan sessies wisselen, hernoemen en
verwijderen, maar er is geen knop om zelf een nieuwe sessie te beginnen.
Wie aan een nieuw onderwerp begint terwijl een benoemde sessie huidig is,
moet dus wachten op de agent of eerst de oude sessie verwijderen.

## Voorstel

- In de sessiebalk komt naast de kiezer een knop **New**. Die opent het
  bestaande inline naamveld; **Start** opent een lege, benoemde sessie en
  maakt die huidig. De vorige sessie blijft bestaan en is via de kiezer terug
  te vinden.
- Een naam die al bestaat wordt geweigerd met dezelfde melding als bij
  hernoemen. Wie die sessie terug wil, kiest haar in de kiezer.
- Een voorlopige huidige sessie blijft ongemoeid: haar leveringen verhuizen
  niet mee. "New" betekent altijd leeg beginnen; benoemen van lopend werk
  blijft **Name**.
- De sessiebalk is altijd zichtbaar, ook zonder sessies, anders is er bij een
  schone start geen plek voor de knop. De lege-toestand-tekst noemt de knop.
- Het venster mag dit; de agent niet. `start_session` blijft de enige
  muterende MCP-tool en houdt zijn drie uitkomsten.

## Acceptatiecriteria

1. Benoemde sessie "Kiosk" is huidig met leveringen → New, "Menu", Start →
   Menu is huidig en leeg; Kiosk staat nog in de kiezer met haar leveringen.
2. Voorlopige sessie met 2 leveringen is huidig → New, "Menu", Start → Menu
   is huidig en leeg; de voorlopige sessie bestaat nog met 2 leveringen.
3. New, "Kiosk" terwijl Kiosk bestaat → foutmelding, huidige sessie wijzigt
   niet.
4. Lege naam → Start is uitgeschakeld.
5. Geen sessies → de sessiebalk staat er met de knop New.
6. Na Start meldt de statusregel de wijziging zoals bij andere app-acties.
