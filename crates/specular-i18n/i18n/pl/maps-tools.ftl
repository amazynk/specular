# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# Polski.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Teren
cheatsheet-map-time = Czas i pogoda
cheatsheet-map-online = Usługi online
cheatsheet-map-photos = Zdjęcia i GPS
cheatsheet-map-output = Wydruk i eksport

## Palette entries and their one-line descriptions.
palette-map-slope = Kolory nachylenia
palette-map-slope-help = Pokoloruj model wysokości według stromizny.
palette-map-aspect = Kolory ekspozycji
palette-map-aspect-help = Pokoloruj model wysokości według kierunku, w który zwrócone są stoki.
palette-map-contours = Pokaż lub ukryj poziomice
palette-map-contours-help = Poziomice na modelu wysokości, opisane co kilka linii.
palette-map-terrain-3d = Teren w 3D
palette-map-terrain-3d-help = Pokaż model wysokości jako powierzchnię 3D z nałożonymi kolorami.
palette-map-time = Pokaż lub ukryj suwak czasu
palette-map-time-help = Przechodź przez czasy siatki lub wzdłuż czasów śladu.
palette-map-play = Odtwórz lub wstrzymaj czas
palette-map-play-help = Przesuwaj suwak czasu samoczynnie, krok po kroku.
palette-map-wind = Pokaż lub ukryj strzałki wiatru
palette-map-wind-help = Strzałki prędkości i kierunku wiatru ze składowych u i v siatki.
palette-map-weather-colours = Następna paleta pogodowa
palette-map-weather-colours-help = Palety kolorów dla temperatury, opadów, wiatru i ciśnienia.
palette-map-services = Usługi map online…
palette-map-services-help = Dodaj warstwy z serwerów WMS, WMTS, WFS, OGC API lub STAC.
palette-map-photos-on-track = Pokaż lub ukryj zdjęcia na śladzie
palette-map-photos-on-track-help = Umieść zdjęcia z folderu wzdłuż śladu GPS według czasu wykonania.
palette-map-geotag = Geotaguj zdjęcia ze śladu…
palette-map-geotag-help = Zapisz położenia ze śladu w plikach XMP obok zdjęć z folderu.
palette-map-export = Eksportuj mapę…
palette-map-export-help = Zapisz mapę jako PNG lub PDF w skali, z legendą, podziałką i strzałką północy.
palette-map-print = Drukuj mapę w skali…
palette-map-print-help = Wydrukuj mapę w wybranej skali, np. 1:{ NUMBER(25000) }.
palette-map-copy-extent = Kopiuj zasięg
palette-map-copy-extent-help = Skopiuj obszar na ekranie jako prostokąt ograniczający lub jako WKT.

## Why a map command is unavailable.
disabled-no-time = Mapa nie ma czasów
disabled-no-wind = Mapa nie ma składowych wiatru (u i v)
disabled-no-track = Brak śladu GPS z czasami
disabled-not-grid = Mapa nie jest siatką wartości

## Settings › Maps and Metadata.
settings-maps-services = Usługi map online
settings-maps-services-help = Dodawaj warstwy z serwerów WMS, WMTS, WFS, OGC API i STAC. Wyłączone: nic nie jest pobierane.
settings-maps-services-allowed = Dozwolone serwery map
settings-maps-services-allowed-help = Serwery zaakceptowane po wyjaśnieniu, co widzą.
settings-metadata-basemap-service = Warstwa z usługi map
settings-metadata-basemap-service-help = Warstwa wybrana w panelu usług online mapy.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NE
    [e] E
    [se] SE
    [s] S
    [sw] SW
    [w] W
    [nw] NW
   *[n] N
}
map-slope-aspect = Nachylenie { $slope }° · ekspozycja { $direction } ({ $aspect }°)
map-slope-flat = Nachylenie { $slope }° · płasko
map-slope-on = Kolory nachylenia
map-aspect-on = Kolory ekspozycji
map-relief-off = Kolory wysokości
map-contours-on = Poziomice włączone
map-contours-off = Poziomice wyłączone
map-legend-slope = Nachylenie
map-legend-aspect = Ekspozycja: kierunek stoków
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° i więcej
map-terrain-3d-on = Teren w 3D. Esc wraca do mapy.
map-terrain-3d-on-exaggerated = Teren w 3D, wysokości × { $times }. Esc wraca do mapy.
map-terrain-exaggeration = Wysokości przewyższone { $times } razy.
map-terrain-drape = Kolory mapy

## Time, weather and wind.
map-time-grid = { $when } · { $step } z { $steps }
map-time-track = { $when } · { $elapsed } od startu
map-time-speed = × { $times }
map-time-play = Odtwórz
map-time-pause = Wstrzymaj
map-weather-map = Kolory: { $name }
map-weather-file = Kolory z pliku
map-wind-on = Strzałki wiatru włączone
map-wind-off = Strzałki wiatru wyłączone
map-wind-at = Wiatr { $speed } { $unit } z { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Skopiowano prostokąt ograniczający. Skopiuj ponownie, aby uzyskać WKT.
map-extent-copied-wkt = Skopiowano zasięg jako WKT

## Online map services.
map-services-title = Usługi map online
map-service-off = Usługi map online są wyłączone.
map-service-off-why = Włącz je, aby dodawać warstwy z serwerów w internecie.
map-service-turn-on = Włącz
map-service-close = Zamknij
map-service-back = Wstecz
map-service-cancel = Anuluj
map-service-connect = Połącz
map-service-allow = Zezwól
map-service-remove = Usuń
map-service-download = Pobierz
map-service-list-hint = Enter otwiera serwer. Delete go usuwa.
map-service-add = Dodaj serwer…
map-service-add-sub = WMS, WMTS, WFS, OGC API lub STAC
map-service-add-hint = Adres serwera. Enter łączy.
map-service-bad-address = To nie jest adres internetowy (http lub https, bez nazwy użytkownika).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Usługa map online
map-service-consent-server = Specular połączy się z { $host }.
map-service-consent-sees = Ten serwer widzi Twój adres internetowy i obszar mapy, o który prosisz.
map-service-consent-private = Twoje pliki i ich nazwy nie są wysyłane.
map-service-asking = Odpytywanie { $host }…
map-service-downloading = Pobieranie z { $host }…
map-service-layers-hint = Warstwy { $kind }. Enter używa jednej.
map-service-no-layers = Serwer nie podaje żadnych warstw.
map-service-offer-tiles = Kafelki: staje się mapą podkładową mapy zdjęć
map-service-offer-features = Obiekty: dodawane do tej mapy dla obszaru na ekranie
map-service-offer-search = Obrazy: szukaj w obszarze na ekranie
map-service-refused-mercator = Niedostępne w Web Mercator (EPSG:3857), którego wymaga mapa podkładowa.
map-service-refused-format = Niedostępne jako obrazy PNG lub JPEG.
map-service-refused-geojson = Niedostępne jako GeoJSON, format odczytywany tutaj.
map-service-items-hint = Znalezione obrazy: { $count }. Enter pobiera jeden.
map-service-no-items = Nie znaleziono tu obrazów.
map-service-item-cloud = { $when } · zachmurzenie { PERCENT($cloud) }
map-service-item-no-cog = Brak Cloud-Optimized GeoTIFF
map-service-download-ask = Pobrać { $size } z { $host }?
map-service-download-ask-unknown = Pobrać ten obraz z { $host }? Jego rozmiar nie jest znany.
map-service-download-cache = Zostanie zapisany w pamięci podręcznej map i otwarty.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = Obraz ma { $size }, za dużo do pobrania.
map-service-opened = Otwieranie pobranego obrazu
map-service-basemap = { $layer } jest teraz mapą podkładową mapy zdjęć. Mapa zdjęć zapyta przed pierwszym pobraniem.
map-service-status = Serwer odpowiedział { $status }.
map-service-unknown = To nie jest usługa map obsługiwana przez tę wersję.
map-service-exception = Serwer zgłosił: { $message }
map-service-malformed = Odpowiedź serwera jest uszkodzona: { $reason }
map-service-remove-ask = Usunąć „{ $server }” z zapisanych serwerów?
map-service-remove-detail = Nic na dysku nie zostanie utracone. Warstwy już na mapie zostaną do jej zamknięcia.
map-service-removed = Usunięto { $server }
map-service-saved = Zapisane serwery map
map-service-saved-none = Jeszcze brak. Dodaj serwer w panelu usług online mapy.
map-service-keywords = WMS WMTS WFS OGC API STAC serwery usługi mapy

## Photos and geotagging.
map-geotag-title = Geotaguj zdjęcia ze śladu
map-photos-on = Zdjęcia na śladzie
map-photos-off = Zdjęcia na śladzie ukryte
map-geotag-offset = Zegar aparatu: { $offset } względem czasu GPS. ← → zmienia o minutę, z Shift o godzinę, z Alt o sekundę.
map-geotag-reading = Odczytywanie zdjęć…
map-geotag-summary = Położenie otrzyma { $count } z { $total } zdjęć.
map-geotag-sidecars = Położenia trafiają do plików XMP obok zdjęć. Pliki zdjęć nie są zmieniane.
map-geotag-writing = Zapisywanie…
map-geotag-write = Zapisz pliki XMP…
map-geotag-guess = Odgadnij przesunięcie
map-geotag-will-write = Otrzyma { $position }
map-geotag-has-position = Ma już położenie: bez zmian
map-geotag-no-time = Brak czasu wykonania: bez zmian
map-geotag-outside = Wykonane { $offset } poza śladem: bez zmian
map-geotag-gap = W przerwie śladu ({ $seconds } s od pozycji): bez zmian
map-geotag-nothing = Przy tym przesunięciu żadne zdjęcie nie otrzyma położenia.
map-geotag-confirm = Zapisać położenia w { $count ->
    [one] { $count } pliku XMP
    [few] { $count } plikach XMP
    [many] { $count } plikach XMP
   *[other] { $count } pliku XMP
}? Same zdjęcia nie są zmieniane.
map-geotag-confirm-ok = Zapisz pliki XMP
map-geotag-done = { $count ->
    [one] Zapisano { $count } plik XMP
    [few] Zapisano { $count } pliki XMP
    [many] Zapisano { $count } plików XMP
   *[other] Zapisano { $count } pliku XMP
}
map-geotag-done-failed = Zapisane pliki XMP: { $count }; nieudane: { $failed }: { $reason }
map-geotag-not-sidecar = To nie jest nazwa pliku sidecar.

## Export and printing.
map-export-title = Eksportuj mapę
map-print-title = Drukuj mapę w skali
map-export-keys = ↑ ↓ wybiera, ← → zmienia, Enter wykonuje.
map-export-format = Format
map-export-scale = Skala
map-export-scale-value = 1:{ $n }
map-export-resolution = Rozdzielczość
map-export-dpi = { $dpi } dpi
map-export-paper = Papier
map-export-size = { $width } × { $height } px · { $across } × { $down } km w terenie
map-export-save = Zapisz…
map-export-print = Drukuj…
map-export-too-large = Za duże w tej skali: { $width } × { $height } px. Wybierz mniejszą skalę lub rozdzielczość.
map-export-no-scale = Ta mapa nie ma skali w terenie.
map-export-no-gpu = Eksport wymaga procesora graficznego.
map-export-subtitle = Skala { $scale } · { $crs }
map-export-data = Dane: { $sources }
map-export-saved = Zapisano { $name }
map-export-printing = Drukowanie…
map-export-printed = Wysłano do drukarki
map-export-print-failed = Nie wydrukowano: { $reason }
