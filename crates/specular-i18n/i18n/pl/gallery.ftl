# machine-drafted, needs native review
# Specular — galeria (tryb miniatur, DESIGN §10) i Ustawienia → Galeria. Polski.

## Paleta i menu

palette-gallery = Galeria
gallery-palette-view = Widok galerii…
gallery-palette-layout = Układ
gallery-palette-sort = Sortuj według
gallery-palette-group = Grupuj według
gallery-palette-filter = Filtruj…
gallery-palette-quick-filters = Szybkie filtry
gallery-palette-details = Widok szczegółów
gallery-palette-info-overlay = Informacje na miniaturach
gallery-palette-stacks = Grupuj RAW+JPEG i serie
gallery-palette-subfolders = Uwzględnij podfoldery
gallery-palette-compare = Porównaj zaznaczone
gallery-palette-rate = Oceń…
gallery-palette-settings = Ustawienia galerii…
gallery-view-menu = Widok
gallery-more-menu = Więcej
gallery-open = Otwórz
gallery-new-window = Otwórz w nowym oknie
gallery-reveal = Pokaż w folderze
gallery-properties = Właściwości
gallery-copy-paths = Kopiuj ścieżki
gallery-select-all = Zaznacz wszystko
gallery-select-none = Odznacz wszystko
gallery-select-invert = Odwróć zaznaczenie
gallery-next-unrated = Następny nieoceniony
gallery-stack-toggle = Rozwiń lub zwiń stos
gallery-run-on = { $count ->
    [one] { $name } ({ $count } plik)
    [few] { $name } ({ $count } pliki)
    [many] { $name } ({ $count } plików)
   *[other] { $name } ({ $count } pliku)
}
gallery-depth-off = Tylko ten folder
gallery-depth-all = Wszystkie podfoldery
gallery-depth-levels = { $depth ->
    [one] { $depth } poziom podfolderów
    [few] { $depth } poziomy podfolderów
    [many] { $depth } poziomów podfolderów
   *[other] { $depth } poziomu podfolderów
}

## Stan i stany puste

gallery-items = { $count ->
    [one] { $count } element
    [few] { $count } elementy
    [many] { $count } elementów
   *[other] { $count } elementu
}
gallery-selected = Zaznaczono { $count } z { $total }
gallery-shown-of = Pokazano { $count } z { $total }
gallery-listing = Odczytywanie folderu… { $count }
gallery-empty = Ten folder nie zawiera obrazów.
gallery-empty-filter = Żaden obraz nie pasuje do filtra.
gallery-list-error = Nie można odczytać folderu: { $reason }
gallery-no-folder = Nie otwarto folderu
gallery-kind-folder = Folder
gallery-drag-count = { $count ->
    [one] { $count } plik
    [few] { $count } pliki
    [many] { $count } plików
   *[other] { $count } pliku
}
gallery-badge-pages = { $count ->
    [one] { $count } strona
    [few] { $count } strony
    [many] { $count } stron
   *[other] { $count } strony
}

## Pasek filtrów

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Nazwa lub rating>=3 camera:X-T5 …
gallery-filter-clear = Wyczyść filtr
gallery-filter-rating = ★ { $stars } lub więcej
gallery-filter-label = Etykieta: { $label }
gallery-filter-picks = Wybrane
gallery-filter-hide-rejects = Ukryj odrzucone
gallery-filter-unrated = Nieocenione
gallery-filter-raw = RAW
gallery-filter-animated = Animowane
gallery-filter-hdr = HDR
gallery-filter-landscape = Poziome
gallery-filter-portrait = Pionowe

## Query errors and facets

gallery-query-unknown-field = Nieznane pole „{ $field }”
gallery-query-unknown-field-suggest = Nieznane pole „{ $field }”: czy chodziło o „{ $suggestion }”?
gallery-query-missing-value = „{ $field }” wymaga wartości
gallery-query-bad-number = „{ $value }” nie jest liczbą ({ $field })
gallery-query-bad-date = „{ $value }” nie jest datą ({ $field }): użyj 2024, 2024-05 lub 2024-05-12
gallery-query-bad-word = „{ $value }” nie pasuje do { $field }: użyj { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = „{ $value }” nie jest nazwą znacznika Exif
gallery-query-no-compare = { $field } nie można tu porównywać za pomocą < ani >
gallery-query-unterminated = Brakuje cudzysłowu zamykającego
gallery-query-unavailable = „{ $field }” wymaga Biblioteki (tu niedostępne)
gallery-facets = Aspekty
gallery-facets-counting = Liczenie…
gallery-facets-empty = Nie ma czego liczyć
gallery-facets-search = Szukaj w aspektach
gallery-facets-no-match = Brak pasujących wartości
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Ekran startowy
gallery-facet-format = Format
gallery-facet-camera = Aparat
gallery-facet-lens = Obiektyw
gallery-facet-rating = Ocena
gallery-facet-year = Rok
gallery-facet-orientation = Orientacja
gallery-facet-label = Etykieta
gallery-facet-properties = Właściwości
gallery-facet-square = Kwadratowe
gallery-facet-alpha = Przezroczystość
gallery-facet-gps = Lokalizacja
gallery-facet-multipage = Wiele stron
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } plik
    [few] { $count } pliki
    [many] { $count } plików
   *[other] { $count } pliku
}

## Grupy

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Ten folder
gallery-group-unknown = Nieznane
gallery-group-unrated = Nieocenione

## Selekcja

gallery-stars = { $stars ->
    [one] { $stars } gwiazdka
    [few] { $stars } gwiazdki
    [many] { $stars } gwiazdek
   *[other] { $stars } gwiazdki
}
gallery-rate-none = Bez oceny
gallery-pick = Wybierz
gallery-reject = Odrzuć
gallery-unflag = Bez flagi
gallery-label-red = Czerwona
gallery-label-yellow = Żółta
gallery-label-green = Zielona
gallery-label-blue = Niebieska
gallery-label-none = Bez etykiety
gallery-compare-hint = Zaznacz od 2 do 4 obrazów, aby je porównać.

## Pliki

gallery-trash-count = { $count ->
    [one] Przenieś { $count } plik do kosza
    [few] Przenieś { $count } pliki do kosza
    [many] Przenieś { $count } plików do kosza
   *[other] Przenieś { $count } pliku do kosza
}
gallery-moved-to = { $count ->
    [one] Przeniesiono { $count } plik do { $folder }
    [few] Przeniesiono { $count } pliki do { $folder }
    [many] Przeniesiono { $count } plików do { $folder }
   *[other] Przeniesiono { $count } pliku do { $folder }
}
gallery-copied-to = { $count ->
    [one] Skopiowano { $count } plik do { $folder }
    [few] Skopiowano { $count } pliki do { $folder }
    [many] Skopiowano { $count } plików do { $folder }
   *[other] Skopiowano { $count } pliku do { $folder }
}
gallery-move-failed = Nie można przenieść { $file }: { $reason }
gallery-drop-move = Przenieś do { $folder }
gallery-drop-copy = Kopiuj do { $folder }
gallery-move-here = Przenieś zaznaczone tutaj
gallery-copy-here = Kopiuj zaznaczone tutaj
gallery-not-an-image = { $file } nie jest obrazem, który { -app-name } może otworzyć.
gallery-cache-cleared = Wyczyszczono pamięć podręczną miniatur
# machine-drafted, needs native review
gallery-thumb-failed = Brak miniatury: { $reason }
gallery-cache-clear-failed = Nie można wyczyścić pamięci podręcznej miniatur: { $reason }

## Właściwości

gallery-prop-name = Nazwa
gallery-prop-folder = Folder
gallery-prop-size = Rozmiar pliku
gallery-prop-dimensions = Wymiary
gallery-prop-format = Format
gallery-prop-date = Data
gallery-prop-camera = Aparat
gallery-prop-lens = Obiektyw
gallery-prop-rating = Ocena
gallery-prop-label = Etykieta
gallery-prop-items = Elementy

## Czytniki ekranu

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } element
    [few] { $count } elementy
    [many] { $count } elementów
   *[other] { $count } elementu
}
gallery-a11y-dims = { $width } na { $height }
gallery-a11y-folder = folder
gallery-a11y-up = W górę do { $folder }
gallery-a11y-archive = archiwum
gallery-a11y-stack = stos: { $count }
gallery-a11y-filter-bar = Filtr
gallery-a11y-filter-field = Filtruj według nazwy lub pola
gallery-a11y-chip = Warunek filtra { $term }
gallery-a11y-chip-remove = Naciśnij, aby usunąć
gallery-a11y-results = { $count ->
    [0] Brak pasujących elementów
    [one] { $count } pasujący element
    [few] { $count } pasujące elementy
    [many] { $count } pasujących elementów
   *[other] { $count } pasującego elementu
}

## Ustawienia → Galeria

settings-section-gallery = Galeria
settings-gallery-layout = Układ
settings-gallery-layout-help = Kwadraty przycinają miniatury do siatki; wyrównane wiersze zachowują kształt obrazów; bez przycinania mieści całe obrazy w równych polach.
settings-gallery-layout-squares = Kwadraty
settings-gallery-layout-justified = Wyrównane wiersze
settings-gallery-layout-uncropped = Bez przycinania
settings-gallery-layout-details = Szczegóły
settings-gallery-thumbnail-size = Rozmiar miniatur
settings-gallery-thumbnail-size-help = Także Ctrl+kółko myszy, gest szczypania lub Ctrl+plus i minus w galerii.
settings-gallery-captions = Pokazuj nazwy plików
settings-gallery-captions-help = Podpis pod każdą miniaturą.
settings-gallery-badges = Pokazuj plakietki
settings-gallery-badges-help = Małe oznaczenia RAW, animacji, HDR, wielu stron, oceny i etykiety.
settings-gallery-sort-order = Sortuj według
settings-gallery-sort-order-help = Kolejność w galerii. Foldery są zawsze pierwsze.
settings-gallery-sort-order-name = Nazwa
settings-gallery-sort-order-date-taken = Data wykonania
settings-gallery-sort-order-modified = Data modyfikacji
settings-gallery-sort-order-size = Rozmiar pliku
settings-gallery-sort-order-type = Typ pliku
settings-gallery-sort-order-dimensions = Wymiary
settings-gallery-sort-descending = Odwrotna kolejność
settings-gallery-sort-descending-help = Sortuj od ostatniego do pierwszego.
settings-gallery-group-by = Grupuj według
settings-gallery-group-by-help = Nagłówki sekcji, które pozostają u góry podczas przewijania.
settings-gallery-group-by-none = Bez grupowania
settings-gallery-group-by-day = Dzień
settings-gallery-group-by-month = Miesiąc
settings-gallery-group-by-year = Rok
settings-gallery-group-by-event = Wydarzenie
settings-gallery-group-by-folder = Folder
settings-gallery-group-by-camera = Aparat
settings-gallery-group-by-format = Format
settings-gallery-group-by-rating = Ocena
settings-gallery-group-by-target = Obiekt
settings-gallery-group-by-filter = Filtr
settings-gallery-per-folder-view = Zapamiętuj widok dla każdego folderu
settings-gallery-per-folder-view-help = Układ, rozmiar, sortowanie i grupowanie są zapisywane osobno dla każdego folderu; w przeciwnym razie wszędzie używany jest jeden widok.
settings-gallery-open-folders-in-gallery = Otwieraj foldery w galerii
settings-gallery-open-folders-in-gallery-help = Po otwarciu folderu pokazuj najpierw miniatury zamiast pierwszego obrazu.
settings-gallery-escape-opens-gallery = Esc otwiera galerię
settings-gallery-escape-opens-gallery-help = Gdy nie ma nic innego do zamknięcia, Esc w przeglądarce przechodzi do galerii, zamiast nic nie robić.
settings-gallery-wrap-around = Strzałki przechodzą w kółko
settings-gallery-wrap-around-help = Na końcu listy strzałki przechodzą na początek, a z początku na koniec. Przytrzymany klawisz nadal zatrzymuje się na końcu.
settings-gallery-show-hidden = Pokazuj ukryte pliki
settings-gallery-show-hidden-help = Uwzględniaj pliki i foldery, których nazwy zaczynają się od kropki.
settings-gallery-show-other-files = Pokazuj inne pliki
settings-gallery-show-other-files-help = Uwzględniaj pliki, których { -app-name } nie może otworzyć, z ogólną ikoną.
settings-gallery-show-archives = Pokazuj archiwa
settings-gallery-show-archives-help = Uwzględniaj komiksy i inne archiwa (zip, cbz, rar, 7z…) jako kafelki. Bezpośrednie otwieranie archiwów nadal działa.
settings-gallery-subfolder-depth = Poziomy podfolderów
settings-gallery-subfolder-depth-help = Pokazuj też obrazy z podfolderów, do podanej liczby poziomów (0: tylko ten folder).
settings-gallery-stacks = Grupuj powiązane pliki
settings-gallery-stacks-help = Pokazuj pary RAW+JPEG i serie jako jeden kafelek. Naciśnij S, aby rozwinąć stos.
settings-gallery-info-overlay = Informacje na miniaturach
settings-gallery-info-overlay-help = Wymiary i ekspozycja na każdej miniaturze.
settings-gallery-prefer-embedded = Preferuj osadzone podglądy
settings-gallery-prefer-embedded-help = Używaj podglądu zapisanego w pliku, gdy jest wystarczająco duży. Znacznie szybsze, nieco mniej dokładne.
settings-gallery-sharper-raw-thumbnails = Ostrzejsze miniatury RAW
settings-gallery-sharper-raw-thumbnails-help = Edytowane zdjęcia RAW są wywoływane do miniatur w 1024 px. Ostrzejsze, nieco wolniejsze. Wyłączone: 512 px.
settings-gallery-rating-sidecars = Zapisuj oceny w plikach XMP
settings-gallery-rating-sidecars-help = Oceny, etykiety i flagi trafiają też do pliku .xmp obok obrazu, który odczytują inne programy do zdjęć.
settings-gallery-cache-size = Rozmiar pamięci podręcznej miniatur
settings-gallery-cache-size-help = Gdy pamięć podręczna jest pełna, najstarsze miniatury są usuwane.
settings-gallery-cache-location = Położenie pamięci podręcznej miniatur
settings-gallery-cache-location-help = Najlepiej sprawdza się folder na szybkim dysku.
settings-gallery-clear-cache = Wyczyść pamięć podręczną miniatur

## Compare
# machine-drafted, needs native review

compare-side = Obok siebie
compare-overlay = Nakładka
compare-difference = Różnica
compare-flicker = Miganie
compare-blend = Przenikanie
compare-kind-absolute = Błąd bezwzględny
compare-kind-squared = Błąd kwadratowy
compare-kind-relative = Błąd względny
compare-scale-down = Mniej czułe
compare-scale-up = Bardziej czułe
compare-scale = Pełna skala { $value }
compare-space = Światło liniowe, jak wyświetlane
compare-space-help = Błędy i miary są mierzone w świetle liniowym, tak jak obraz jest wyświetlany, po mapowaniu tonów, dla każdego kanału łącznie z alfą.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = próbkowano przy { $width } × { $height }
compare-aligned = Wyrównane do lewego górnego rogu, porównano { $width } × { $height }
compare-vs = { $b } względem { $a }
compare-overlay-label = { $b } na { $a }, { PERCENT($percent) }
compare-pending = Pomiar…
# machine-drafted, needs native review
compare-diff-failed = Nie można pokazać różnicy: { $reason }
compare-no-overlap = Nic do porównania w widoku
compare-zoom-fit = Dopasuj
compare-zoom = { PERCENT($percent) }
compare-showing = Wyświetlane: { $name }
compare-a11y-group = Porównanie
compare-a11y-blend = Przenikanie między obrazami
compare-a11y-zoom = Powiększenie
gallery-palette-compare-side = Porównaj obok siebie
gallery-palette-compare-overlay = Porównaj jako nakładkę
gallery-palette-compare-difference = Porównaj różnicę: { $kind }
gallery-palette-compare-flicker = Migaj między porównywanymi obrazami
gallery-palette-compare-actual = Porównaj przy { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Spacja lub Ctrl+Enter zaznacza kilka

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Profil kolorów
gallery-facet-date = Data wykonania
gallery-facet-picked = Wybrane
gallery-facet-rejected = Odrzucone
gallery-facet-flagged = Wybrane lub odrzucone
gallery-facet-rated = Ocenione
gallery-facet-labelled = Z etykietą

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Głębia bitowa
gallery-facet-pages = Strony
gallery-facet-width = Szerokość
gallery-facet-height = Wysokość
gallery-facet-megapixels = Megapiksele
gallery-picker-bits = { $bits }-bitowa
gallery-picker-pages = { $pages ->
    [one] { $pages } strona
    [few] { $pages } strony
    [many] { $pages } stron
   *[other] { $pages } strony
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } Mpx
gallery-picker-group-any = Dowolny w grupie
gallery-picker-group-keep = Sugestia: zachowaj
gallery-picker-group-discard = Sugestia: odrzuć
gallery-picker-ops = Porównaj
gallery-picker-op-eq = Równe
gallery-picker-op-ge = Co najmniej
gallery-picker-op-le = Co najwyżej
gallery-picker-ge = co najmniej { $value }
gallery-picker-gt = więcej niż { $value }
gallery-picker-le = co najwyżej { $value }
gallery-picker-lt = mniej niż { $value }

# Map view.
gallery-facet-place = Miejsce
gallery-palette-map = Mapa
map-title = Mapa
map-summary = Zdjęcia z lokalizacją: { $count }
map-photos = Zdjęcia: { $count }
map-zoom-in = Powiększ
map-zoom-out = Pomniejsz
map-show-all = Pokaż wszystko
map-done = Gotowe
map-credit = Dane mapy: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Mapa: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Kafelki mapy online
map-basemap-consent-server = Kafelki mapy pochodzą z { $host }.
map-basemap-consent-sees = Ten serwer widzi Twój adres internetowy i oglądane obszary.
map-basemap-consent-private = Twoje zdjęcia, ich nazwy i lokalizacje nie są wysyłane.
map-basemap-consent-cache = Kafelki są przechowywane na tym komputerze, do { $size }.
map-basemap-allow = Użyj mapy online
map-basemap-decline = Zostaw mapę offline
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Mapa podkładowa nie jest wyświetlana: { $reason }
map-basemap-bad-address = adres serwera kafelków musi zawierać {"{"}z{"}"}, {"{"}x{"}"} i {"{"}y{"}"}.
map-basemap-unknown-tiles = kafelki w pliku mapy są nieznanego rodzaju.
map-loading = Odczytywanie lokalizacji zdjęć…
map-empty = Żadne z tych zdjęć nie ma lokalizacji
map-hidden = Lokalizacje zdjęć są ukryte (Ustawienia › Metadane i prywatność)
map-a11y-group = Mapa lokalizacji zdjęć
map-a11y-status = Pokazane miejsca: { $count }, poziom powiększenia { $zoom }
