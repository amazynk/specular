# machine-drafted, needs native review
# Specular — Raport otwarcia: dlaczego otwieranie obrazu trwało tyle, ile trwało (panel informacji,
# sekcja „Wydajność”; DESIGN.md §12.7).

open-report-title = Wydajność

## Headlines
open-report-fast = Otwarto w { $ms } ms
open-report-display-cache = Wyświetlono z pamięci podręcznej wyświetlania
open-report-slow-read = Wolny nośnik: { $percent } czasu zajął odczyt pliku ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Wolny nośnik: { $percent } czasu zajął odczyt pliku (mały plik: głównie czekanie na odpowiedź nośnika)
open-report-archive-read = { $percent } czasu zajęło rozpakowywanie archiwum
open-report-startup = { $percent } czasu zajęło uruchamianie programu { -app-name }
open-report-checked = Zdekodowano dwukrotnie: pierwszy wynik dekodera sprzętowego porównano z wynikiem CPU ({ $percent } czasu)
open-report-hardware = Zdekodowano dekoderem sprzętowym, który zajął { $percent } czasu
open-report-decode = Duży obraz: dekodowanie { $mpx } megapikseli zajęło { $percent } czasu
open-report-upload = { $percent } czasu zajęło przesłanie obrazu do GPU
open-report-waiting = { $percent } czasu zajęło czekanie na inne zadania
open-report-stage = { $stage } zajęło { $percent } czasu
open-report-memory-short = Brakowało pamięci, więc system mógł musieć korzystać z wymiany

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Zdekodowano programowo: dekoder sprzętowy był jeszcze wykrywany
open-report-sw-no-device = Zdekodowano programowo: { $api } nie jest dostępne na tym komputerze
open-report-sw-no-codec = Zdekodowano programowo: { $api } nie ma dekodera { $codec } na tym GPU
open-report-sw-blocked-mismatch = Zdekodowano programowo: dekoder sprzętowy wyłączono, bo jego wynik nie zgadzał się z wynikiem CPU
open-report-sw-blocked-error = Zdekodowano programowo: dekoder sprzętowy wyłączono po błędzie
open-report-sw-blocked-lost = Zdekodowano programowo: dekoder sprzętowy wyłączono, bo przestał odpowiadać
open-report-sw-failed = Zdekodowano programowo: dekoder sprzętowy zawiódł przy tym obrazie
open-report-sw-busy = Zdekodowano programowo: dekoder sprzętowy był zajęty
open-report-sw-unsupported = Zdekodowano programowo: dekoder sprzętowy nie obsługuje tego obrazu
open-report-sw-unavailable = Zdekodowano programowo: dekodowanie sprzętowe jest wyłączone do ponownego uruchomienia programu { -app-name }
open-report-sw-cpu-only = Zdekodowano programowo: przetwarzanie ustawiono na „Tylko CPU”
open-report-sw-setting-off = Zdekodowano programowo: dekodowanie sprzętowe jest wyłączone w ustawieniach
open-report-sw-unchecked = Zdekodowano programowo: dekoder sprzętowy nie został jeszcze sprawdzony (najpierw jest próbowany na obrazie przygotowywanym w tle)
open-report-sw-lower-power = Zdekodowano programowo: na baterii CPU zużywa na to mniej energii
open-report-sw-faster = Zdekodowano programowo: na tym komputerze to szybsze (około { $cpu } ms wobec { $hw } ms na dekoderze sprzętowym)
open-report-sw-scaled = Zdekodowano programowo: dekoder sprzętowy dekoduje obrazy tylko w pełnym rozmiarze

## Hints
open-report-hint-copy-local = Skopiuj plik na dysk lokalny, aby otwierał się szybciej.
open-report-hint-extract = Rozpakuj archiwum do folderu, aby jego strony otwierały się szybciej.
open-report-hint-try-again = Aby ponownie wypróbować dekoder sprzętowy: Ustawienia → Wydajność → Spróbuj ponownie.
open-report-hint-automatic = Ustaw „Gdzie przetwarzać obrazy” na Automatycznie w Ustawienia → Wydajność.
open-report-hint-turn-on = Ustaw „Dekodowanie sprzętowe” na Automatycznie w Ustawienia → Wydajność.
open-report-hint-free-memory = Zamknij inne programy, aby zwolnić pamięć.
open-report-hint-once-only = Tak sprawdzany jest tylko pierwszy obraz.

## Rows
open-report-total = Do pełnej ostrości
open-report-first-pixels = Pierwsze piksele
# machine-drafted, needs native review
open-report-whole = Cały obraz widoczny
open-report-stage-startup = Uruchamianie
open-report-stage-read = Odczyt pliku
open-report-stage-parse = Odczyt nagłówka
open-report-stage-colour = Przygotowanie zarządzania kolorem
open-report-stage-decode = Dekodowanie
open-report-stage-upload = Wysyłanie do GPU
open-report-stage-waiting = Czekanie na inne zadania
open-report-decoder = Zdekodowane przez
open-report-decoder-checked = { $engine }, sprawdzone względem CPU
open-report-decoder-cache = Pamięć podręczna wyświetlania
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, w 1/{ $scale } rozmiaru
open-report-decoder-scaled-full = { $engine }, w pełnym rozmiarze, potem zmniejszone do 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, w 1/{ $scale } rozmiaru, bez indeksu szybkiego startu
open-report-colour-conversion = Kolory przekształcone przez

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Powtarza się
open-report-advice-change = Pomogłoby
open-report-advice-gain = Szacowany zysk
open-report-advice-software = Przy { $count } z ostatnich { $total } wolnych obrazów { $format } z tego powodu dekodowano programowo
open-report-advice-slow-storage = Przy { $count } z ostatnich { $total } wolnych obrazów opóźnienie spowodował odczyt pliku (około { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = Przy { $count } z ostatnich { $total } wolnych obrazów opóźnienie spowodował odczyt pliku (małe pliki: głównie czekanie na odpowiedź nośnika)
open-report-advice-archive = Przy { $count } z ostatnich { $total } wolnych obrazów opóźnienie spowodowało rozpakowywanie archiwum
open-report-advice-hardware-decoder = GPU ze sprzętowym dekoderem { $codec }
open-report-advice-gain-range = Około { $low }–{ $high } razy szybciej
open-report-advice-gain-one = Około { $low } razy szybciej

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Wskazówki dotyczące wydajności
open-report-insights-indicator = Dlaczego tak wolno?
open-report-insights-announce = Wskazówka dotycząca wydajności: obrazy wciąż otwierają się powoli z tego samego powodu. Otwórz „Dlaczego tak wolno?”, aby zobaczyć, co by pomogło.
open-report-insights-hide = Ukryj
open-report-insights-last = Ostatni wolny obraz: { $total } do pełnej ostrości
open-report-insights-now = Co możesz zrobić teraz
open-report-insights-hardware = Sprzęt, który by pomógł
open-report-insights-none = Brak powtarzającego się spowolnienia
open-report-insights-none-detail = Wskazówki pojawiają się, gdy kilka obrazów otwiera się powoli z tego samego powodu. Aby je znaleźć, { -app-name } przechowuje na tym komputerze wolne otwarcia z ostatniego tygodnia; nic nie jest nigdzie wysyłane.
open-report-insights-software = Obrazy { $format } są dekodowane na CPU
open-report-insights-storage = Odczyt plików opóźnia obrazy
open-report-insights-archive = Rozpakowywanie archiwów opóźnia obrazy
open-report-insights-nothing-now = Żadne ustawienie w { -app-name } tego nie zmieni.
open-report-insights-display-cache = Włącz pamięć podręczną wyświetlania: obrazy otwierane wcześniej będą wtedy otwierane z szybkiej kopii na tym komputerze.
open-report-insights-prefetch = Trzymaj w gotowości więcej sąsiednich obrazów, aby kolejne były przygotowywane, gdy oglądasz.
open-report-insights-hw-ssd = Szybki dysk wewnętrzny (SSD NVMe) na te pliki
open-report-insights-action-turn-on = Włącz
open-report-insights-action-try-again = Spróbuj ponownie
open-report-insights-action-automatic = Ustaw na Automatycznie
open-report-insights-dismiss-storage = Nie pokazuj więcej dla nośnika
open-report-insights-dismiss-decoder = Nie pokazuj więcej dla dekodowania
open-report-insights-off = Wyłącz wskazówki
open-report-insights-on = Włącz wskazówki
open-report-insights-restore = Pokaż ponownie ukryte wskazówki
open-report-insights-power = Ten komputer oszczędza energię (działa na baterii lub w trybie oszczędzania energii), co spowalnia dekodowanie na CPU. Po podłączeniu zasilania obrazy mogą otwierać się szybciej.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Wskazówka dotycząca wydajności: miniatury wciąż pojawiają się powoli z tego samego powodu. Otwórz „Dlaczego tak wolno?”, aby zobaczyć, co by pomogło.
open-report-insights-last-thumbnail = Ostatnia wolna miniatura: { $total } na utworzenie
open-report-insights-thumbnails-software = Miniatury obrazów { $format } są dekodowane na CPU
open-report-insights-thumbnails-storage = Odczyt plików opóźnia miniatury
open-report-insights-thumbnails-archive = Rozpakowywanie archiwów opóźnia miniatury
open-report-insights-thumbnail-cache = Miniatur nie można przechowywać na tym komputerze (pamięć podręczna miniatur jest niedostępna), więc są tworzone od nowa przy każdym wyświetleniu folderu.
open-report-advice-thumbnails-software = Przy { $count } z ostatnich { $total } wolnych miniatur { $format } z tego powodu dekodowano programowo
open-report-advice-thumbnails-slow-storage = Przy { $count } z ostatnich { $total } wolnych miniatur opóźnienie spowodował odczyt pliku (około { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = Przy { $count } z ostatnich { $total } wolnych miniatur opóźnienie spowodował odczyt pliku (małe pliki: głównie czekanie na odpowiedź nośnika)
open-report-advice-thumbnails-archive = Przy { $count } z ostatnich { $total } wolnych miniatur opóźnienie spowodowało rozpakowywanie archiwum
