# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Polski.

keys-mode-drawing = Rysunki
cheatsheet-drawing = Rysunki
palette-drawing-layers = Pokaż lub ukryj panel warstw
palette-drawing-all-layers = Pokaż wszystkie warstwy lub warstwy jak zapisano
palette-drawing-paper = Zamień ciemny i jasny papier
palette-drawing-lineweights = Pokaż lub ukryj szerokości linii
disabled-not-drawing = Tylko dla rysunków (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Model
# machine-drafted, needs native review
cad-pen-name = Pisak { $n }
cad-page-name = Strona { $n }
cad-layer-name = Warstwa { $n }
cad-map-name = Mapa
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Warstwy
cad-board-top-name = Strona górna
cad-board-bottom-name = Strona dolna
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Ramka
cad-trace-name = Przebieg { $n }
cad-contours-positive-name = Kontury dodatnie
cad-contours-negative-name = Kontury ujemne
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = Model 3D
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Warstwy · widoczne { $shown } z { $total }
# Toasts.
drawing-layers-all-toast = Wszystkie warstwy widoczne
drawing-layers-saved-toast = Warstwy jak zapisano w rysunku
drawing-paper-dark-toast = Ciemny papier
drawing-paper-light-toast = Jasny papier
drawing-lineweights-on = Szerokości linii widoczne
drawing-lineweights-off = Szerokości linii wyłączone: cienkie linie

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Dodaj adnotacje
palette-markup-help = Chmurki, tekst, strzałki, pieczątki, pomiary i pinezki ze zdjęciami, zapisywane obok pliku.
palette-compare-sheets = Porównaj arkusze
palette-compare-sheets-help = Nałóż inną rewizję tego arkusza, wyrównaną, z obrysowanymi zmianami.
markup-title = Adnotacje
markup-list = Adnotacje na tej stronie
markup-count = { $count ->
    [0] Brak adnotacji na tej stronie
    [one] 1 adnotacja na tej stronie
    [few] { $count } adnotacje na tej stronie
    [many] { $count } adnotacji na tej stronie
   *[other] { $count } adnotacji na tej stronie
}
markup-empty = Wybierz narzędzie i kliknij arkusz.
markup-item = { $n }. { $kind }
markup-tool-select = Zaznacz
markup-tool-cloud = Chmurka
markup-tool-text = Tekst
markup-tool-arrow = Strzałka
markup-tool-stamp = Pieczątka
markup-tool-length = Długość
markup-tool-area = Pole
markup-tool-pin = Pinezka ze zdjęciem
markup-hint-select = Kliknij element, aby go zaznaczyć. Przeciągnij, aby go przesunąć. Enter edytuje jego tekst.
markup-hint-cloud = Przeciągnij prostokąt albo klikaj narożniki i naciśnij Enter.
markup-hint-text = Kliknij miejsce tekstu.
markup-hint-arrow = Przeciągnij od etykiety do punktu albo kliknij oba.
markup-hint-stamp = Kliknij miejsce pieczątki.
markup-hint-length = Przeciągnij albo kliknij oba końce.
markup-hint-area = Klikaj narożniki, a potem naciśnij Enter.
markup-hint-pin = Kliknij punkt, a potem wybierz zdjęcie.
markup-kind-cloud = Chmurka
markup-kind-text = Tekst
markup-kind-arrow = Strzałka
markup-kind-stamp = Pieczątka
markup-kind-length = Długość
markup-kind-area = Pole
markup-kind-pin = Zdjęcie
markup-colour = Kolor: { $colour }
markup-colour-red = Czerwony
markup-colour-blue = Niebieski
markup-colour-green = Zielony
markup-colour-orange = Pomarańczowy
markup-colour-magenta = Purpurowy
markup-colour-black = Czarny
markup-width = Linia: { $width }
markup-width-thin = Cienka
markup-width-medium = Średnia
markup-width-thick = Gruba
markup-stamp = Pieczątka: { $words }
markup-stamp-approved = Zatwierdzono
markup-stamp-rejected = Odrzucono
markup-stamp-reviewed = Sprawdzono
markup-stamp-custom = Własny tekst
markup-undo = Cofnij
markup-redo = Ponów
markup-delete = Usuń
markup-open-photo = Otwórz zdjęcie
markup-from-measurements = Dodaj pomiary
markup-from-measurements-done = { $count ->
    [one] Dodano 1 pomiar jako adnotację
    [few] Dodano { $count } pomiary jako adnotacje
    [many] Dodano { $count } pomiarów jako adnotacje
   *[other] Dodano { $count } pomiaru jako adnotacje
}
markup-export-pdf = Eksportuj PDF…
markup-export-png = Eksportuj PNG…
markup-done = Gotowe
markup-ask-text = Tekst (Enter wstawia, Esc anuluje)
markup-ask-label = Etykieta (opcjonalna; Enter wstawia)
markup-ask-stamp = Tekst pieczątki (Enter wstawia)
markup-ask-edit = Nowy tekst (Enter zapisuje)
markup-added = Dodano: { $kind }
markup-moved = Przesunięto
markup-pin-hint = Kliknij pinezkę ponownie lub naciśnij Enter, aby otworzyć jej zdjęcie.
markup-deleted = Usunięto
markup-undone = Cofnięto
markup-redone = Ponowiono
markup-delete-ask = Usunąć { $name }?
markup-delete-detail = Element zostanie usunięty z pliku adnotacji. Sam rysunek się nie zmienia.
markup-delete-ok = Usuń
markup-photo-missing = { $name } już nie istnieje.
markup-load-failed = Nie odczytano adnotacji: { $reason }
markup-save-failed = Nie zapisano adnotacji: { $reason }
markup-sidecar-too-large = plik adnotacji jest za duży
markup-pdf-unreadable = nie udało się odczytać pliku PDF
markup-exported = Zapisano { $name }
markup-export-failed = Nie wyeksportowano: { $reason }
markup-export-same-file = wybierz inną nazwę; oryginał nigdy nie jest nadpisywany
markup-export-encrypted = plik PDF jest zaszyfrowany. Zamiast tego wyeksportuj PNG.
markup-export-damaged = struktura pliku PDF jest uszkodzona. Zamiast tego wyeksportuj PNG.
sheets-title = Porównaj arkusze
sheets-other = Inny arkusz…
sheets-other-named = Inny: { $name }
sheets-page = Strona { $page } z { $count }
sheets-auto = Wyrównaj
sheets-pick = Wskaż 2 punkty
sheets-overlay = Nakładka
sheets-difference = Różnica
sheets-blend = Przenikanie
sheets-blink = Miganie
sheets-slider = Inny arkusz { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Brak zmian
    [one] Oznacz chmurką 1 zmianę
    [few] Oznacz chmurkami { $count } zmiany
    [many] Oznacz chmurkami { $count } zmian
   *[other] Oznacz chmurkami { $count } zmiany
}
sheets-done = Gotowe
sheets-choose = Wybierz drugi arkusz.
sheets-reading = Odczytywanie arkusza…
sheets-aligning = Wyrównywanie…
sheets-aligned = Wyrównano: skala { $scale }, obrót { $angle }°, dopasowanie do { $error } px. { $changes ->
    [0] Nie znaleziono zmian.
    [one] 1 zmiana.
    [few] { $changes } zmiany.
    [many] { $changes } zmian.
   *[other] { $changes } zmiany.
}
sheets-align-failed = Nie udało się wyrównać automatycznie. Wskaż 2 punkty.
sheets-pick-this = Punkt { $n }: kliknij go na tym arkuszu.
sheets-pick-other = Punkt { $n }: kliknij ten sam punkt na drugim arkuszu.
sheets-pick-same = Oba punkty są takie same. Wskaż 2 punkty ponownie.
sheets-read-failed = Nie odczytano arkusza: { $reason }
sheets-failed = Ten krok się nie powiódł. Spróbuj ponownie lub wybierz inny arkusz.
sheets-legend-this = Tylko ten arkusz
sheets-legend-other = Tylko drugi arkusz
sheets-legend-both = Oba
sheets-cloud-text = Zmiana
sheets-clouds-added = { $count ->
    [0] Nie dodano chmurek
    [one] Dodano 1 chmurkę do adnotacji
    [few] Dodano { $count } chmurki do adnotacji
    [many] Dodano { $count } chmurek do adnotacji
   *[other] Dodano { $count } chmurki do adnotacji
}
sheets-opened = Porównaj arkusze
sheets-closed = Zamknięto porównanie
markup-ask-new-line = Shift+Enter: nowy wiersz

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Rysunki
settings-drawings-shx-folders = Foldery czcionek SHX
settings-drawings-shx-folders-help = Foldery z czcionkami SHX dla tekstu w plikach DXF i DWG, oddzielone znakiem ;. Najpierw przeszukiwany jest folder samego rysunku.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = To nie jest folder: { $folder }
