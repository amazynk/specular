# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Konwertuj…

## Settings → Convert
settings-section-convert = Konwersja
settings-convert-format = Format
settings-convert-format-help = Format, w którym zapisuje polecenie „Konwertuj…”. PNG i TIFF są bezstratne; WebP i JPEG XL mogą być zapisywane na oba sposoby.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = Jakość JPEG
settings-convert-jpeg-quality-help = Wyższa wartość zachowuje więcej szczegółów w większych plikach. Przy 90 trudno odróżnić wynik od oryginału.
settings-convert-jpeg-full-chroma = Kolor w pełnej rozdzielczości w JPEG
settings-convert-jpeg-full-chroma-help = Zachowuje kolor w pełnej rozdzielczości (4:4:4) zamiast połowy (4:2:0): ostrzejsze krawędzie kolorów, większe pliki.
settings-convert-png-effort = Kompresja PNG
settings-convert-png-effort-help = Wyższa wartość daje mniejsze pliki, ale wolniej. 1 jest najszybsza.
settings-convert-jxl-effort = Wysiłek JPEG XL
settings-convert-jxl-effort-help = Wyższa wartość daje mniejsze pliki, ale wolniej.
settings-convert-tiff-compression = Kompresja TIFF
settings-convert-tiff-compression-help = Deflate jest bezstratna i odczyta ją każdy program obsługujący TIFF.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Brak
settings-convert-depth = Głębia bitowa
settings-convert-depth-help = Automatycznie zapisuje 16 bitów na próbkę, gdy obraz ma ich więcej niż 8, a format je obsługuje (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automatycznie
settings-convert-depth-8 = 8 bitów
settings-convert-depth-16 = 16 bitów
settings-convert-resize = Zmiana rozmiaru
settings-convert-resize-help = Obrazy są zmniejszane w świetle liniowym filtrem wysokiej jakości.
settings-convert-resize-none = Oryginalny rozmiar
settings-convert-resize-fit = Zmieść w
settings-convert-resize-long-edge = Dłuższy bok
settings-convert-resize-percent = Procent
settings-convert-width = Szerokość
settings-convert-width-help = Największa szerokość, jaką może mieć obraz przy opcji Zmieść w.
settings-convert-height = Wysokość
settings-convert-height-help = Największa wysokość, jaką może mieć obraz przy opcji Zmieść w.
settings-convert-long-edge = Dłuższy bok
settings-convert-long-edge-help = Długość dłuższego boku obrazu przy opcji Dłuższy bok.
settings-convert-percent = Procent
settings-convert-percent-help = Rozmiar względem oryginału przy opcji Procent.
settings-convert-upscale = Powiększaj małe obrazy
settings-convert-upscale-help = Gdy wyłączone, obrazy mniejsze niż żądany rozmiar zachowują swój rozmiar.
settings-convert-colour = Kolor
settings-convert-colour-help = Zachowaj zapisuje profil kolorów obrazu razem z nim. Konwertuj do sRGB nadaje się do sieci i urządzeń bez zarządzania kolorami.
settings-convert-colour-keep = Zachowaj profil
settings-convert-colour-srgb = Konwertuj do sRGB
settings-convert-hdr = Obrazy HDR
settings-convert-hdr-help = PNG i JPEG XL zachowują HDR bez zmian; pozostałe formaty są zawsze mapowane tonalnie do SDR.
settings-convert-hdr-keep = Zachowaj, gdy to możliwe
settings-convert-hdr-tone-map = Mapuj tonalnie do SDR
settings-convert-metadata = Metadane
settings-convert-metadata-help = Exif i XMP, np. aparat, data wykonania i lokalizacja. Profil kolorów jest zawsze zachowywany.
settings-convert-metadata-all = Zachowaj wszystkie
settings-convert-metadata-no-location = Wszystkie oprócz lokalizacji
settings-convert-metadata-strip = Usuń
settings-convert-folder = Folder docelowy
settings-convert-folder-help = Gdzie trafiają przekonwertowane pliki. Gdy nie jest ustawiony, trafiają do podfolderu obok każdego obrazu.
settings-convert-folder-unset = Podfolder obok obrazów
settings-convert-subfolder = Nazwa podfolderu
settings-convert-subfolder-help = Podfolder używany, gdy nie ustawiono folderu docelowego. Puste: folder, w którym są obrazy.
settings-convert-pattern = Nazwy plików
settings-convert-pattern-help = {"{"}name{"}"} to oryginalna nazwa, {"{"}n:03{"}"} numer w partii, {"{"}date:%Y-%m-%d{"}"} data wykonania, {"{"}w{"}"} i {"{"}h{"}"} nowy rozmiar, {"{"}ext{"}"} rozszerzenie formatu.
settings-convert-conflicts = Gdy nazwa jest zajęta
settings-convert-conflicts-help = Istniejące pliki nigdy nie są zastępowane.
settings-convert-conflicts-suffix = Dodaj numer
settings-convert-conflicts-skip = Pomiń obraz

## The dialog
convert-title = Konwertuj
convert-images = Obrazy
convert-images-current = Ten obraz
convert-images-selection = Zaznaczenie ({ $count })
convert-images-folder = Folder ({ $count })
convert-images-picked = Wybrane ({ $count })
convert-quality = Jakość
convert-chroma = Szczegóły koloru
convert-chroma-half = 4:2:0 (mniejszy)
convert-chroma-full = 4:4:4 (ostrzejszy)
convert-effort = Wysiłek
convert-on = Wł.
convert-off = Wył.
convert-pixels = { $value } px
convert-effort-fastest = { $value } (najszybciej)
convert-folder-subfolder = Podfolder „{ $name }”
convert-folder-same = Ten sam folder co obraz
convert-choose-folder = Wybierz folder…
convert-name = Nazwa
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Nawias {"{"} nie jest zamknięty.
convert-name-error-stray = Nawias {"}"} nie ma pary {"{"}. Aby wstawić nawias klamrowy, wpisz {"}}"}.
convert-name-error-unknown = { $token } nie jest częścią nazwy. Użyj name, n, date, w, h lub ext.
convert-name-error-format = { $token } ma nieprawidłowy format.
convert-name-error-empty = Wpisz wzorzec nazwy.
convert-start = Konwertuj
convert-stop = Zatrzymaj
convert-hide = Ukryj
convert-close = Zamknij
convert-progress = Konwertowanie { $done } z { $total }
convert-progress-eta = Konwertowanie { $done } z { $total } · zostało około { $time }
convert-eta-seconds = { $count ->
    [one] { $count } sekunda
    [few] { $count } sekundy
    [many] { $count } sekund
   *[other] { $count } sekundy
}
convert-eta-minutes = { $count ->
    [one] { $count } minuta
    [few] { $count } minuty
    [many] { $count } minut
   *[other] { $count } minuty
}
convert-done = { $count ->
    [one] Przekonwertowano { $count } obraz
    [few] Przekonwertowano { $count } obrazy
    [many] Przekonwertowano { $count } obrazów
   *[other] Przekonwertowano { $count } obrazu
}
convert-done-skipped = Pominięto: { $count } (nazwa była zajęta)
convert-done-failed = Nieudane: { $count }
convert-stopped = Zatrzymano po { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = nie można otworzyć ({ $error })
convert-failure-decode = nie można odczytać ({ $error })
convert-failure-encode = nie można zapisać w tym formacie ({ $error })
convert-failure-write = nie można zapisać ({ $error })
convert-failure-same = nowy plik zastąpiłby oryginał
convert-report-more = i { $count } więcej
convert-running = Konwersja już trwa.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = Bezstratny WebP
settings-convert-webp-lossless-help = Zachowuje dokładnie każdy piksel. Po wyłączeniu pliki WebP są znacznie mniejsze przy ustawionej jakości WebP.
settings-convert-webp-quality = Jakość WebP
settings-convert-webp-quality-help = Wyższa wartość zachowuje więcej szczegółów w większych plikach. Używane, gdy WebP nie jest bezstratny.
settings-convert-webp-effort = Wysiłek WebP
settings-convert-webp-effort-help = Wyższa wartość daje mniejsze pliki, ale wolniej. Używane, gdy WebP nie jest bezstratny.
settings-convert-jxl-lossless = Bezstratny JPEG XL
settings-convert-jxl-lossless-help = Zachowuje dokładnie każdy piksel. Po wyłączeniu pliki JPEG XL są znacznie mniejsze przy ustawionej jakości JPEG XL.
settings-convert-jxl-quality = Jakość JPEG XL
settings-convert-jxl-quality-help = Wyższa wartość zachowuje więcej szczegółów w większych plikach. Przy 90 trudno odróżnić wynik od oryginału. Używane, gdy JPEG XL nie jest bezstratny.
settings-convert-avif-quality = Jakość AVIF
settings-convert-avif-quality-help = Wyższa wartość zachowuje więcej szczegółów w większych plikach.
settings-convert-avif-effort = Wysiłek AVIF
settings-convert-avif-effort-help = Wyższa wartość daje mniejsze pliki, ale wolniej. 1 jest najszybsza.
convert-lossless = Bezstratnie
