# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Bibliotek

## Top bar
library-title = Bibliotek
library-locations-button = Placeringer

## Locations sidebar
library-locations = Placeringer
library-all = Alle placeringer
library-add = Tilføj placering…
library-remove = Fjern { $location } fra biblioteket
library-offline = Offline
library-location-pending = { $count } mangler indeksering
library-a11y-location = { $location }, { $count ->
    [one] { $count } billede
   *[other] { $count } billeder
}
library-a11y-location-offline = { $location }, Offline, { $count ->
    [one] { $count } billede
   *[other] { $count } billeder
}

## Indexing progress
library-walking = Billeder fundet: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Billeder fundet: { $count }
library-reading = Indekserer… { $done } af { $total }
library-held = Indekseringen holder pause, mens du browser
library-up-to-date = Opdateret
# machine-drafted, needs native review
library-stopped = Indekseringen er stoppet
library-stopped-pending = Indekseringen er stoppet · { $count } mangler
library-resume = Genoptag
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indekseringen er stoppet: { $reason }
library-reason-stalled = intet har bevæget sig i { $minutes ->
    [one] et minut
   *[other] { $minutes } minutter
}. En fil eller et drev svarer måske ikke.
library-retry = Prøv igen
library-retry-description = Starter indekseringen igen. Det, der er indekseret, kan stadig søges.
library-save-diagnostics = Gem diagnosepakke…
library-stop-description = Stopper indekseringen. Det, der er indekseret indtil nu, kan stadig søges; Genoptag eller Genscan (F5) fortsætter.
library-resume-description = Gennemgår alle placeringer igen og indekserer det, der mangler.
library-opening = Åbner biblioteket…
library-searching = Søger…

## Empty and error states
library-empty = Ingen placeringer endnu. Tilføj en mappe: dens billeder og billederne i dens undermapper vises her i ét gitter.
library-empty-offline = Disse placeringer er offline. Deres billeder vises igen, når de er tilbage.
library-indexing = Leder efter billeder…
# machine-drafted, needs native review
library-remove-confirm = Fjern »{ $location }« fra biblioteket?
library-remove-confirm-detail = Dine fotos bliver på disken med deres bedømmelser, etiketter og nøgleord. Biblioteket glemmer, hvad det har indekseret der; tilføjer du mappen igen, indekseres den igen.
library-remove-confirm-ok = Fjern
library-indexing-in = Leder efter billeder i { $location }… { $count } fundet indtil videre
# machine-drafted, needs native review
library-empty-stopped = Indekseringen blev stoppet, før disse billeder blev læst. Genoptag fortsætter derfra.
library-unavailable = Biblioteket kunne ikke åbnes: { $reason }
library-read-only = Dette bibliotek er skrevet af en nyere Specular: det kan gennemsøges, men ikke opdateres.

## Screen readers
library-a11y-list = Bibliotek, { $count ->
    [one] { $count } billede
   *[other] { $count } billeder
}
library-a11y-added = { $location } føjet til biblioteket
library-a11y-removed = { $location } fjernet fra biblioteket

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Tilføj alt i ”{ $folder }”?
library-home-confirm-detail = Den indeholder programfiler, downloads og caches, ikke kun fotos. Indekseringen tager længere tid, og biblioteket fyldes med ikoner og andre billeder.
library-home-confirm-pictures = Din mappe Billeder indeholder kun dine billeder.
library-home-confirm-use-pictures = Tilføj Billeder
library-home-confirm-keep = Tilføj hjemmemappen

## Folder watches
# machine-drafted, needs native review
library-unwatched = Nogle mapper overvåges ikke
library-unwatched-detail = { $minutes ->
    [one] Ændringer vises inden for et minut
   *[other] Ændringer vises inden for { $minutes } min
}
library-unwatched-description = Specular har ikke flere mappeovervågninger tilbage. Åbner hjælpen om, hvordan grænsen hæves.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Bibliotek
settings-library-skip-folders = Spring mapper over med navnet
settings-library-skip-folders-help = Biblioteket leder ikke i mapper med disse navne, adskilt af kommaer. Skjulte mapper springes altid over.
settings-library-skip-marked = Spring mapper over, der indeholder
settings-library-skip-marked-help = Heller ikke i mapper, der indeholder et af disse: .git til kodeudtræk, pyvenv.cfg til Python-miljøer, CACHEDIR.TAG til caches. En placerings egen mappe springes aldrig over.
settings-library-show-timeline = Tidslinje
settings-library-show-timeline-help = Viser, hvor mange fotos hvert år, måned eller dag har, over bibliotekets gitter.
