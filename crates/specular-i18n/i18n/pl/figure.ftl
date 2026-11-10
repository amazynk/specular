# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Eksportuj rysunek…
# machine-drafted, needs native review
palette-export-figure-help = Zapisz widok jako rysunek z podziałką, paskiem kolorów i opisami.
# machine-drafted, needs native review
palette-toggle-key-image = Oznacz jako obraz kluczowy
# machine-drafted, needs native review
palette-toggle-key-image-help = Oznacz ten obraz jako kluczowy lub usuń oznaczenie.
# machine-drafted, needs native review
palette-copy-key-images = Kopiuj obrazy kluczowe
# machine-drafted, needs native review
palette-copy-key-images-help = Skopiuj obrazy kluczowe jako jeden obraz.
# machine-drafted, needs native review
palette-export-key-images = Eksportuj obrazy kluczowe…
# machine-drafted, needs native review
palette-export-key-images-help = Zapisz obrazy kluczowe jako slajdy lub folder z plikami PNG.
# machine-drafted, needs native review
palette-export-teaching-file = Eksportuj plik dydaktyczny…
# machine-drafted, needs native review
palette-export-teaching-file-help = Zapisz obrazy kluczowe bez danych pacjenta, do celów dydaktycznych.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Eksportuj rysunek
# machine-drafted, needs native review
figure-format = Format
# machine-drafted, needs native review
figure-format-png = PNG, 16-bitowy
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16-bitowy
# machine-drafted, needs native review
figure-region = Obszar
# machine-drafted, needs native review
figure-region-visible = Widoczny obszar
# machine-drafted, needs native review
figure-region-whole = Cały obraz
# machine-drafted, needs native review
figure-size = Rozmiar
# machine-drafted, needs native review
figure-size-pixels = Piksele obrazu (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = Szerokość { $width } px
# machine-drafted, needs native review
figure-scale-bar = Podziałka
# machine-drafted, needs native review
figure-scale-bar-none = Brak kalibracji
# machine-drafted, needs native review
figure-colour-bar = Pasek kolorów
# machine-drafted, needs native review
figure-colour-bar-none = Brak mapy kolorów lub okna
# machine-drafted, needs native review
figure-inset = Wstawka
# machine-drafted, needs native review
figure-inset-rectangle = Ostatni prostokąt
# machine-drafted, needs native review
figure-inset-none = Najpierw narysuj prostokąt
# machine-drafted, needs native review
figure-channels = Kanały
# machine-drafted, needs native review
figure-channels-merged = Połączone
# machine-drafted, needs native review
figure-channels-split = Osobno i połączone
# machine-drafted, needs native review
figure-channels-one = Jeden kanał
# machine-drafted, needs native review
figure-measurements = Pomiary
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Włączone ({ $count })
    [few] Włączone ({ $count })
    [many] Włączone ({ $count })
   *[other] Włączone ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Brak
# machine-drafted, needs native review
figure-title-row = Tytuł
# machine-drafted, needs native review
figure-provenance = Pochodzenie
# machine-drafted, needs native review
figure-provenance-metadata = W pliku
# machine-drafted, needs native review
figure-provenance-drawn = W pliku i narysowane
# machine-drafted, needs native review
figure-file = Plik
# machine-drafted, needs native review
figure-choose = Wybierz…
# machine-drafted, needs native review
figure-on = Włączone
# machine-drafted, needs native review
figure-off = Wyłączone
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Włączone · { $source }
# machine-drafted, needs native review
figure-export = Eksportuj
# machine-drafted, needs native review
figure-working = Eksportowanie…
# machine-drafted, needs native review
figure-not-full-res = Piksele w pełnej rozdzielczości nie są wczytane. Powiększ do { PERCENT(100) } lub wybierz rozmiar.
# machine-drafted, needs native review
figure-channel-red = Czerwony
# machine-drafted, needs native review
figure-channel-green = Zielony
# machine-drafted, needs native review
figure-channel-blue = Niebieski
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, od { $lo } do { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Okno od { $lo } do { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Kolory kanałów { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Kanał { $channel }
# machine-drafted, needs native review
figure-display-hilo = Znaczniki HiLo
# machine-drafted, needs native review
figure-display-as-stored = Tak jak zapisano
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Kalibracja: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Wyświetlanie: { $value }
# machine-drafted, needs native review
figure-not-image = Rysunki tworzy się z obrazu w przeglądarce.
# machine-drafted, needs native review
figure-not-ready = Obraz jest jeszcze wczytywany.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Za duży: { $width } × { $height } px. Wybierz widoczny obszar lub rozmiar.

## Key images
# machine-drafted, needs native review
figure-keys-title = Eksportuj obrazy kluczowe
# machine-drafted, needs native review
figure-teaching-title = Eksportuj plik dydaktyczny
# machine-drafted, needs native review
figure-keys-images = Obrazy
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } obraz kluczowy
    [few] { $count } obrazy kluczowe
    [many] { $count } obrazów kluczowych
   *[other] { $count } obrazu kluczowego
}
# machine-drafted, needs native review
figure-keys-pptx = Slajdy (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Folder z plikami PNG
# machine-drafted, needs native review
figure-keys-preset = Ustawienie wstępne
# machine-drafted, needs native review
figure-keys-standard = Standardowe
# machine-drafted, needs native review
figure-keys-teaching = Plik dydaktyczny
# machine-drafted, needs native review
figure-keys-overlays-on = Narysowane i w notatkach
# machine-drafted, needs native review
figure-keys-masks = Tekst wtopiony w obraz
# machine-drafted, needs native review
figure-keys-masks-auto = Zaczerniony
# machine-drafted, needs native review
figure-keys-notes = Notatki do przypadku
# machine-drafted, needs native review
figure-keys-destination = Zapisz w
# machine-drafted, needs native review
figure-keys-empty = Nie ma jeszcze obrazów kluczowych. Najpierw oznacz obraz jako kluczowy.
# machine-drafted, needs native review
figure-keys-teaching-note = Dane pacjenta są pomijane, a tekst wtopiony w obraz zostaje zaczerniony. Przed udostępnieniem sprawdź każdy obraz.
# machine-drafted, needs native review
figure-keys-none-left = Żadnego obrazu nie udało się wyeksportować bez danych pacjenta.
# machine-drafted, needs native review
figure-keys-copied = Skopiowano obrazy kluczowe
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Oznaczono jako obraz kluczowy ({ $count })
    [few] Oznaczono jako obraz kluczowy ({ $count })
    [many] Oznaczono jako obraz kluczowy ({ $count })
   *[other] Oznaczono jako obraz kluczowy ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] To już nie obraz kluczowy (zostaje { $count })
    [few] To już nie obraz kluczowy (zostają { $count })
    [many] To już nie obraz kluczowy (zostaje { $count })
   *[other] To już nie obraz kluczowy (zostaje { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Obraz { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Obraz kluczowy { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = Zapisano: { $name }
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] Zapisano: { $name }. Pominięto { $count } obraz: nie znaleziono wtopionego w niego tekstu.
    [few] Zapisano: { $name }. Pominięto { $count } obrazy: nie znaleziono wtopionego w nie tekstu.
    [many] Zapisano: { $name }. Pominięto { $count } obrazów: nie znaleziono wtopionego w nie tekstu.
   *[other] Zapisano: { $name }. Pominięto { $count } obrazu: nie znaleziono wtopionego w nie tekstu.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Nie udało się wyeksportować: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } już jest w tym folderze. Wybierz pusty folder.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Obraz kluczowy
