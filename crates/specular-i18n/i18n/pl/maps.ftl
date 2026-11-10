# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# Polski.

keys-mode-map = Mapy
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Warstwy
cheatsheet-map-coordinates = Współrzędne
cheatsheet-map-measure = Pomiary

## Palette entries and their one-line descriptions.
palette-map-layers = Pokaż lub ukryj panel warstw
palette-map-layers-help = Warstwy mapy: kolejność, krycie, widoczność i styl.
palette-map-add-layer = Dodaj warstwę z pliku…
palette-map-add-layer-help = Narysuj inny plik mapy na tej, np. ślad GPX na shapefile.
palette-map-zoom-to-layer = Powiększ do warstwy
palette-map-zoom-to-layer-help = Pokaż całą warstwę wybraną w panelu warstw.
palette-map-attribute-table = Pokaż lub ukryj tabelę atrybutów
palette-map-attribute-table-help = Obiekty wybranej warstwy jako tabela: sortowanie, filtrowanie, wyszukiwanie i zaznaczanie.
palette-map-coordinate-format = Następny format współrzędnych
palette-map-coordinate-format-help = Stopnie dziesiętne, stopnie minuty sekundy, UTM lub MGRS.
palette-map-units = Jednostki metryczne lub imperialne
palette-map-units-help = Długości i powierzchnie w metrach albo w stopach i milach.
palette-map-copy-coordinate = Kopiuj współrzędne
palette-map-copy-coordinate-help = Skopiuj położenie pod wskaźnikiem lub środka mapy.
palette-map-go-to-coordinate = Przejdź do współrzędnych…
palette-map-go-to-coordinate-help = Wpisz położenie w dowolnym formacie, aby wyśrodkować tam mapę.
palette-map-measure-distance = Zmierz odległość
palette-map-measure-distance-help = Klikaj punkty na mapie: długość na powierzchni Ziemi i azymut.
palette-map-measure-area = Zmierz powierzchnię
palette-map-measure-area-help = Klikaj narożniki obszaru: jego powierzchnia i obwód na powierzchni Ziemi.
palette-map-elevation-profile = Profil wysokości
palette-map-elevation-profile-help = Narysuj linię na modelu wysokości, aby zobaczyć wykres wysokości.
palette-map-overview = Pokaż lub ukryj mapę przeglądową
palette-map-overview-help = Mała mapa całego zasięgu z zaznaczoną częścią widoczną na ekranie.

## Why a map command is unavailable.
disabled-not-map = Tylko dla map, przy włączonych narzędziach map
disabled-no-lonlat = Mapa nie ma znanego układu współrzędnych
disabled-no-features = Mapa nie ma obiektów
disabled-no-heights = Mapa nie ma wysokości

## Settings › Maps.
settings-section-maps = Mapy
settings-maps-coordinate-format = Format współrzędnych
settings-maps-coordinate-format-help = Jak położenia są pokazywane i kopiowane. Przejdź do współrzędnych odczytuje każdy format.
settings-maps-coordinate-format-dd = Stopnie dziesiętne
settings-maps-coordinate-format-dms = Stopnie, minuty, sekundy
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Jednostki
settings-maps-units-help = Długości i powierzchnie na mapach: podziałka i pomiary. Automatycznie: zgodnie z Metadane › Jednostki.
settings-maps-units-auto = Automatycznie
settings-maps-units-metric = Metryczne
settings-maps-units-imperial = Imperialne

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Szer., dł. { $position }
}
map-height = Wysokość { $height }
map-height-label = Wysokość
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Podziałka: { $length }
map-north-label = Strzałka północy: północ jest { $degrees }° w prawo od góry
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] stopnie, minuty, sekundy
    [utm] UTM
    [mgrs] MGRS
   *[dd] stopnie dziesiętne
}
map-format-toast = Współrzędne: { $format }
map-units-metric = Jednostki: metry i kilometry
map-units-imperial = Jednostki: stopy i mile
toast-copied-coordinate = Skopiowano współrzędne

## Go to coordinate.
map-goto-hint = Stopnie dziesiętne, DMS, UTM lub MGRS. Enter przechodzi.
map-goto-empty = Wpisz położenie.
map-goto-unrecognised = To nie jest położenie w znanym formacie.
map-goto-range = Szerokość lub długość poza zakresem.
map-goto-no-way = Ta mapa nie może umieścić położenia.
map-goto-outside = To położenie jest poza mapą.
map-goto-toast = Wyśrodkowano na { $position }

## The layer panel.
map-layers-title = Warstwy: widoczne { $shown } z { $total }
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } obiekt
    [few] { $features } obiekty
    [many] { $features } obiektów
   *[other] { $features } obiektu
}, krycie { PERCENT($opacity) }
map-kind-area = Powierzchnie
map-kind-line = Linie
map-kind-point = Punkty
map-kind-empty = Brak obiektów
map-layer-visible = Pokaż lub ukryj warstwę
map-layer-opacity-down = Mniejsze krycie
map-layer-opacity-up = Większe krycie
map-layer-colour = Następny kolor
map-layer-width = Następna grubość linii
map-layer-point = Następny rozmiar punktu
map-layer-up = Przesuń w górę
map-layer-down = Przesuń w dół
map-layer-remove = Usuń
map-layer-keep = Zachowaj
map-layer-remove-ask = Usunąć { $name } z mapy? Plik pozostanie na dysku.
map-layer-adding = Dodawanie warstwy…
map-removed = Usunięto { $name } z mapy
map-added = { $layers ->
    [one] Dodano { $name }
    [few] Dodano { $name }: { $layers } warstwy
    [many] Dodano { $name }: { $layers } warstw
   *[other] Dodano { $name }: { $layers } warstwy
}
map-add-failed = Nie można dodać { $name }: { $reason }
map-add-filter = Pliki map
map-add-not-map = to nie jest mapa.
map-add-not-vector = nie ma obiektów.
map-add-no-crs = nie ma pliku odwzorowania (.prj).

## Identify.
map-identify-more = i { $count } więcej w tabeli atrybutów
map-cell = Komórka { $x }, { $y }
map-band = Kanał { $n }
map-no-data = Brak danych

## The attribute table.
map-table-title = { $layer }: obiekty { $shown } z { $total }, zaznaczone { $selected }
map-table-search = Szukaj albo pole = wartość
map-table-selected-only = Tylko zaznaczone
map-table-empty = Brak pasujących obiektów.
map-table-many = wiele
map-table-stats-number = { $field }: wartości { $count }, pustych { $empty } · min. { $min } · maks. { $max } · średnia { $mean } · suma { $sum } · odch. std. { $sd }
map-table-stats-text = { $field }: wartości { $count }, pustych { $empty } · różnych { $distinct }
map-table-stats-text-top = { $field }: wartości { $count }, pustych { $empty } · różnych { $distinct } · najczęstsza { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Klikaj punkty wzdłuż trasy.
map-measure-hint-area = Klikaj narożniki obszaru.
map-measure-hint-profile = Klikaj punkty wzdłuż linii.
map-measure-keys = Enter kończy. Backspace usuwa ostatni punkt. Esc zamyka.
map-measure-distance = Odległość { $length }
map-measure-bearing = Azymut { $bearing }
map-measure-segment = Ostatni odcinek { $length }, azymut { $bearing }
map-measure-area = Powierzchnia { $area }
map-measure-perimeter = Obwód { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Długość { $length } · najniżej { $min } · najwyżej { $max } · podejście { $climb } · zejście { $descent }
map-profile-no-heights = Długość { $length }. Brak wysokości wzdłuż linii.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: brak wysokości
