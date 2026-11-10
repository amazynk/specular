# machine-drafted, needs native review
# Odtwarzanie animacji i tryb czytania (archiwa, komiksy). Polski.

disabled-not-animated = Ten obraz nie jest animowany
disabled-in-archive = Stron w archiwum nie można przenieść do kosza

## Paleta poleceń i ściągawka

palette-play-pause = Odtwórz lub wstrzymaj animację
palette-next-frame = Następna klatka
palette-prev-frame = Poprzednia klatka
palette-faster = Odtwarzaj szybciej
palette-slower = Odtwarzaj wolniej
palette-normal-speed = Normalna prędkość odtwarzania
palette-loop-forever = Zapętlaj animację bez końca
palette-right-to-left = Kolejność stron od prawej do lewej
# machine-drafted, needs native review
palette-contents = Spis treści
palette-spreads = Rozkładówki dwustronicowe

## Sterowanie animacją (pasek nad paskiem narzędzi)

# Nazwa dostępności grupy przycisków odtwarzania.
transport-region = Sterowanie animacją
transport-play = Odtwórz
transport-pause = Wstrzymaj
transport-prev-frame = Poprzednia klatka
transport-next-frame = Następna klatka
transport-slower = Wolniej
transport-faster = Szybciej
# $index to wyświetlana klatka, $count to liczba klatek.
transport-frame = Klatka { $index } / { $count }
# Liczba klatek nie jest znana, dopóki animacja nie zostanie odtworzona raz.
transport-frame-unknown = Klatka { $index }
# $speed to sformatowana liczba, np. 1,5.
transport-speed = { $speed }×
transport-speed-name = Prędkość odtwarzania { $speed }×
# machine-drafted, needs native review
motion-badge = Ruch
motion-badge-live = Live
motion-play = Odtwórz klip ruchomy
motion-play-live = Odtwórz Live Photo
motion-stop = Wróć do zdjęcia
toast-motion-failed = Nie można odtworzyć klipu ruchomego: { $reason }

## Powiadomienia

toast-speed = Prędkość odtwarzania { $speed }×
toast-loop-forever-on = Zapętlanie bez końca
toast-loop-forever-off = Zapętlanie zgodnie z plikiem
toast-right-to-left-on = Strony od prawej do lewej
toast-right-to-left-off = Strony od lewej do prawej
toast-spreads-on = Rozkładówki dwustronicowe
toast-spreads-off = Po jednej stronie

## Archiwa

# Tytuł okna dla strony archiwum.
title-archive-page = { $page } — { $archive }
# Odczyt pozycji, gdy pokazane są dwie strony.
readout-position-spread = { $first }–{ $last } / { $count }
# $format to RAR.
archive-needs-sandbox = Archiwa { $format } może otwierać tylko odizolowany proces dekodera, którego ta wersja jeszcze nie zawiera.
archive-empty = To archiwum nie zawiera obrazów.
archive-failed = Nie można otworzyć archiwum: { $error }
archive-encrypted = To archiwum jest zaszyfrowane. Otwórz je ponownie, aby wpisać hasło.
archive-wrong-password = Hasło do tego archiwum jest nieprawidłowe.
password-title = Wymagane hasło
password-archive = Archiwum „{ $archive }” jest zaszyfrowane.
password-label = Hasło
password-show = Pokaż hasło
password-wrong = Nieprawidłowe hasło. Spróbuj ponownie.
password-open = Otwórz
password-cancel = Anuluj

## Ustawienia

settings-viewing-animation-autoplay = Odtwarzaj animacje automatycznie
settings-viewing-animation-autoplay-help = Uruchamiaj animowane obrazy (GIF, APNG, WebP…) od razu po otwarciu.
settings-viewing-animation-loop = Zapętlanie animacji
settings-viewing-animation-loop-help = Ile razy animacje się powtarzają.
settings-viewing-animation-loop-file = Zgodnie z plikiem
settings-viewing-animation-loop-forever = Bez końca
settings-viewing-animation-loop-once = Odtwórz raz
settings-viewing-animation-short-delays = Bardzo krótkie opóźnienia klatek
settings-viewing-animation-short-delays-help = Klatki trwające 10 ms lub krócej: spowolnij do 100 ms jak przeglądarki internetowe albo odtwarzaj tak, jak zapisano.
settings-viewing-animation-short-delays-browser = Jak przeglądarki (100 ms)
settings-viewing-animation-short-delays-as-stored = Jak zapisano
settings-browsing-reading-order = Kolejność stron w archiwach
settings-browsing-reading-order-help = Archiwa komiksów czyta się od prawej do lewej, gdy tak podaje ich ComicInfo.xml (manga), chyba że ustawiono to tutaj.
settings-browsing-reading-order-auto = Z archiwum
settings-browsing-reading-order-left-to-right = Od lewej do prawej
settings-browsing-reading-order-right-to-left = Od prawej do lewej
settings-browsing-page-layout = Strony w archiwach
settings-browsing-page-layout-help = Pokazuj jedną stronę lub dwie obok siebie; okładka jest zawsze pokazywana osobno.
settings-browsing-page-layout-auto = Z archiwum
settings-browsing-page-layout-single = Jedna strona
settings-browsing-page-layout-spreads = Rozkładówki dwustronicowe

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Następna strona
palette-prev-page = Poprzednia strona
palette-first-page = Pierwsza strona
palette-last-page = Ostatnia strona
disabled-one-page = Obraz ma tylko jedną stronę
page-indicator = Strona { $index } z { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Warstwa { $index }
page-label-face = Ściana { $face }
page-label-sphere = Mapa sferyczna
page-label-slice = Przekrój { $index }
page-label-join = { $a } · { $b }
page-label-slide = Preparat
page-label-slide-label = Etykieta
page-label-slide-macro = Przegląd
page-label-slide-thumbnail = Miniatura
# machine-drafted, needs native review
page-label-thermal = Termogram
page-label-visible = Zdjęcie w świetle widzialnym
# machine-drafted, needs native review
page-label-left-view = Widok lewy
# machine-drafted, needs native review
page-label-right-view = Widok prawy
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } z { $count }
page-label-t = T { $index } z { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Kanały domyślne
page-label-product-bands = Wszystkie kanały
page-label-product-true-colour = Barwy naturalne
page-label-product-false-colour = Barwy fałszywe (bliska podczerwień)
page-label-product-swir = Podczerwień krótkofalowa
page-label-product-agriculture = Rolnictwo
page-label-product-scene-class = Klasyfikacja sceny
page-label-product-cloud = Prawdopodobieństwo zachmurzenia
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Strona
info-page-position = Pozycja
info-page-contents = Zawartość

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO nie jest używane: { $reason }
ocio-builtin-config = $OCIO wskazuje wbudowaną konfigurację (ocio://); zapisz ją jako plik i wybierz ten plik.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits }-bitowy

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Obraz scalony
page-label-from-layers = Złożone z warstw
page-label-layer-hidden = { $name } (ukryta)
page-label-layer-unnamed = Warstwa bez nazwy { $index }
