# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Bibliotheek

## Top bar
library-title = Bibliotheek
library-locations-button = Locaties

## Locations sidebar
library-locations = Locaties
library-all = Alle locaties
library-add = Locatie toevoegen…
library-remove = { $location } uit de bibliotheek verwijderen
library-offline = Offline
library-location-pending = { $count } te indexeren
library-a11y-location = { $location }, { $count ->
    [one] { $count } afbeelding
   *[other] { $count } afbeeldingen
}
library-a11y-location-offline = { $location }, Offline, { $count ->
    [one] { $count } afbeelding
   *[other] { $count } afbeeldingen
}

## Indexing progress
library-walking = Afbeeldingen gevonden: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Afbeeldingen gevonden: { $count }
library-reading = Indexeren… { $done } van { $total }
library-held = Indexeren pauzeert terwijl u bladert
library-up-to-date = Bijgewerkt
# machine-drafted, needs native review
library-stopped = Indexeren gestopt
library-stopped-pending = Indexeren gestopt · { $count } nog te indexeren
library-resume = Hervatten
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indexeren gestopt: { $reason }
library-reason-stalled = er bewoog niets gedurende { $minutes ->
    [one] een minuut
   *[other] { $minutes } minuten
}. Een bestand of schijf reageert misschien niet.
library-retry = Opnieuw proberen
library-retry-description = Start het indexeren opnieuw. Wat al is geïndexeerd, blijft doorzoekbaar.
library-save-diagnostics = Diagnosepakket opslaan…
library-stop-description = Stopt het indexeren. Wat al is geïndexeerd, blijft doorzoekbaar; Hervatten of Opnieuw scannen (F5) gaat verder.
library-resume-description = Doorzoekt alle locaties opnieuw en indexeert wat nog over is.
library-opening = Bibliotheek openen…
library-searching = Zoeken…

## Empty and error states
library-empty = Nog geen locaties. Voeg een map toe: de afbeeldingen erin en in de submappen verschijnen hier in één raster.
library-empty-offline = Deze locaties zijn offline. Hun afbeeldingen verschijnen weer zodra ze terug zijn.
library-indexing = Afbeeldingen zoeken…
# machine-drafted, needs native review
library-remove-confirm = ‘{ $location }’ uit de bibliotheek verwijderen?
library-remove-confirm-detail = Je foto’s blijven op de schijf staan, met hun beoordelingen, labels en trefwoorden. De bibliotheek vergeet wat ze daar heeft geïndexeerd; voeg je de map opnieuw toe, dan wordt die opnieuw geïndexeerd.
library-remove-confirm-ok = Verwijderen
library-indexing-in = Afbeeldingen zoeken in { $location }… tot nu toe { $count } gevonden
# machine-drafted, needs native review
library-empty-stopped = Het indexeren is gestopt voordat deze afbeeldingen waren gelezen. Hervatten gaat daar verder.
library-unavailable = De bibliotheek kon niet worden geopend: { $reason }
library-read-only = Deze bibliotheek is geschreven door een nieuwere Specular: u kunt erin zoeken, maar niet bijwerken.

## Screen readers
library-a11y-list = Bibliotheek, { $count ->
    [one] { $count } afbeelding
   *[other] { $count } afbeeldingen
}
library-a11y-added = { $location } toegevoegd aan de bibliotheek
library-a11y-removed = { $location } verwijderd uit de bibliotheek

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Alles in ‘{ $folder }’ toevoegen?
library-home-confirm-detail = Er staan programmabestanden, downloads en caches in, niet alleen foto’s. Indexeren duurt langer en de bibliotheek vult zich met pictogrammen en andere afbeeldingen.
library-home-confirm-pictures = Je map Afbeeldingen bevat alleen je afbeeldingen.
library-home-confirm-use-pictures = Afbeeldingen toevoegen
library-home-confirm-keep = Persoonlijke map toevoegen

## Folder watches
# machine-drafted, needs native review
library-unwatched = Sommige mappen worden niet bewaakt
library-unwatched-detail = { $minutes ->
    [one] Wijzigingen verschijnen binnen een minuut
   *[other] Wijzigingen verschijnen binnen { $minutes } min
}
library-unwatched-description = Specular heeft geen mapbewakingen meer over. Opent de help over het verhogen van de limiet.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Bibliotheek
settings-library-skip-folders = Mappen overslaan met de naam
settings-library-skip-folders-help = De bibliotheek zoekt niet in mappen met deze namen, gescheiden door komma’s. Verborgen mappen worden altijd overgeslagen.
settings-library-skip-marked = Mappen overslaan die bevatten
settings-library-skip-marked-help = Ook niet in mappen met een van deze: .git voor code-checkouts, pyvenv.cfg voor Python-omgevingen, CACHEDIR.TAG voor caches. De map van een locatie zelf wordt nooit overgeslagen.
settings-library-show-timeline = Tijdlijn
settings-library-show-timeline-help = Toont boven het raster van de bibliotheek hoeveel foto’s elk jaar, elke maand of dag heeft.
