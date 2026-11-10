# machine-drafted, needs native review
# Specular — okno Ustawień. Polski.
#
# Schemat identyfikatorów (zob. i18n/en/settings.ftl): settings-section-<sekcja>,
# settings-<sekcja>-<klucz>, …-help, …-<wartość>.

## Okno

settings-title = Ustawienia
settings-search-placeholder = Szukaj ustawień
settings-search-no-results = Brak ustawień pasujących do { $query }
settings-reset = Resetuj
settings-reset-section = Resetuj sekcję
settings-modified = Zmienione
settings-default-value = Domyślnie: { $value }
settings-restart-required = Zmiana zadziała po ponownym uruchomieniu
settings-restart-now = Uruchom ponownie
settings-undo = Cofnij
settings-redo = Ponów
settings-value-on = Wł.
settings-value-off = Wył.
settings-value-auto = Automatycznie
settings-value-system = Zgodnie z systemem
settings-value-system-language = Domyślny systemowy
settings-value-follow-language = Zgodnie z językiem interfejsu
settings-value-default-location = Domyślne położenie
settings-choose = Wybierz…
settings-clear = Wyczyść
settings-file-reloaded = Plik settings.toml został zmieniony poza programem { -app-name } i wczytano go ponownie.
settings-warning-unknown-key = Nieznane ustawienie { $key } w settings.toml. Pozostaje w pliku, ale jest ignorowane.
settings-warning-invalid-value = Nieprawidłowa wartość { $key }: { $reason } Zamiast niej używana jest wartość domyślna.
settings-warning-newer-version = Plik settings.toml zapisała nowsza wersja programu { -app-name }. Ustawienia nieznane tej wersji zostają zachowane.
settings-warning-syntax = Nie można odczytać settings.toml (wiersz { $line }): { $reason } Do czasu poprawienia używane są wartości domyślne.

## Błędy walidacji (wypisywane też przez `specular --set`)

settings-error-unknown-key = Nie ma ustawienia o nazwie { $key }.
settings-error-unknown-key-suggestion = Nie ma ustawienia o nazwie { $key }. Czy chodziło o { $suggestion }?
settings-error-not-bool = Oczekiwano wartości on lub off (true lub false).
settings-error-not-integer = Oczekiwano liczby całkowitej.
settings-error-not-number = Oczekiwano liczby.
settings-error-out-of-range = Wartość musi mieścić się między { $min } a { $max }.
settings-error-out-of-range-auto = Wartość musi wynosić 0 (automatycznie) lub mieścić się między { $min } a { $max }.
settings-error-unknown-option = Oczekiwano jednej z wartości: { $options }.
settings-error-bad-colour = Oczekiwano koloru, np. #1E1E1E lub #1E1E1E80.
settings-error-bad-language = Oczekiwano znacznika języka, np. de, pt-BR lub zh-Hans, albo „system”.
settings-error-bad-path = Oczekiwano ścieżki do pliku lub folderu.
settings-error-not-settable = Ten wpis jest poleceniem i nie ma wartości.
settings-error-syntax = Oczekiwano klucz=wartość, np. viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Nie może zawierać tabulatorów, podziałów wierszy ani innych znaków sterujących.
settings-error-invalid = { $reason }
settings-error-tile-url = Wymaga adresu http lub https z {"{"}z{"}"}, {"{"}x{"}"} i {"{"}y{"}"}.

## Jednostki

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } obraz
        [few] { $value } obrazy
        [many] { $value } obrazów
       *[other] { $value } obrazu
    }
unit-threads =
    { $value ->
        [one] { $value } wątek
        [few] { $value } wątki
        [many] { $value } wątków
       *[other] { $value } wątku
    }
unit-bytes =
    { $value ->
        [one] { $value } bajt
        [few] { $value } bajty
        [many] { $value } bajtów
       *[other] { $value } bajta
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Sekcje

settings-section-general = Ogólne
settings-section-viewing = Wyświetlanie
settings-section-browsing = Przeglądanie
settings-section-colour = Kolor
settings-section-metadata = Metadane i prywatność
# machine-drafted, needs native review
settings-section-privacy = Prywatność
settings-section-performance = Wydajność
settings-section-accessibility = Dostępność
settings-section-keys = Klawisze
settings-section-advanced = Zaawansowane

## Ogólne

settings-general-language = Język interfejsu
settings-general-language-help = Zmiany działają natychmiast. „Domyślny systemowy” oznacza język systemu operacyjnego.
settings-general-region-formats = Formaty regionalne
settings-general-region-formats-help = Sposób zapisu dat, godzin, liczb i rozmiarów. Domyślnie zgodny z językiem interfejsu.
settings-general-theme = Motyw
settings-general-theme-help = Jasny lub ciemny wygląd okien i kontrolek. Obrazy nigdy nie są zmieniane.
settings-general-theme-system = Zgodnie z systemem
settings-general-theme-light = Jasny
settings-general-theme-dark = Ciemny
# machine-drafted, needs native review
settings-general-on-start = Gdy { -app-name } otwiera się bez pliku
settings-general-on-start-help = Co pokazać, gdy { -app-name } zostanie otwarty samodzielnie. Plik otwarty z menedżera plików lub wiersza poleceń zawsze pojawia się w przeglądarce.
settings-general-on-start-start-screen = Ekran startowy
settings-general-on-start-gallery = Galeria ostatniego folderu
settings-general-on-start-library = Biblioteka
settings-general-start-tips = Pokazuj „Warto wypróbować” na ekranie startowym
settings-general-start-tips-help = Rząd kart z przydatnymi funkcjami i ich klawiszami. „Pokaż powitanie” w palecie poleceń przywraca go.
settings-general-tips = Pokazuj wskazówki
settings-general-tips-help = Krótka wskazówka, jeden raz, gdy funkcja może po raz pierwszy pomóc, np. klawisz pokazujący folder jako galerię. Nigdy podczas pokazu slajdów.
settings-general-tips-shown = Już pokazane wskazówki
settings-general-tips-shown-help = Jednorazowe wskazówki, które już się pojawiły, aby się nie powtarzały.
# machine-drafted, needs native review
settings-general-your-name = Twoje imię i nazwisko
settings-general-your-name-help = Widoczne na dodawanych przez Ciebie pieczątkach i adnotacjach. Zapisywane tylko w tworzonych przez Ciebie plikach adnotacji.
settings-general-single-instance = Otwieraj pliki w istniejącym oknie
settings-general-single-instance-help = Gdy { -app-name } już działa, kolejny otwierany obraz pojawi się w tym oknie, zamiast uruchamiać nowe.
settings-general-file-associations = Skojarzenia plików…
settings-general-file-associations-help = Ustaw { -app-name } jako domyślną przeglądarkę obsługiwanych formatów obrazów.

## Wyświetlanie

settings-viewing-initial-zoom = Początkowe powiększenie
settings-viewing-initial-zoom-help = W jakim rozmiarze pokazywany jest nowo otwarty obraz.
settings-viewing-initial-zoom-fit = Dopasuj do okna
settings-viewing-initial-zoom-fit-if-larger = Dopasuj duże obrazy, małe pokaż w { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Rzeczywisty rozmiar ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Wypełnij okno
settings-viewing-wheel-behaviour = Kółko myszy
settings-viewing-wheel-behaviour-help = Co robi przewijanie kółkiem. Przytrzymaj Ctrl, aby wykonać drugą czynność.
settings-viewing-wheel-behaviour-help-macos = Co robi przewijanie kółkiem. Przytrzymaj ⌘, aby wykonać drugą czynność.
settings-viewing-wheel-behaviour-zoom = Powiększanie
settings-viewing-wheel-behaviour-navigate = Następny i poprzedni obraz
settings-viewing-keep-zoom = Zachowaj powiększenie między obrazami
settings-viewing-keep-zoom-help = Po powiększeniu następny i poprzedni obraz otwierają się z tym samym powiększeniem i pokazują ten sam fragment kadru: przydatne do sprawdzania ostrości w serii.
settings-viewing-zoom-animation = Animuj powiększanie
settings-viewing-zoom-animation-help = Powiększaj płynnie zamiast skokowo. Wyłączane automatycznie, gdy włączono „Ogranicz ruch”.
settings-viewing-zoom-animation-speed = Szybkość animacji powiększania
settings-viewing-zoom-animation-speed-help = Jak szybko kończą się animowane powiększenia.
settings-viewing-zoom-animation-speed-relaxed = Spokojna
settings-viewing-zoom-animation-speed-normal = Normalna
settings-viewing-zoom-animation-speed-fast = Szybka
settings-viewing-background = Tło
settings-viewing-background-help = Kolor za obrazem w oknie.
settings-viewing-background-theme = Zgodnie z motywem
settings-viewing-background-black = Czarne
settings-viewing-background-dark-grey = Ciemnoszare
settings-viewing-background-light-grey = Jasnoszare
settings-viewing-background-white = Białe
settings-viewing-background-custom = Własny kolor
settings-viewing-background-color = Własny kolor tła
settings-viewing-background-color-help = Używany, gdy w ustawieniu Tło wybrano „Własny kolor”.
settings-viewing-fullscreen-background = Tło pełnego ekranu
settings-viewing-fullscreen-background-help = Kolor za obrazem w trybie pełnoekranowym.
settings-viewing-transparency = Obszary przezroczyste
settings-viewing-transparency-help = Jak rysowane są przezroczyste części obrazu.
settings-viewing-transparency-checkerboard = Szachownica
settings-viewing-transparency-background = Kolor tła
settings-viewing-transparency-black = Czarne
settings-viewing-transparency-white = Białe
settings-viewing-pixel-grid-threshold = Pokazuj siatkę pikseli od
settings-viewing-pixel-grid-threshold-help = Rysuj siatkę między pikselami przy co najmniej takim powiększeniu.
settings-viewing-smoothing-high-zoom = Powiększone piksele
settings-viewing-smoothing-high-zoom-help = Jak wyglądają piksele, gdy obraz jest pokazany w skali powyżej { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Wygładzone, od { PERCENT(400) } ostre
settings-viewing-smoothing-high-zoom-smooth = Zawsze wygładzone
settings-viewing-smoothing-high-zoom-pixelated = Zawsze ostre kwadraty
settings-viewing-double-click = Dwukrotne kliknięcie obrazu
settings-viewing-double-click-help = Dwukrotne kliknięcie pustego tła zawsze przełącza pełny ekran.
settings-viewing-double-click-toggle-zoom = Przełącz dopasowanie i { PERCENT(100) }
settings-viewing-double-click-fullscreen = Przełącz pełny ekran
settings-viewing-double-click-nothing = Nic nie rób
settings-viewing-fullscreen-on-open = Otwieraj obrazy na pełnym ekranie
settings-viewing-fullscreen-on-open-help = Uruchamiaj w trybie pełnoekranowym, gdy obraz jest otwierany spoza programu { -app-name }.
settings-viewing-fullscreen-mode = Styl pełnego ekranu
settings-viewing-fullscreen-mode-help = Natywny pełny ekran używa własnego obszaru (Space); natychmiastowy zakrywa bieżący bez animacji.
settings-viewing-fullscreen-mode-native = Natywny (własny Space)
settings-viewing-fullscreen-mode-borderless = Natychmiastowy
settings-viewing-toolbar-autohide = Automatycznie ukrywaj pasek narzędzi
settings-viewing-toolbar-autohide-help = Pasek narzędzi i strzałki przy krawędziach znikają, gdy wskaźnik się nie rusza, i wracają, gdy się poruszy.
settings-viewing-toolbar-autohide-delay = Ukryj pasek narzędzi po
settings-viewing-toolbar-autohide-delay-help = Jak długo wskaźnik musi pozostać nieruchomy, zanim pasek narzędzi zniknie.
settings-viewing-toolbar-items = Przyciski paska narzędzi
settings-viewing-toolbar-items-help = Które przyciski pokazuje pasek narzędzi i w jakiej kolejności.
settings-viewing-toolbar-drag = Zmieniaj układ paska narzędzi przeciąganiem
settings-viewing-toolbar-drag-help = Przeciągnij przycisk wzdłuż paska narzędzi, aby go przenieść. Krótkie kliknięcie nadal go naciska; Esc odkłada przeciągany przycisk z powrotem.
settings-viewing-toolbar-items-open = Otwórz
settings-viewing-toolbar-items-prev = Poprzedni obraz
settings-viewing-toolbar-items-next = Następny obraz
settings-viewing-toolbar-items-zoom-out = Pomniejsz
settings-viewing-toolbar-items-zoom-level = Poziom powiększenia
settings-viewing-toolbar-items-zoom-in = Powiększ
settings-viewing-toolbar-items-fit = Dopasuj do okna
settings-viewing-toolbar-items-rotate-left = Obróć w lewo
settings-viewing-toolbar-items-rotate-right = Obróć w prawo
settings-viewing-toolbar-items-info = Informacje
settings-viewing-toolbar-items-copy = Kopiuj obraz
settings-viewing-toolbar-items-delete = Przenieś do kosza
settings-viewing-toolbar-items-palette = Paleta poleceń
settings-viewing-toolbar-items-settings = Ustawienia
settings-viewing-toolbar-items-fullscreen = Pełny ekran
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Elastyczny odstęp
settings-viewing-toolbar-items-separator = Separator
settings-toolbar-preview = Podgląd paska narzędzi
settings-toolbar-hidden = Ukryte przyciski
settings-toolbar-add-spacer = Dodaj elastyczny odstęp
settings-toolbar-add-separator = Dodaj separator
settings-toolbar-reset = Przywróć domyślne
settings-toolbar-remove = Usuń: { $item }
settings-toolbar-hint = Przeciągnij wiersz, aby go przenieść, albo zaznacz go i naciśnij Alt+↑ lub Alt+↓. Tab przechodzi między polem wyszukiwania a listą.
settings-toolbar-move-up = Przenieś w górę
settings-toolbar-move-down = Przenieś w dół
settings-toolbar-move-start = Przenieś na początek
settings-toolbar-move-end = Przenieś na koniec
settings-toolbar-menu-hide = Ukryj
settings-toolbar-menu-show = Pokaż
settings-toolbar-menu-remove = Usuń
settings-toolbar-row-menu = Więcej działań: { $item }
settings-toolbar-position = Pozycja { $index } z { $count }
settings-toolbar-hidden-state = Ukryty

## Przeglądanie

settings-browsing-sort-order = Sortuj obrazy według
settings-browsing-sort-order-help = Kolejność przechodzenia przez folder. Sortowanie po nazwie stawia „img2” przed „img10” i stosuje alfabet Twojego języka.
settings-browsing-sort-order-natural = Nazwa
settings-browsing-sort-order-date-taken = Data wykonania
settings-browsing-sort-order-modified = Data modyfikacji
settings-browsing-sort-order-size = Rozmiar pliku
settings-browsing-sort-order-type = Typ pliku
settings-browsing-sort-descending = Odwrotna kolejność
settings-browsing-sort-descending-help = Sortuj od ostatniego do pierwszego.
settings-browsing-loop-folder = Zapętlaj na końcu folderu
settings-browsing-loop-folder-help = Po ostatnim obrazie przejdź do pierwszego i odwrotnie.
settings-browsing-include-subfolders = Uwzględnij podfoldery
settings-browsing-include-subfolders-help = Przeglądaj także obrazy w podfolderach.
settings-browsing-hold-acceleration = Przyspieszanie przy przytrzymaniu
settings-browsing-hold-acceleration-help = Jak szybko przyspiesza przewijanie obrazów, gdy przytrzymany jest klawisz lub przycisk.
settings-browsing-hold-acceleration-off = Bez przyspieszania
settings-browsing-hold-acceleration-gentle = Łagodne
settings-browsing-hold-acceleration-normal = Normalne
settings-browsing-hold-acceleration-fast = Szybkie
settings-browsing-confirm-trash = Pytaj przed przeniesieniem do kosza
settings-browsing-confirm-trash-help = Przeniesienie do kosza zawsze można cofnąć, więc domyślnie { -app-name } nie pyta.
settings-browsing-arrows-follow-reading-direction = Strzałki zgodne z kierunkiem czytania
settings-browsing-arrows-follow-reading-direction-help = W językach pisanych od prawej do lewej strzałka w lewo przechodzi do następnego obrazu.

## Kolor

settings-colour-management = Zarządzanie kolorami
settings-colour-management-help = Pokazuj kolory tak, jak miały wyglądać, na podstawie profilu kolorów obrazu i profilu wyświetlacza.
settings-colour-display-profile = Profil wyświetlacza
settings-colour-display-profile-help = Profil kolorów wyświetlacza. Automatycznie oznacza profil zgłaszany przez system operacyjny.
settings-colour-rendering-intent = Metoda odwzorowania
settings-colour-rendering-intent-help = Jak kolory, których wyświetlacz nie potrafi pokazać, są sprowadzane do jego zakresu.
settings-colour-rendering-intent-perceptual = Percepcyjna
settings-colour-rendering-intent-relative-colorimetric = Względna kolorymetryczna
settings-colour-rendering-intent-saturation = Nasycenie
settings-colour-rendering-intent-absolute-colorimetric = Bezwzględna kolorymetryczna
settings-colour-black-point-compensation = Kompensacja punktu czerni
settings-colour-black-point-compensation-help = Zachowuj szczegóły w cieniach, gdy czerń wyświetlacza jest jaśniejsza niż czerń obrazu.
settings-colour-untagged-images = Obrazy bez profilu kolorów
settings-colour-untagged-images-help = Jak pokazywać obrazy, które nie określają używanej przestrzeni kolorów.
settings-colour-untagged-images-srgb = Traktuj jako sRGB
settings-colour-untagged-images-display = Pokaż bez zmian
settings-colour-dithering = Rozpraszanie (dithering)
settings-colour-dithering-help = Dodawaj delikatny szum niebieski przy zmniejszaniu głębi kolorów, co zapobiega pasmom na niebie i w gradientach.
settings-colour-hdr-output = Wyjście HDR
settings-colour-hdr-output-help = Pokazuj obrazy HDR z pełną jasnością na wyświetlaczach, które to obsługują. Automatycznie używa HDR, gdy wyświetlacz zgłasza dodatkową jasność; w przeciwnym razie jasne obszary są łagodnie kompresowane.
settings-colour-hdr-output-auto = Automatycznie
settings-colour-hdr-output-on = Wł.
settings-colour-hdr-output-off = Wył.
# machine-drafted, needs native review
settings-colour-ocio-config = Konfiguracja OCIO
settings-colour-ocio-config-help = Konfiguracja OpenColorIO (.ocio) dla obrazów EXR, Radiance HDR i PFM. Puste oznacza konfigurację wskazaną przez $OCIO.
settings-colour-ocio-display = Wyświetlacz OCIO
settings-colour-ocio-display-help = Wyświetlacz z konfiguracji OCIO odpowiadający twojemu monitorowi.
settings-colour-ocio-display-default = Pierwszy z konfiguracji
settings-colour-ocio-view = Widok OCIO
settings-colour-ocio-view-help = Jak obrazy scene-linear są pokazywane na tym wyświetlaczu, np. widok z mapowaniem tonów.
settings-colour-ocio-view-default = Pierwszy z wyświetlacza
settings-colour-ocio-input = Wejściowa przestrzeń kolorów OCIO
settings-colour-ocio-input-help = Przestrzeń kolorów wartości obrazów.
settings-colour-ocio-input-default = Rola scene-linear z konfiguracji
# machine-drafted, needs native review
settings-colour-proof-profile = Profil softproofa
settings-colour-proof-profile-help = Profil drukarki użyty ostatnio przez softproof w Wywoływaniu.
settings-colour-proof-intent = Metoda renderowania softproofa
settings-colour-proof-intent-help = Jak dopasować kolory, których drukarka nie wydrukuje.
settings-colour-proof-intent-perceptual = Percepcyjna
settings-colour-proof-intent-relative = Kolorymetryczna względna
settings-colour-proof-bpc = Kompensacja punktu czerni w softproofie
settings-colour-proof-bpc-help = Odwzorowuje czerń na czerń drukarki.
settings-colour-proof-paper = Biel papieru w softproofie
settings-colour-proof-paper-help = Pokazuje własny kolor papieru.
settings-colour-proof-ink = Czerń atramentu w softproofie
settings-colour-proof-ink-help = Pokazuje, jak ciemna jest czerń atramentu.

## Metadane i prywatność

settings-metadata-show-info-on-open = Pokazuj panel informacji po otwarciu
settings-metadata-show-info-on-open-help = Otwieraj panel z danymi aparatu, datą i miejscem przy każdym otwarciu obrazu.
settings-metadata-info-detail = Panel informacji pokazuje
settings-metadata-info-detail-help = Podsumowanie wyjaśnia najważniejsze szczegóły prostym językiem; wszystkie pola to pełna lista danych zapisanych w pliku.
settings-metadata-info-detail-summary = Podsumowanie
settings-metadata-info-detail-all = Wszystkie pola
settings-metadata-reverse-geocoding = Pokazuj nazwy miejsc dla pozycji GPS
settings-metadata-reverse-geocoding-help = Znajduj najbliższą miejscowość na podstawie wbudowanych danych. Działa offline: pozycje nie opuszczają komputera.
settings-metadata-hide-gps = Ukrywaj pozycje GPS na ekranie
settings-metadata-hide-gps-help = Nie pokazuj współrzędnych ani miejsc, np. podczas udostępniania ekranu. Pliki nie są zmieniane.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Zapisuj zmiany z wywoływania obok zdjęcia
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Zapisuje IMG_1234.CR2.specular.xmp obok pliku, więc zmiany wędrują razem z nim. Wyłączone: zmiany zostają tylko w bibliotece.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importuj ustawienia Lightroom w Wywoływaniu
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Zdjęcie z ustawieniami Lightroom i bez edycji otwiera się z nimi, w przybliżeniu. Wyłączone: Wywoływanie pokazuje przycisk.
settings-metadata-units = Jednostki
settings-metadata-units-help = Jednostki wysokości, odległości i prędkości.
settings-metadata-units-auto = Zgodnie z formatami regionalnymi
settings-metadata-units-metric = Metryczne
settings-metadata-units-imperial = Imperialne
# machine-drafted, needs native review
settings-metadata-basemap = Mapa podkładowa
settings-metadata-basemap-help = Mapa pod lokalizacjami zdjęć. Mapy online pobierają kafelki oglądanego obszaru.
settings-metadata-basemap-off = Tylko wbudowana mapa (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Inny serwer kafelków (online)
settings-metadata-basemap-file = Własny plik mapy
settings-metadata-basemap-url = Adres serwera kafelków
settings-metadata-basemap-url-help = Adres z {"{"}z{"}"}, {"{"}x{"}"} i {"{"}y{"}"}, np. https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Plik mapy
settings-metadata-basemap-path-help = Plik MBTiles lub PMTiles. Działa offline.
settings-metadata-basemap-cache = Pamięć podręczna kafelków mapy
settings-metadata-basemap-cache-help = Miejsce na dysku na pobrane kafelki. Najstarsze są usuwane najpierw.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Dozwolone serwery kafelków
settings-metadata-basemap-allowed-help = Serwery map, z których zgodzono się pobierać kafelki.

## Wydajność

settings-performance-ram-cache = Pamięć podręczna
settings-performance-ram-cache-help = Pamięć do przechowywania zdekodowanych obrazów w gotowości. Automatycznie dobiera wielkość do ilości pamięci komputera.
settings-performance-vram-cache = Pamięć podręczna w pamięci GPU
settings-performance-vram-cache-help = Pamięć GPU na obrazy i kafelki. Automatycznie dobiera wielkość do GPU.
settings-performance-prefetch-depth = Obrazy przygotowywane z wyprzedzeniem
settings-performance-prefetch-depth-help = Ile obrazów w kierunku przeglądania jest dekodowanych zawczasu.
settings-performance-decoder-threads = Wątki dekodera
settings-performance-decoder-threads-help = Wątki CPU używane do dekodowania obrazów. Automatycznie używa wszystkich rdzeni wydajnościowych i dostosowuje się do pliku.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Którego GPU używać, gdy komputer ma więcej niż jeden.
settings-performance-gpu-adapter-high-performance = Najwyższa wydajność
settings-performance-gpu-adapter-low-power = Najniższe zużycie energii
settings-performance-processing = Gdzie przetwarzać obrazy
settings-performance-processing-help = Część pracy może wykonywać CPU, GPU lub dekoder sprzętowy. Tryb automatyczny mierzy ten komputer i wybiera najszybszy sposób dla oglądanego obrazu oraz najoszczędniejszy dla pracy w tle. To, co wybrano na tym komputerze, i dlaczego, pokazano poniżej.
settings-performance-processing-automatic = Automatycznie
settings-performance-processing-prefer-gpu = Preferuj GPU
settings-performance-processing-cpu-only = Tylko CPU
settings-performance-drag-preview = Podgląd podczas przeciągania
settings-performance-drag-preview-help = Jak wywoływanie pokazuje zdjęcie, gdy przeciągasz suwak. Automatycznie używa mniejszego obrazu tylko wtedy, gdy komputer nie nadąża.
settings-performance-drag-preview-automatic = Automatycznie
settings-performance-drag-preview-sharp = Zawsze ostry
settings-performance-drag-preview-fast = Zawsze szybki

## Ustawienia → Wydajność → Jak { -app-name } działa na tym komputerze. $engine stoi w mianowniku
## lub bierniku (dla tych nazw rodzaju męskiego nieżywotnego formy są takie same).
settings-paths-title = Jak { -app-name } działa na tym komputerze
settings-paths-intro = { -app-name } mierzy na tym komputerze czas każdego sposobu wykonania tych zadań i używa najlepszego. Czasy dotyczą zdjęcia o rozdzielczości { $mpx } Mpx.
settings-paths-intro-measuring = Pomiary nie są jeszcze zakończone: { -app-name } mierzy, gdy komputer jest bezczynny.
settings-paths-intro-battery = Zasilanie z baterii: wybór faworyzuje mniejsze zużycie energii.
settings-paths-keywords = GPU CPU NPU karta graficzna akceleracja sprzętowa dekoder benchmark test szybkość wydajność
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, wszystkie rdzenie
paths-engine-gpu = GPU
paths-engine-hw-decoder = Dekoder sprzętowy
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (wybrane w ustawieniach)
settings-paths-measured-faster = Zmierzono na tym komputerze: { $ratio } razy szybciej niż { $other }.
settings-paths-measured-similar = Zmierzono na tym komputerze: mniej więcej tak samo szybko jak { $other }.
settings-paths-measured-power = Przy zasilaniu z baterii zużywa to najmniej energii (zmierzono na tym komputerze).
settings-paths-not-measured = Jeszcze niezmierzone: typowy wybór dla tego rodzaju komputera.
settings-paths-set-by = Wybrane przez ustawienie „{ $setting }: { $option }”.
settings-paths-blocked-check = Wyłączono { $engine } ({ $date }): wynik nie zgadzał się z wynikiem CPU.
settings-paths-blocked-error = Wyłączono { $engine } ({ $date }) po błędzie.
settings-paths-blocked-lost = Wyłączono { $engine } ({ $date }) z powodu braku odpowiedzi.
settings-paths-blocked-undated = Wyłączono { $engine } po błędzie.
settings-paths-checking = { $engine }: zanim zostanie użyty dla oglądanego obrazu, wynik jest sprawdzany względem CPU.
settings-paths-exploring = W tle trwa teraz próba: { $engine }.
settings-paths-unavailable = Niedostępne na tym komputerze: { $engine }.
settings-paths-background-cheaper = W tle: { $engine } — według pomiarów zużywa o { $percent } mniej czasu CPU.
settings-paths-background-power = W tle: { $engine }, aby oszczędzać energię.
settings-paths-background = W tle: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms czasu CPU, { $runs ->
    [one] { $runs } pomiar
    [few] { $runs } pomiary
    [many] { $runs } pomiarów
   *[other] { $runs } pomiaru
} ({ $confidence })
settings-paths-detail-guess = { $engine }: około { $time } ms (szacunkowo)
settings-paths-confidence-low = niska pewność
settings-paths-confidence-medium = średnia pewność
settings-paths-confidence-high = wysoka pewność
settings-paths-machine = Zmierzono dla: { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Spróbuj ponownie
settings-paths-measure-again = Zmierz ponownie
settings-paths-measure-again-question = Zapomnieć pomiary z tego komputera, w tym wyłączone ścieżki, i zmierzyć ponownie, gdy komputer będzie bezczynny?
settings-paths-measure-again-confirm = Zapomnij pomiary
settings-performance-display-cache = Przechowuj pamięć podręczną wyświetlania na dysku
settings-performance-display-cache-help = Zapisuj kopie obrazów w rozmiarze ekranu, aby oglądane już foldery otwierały się natychmiast.
settings-performance-display-cache-size = Rozmiar pamięci podręcznej wyświetlania
settings-performance-display-cache-size-help = Gdy pamięć podręczna jest pełna, najstarsze wpisy są usuwane.
settings-performance-display-cache-location = Położenie pamięci podręcznej wyświetlania
settings-performance-display-cache-location-help = Najlepiej sprawdza się folder na szybkim dysku.
settings-performance-fast-simd = Używaj instrukcji SIMD
settings-performance-fast-simd-help = Używaj instrukcji wektorowych CPU do dekodowania. Wyłączaj tylko w celu diagnozy problemu.
settings-performance-fast-dct-scaling = Dekoduj w rozmiarze ekranu
settings-performance-fast-dct-scaling-help = Dekoduj duże pliki JPEG od razu w wyświetlanym rozmiarze. Wyłączaj tylko w celu diagnozy problemu.
settings-performance-fast-parallel-decode = Dekoduj na wielu rdzeniach
settings-performance-fast-parallel-decode-help = Dziel duże obrazy między rdzenie CPU. Wyłączaj tylko w celu diagnozy problemu.
settings-performance-fast-restart-index = Indeksuj duże pliki JPEG do powiększania
settings-performance-fast-restart-index-help = Zapamiętuj, gdzie zaczyna się każda część pliku JPEG, aby powiększone fragmenty szybko się dekodowały. Wyłączaj tylko w celu diagnozy problemu.
settings-performance-fast-yuv-upload = Konwertuj kolory na GPU
settings-performance-fast-yuv-upload-help = Konwertuj kolory JPEG na GPU lub CPU. Automatycznie wybiera szybszą opcję na tym komputerze. Wyłączaj tylko w celu diagnozy problemu.
settings-performance-fast-yuv-upload-auto = Automatycznie
settings-performance-fast-yuv-upload-on = Wł.
settings-performance-fast-yuv-upload-off = Wył.
settings-performance-fast-async-io = Asynchroniczny odczyt plików
settings-performance-fast-async-io-help = Odczytuj z wyprzedzeniem za pomocą najszybszego interfejsu plików systemu operacyjnego. Wyłączaj tylko w celu diagnozy problemu.
settings-performance-fast-mmap = Pliki mapowane w pamięci
settings-performance-fast-mmap-help = Odczytuj pliki bez kopiowania. Wyłączaj tylko w celu diagnozy problemu.
settings-performance-hardware-decode = Dekodowanie sprzętowe
settings-performance-hardware-decode-help = Pozwól wbudowanemu dekoderowi GPU otwierać zdjęcia JPEG. Tryb automatyczny używa go tam, gdzie pomaga, zwykle dla obrazów przygotowywanych w tle. Najpierw jest sprawdzany względem zwykłego dekodera, a to, czego nie obsłuży, otwiera się normalnie.
settings-performance-hardware-decode-auto = Automatycznie
settings-performance-hardware-decode-on = Wł.
settings-performance-hardware-decode-off = Wył.
settings-performance-frame-time-hud = Wskaźnik czasu klatki
settings-performance-frame-time-hud-help = Pokazuj czasy dekodowania i klatek oraz używane szybkie ścieżki.
settings-performance-stall-warning = Zgłaszaj zablokowane wczytywanie po
settings-performance-stall-warning-help = Gdy obraz wczytuje się dłużej niż ten czas, zapisz jego stan i oczekujące zadania w dzienniku (standardowe wyjście błędów lub SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Wskazówki dotyczące wydajności
settings-performance-insights-help = Gdy obrazy wciąż otwierają się powoli z tego samego powodu, pokazuj w rogu „Dlaczego tak wolno?” wraz z tym, co by pomogło. Zebrane informacje pozostają na tym komputerze.
settings-performance-insights-storage = Wskazówki dotyczące wolnego nośnika
settings-performance-insights-storage-help = Pokazuj wskazówki, gdy odczyt plików (z udziału sieciowego, wolnego dysku lub archiwów) wciąż opóźnia otwieranie obrazów.
settings-performance-insights-decoder = Wskazówki dotyczące dekodowania
settings-performance-insights-decoder-help = Pokazuj wskazówki, gdy obrazy wciąż są dekodowane na CPU, choć mógłby pomóc dekoder sprzętowy.
# machine-drafted, needs native review
settings-performance-insights-restore = Pokaż ponownie ukryte wskazówki
settings-performance-insights-restore-help = Przywraca wskazówki ukryte przyciskiem × w „Dlaczego to trwa tak długo?” – zarówno dla otwierania obrazów, jak i miniatur. Niedostępne, gdy żadna nie jest ukryta.

## Dostępność

settings-accessibility-reduce-motion = Ogranicz ruch
settings-accessibility-reduce-motion-help = Zastąp animacje powiększania, przesuwania i zanikania natychmiastowymi zmianami.
settings-accessibility-reduce-motion-system = Zgodnie z systemem
settings-accessibility-reduce-motion-on = Wł.
settings-accessibility-reduce-motion-off = Wył.
settings-accessibility-increase-contrast = Zwiększ kontrast
settings-accessibility-increase-contrast-help = Wyraźniejsze obramowania i tekst, mniej przezroczystości.
settings-accessibility-increase-contrast-system = Zgodnie z systemem
settings-accessibility-increase-contrast-on = Wł.
settings-accessibility-increase-contrast-off = Wył.
settings-accessibility-text-scale = Rozmiar tekstu
settings-accessibility-text-scale-help = Rozmiar tekstu względem rozmiaru standardowego.
settings-accessibility-ui-scale = Skala interfejsu
settings-accessibility-ui-scale-help = Rozmiar całego interfejsu, niezależnie od skalowania ekranu ustawionego w systemie operacyjnym.
settings-accessibility-focus-ring-always = Zawsze pokazuj obramowanie fokusu
settings-accessibility-focus-ring-always-help = Pokazuj, która kontrolka ma fokus klawiatury, także podczas korzystania z myszy.
settings-accessibility-screen-reader-verbosity = Szczegółowość czytnika ekranu
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Ile informacji czytnik ekranu podaje przy przechodzeniu między obrazami i ile z odczytu pod wskaźnikiem odczytuje.
settings-accessibility-screen-reader-verbosity-concise = Zwięźle
settings-accessibility-screen-reader-verbosity-normal = Normalnie
settings-accessibility-screen-reader-verbosity-verbose = Szczegółowo
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Odczytaj wartość pod wskaźnikiem
settings-accessibility-announce-cursor-help = Czytnik ekranu odczytuje wartość piksela, HU, temperaturę lub położenie pod wskaźnikiem albo celownikiem klawiatury, gdy ten przestanie się poruszać.

## Prywatność

# machine-drafted, needs native review
settings-privacy-no-network = Bez sieci
# machine-drafted, needs native review
settings-privacy-no-network-help = Wyłączone: łączą się tylko te funkcje, każda dopiero po włączeniu lub otwarciu czegoś za jej pomocą: sprawdzanie aktualizacji, pobieranie modeli lokalnej AI, mapy online, obrazy z adresu internetowego lub chmury oraz dodane serwery (DICOMweb, OMERO, mapy). Włączone: nic się nie łączy.

## Zaawansowane

settings-advanced-portable-mode = Tryb przenośny
settings-advanced-portable-mode-help = Przechowuj ustawienia i pamięć podręczną w folderze obok programu zamiast w profilu użytkownika.
settings-advanced-log-level = Szczegółowość dziennika
settings-advanced-log-level-help = Ile informacji { -app-name } zapisuje w pliku dziennika.
settings-advanced-log-level-error = Tylko błędy
settings-advanced-log-level-warn = Ostrzeżenia
settings-advanced-log-level-info = Informacje
settings-advanced-log-level-debug = Debugowanie
settings-advanced-log-level-trace = Wszystko
settings-advanced-developer-mode = Tryb dewelopera
settings-advanced-developer-mode-help = Pokazuj narzędzia do rozwijania i testowania programu { -app-name }, np. pseudojęzyki na liście języków.
# machine-drafted, needs native review
settings-advanced-audit-log = Lokalny dziennik audytu
# machine-drafted, needs native review
settings-advanced-audit-log-help = Zapisuj otwarte obrazy medyczne, eksporty, anonimizacje, wydruki i wysyłki do PACS w pliku audit.jsonl w folderze danych. Dziennik pozostaje na tym komputerze.
settings-advanced-hw-report = Raport sprzętowy…
settings-advanced-hw-report-help = Pokaż, jakie funkcje CPU i GPU wykrył { -app-name } i których szybkich ścieżek używa.
# machine-drafted, needs native review
settings-advanced-renderer-info = Informacje o renderowaniu…
settings-advanced-renderer-info-help = Pokazuje kartę graficzną, sterownik, formaty wyświetlania i limity pamięci używane przez { -app-name }, gotowe do skopiowania do zgłoszenia błędu.
settings-advanced-edit-settings-file = Edytuj settings.toml
settings-advanced-edit-settings-file-help = Edytuj plik ustawień we wbudowanym edytorze { -app-name }. Sprawdza on plik podczas pisania i zapisuje go tylko wtedy, gdy nie ma w nim błędów.
settings-advanced-import = Importuj ustawienia…
settings-advanced-import-help = Wczytaj ustawienia z pliku wyeksportowanego z programu { -app-name }.
settings-advanced-export = Eksportuj ustawienia…
settings-advanced-export-help = Zapisz wszystkie ustawienia w jednym pliku, jako kopię zapasową lub dla innego komputera.
settings-advanced-reset-all = Resetuj wszystkie ustawienia…
settings-advanced-reset-all-help = Przywróć wszystkie ustawienia do wartości domyślnych. Można to cofnąć.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Zapisz
config-editor-revert = Przywróć
config-editor-close = Zamknij
config-editor-modified = Zmieniono
config-editor-text = Plik ustawień
config-editor-position = Wiersz { $line }, kolumna { $column }
config-editor-saved = Zapisano i zastosowano settings.toml.
config-editor-cant-save = Nie można zapisać. Problemy do naprawienia: { $count }.
config-editor-save-failed = Nie można zapisać settings.toml: { $reason }
config-editor-conflict = Plik settings.toml został zmieniony w innym miejscu.
config-editor-reload = Wczytaj ponownie
config-editor-keep-mine = Zachowaj moje
config-editor-discard-question = Odrzucić niezapisane zmiany w settings.toml?
config-editor-discard = Odrzuć
config-editor-keep-editing = Edytuj dalej
config-editor-find-placeholder = Znajdź w pliku
config-editor-find-count = { $current } z { $total }
config-editor-find-none = Brak wyników
config-editor-find-previous = Poprzedni wynik
config-editor-find-next = Następny wynik
config-editor-find-close = Zamknij wyszukiwanie
config-editor-problems = Problemy
config-editor-problems-count =
    { $count ->
        [0] Brak problemów
       *[other] Problemy: { $count }
    }
config-editor-no-problems = Brak problemów. Plik można zapisać.
config-editor-problem-at = Wiersz { $line }: { $message }
config-editor-severity-error = Błąd
config-editor-severity-warning = Ostrzeżenie
config-editor-severity-info = Uwaga
config-editor-problem-syntax = Nieprawidłowy TOML: { $reason }
config-editor-problem-duplicate = { $key } ustawiono dwukrotnie w tej samej tabeli. Usuń jedno z nich.
config-editor-problem-unknown-section = Nie ma sekcji ustawień [{ $key }].
config-editor-problem-unknown-section-suggestion = Nie ma sekcji ustawień [{ $key }]. Czy chodziło o [{ $suggestion }]?
config-editor-problem-unknown-key = Nie ma ustawienia { $key }.
config-editor-problem-unknown-key-suggestion = Nie ma ustawienia { $key }. Czy chodziło o { $suggestion }?
config-editor-problem-invalid = Nieprawidłowa wartość { $key }: { $reason }
config-editor-problem-migrated = { $key } zapisano tak, jak robiła to starsza wersja { -app-name }. Jest konwertowane przy odczycie.
config-editor-problem-newer-version = Ten plik zapisała nowsza wersja { -app-name } (format { $found }; ta wersja odczytuje { $supported }). Nieznane ustawienia są zachowywane.
config-editor-problem-command = Polecenie { $index } w [[commands]]: { $reason }
config-editor-problem-hook = Hook { $index } w [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Plik skrótów klawiszowych
config-editor-keymap-saved = Zapisano i zastosowano keymap.toml.
config-editor-keymap-save-failed = Nie można zapisać keymap.toml: { $reason }
config-editor-keymap-conflict = Plik keymap.toml zmienił się gdzie indziej.
config-editor-keymap-discard-question = Odrzucić niezapisane zmiany w keymap.toml?
config-editor-new-file = nowy plik, zostanie utworzony przy zapisie
config-editor-no-folder = nie ma folderu ustawień, w którym można go zapisać.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Przywrócić wszystkie ustawienia do wartości domyślnych?
settings-reset-all-confirm-detail = Dotyczy to wszystkich sekcji Ustawień. Można to potem cofnąć.
settings-reset-all-confirm-ok = Przywróć wszystko
settings-reset-all-done = Wszystkie ustawienia przywrócono do wartości domyślnych.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Koder sprzętowy

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = { $engine }: przed użyciem wynik jest sprawdzany względem CPU.
settings-paths-unit-thumbnail = Czasy dotyczą jednej miniatury { $size }
settings-paths-unit-frame = Czasy dotyczą jednej klatki wideo { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Folder klatek kalibracyjnych
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Klatki flat i dark dla Wywołania. Nazwy plików lub folderów zawierają „flat” lub „dark”.

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Biały
settings-colour-light-grey = Jasnoszary
settings-colour-grey = Szary
settings-colour-dark-grey = Ciemnoszary
settings-colour-black = Czarny
settings-colour-yellow = Żółty
settings-colour-orange = Pomarańczowy
settings-colour-red = Czerwony
settings-colour-magenta = Purpurowy
settings-colour-blue = Niebieski
settings-colour-cyan = Cyjan
settings-colour-green = Zielony
