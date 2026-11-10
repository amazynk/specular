# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Biblioteka

## Top bar
library-title = Biblioteka
library-locations-button = Lokalizacje

## Locations sidebar
library-locations = Lokalizacje
library-all = Wszystkie lokalizacje
library-add = Dodaj lokalizację…
library-remove = Usuń { $location } z biblioteki
library-offline = Offline
library-location-pending = Do zindeksowania: { $count }
library-a11y-location = { $location }, { $count ->
    [one] { $count } obraz
    [few] { $count } obrazy
    [many] { $count } obrazów
   *[other] { $count } obrazu
}
library-a11y-location-offline = { $location }, Offline, { $count ->
    [one] { $count } obraz
    [few] { $count } obrazy
    [many] { $count } obrazów
   *[other] { $count } obrazu
}

## Indexing progress
library-walking = Znalezione obrazy: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Znalezione obrazy: { $count }
library-reading = Indeksowanie… { $done } z { $total }
library-held = Indeksowanie wstrzymuje się podczas przeglądania
library-up-to-date = Aktualna
# machine-drafted, needs native review
library-stopped = Indeksowanie zatrzymane
library-stopped-pending = Indeksowanie zatrzymane · do zindeksowania: { $count }
library-resume = Wznów
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indeksowanie zatrzymane: { $reason }
library-reason-stalled = nic się nie ruszyło od { $minutes ->
    [one] minuty
    [few] { $minutes } minut
    [many] { $minutes } minut
   *[other] { $minutes } minuty
}. Plik lub dysk może nie odpowiadać.
library-retry = Spróbuj ponownie
library-retry-description = Uruchamia indeksowanie od nowa. To, co już zindeksowano, można nadal przeszukiwać.
library-save-diagnostics = Zapisz pakiet diagnostyczny…
library-stop-description = Zatrzymuje indeksowanie. To, co już zindeksowano, można nadal przeszukiwać; Wznów lub Odśwież (F5) kontynuuje.
library-resume-description = Ponownie przegląda wszystkie lokalizacje i indeksuje to, co zostało.
library-opening = Otwieranie biblioteki…
library-searching = Wyszukiwanie…

## Empty and error states
library-empty = Brak lokalizacji. Dodaj folder: jego obrazy i obrazy z podfolderów pojawią się tu w jednej siatce.
library-empty-offline = Te lokalizacje są offline. Ich obrazy pojawią się ponownie, gdy wrócą.
library-indexing = Szukanie obrazów…
# machine-drafted, needs native review
library-remove-confirm = Usunąć „{ $location }” z biblioteki?
library-remove-confirm-detail = Twoje zdjęcia zostają na dysku razem z ocenami, etykietami i słowami kluczowymi. Biblioteka zapomina, co tam zaindeksowała; po ponownym dodaniu folderu zaindeksuje go od nowa.
library-remove-confirm-ok = Usuń
library-indexing-in = Szukanie obrazów w { $location }… dotąd znaleziono: { $count }
# machine-drafted, needs native review
library-empty-stopped = Indeksowanie zatrzymano, zanim te obrazy zostały odczytane. Wznów kontynuuje od tego miejsca.
library-unavailable = Nie można otworzyć biblioteki: { $reason }
library-read-only = Tę bibliotekę zapisał nowszy Specular: można ją przeszukiwać, ale nie aktualizować.

## Screen readers
library-a11y-list = Biblioteka, { $count ->
    [one] { $count } obraz
    [few] { $count } obrazy
    [many] { $count } obrazów
   *[other] { $count } obrazu
}
library-a11y-added = Dodano { $location } do biblioteki
library-a11y-removed = Usunięto { $location } z biblioteki

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Dodać całą zawartość „{ $folder }”?
library-home-confirm-detail = Zawiera pliki programów, pobrane pliki i pamięci podręczne, nie tylko zdjęcia. Indeksowanie trwa dłużej, a biblioteka wypełnia się ikonami i innymi obrazami.
library-home-confirm-pictures = Twój folder Obrazy zawiera tylko Twoje obrazy.
library-home-confirm-use-pictures = Dodaj Obrazy
library-home-confirm-keep = Dodaj folder domowy

## Folder watches
# machine-drafted, needs native review
library-unwatched = Niektóre foldery nie są obserwowane
library-unwatched-detail = { $minutes ->
    [one] Zmiany pojawiają się w ciągu minuty
    [few] Zmiany pojawiają się w ciągu { $minutes } min
    [many] Zmiany pojawiają się w ciągu { $minutes } min
   *[other] Zmiany pojawiają się w ciągu { $minutes } min
}
library-unwatched-description = Specularowi zabrakło obserwacji folderów. Otwiera pomoc o podnoszeniu limitu.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Biblioteka
settings-library-skip-folders = Pomijaj foldery o nazwie
settings-library-skip-folders-help = Biblioteka nie przeszukuje folderów o tych nazwach, oddzielonych przecinkami. Ukryte foldery są zawsze pomijane.
settings-library-skip-marked = Pomijaj foldery zawierające
settings-library-skip-marked-help = Ani folderów zawierających coś z tego: .git dla repozytoriów kodu, pyvenv.cfg dla środowisk Pythona, CACHEDIR.TAG dla pamięci podręcznych. Własny folder lokalizacji nigdy nie jest pomijany.
settings-library-show-timeline = Oś czasu
settings-library-show-timeline-help = Pokazuje nad siatką biblioteki, ile zdjęć ma każdy rok, miesiąc lub dzień.
