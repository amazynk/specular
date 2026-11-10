# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Bibliotek

## Top bar
library-title = Bibliotek
library-locations-button = Platser

## Locations sidebar
library-locations = Platser
library-all = Alla platser
library-add = Lägg till plats…
library-remove = Ta bort { $location } från biblioteket
library-offline = Frånkopplad
library-location-pending = { $count } att indexera
library-a11y-location = { $location }, { $count ->
    [one] { $count } bild
   *[other] { $count } bilder
}
library-a11y-location-offline = { $location }, Frånkopplad, { $count ->
    [one] { $count } bild
   *[other] { $count } bilder
}

## Indexing progress
library-walking = Hittade bilder: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Hittade bilder: { $count }
library-reading = Indexerar… { $done } av { $total }
library-held = Indexeringen pausar medan du bläddrar
library-up-to-date = Uppdaterat
# machine-drafted, needs native review
library-stopped = Indexeringen stoppad
library-stopped-pending = Indexeringen stoppad · { $count } att indexera
library-resume = Återuppta
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indexeringen stoppade: { $reason }
library-reason-stalled = inget har hänt på { $minutes ->
    [one] en minut
   *[other] { $minutes } minuter
}. En fil eller enhet svarar kanske inte.
library-retry = Försök igen
library-retry-description = Startar indexeringen igen. Det som har indexerats går fortfarande att söka i.
library-save-diagnostics = Spara diagnostikpaket…
library-stop-description = Slutar indexera. Det som har indexerats hittills går fortfarande att söka i; Återuppta eller Sök igenom igen (F5) fortsätter.
library-resume-description = Går igenom alla platser igen och indexerar det som återstår.
library-opening = Öppnar biblioteket…
library-searching = Söker…

## Empty and error states
library-empty = Inga platser än. Lägg till en mapp: dess bilder och bilderna i dess undermappar visas här i ett rutnät.
library-empty-offline = Platserna är frånkopplade. Bilderna visas igen när de är tillbaka.
library-indexing = Letar efter bilder…
# machine-drafted, needs native review
library-remove-confirm = Ta bort ”{ $location }” från biblioteket?
library-remove-confirm-detail = Dina bilder ligger kvar på disken med betyg, etiketter och nyckelord. Biblioteket glömmer vad det indexerade där; lägger du till mappen igen indexeras den på nytt.
library-remove-confirm-ok = Ta bort
library-indexing-in = Letar efter bilder i { $location }… { $count } hittade hittills
# machine-drafted, needs native review
library-empty-stopped = Indexeringen stoppades innan bilderna lästes. Återuppta fortsätter därifrån.
library-unavailable = Biblioteket kunde inte öppnas: { $reason }
library-read-only = Biblioteket skrevs av en nyare Specular: det går att söka i men inte uppdatera.

## Screen readers
library-a11y-list = Bibliotek, { $count ->
    [one] { $count } bild
   *[other] { $count } bilder
}
library-a11y-added = { $location } lades till i biblioteket
library-a11y-removed = { $location } togs bort från biblioteket

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Lägga till allt i ”{ $folder }”?
library-home-confirm-detail = Den innehåller programfiler, nedladdningar och cacher, inte bara foton. Indexeringen tar längre tid och biblioteket fylls med ikoner och andra bilder.
library-home-confirm-pictures = Din mapp Bilder innehåller bara dina bilder.
library-home-confirm-use-pictures = Lägg till Bilder
library-home-confirm-keep = Lägg till hemmappen

## Folder watches
# machine-drafted, needs native review
library-unwatched = Vissa mappar bevakas inte
library-unwatched-detail = { $minutes ->
    [one] Ändringar syns inom en minut
   *[other] Ändringar syns inom { $minutes } min
}
library-unwatched-description = Specular har slut på mappbevakningar. Öppnar hjälpen om hur du höjer gränsen.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Bibliotek
settings-library-skip-folders = Hoppa över mappar med namnet
settings-library-skip-folders-help = Biblioteket söker inte i mappar med dessa namn, avgränsade med kommatecken. Dolda mappar hoppas alltid över.
settings-library-skip-marked = Hoppa över mappar som innehåller
settings-library-skip-marked-help = Inte heller i mappar som innehåller något av detta: .git för kodutcheckningar, pyvenv.cfg för Python-miljöer, CACHEDIR.TAG för cacher. En platss egen mapp hoppas aldrig över.
settings-library-show-timeline = Tidslinje
settings-library-show-timeline-help = Visar hur många foton varje år, månad eller dag har, ovanför bibliotekets rutnät.
