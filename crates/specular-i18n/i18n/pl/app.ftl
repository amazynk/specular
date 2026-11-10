# machine-drafted, needs native review
# Specular — interfejs przeglądarki, paleta poleceń, błędy, okno „O programie”. Polski.

-app-name = Specular

## Podpowiedzi i etykiety dostępności paska narzędzi

toolbar-open = Otwórz obraz…
toolbar-prev = Poprzedni obraz
toolbar-next = Następny obraz
toolbar-back-gallery = Powrót do galerii
toolbar-zoom-out = Pomniejsz
toolbar-zoom-in = Powiększ
toolbar-zoom-level = Poziom powiększenia i ustawienia
toolbar-zoom-fit = Dopasuj do okna
toolbar-zoom-100 = Rzeczywisty rozmiar ({ PERCENT(100) })
toolbar-rotate-left = Obróć w lewo
toolbar-rotate-right = Obróć w prawo
toolbar-fullscreen-enter = Pełny ekran
toolbar-fullscreen-exit = Zamknij pełny ekran
toolbar-info = Informacje o obrazie
toolbar-settings = Ustawienia
toolbar-delete = Przenieś do kosza
toolbar-copy = Kopiuj obraz
toolbar-palette = Paleta poleceń
toolbar-go-to = Przejdź do obrazu…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Dostosuj pasek narzędzi…
toolbar-more = Więcej
toolbar-announce-moved = { $item }: pozycja { $index } z { $count }
toolbar-announce-added = Dodano { $item }: pozycja { $index } z { $count }
toolbar-announce-removed = Usunięto z paska narzędzi: { $item }
toolbar-announce-reset = Przywrócono domyślny pasek narzędzi
# Etykieta podpowiedzi, po której następuje skrót klawiszowy.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Podpowiedź kontrolki, której nie można teraz użyć; $reason to jeden z komunikatów disabled-*.
tooltip-unavailable = { $label } (niedostępne: { $reason })
# Dlaczego kontrolka lub polecenie jest niedostępne.
disabled-no-image = Nie otwarto obrazu
disabled-still-loading = Obraz jest jeszcze wczytywany
disabled-decode-failed = Nie można zdekodować tego obrazu
disabled-no-other-images = Brak innych obrazów w tym folderze

## Odczyty

# Pozycja w folderze, np. „12 / 348”.
readout-position = { $index } / { $count }
# Odczytywane przez czytniki ekranu.
readout-position-a11y = Obraz { $index } z { $count }
# $zoom to już sformatowana wartość procentowa, np. „150%”.
readout-zoom = { $zoom }
readout-zoom-a11y = Powiększenie { $zoom }
readout-zoom-fit = Dopasowanie ({ $zoom })
zoom-preset-fit = Dopasuj do okna
zoom-preset-fill = Wypełnij okno
zoom-preset-actual = Rzeczywisty rozmiar
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Pełny ekran
# $zoom to już sformatowana wartość procentowa.
zoom-preset-percent = { $zoom }
# Wymiary obrazu w pikselach, np. „6000 × 4000”.
readout-dimensions = { $width } × { $height }
# Liczba obrazów w bieżącym folderze.
status-image-count =
    { $count ->
        [one] { $count } obraz
        [few] { $count } obrazy
        [many] { $count } obrazów
       *[other] { $count } obrazu
    }

## Paleta poleceń

palette-placeholder = Wpisz polecenie lub ustawienie…
palette-no-results = Brak pasujących poleceń
# Zmiana ustawienia z palety, np. „Ustaw: Kółko myszy → Następny obraz”.
palette-set-setting = Ustaw: { $setting } → { $value }
palette-next = Następny obraz
palette-prev = Poprzedni obraz
palette-first = Pierwszy obraz
palette-last = Ostatni obraz
palette-go-to = Przejdź do obrazu…
palette-zoom-in = Powiększ
palette-zoom-out = Pomniejsz
palette-zoom-fit = Dopasuj do okna
palette-zoom-fill = Wypełnij okno
# machine-drafted, needs native review
palette-zoom-fit-width = Dopasuj do szerokości
palette-zoom-fit-height = Dopasuj do wysokości
palette-zoom-100 = Powiększenie { PERCENT(100) }
palette-zoom-200 = Powiększenie { PERCENT(200) }
palette-zoom-300 = Powiększenie { PERCENT(300) }
palette-zoom-400 = Powiększenie { PERCENT(400) }
palette-zoom-rectangle = Powiększ zaznaczenie
palette-rotate-left = Obróć w lewo
palette-rotate-right = Obróć w prawo
palette-flip-horizontal = Odbij w poziomie
palette-flip-vertical = Odbij w pionie
palette-toggle-fullscreen = Przełącz pełny ekran
palette-navigator = Pokaż lub ukryj nawigator
palette-loupe = Lupa
palette-pixel-grid = Pokaż lub ukryj siatkę pikseli
palette-info-panel = Pokaż lub ukryj panel informacji
palette-copy-image = Kopiuj obraz
# machine-drafted, needs native review
palette-read-codes = Odczytaj kody
# machine-drafted, needs native review
palette-find-text = Znajdź tekst
# machine-drafted, needs native review
palette-face-boxes = Pokaż twarze
palette-copy-path = Kopiuj ścieżkę pliku
palette-move-to-trash = Przenieś do kosza
palette-undo-trash = Cofnij przeniesienie do kosza
palette-open = Otwórz obraz…
# machine-drafted, needs native review
palette-open-address = Otwórz adres…
palette-settings = Ustawienia
# machine-drafted, needs native review
palette-customise-toolbar = Dostosuj pasek narzędzi…
palette-cheat-sheet = Skróty klawiszowe
palette-hud = Pokaż lub ukryj wskaźnik czasu klatki
palette-language = Język
palette-theme = Motyw
# machine-drafted, needs native review
palette-interface-larger = Większy interfejs
palette-interface-smaller = Mniejszy interfejs
palette-interface-reset = Interfejs { PERCENT(100) }
palette-quit = Zakończ { -app-name }

## Ściągawka

cheatsheet-title = Skróty klawiszowe
cheatsheet-navigation = Nawigacja
cheatsheet-zoom-pan = Powiększanie i przesuwanie
cheatsheet-view = Widok
cheatsheet-file = Plik
cheatsheet-window = Okno
cheatsheet-help = Pomoc
cheatsheet-hold = Przytrzymaj { $key }
cheatsheet-close-hint = Naciśnij { $key } lub Esc, aby zamknąć

## Błędy

# machine-drafted, needs native review
viewer-no-preview = Brak podglądu w tym pliku.
error-cannot-open = Nie można otworzyć { $file }
error-cannot-open-reason = Nie można otworzyć { $file }: { $reason }
error-unsupported-format = { $file } ma format, którego { -app-name } jeszcze nie obsługuje.
error-file-truncated = Plik { $file } jest niekompletny. Wyświetlono część, którą udało się odczytać.
error-file-too-large = Plik { $file } jest za duży ({ $megapixels } Mpx; limit to { $limit } Mpx).
error-not-found = Plik { $file } już nie istnieje.
error-decoder-stopped = dekoder zakończył pracę bez utworzenia obrazu
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = potrzebna jest powierzchnia mózgu (plik .surf.gii z { $vertices } wierzchołkami) w tym samym folderze
error-permission-denied = Brak uprawnień do odczytu { $file }.
error-details = Szczegóły
crash-last-time = { -app-name } został ostatnio nieoczekiwanie zamknięty; raport zapisano w { $path }
error-trash-failed = Nie można przenieść { $file } do kosza: { $reason }
# machine-drafted, needs native review
# Przyczyny, wyświetlane po „Nie można otworzyć PLIK:” i innych komunikatach „Nie można …:”,
# dlatego zaczynają się małą literą.
error-reason-not-found = pliku już tam nie ma. Mógł zostać przeniesiony, zmieniony lub usunięty.
error-reason-permission = nie masz uprawnień do jego odczytu. Sprawdź właściciela i uprawnienia.
error-reason-disk-full = dysk jest pełny. Zwolnij trochę miejsca i spróbuj ponownie.
error-reason-read-only = dysk jest tylko do odczytu.
error-reason-truncated = plik jest niekompletny. Może nadal być kopiowany lub pobierany.
error-reason-is-folder = to folder, a nie plik.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = serwer odmówił dostępu. Dane mogą być prywatne; dla zasobnika S3 najpierw skonfiguruj poświadczenia AWS.
error-reason-remote-missing = pod tym adresem nie ma danych Zarr.
# $status is an HTTP status code ("500").
error-reason-remote-status = serwer odpowiedział błędem { $status }.
error-reason-offline = nie można połączyć się z serwerem. Sprawdź adres i połączenie.
# machine-drafted, needs native review
error-reason-network-off = sieć jest wyłączona w Ustawienia › Prywatność.
error-reason-timed-out = dysk lub serwer nie odpowiedział na czas. Sprawdź połączenie i spróbuj ponownie.
error-reason-memory = brakuje pamięci. Zamknij inne obrazy lub aplikacje i spróbuj ponownie.
error-reason-unsupported = jest w formacie, którego { -app-name } jeszcze nie odczytuje.
# $detail to słowa samego dekodera, techniczne i po angielsku („12-bit lossless JPEG”).
error-reason-unsupported-detail = używa funkcji, której { -app-name } jeszcze nie odczytuje ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = nie jest żadnym z plików .{ $ext }, które odczytuje { -app-name } ({ $formats }).
error-reason-damaged = plik jest uszkodzony lub nie jest tym, na co wskazuje nazwa ({ $detail }).
error-reason-too-large = jest za duży, by go otworzyć.
# machine-drafted, needs native review
error-reason-needs-memory = wymaga { $needed } wolnej pamięci RAM, a teraz wolne jest tylko { $free }. Zamknij inne aplikacje i spróbuj ponownie albo otwórz go na komputerze z większą ilością pamięci.
error-needs-memory = Ten obraz wymaga { $needed } wolnej pamięci RAM, a teraz wolne jest tylko { $free }. Zamknij inne aplikacje i spróbuj ponownie albo otwórz go na komputerze z większą ilością pamięci.
error-needs-memory-short = Wymaga { $needed } wolnej RAM
error-reason-cancelled = otwieranie zostało przerwane.
error-reason-trash-access = pliku już tam nie ma lub nie masz uprawnień do jego zmiany.
error-reason-restore-collision = w jego folderze jest już plik o tej samej nazwie.
error-reason-trash-root = całego dysku nie można przenieść do kosza.
error-reason-not-in-trash = nie ma go już w koszu.
error-reason-program-missing = program nie jest zainstalowany lub został przeniesiony. Wybierz go ponownie lub zainstaluj od nowa.
error-reason-program-not-allowed = program nie ma zezwolenia na uruchomienie. Sprawdź, czy jest wykonywalny.
# machine-drafted, needs native review
error-reason-permission-write = nie masz uprawnień do jego zmiany. Sprawdź właściciela i uprawnienia.
error-reason-file-has-errors = zawiera błędy. Najpierw je popraw, aby nic w nim nie zostało utracone.
error-reason-not-dicom = to nie jest plik DICOM.
error-reason-no-image = nie zawiera obrazu.
error-reason-needs-layout = to wolumen bez nagłówka. Do jego otwarcia potrzebne są rozmiar, typ próbek i kolejność bajtów.
error-reason-not-archive = to nie jest archiwum, które { -app-name } potrafi odczytać.
error-reason-needs-sandbox = ten rodzaj archiwum otwiera tylko chroniony czytnik { -app-name }, który jest niedostępny.
error-reason-encrypted = jest zaszyfrowane i wymaga hasła.
error-reason-wrong-password = hasło jest nieprawidłowe.
error-reason-checksum = plik jest uszkodzony: jego zawartość nie zgadza się z sumą kontrolną.
error-reason-missing-entry = brakuje pliku w jego wnętrzu.
error-reason-busy = używa go inny program. Zamknij go tam i spróbuj ponownie.
error-reason-catalogue-damaged = plik katalogu jest uszkodzony.
error-reason-cannot-open = nie można otworzyć jego pliku. Sprawdź, czy jego folder istnieje i czy możesz w nim zapisywać.
error-reason-newer-version = został zapisany przez nowszą wersję { -app-name }. Zaktualizuj { -app-name }, aby go zmienić.
error-reason-internal = coś poszło nie tak wewnątrz { -app-name }. Szczegóły są w dzienniku.
error-reason-volume-too-few = { $count ->
    [one] jeden przekrój to za mało, by zbudować wolumen.
    [few] { $count } przekroje to za mało, by zbudować wolumen.
    [many] { $count } przekrojów to za mało, by zbudować wolumen.
   *[other] { $count } przekroju to za mało, by zbudować wolumen.
}
error-reason-volume-no-geometry = przekrój nie podaje, gdzie leży ani w którą stronę jest zwrócony.
error-reason-volume-not-parallel = przekroje nie są równoległe.
error-reason-volume-shared-position = kilka obrazów dzieli każdą z pozycji (fazy, echa lub seria czasowa); pozycje: { $count }.
error-reason-volume-missing = brakuje przekrojów, około: { $count }.
error-reason-volume-mismatch = przekroje różnią się rozmiarem lub odstępem pikseli.
error-reason-volume-not-grey = przekroje są kolorowe, a nie w odcieniach szarości.
error-reason-volume-skewed = osie wolumenu nie są prostopadłe.
error-reason-volume-too-large = potrzebuje { $need } MiB, a dozwolone jest tylko { $budget } MiB.
error-reason-url-not-specular = to nie jest link { -app-name }.
error-reason-url-too-long = jest za długi.
error-reason-url-damaged = jest uszkodzony. Mógł zostać obcięty lub zmieniony podczas kopiowania.
error-reason-url-unknown = prosi o coś, czego ta wersja { -app-name } nie zna. Aktualizacja { -app-name } może pomóc.
error-reason-url-not-absolute = ścieżki plików muszą być pełne.
error-reason-url-no-paths = nie wskazuje żadnego pliku.
error-reason-url-too-many = wskazuje za dużo plików.

## Powiadomienia

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Nie udało się dokończyć: { $reason }
toast-moved-to-trash = Przeniesiono do kosza
toast-moved-to-trash-named = Przeniesiono { $file } do kosza
toast-moved-to-trash-count =
    { $count ->
        [one] Przeniesiono { $count } obraz do kosza
        [few] Przeniesiono { $count } obrazy do kosza
        [many] Przeniesiono { $count } obrazów do kosza
       *[other] Przeniesiono { $count } obrazu do kosza
    }
toast-undo = Cofnij
toast-restored = Przywrócono { $file }
toast-copied-image = Skopiowano obraz
toast-copied-path = Skopiowano ścieżkę
toast-copied-report = Skopiowano raport
toast-copied-value = Skopiowano wartość
toast-loop-start = Powrót do pierwszego obrazu
toast-loop-end = Przejście do ostatniego obrazu
# machine-drafted, needs native review
toast-interface-size = Rozmiar interfejsu { PERCENT($percent) }
toast-language-changed = Zmieniono język na { $language }

## Stany puste

empty-drop = Upuść tu obraz lub naciśnij { $shortcut }
empty-open-button = Otwórz obraz…
empty-drop-release = Zwolnij, aby otworzyć
empty-folder = Ten folder nie zawiera obrazów, które { -app-name } może otworzyć.
# machine-drafted, needs native review
# Pod „Otwórz obraz…” w pustym oknie: gdzie znaleźć całą resztę.
empty-hint-keys = Naciśnij { $cheatsheet }, aby zobaczyć skróty klawiszowe, lub { $palette }, aby znaleźć dowolne polecenie.
empty-hint-cheatsheet = Naciśnij { $cheatsheet }, aby zobaczyć skróty klawiszowe.
empty-hint-palette = Naciśnij { $palette }, aby znaleźć dowolne polecenie.
# Pod wskaźnikiem ładowania, gdy otwieranie pliku trwa dłużej niż chwilę.
viewer-opening = Otwieranie { $file }…
fullscreen-exit-hint = Naciśnij Esc, aby zamknąć pełny ekran
# Pokazywane po wejściu w pełny ekran; Esc najpierw resetuje powiększenie.
fullscreen-back-hint = Naciśnij Esc, aby zresetować powiększenie lub zamknąć pełny ekran
# Ściągawka i paleta: co robi Esc, jeden krok na naciśnięcie.
action-back = Zamknij panel (w oknie dialogowym najpierw krok wstecz), zresetuj powiększenie, wyjdź z pełnego ekranu; w galerii folder wyżej

## Wybór języka

language-picker-title = Język
# machine-drafted, needs native review
language-picker-search = Szukaj języków
# $percent to już sformatowana wartość procentowa, np. „87%”.
language-completeness = Przetłumaczono: { $percent }
language-system-default = Domyślny systemowy ({ $language })
language-pseudo-accented = Pseudojęzyk: akcenty i dłuższy tekst
language-pseudo-rtl = Pseudojęzyk: lustrzany, od prawej do lewej

## Okno „O programie”

about-title = { -app-name } — informacje
about-version = Wersja { $version }
about-tagline = Szybka i wierna przeglądarka obrazów.
about-copyright = © { $year } Autorzy programu { -app-name }
about-license = Udostępniono na licencji MIT License / Apache License 2.0.
about-geonames = Nazwy miejsc z GeoNames (geonames.org), na licencji Creative Commons Uznanie autorstwa 4.0.
about-unicode = Dane regionalne z Unicode CLDR przez ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Profile obiektywów z bazy Lensfun (lensfun.github.io), na licencji Creative Commons Uznanie autorstwa-Na tych samych warunkach 3.0.
about-third-party = Licencje stron trzecich
about-third-party-file = { -app-name } zawiera oprogramowanie i dane stron trzecich na ich własnych licencjach; zob. plik THIRD-PARTY.md instalowany z programem { -app-name }.
about-copy-info = Kopiuj informacje o systemie
about-close = Zamknij

## Interfejs przeglądarki (aplikacja, etap 1)

app-close = Zamknij
# Plakietka na obrazie, którego plik kończy się przedwcześnie.
app-truncated-badge = Niekompletny plik
app-info-summary = Podsumowanie
app-info-all-fields = Wszystkie pola
app-info-copy-value = Kopiuj wartość
app-info-copy-field = Kopiuj wartość pola { $field }
app-info-loading = Odczytywanie metadanych…
# machine-drafted, needs native review
# Wszystkie pola, dla pliku bez metadanych poza pikselami.
app-info-no-fields = Ten plik nie ma pól metadanych.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = Mapa wzmocnienia HDR
app-info-gain-map-headroom = Do { $headroom }× jasności SDR
app-copy-failed = Nie można skopiować: { $reason }
app-restore-failed = Nie można przywrócić { $file }: { $reason }
app-open-dialog-title = Otwórz obraz
app-open-dialog-filter = Obsługiwane pliki
app-open-dialog-all = Wszystkie pliki
app-open-group-images = Obrazy
app-open-group-raw = Pliki RAW z aparatu
app-open-group-video = Wideo
app-open-group-sound = Dźwięk
app-open-group-documents = Dokumenty
app-open-group-drawings = Rysunki i CAD
app-open-group-models = Modele 3D
app-open-group-maps = Mapy i trasy
app-open-group-science = Nauka i medycyna
app-open-group-fonts = Czcionki
app-open-group-other = Inne

## Informacje o renderowaniu (Informacje, Ustawienia › Zaawansowane)

# machine-drafted, needs native review
renderer-title = Informacje o renderowaniu
renderer-section-adapter = Karta graficzna
renderer-section-limits = Limity i funkcje
renderer-section-display = Ekran
renderer-section-memory = Pamięć
renderer-section-decoders = Opcjonalne dekodery
renderer-unknown = Nieznane
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ID producenta i urządzenia
# Wayland, X11, Windows or macOS.
renderer-display-server = Serwer wyświetlania
renderer-features = Używane funkcje
renderer-surface-format = Format okna
renderer-surface-formats = Dostępne formaty
renderer-present-mode = Tryb prezentacji
renderer-present-modes = Dostępne tryby
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Włączone (zapas { $headroom }×)
renderer-hdr-off = Wyłączone (ekran może wyświetlać HDR)
renderer-hdr-unavailable = Niedostępne na tym ekranie
renderer-decode-budget = Dekodowanie może użyć
renderer-decode-budget-detail = { PERCENT(80) } wolnej pamięci, współdzielone przez wszystkie otwierane obrazy.
renderer-display-cache = Pamięć podręczna wyświetlania

## Wskaźnik wydajności (F12)

app-hud-scale = Skala
app-hud-level = Poziom
app-hud-level-thumbnail = Osadzona miniatura
app-hud-level-full = Pełna rozdzielczość
# $denominator to 2, 4 lub 8: obraz zdekodowany w rozmiarze 1/2, 1/4 lub 1/8.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Otwarcie
app-hud-decode = Dekodowanie
app-hud-upload = Przesyłanie
app-hud-frame = Klatka
app-hud-threads = Wątki
app-hud-threads-value = { $threads } (w kolejce: { $queued })
app-hud-tiles = Kafelki
app-hud-tiles-value = { $tiles } z { $capacity }
app-hud-ram = Pamięć
app-hud-vram = Pamięć GPU
app-hud-gpu = GPU

# Potwierdzenie przeniesienia do kosza
app-confirm-trash-title = Przenieść { $file } do kosza?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Pliki towarzyszące też trafią do kosza: { $files }
app-cancel = Anuluj
# Ostrzeżenie przed dekodowaniem bardzo dużego obrazu. $need i $available to rozmiary pamięci.
toast-low-memory = Ten obraz może wymagać ok. { $need } pamięci, a dostępne jest tylko { $available }. Może wczytywać się wolno lub wcale.

# Raport sprzętowy (Ustawienia → Zaawansowane). Wartości uzupełnia aplikacja.
hw-report-title = Raport sprzętowy
hw-report-copy = Kopiuj raport
hw-report-version = Wersja
hw-report-cpu = CPU
hw-report-cores = Rdzenie
hw-report-cores-value = logiczne: { $logical }, fizyczne: { $physical }
hw-report-performance-cores = Rdzenie wydajnościowe
hw-report-cpu-features = Szybkie ścieżki CPU
hw-report-none = Brak
hw-report-memory = Pamięć
hw-report-memory-value = łącznie { $total }, dostępne { $available }
hw-report-graphics-api = Interfejs graficzny
hw-report-driver = Sterownik
hw-report-max-texture = Największa tekstura
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Wątki dekodowania
hw-report-ram-cache = Pamięć podręczna obrazów
hw-report-vram-cache = Pamięć podręczna w pamięci GPU
hw-report-processing = Przetwarzanie
hw-report-path-jpeg-decode = Dekodowanie JPEG
hw-report-path-ycbcr-to-rgb = Konwersja kolorów
hw-report-path-png-unfilter = Dekodowanie PNG
hw-report-path-heic-decode = Dekodowanie HEIC
hw-report-path-inference = Analiza obrazu
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, równolegle
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = dekoder sprzętowy
hw-report-engine-npu = NPU
hw-report-build-debug = kompilacja debugowa
hw-report-build-release = kompilacja wydaniowa

## Dostępność (czytniki ekranu)
# Obraz, jak go ogłasza czytnik ekranu: nazwa pliku i rozmiar w pikselach.
a11y-image = { $name }, { $width } × { $height } pikseli
# Obszar, w którym pojawiają się krótkie powiadomienia.
a11y-notifications = Powiadomienia
# machine-drafted, needs native review
a11y-transport-playback = Odtwarzanie
a11y-transport-pages = Strony
a11y-settings-sections = Sekcje

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Kody na tym obrazie
codes-looking = Szukanie kodów…
codes-none = Na tym obrazie nie znaleziono kodów.
codes-failed = Nie można odczytać obrazu: { $reason }
codes-copy = Kopiuj „{ $text }”
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Otwórz link do { $host }…
codes-network-off = Żadna sieć nie jest włączona, więc linki się nie otwierają.
codes-open-title = Otworzyć link do { $host }?
codes-open-detail = Otworzy się w przeglądarce internetowej. Otwieraj tylko linki, którym ufasz.
codes-open = Otwórz w przeglądarce
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Tekst na tym obrazie
# machine-drafted, needs native review
ocr-looking = Odczytywanie tekstu…
# machine-drafted, needs native review
ocr-none = Na tym obrazie nie znaleziono tekstu.
# machine-drafted, needs native review
ocr-failed = Nie można odczytać tekstu: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Funkcja Znajdź tekst wymaga modeli tekstu z pakietu lokalnej AI (Ustawienia › Lokalna AI).
# machine-drafted, needs native review
ocr-loading = Wczytywanie modeli tekstu…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Kopiuj „{ $text }”
# machine-drafted, needs native review
ocr-copy-all = Kopiuj cały tekst
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Pewność: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = System
hw-report-section-gpu = GPU
hw-report-section-decoding = Dekodowanie
hw-report-operating-system = System operacyjny
hw-report-adapter = Adapter
hw-report-gpu-type = Typ
hw-report-gpu-init = Czas uruchamiania
hw-report-max-layers = Warstwy tekstur
hw-report-compression = Kompresja tekstur
hw-report-unified-memory = Pamięć współdzielona z CPU
hw-report-display = Ekran
hw-report-hw-decode = Dekodowanie sprzętowe
hw-report-core-cluster = Grupa rdzeni
hw-report-power = Zasilanie
hw-report-calibration = Pomiary szybkości
hw-report-codec-others = Inne
hw-report-performance-cores-value = { $count } z { $total }
hw-report-cluster-value = { $name } — fizyczne: { $physical }, logiczne: { $logical }
hw-report-thermal-nominal = Normalna temperatura
hw-report-thermal-fair = Ciepło
hw-report-thermal-serious = Gorąco, spowalnia
hw-report-thermal-critical = Przegrzanie
hw-report-power-battery = Zasilanie z baterii ({ $percent })
hw-report-power-battery-unknown = Zasilanie z baterii
hw-report-power-mains = Zasilanie sieciowe
hw-report-power-mains-percent = Zasilanie sieciowe ({ $percent })
hw-report-power-saving = Oszczędzanie energii włączone
hw-report-gpu-discrete = Dedykowany
hw-report-gpu-integrated = Zintegrowany
hw-report-gpu-virtual = Wirtualny
hw-report-gpu-software = Programowy, na CPU
hw-report-gpu-other = Inny
hw-report-gpu-unavailable = Niedostępny
hw-report-ms = { $ms } ms
hw-report-yes = Tak
hw-report-no = Nie
hw-report-unified-platform-differs = System podaje inaczej
hw-report-display-value = { $fps } Hz, zapas HDR teraz { $now }, maksymalnie { $max }
hw-report-hw-decode-value = { $api } na { $device }
hw-report-hw-decode-checking = Nadal trwa sprawdzanie…
hw-report-codec-up-to = Do { $width } × { $height }
hw-report-codec-none = Niedostępne
hw-report-codec-h264-encode = Kodowanie H.264
hw-report-supported = obsługiwane: { $list }
hw-report-unsupported = nieobsługiwane: { $list }
hw-report-calibration-done = Zmierzone na tym komputerze
hw-report-calibration-pending = Jeszcze niezmierzone
hw-report-paths-power-saving = Oszczędzanie energii: preferowane są ścieżki zużywające mniej energii
hw-report-paths-caption = Ścieżki dla obrazu { $mpx } Mpx
hw-report-col-task = Zadanie
hw-report-col-path = Ścieżka
hw-report-col-time = Czas
hw-report-col-cpu = Czas CPU (wszystkie wątki)
hw-report-col-runs = Pomiary
hw-report-col-use = Użycie
hw-report-runs-untested = Nie testowano
hw-report-use-viewing = Wyświetlanie ({ $reason })
hw-report-use-background = W tle ({ $reason })
hw-report-reason-only = jedyna opcja
hw-report-reason-faster = najszybsza
hw-report-reason-cheaper = najmniej czasu CPU
hw-report-reason-lower-power = najmniej energii
hw-report-reason-explore = w trakcie próby
hw-report-reason-forced = ustawione w Ustawieniach
hw-report-reason-prefer-gpu = Preferuj GPU
hw-report-reason-cpu-only = Tylko CPU
hw-report-reason-calibrate = w trakcie pomiaru
hw-report-reason-scaled = zmniejszony rozmiar
hw-report-trust-unverified = Jeszcze niesprawdzone
hw-report-trust-blocked = Wyłączone po błędzie
hw-report-trust-absent = Brak na tym komputerze

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galeria
cheatsheet-mode-compare = Porównanie
cheatsheet-gallery-browse = Przeglądanie
cheatsheet-gallery-select = Zaznaczanie
cheatsheet-gallery-mark = Ocena i flagi
cheatsheet-gallery-move = Przechodzenie między obrazami
cheatsheet-gallery-page = Strona w górę lub w dół
cheatsheet-gallery-ends = Pierwszy lub ostatni obraz
cheatsheet-gallery-up = W górę do folderu nadrzędnego
cheatsheet-gallery-history-back = Wstecz do poprzedniego folderu
cheatsheet-gallery-history-forward = Dalej do następnego folderu
cheatsheet-gallery-back = Zamknij widok na wierzchu, potem wróć
cheatsheet-gallery-viewer = Powrót do przeglądarki
cheatsheet-gallery-album-add = Dodaj do ostatniego albumu
cheatsheet-gallery-select-one = Zaznacz lub odznacz
cheatsheet-gallery-quick-look = Szybki podgląd
cheatsheet-gallery-filter = Filtruj
cheatsheet-gallery-bigger = Większe miniatury
cheatsheet-gallery-smaller = Mniejsze miniatury
cheatsheet-gallery-default-size = Standardowy rozmiar miniatur
cheatsheet-gallery-refresh = Odśwież folder
cheatsheet-gallery-menu = Menu kontekstowe
cheatsheet-compare-views = Porównanie
cheatsheet-compare-overlay = Nakładka lub obok siebie
cheatsheet-compare-difference = Różnica (ponownie: następny rodzaj)
cheatsheet-compare-step-down = Mniej przenikania lub wzmocnienia różnicy
cheatsheet-compare-step-up = Więcej przenikania lub wzmocnienia różnicy
cheatsheet-compare-pick = Obraz, na który działają klawisze
cheatsheet-compare-close = Zamknij porównanie
cheatsheet-channels = Kanały
cheatsheet-medical-window = Okno i poziom
cheatsheet-medical-measure = Pomiary i adnotacje
cheatsheet-medical-frames = Klatki i serie
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D i rekonstrukcje
cheatsheet-medical-analysis = Segmentacja, fuzja i PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Badania i udostępnianie
cheatsheet-palette-only-title = W palecie poleceń ({ $key })
cheatsheet-palette-only-title-nokey = W palecie poleceń
cheatsheet-palette-only-tag = Paleta
cheatsheet-palette-only-line = { $action }: brak klawisza; w palecie poleceń ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: brak klawisza; w palecie poleceń
# Esc w ściądze (długi opis z palety się nie mieści).
cheatsheet-back = Zamknij lub wróć

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Eksport wideo
hw-report-engine-hw-encoder = koder sprzętowy

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (na miniaturę { $size })
hw-report-task-frame = { $task } (na klatkę { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Tylko programowo

about-natural-earth = Dane mapy z Natural Earth (naturalearthdata.com), domena publiczna.
app-info-show-on-map = Pokaż na mapie
