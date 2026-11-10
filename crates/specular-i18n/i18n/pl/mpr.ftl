# Multiplanar reformatting (MPR).

palette-mpr = Rekonstrukcja wielopłaszczyznowa (MPR)
palette-mpr-slab = Następna warstwa MPR (cienka, MIP, MinIP, średnia)
disabled-no-volume = To nie jest objętość: otwórz serię, plik wieloklatkowy, objętość 3D lub stos z
disabled-mpr-closed = Najpierw otwórz widoki wielopłaszczyznowe
mpr-axial = Osiowa
mpr-coronal = Czołowa
mpr-sagittal = Strzałkowa
mpr-oblique = Skośna
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = Widok 3D
mpr-thin = Cienka
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Średnia
mpr-slab = Warstwa: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Warstwa: { $mode }, { $thickness } Å
mpr-slab-unit = Warstwa: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Warstwa: { $mode }, woksele: { $thickness }
mpr-thicker = Grubsza warstwa
mpr-thinner = Cieńsza warstwa
mpr-reset = Resetuj widoki
mpr-close = Zamknij MPR
mpr-maximise = Maksymalizuj widok
mpr-export-planes = Eksportuj płaszczyzny
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Linie prądu z { $files }: warstwa { $thickness } mm
mpr-tracts-off = Linie prądu z { $files }: ukryte
mpr-tracts-none = Brak linii prądu TrackVis ani MRtrix obok tego wolumenu
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = woksele: { $value }
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } z { $count }
mpr-axis-channel = Kanał
mpr-axis-unnamed = Oś { $axis }
mpr-one-channel = MPR pokazuje jeden kanał naraz
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = Kanały w kolorze: { $count }, każdy w swoim zakresie wyświetlania
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Pokazano jeden kanał: { $reason }
mpr-isotropic = Brak rozmiaru woksela w pliku: woksele pokazane jako sześciany
mpr-isotropic-z = Brak odstępu przekrojów w pliku: przekroje pokazane co jeden piksel
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Wczytywanie przekrojów: { $loaded } z { $count }
mpr-loaded = Przekroje: { $count }, { $width } × { $height }
mpr-downsampled = Pokazano w rozdzielczości 1/{ $factor } (limity pamięci lub grafiki)
mpr-converted = Wartości pokazane jako 32-bitowe liczby zmiennoprzecinkowe
mpr-opening = Otwieranie objętości…
mpr-gpu-memory = { $size } na karcie graficznej
mpr-refused = Nie można zbudować objętości: { $reason }
mpr-failed = Nie wczytano objętości: { $reason }
mpr-no-gpu = Karta graficzna nie pomieści tej objętości: { $reason }
mpr-planes-saved = { $count ->
    [one] Zapisano { $count } płaszczyznę w { $folder }
    [few] Zapisano { $count } płaszczyzny w { $folder }
    [many] Zapisano { $count } płaszczyzn w { $folder }
   *[other] Zapisano { $count } płaszczyzny w { $folder }
}
mpr-planes-failed = Nie udało się zapisać płaszczyzn: { $reason }
mpr-view-a11y = Widok { $view }, { $position } mm
mpr-crosshair-a11y = Celownik w { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Celownik w { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Celownik w { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Celownik w wokselu { $x }, { $y }, { $z }
mpr-controls = Sterowanie MPR

# 3D view palette entries.
palette-mpr-preset = Następne ustawienie 3D (TK kość, tkanki miękkie, płuca, angio; MR)
palette-mpr-render = Następne renderowanie 3D (cieniowane, Cinematic, MIP, MinIP)
palette-mpr-clip = Następne przycięcie 3D (brak, ramka widoków 2D, wycięty narożnik)
palette-mpr-export-stl = Eksportuj powierzchnię 3D jako STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Następna warstwa linii prądu w MPR
palette-mpr-maximise = Maksymalizuj widok MPR lub pokaż wszystkie widoki
palette-mpr-export-planes = Eksportuj płaszczyzny MPR jako obrazy…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] TK kość
    [soft] TK tkanki miękkie
    [lung] TK płuca
    [angio] Angio-TK
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Cieniowane
}
vr-clip = { $clip ->
    [box] Przycięcie: ramka widoków
    [cut] Przycięcie: wycięty narożnik
   *[off] Bez przycięcia
}
vr-smooth = Wygładź powierzchnię
vr-export = Eksportuj STL
vr-threshold = Powierzchnia przy { $value }
vr-stl-exporting = Budowanie powierzchni…
vr-stl-saved = { $count ->
    [0] Nic przy tym progu: nie zapisano powierzchni
   *[other] Zapisano do { $name }, trójkąty: { $count }
}
vr-stl-failed = Nie można zapisać powierzchni: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, powierzchnia przy { $threshold }. Shift+strzałki obracają, plus i minus powiększają.

## Surowa objętość: jak zbudowany jest plik .raw / .vol bez nagłówka

# machine-drafted, needs native review
raw-volume-title = Otwórz surową objętość
raw-volume-file = { $file } nie ma nagłówka. Określ, jak ułożone są próbki.
raw-volume-guess-label = Sugerowany układ
raw-volume-guess = { $index } z { $count }
raw-volume-custom = Własny
raw-volume-width = Szerokość (woksele)
raw-volume-height = Wysokość (woksele)
raw-volume-slices = Przekroje
raw-volume-type = Typ próbki
raw-volume-type-unsigned = { $bits }-bitowa bez znaku
raw-volume-type-signed = { $bits }-bitowa ze znakiem
raw-volume-type-float = { $bits }-bitowa zmiennoprzecinkowa
raw-volume-order = Kolejność bajtów
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Przesunięcie nagłówka
raw-volume-bytes = { $count ->
    [one] { $count } bajt
    [few] { $count } bajty
    [many] { $count } bajtów
   *[other] { $count } bajta
}
raw-volume-pixel = Rozmiar piksela
raw-volume-spacing = Odstęp przekrojów
raw-volume-mm = { $value } mm
raw-volume-exact = Dokładnie wypełnia plik ({ $size })
raw-volume-fits = Używa { $size } z { $file }; pozostaje { $left }
raw-volume-too-big = Wymaga { $size }, ale plik ma { $file }
raw-volume-cancel = Anuluj
raw-volume-open = Otwórz
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Łączność: { $seed }; przesuń celownik na inny punkt seed
# machine-drafted, needs native review
mpr-measure-hint-edit = Przeciągnij koniec, aby go przesunąć; na drugi koniec, aby usunąć pomiar
# machine-drafted, needs native review
mpr-measure-removed = Pomiar usunięty
mpr-measure-lengths-only = MPR mierzy tylko długości
mpr-more = Więcej…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Krzywoliniowa MPR wzdłuż klikniętej krzywej
palette-mpr-curve-layout = Krzywoliniowa MPR: wyprostowana lub rozciągnięta
palette-mpr-curve-save = Zapisz krzywą…
palette-mpr-curve-load = Wczytaj krzywą…
palette-mpr-export-segments = Eksportuj każdy segment jako STL…
palette-mpr-dental = Układ stomatologiczny (pantomogram i przekroje poprzeczne)
palette-mpr-nerve = Wyznacz kanał żuchwy
cpr-kind = { $kind ->
    [stretched] Rozciągnięta
   *[straightened] Wyprostowana
}
cpr-button = Krzywoliniowa
cpr-controls = Sterowanie krzywoliniową MPR
cpr-at = W { $position }
cpr-section = Przekrój poprzeczny
cpr-hint-start = Klikaj punkty wzdłuż struktury w widoku { $view }
cpr-hint-more = Kliknij jeszcze jeden punkt
cpr-add-points = Dodaj punkty
cpr-undo-point = Cofnij punkt
# machine-drafted, needs native review
cpr-mark = Oznacz tutaj
cpr-mark-set = Ustawiono znacznik { $n }.
cpr-mark-removed = Usunięto znacznik.
cpr-mark-here = Znacznik { $n }
cpr-diameter = Średnica { $value }
cpr-diameters = Średnica od { $min } do { $max }
cpr-mark-span = Znaczniki od { $from } do { $to }: { $length } wzdłuż linii środkowej
cpr-mark-total = Wszystkie znaczniki: { $length }
cpr-clear = Wyczyść krzywą
cpr-save = Zapisz krzywą
cpr-load = Wczytaj krzywą
cpr-done = Gotowe
cpr-a11y = { $name } na długości { $length }, kursor w { $position }. Kółko przesuwa kursor.
cpr-none = Brak krzywej: najpierw kliknij punkty
cpr-cleared = Krzywa wyczyszczona
cpr-saved = Krzywa zapisana: { $name }
cpr-save-failed = Nie można zapisać krzywej: { $reason }
cpr-loaded = Krzywa wczytana
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] Wczytano 1 linię środkową
    [few] Wczytano { $count } linie środkowe
    [many] Wczytano { $count } linii środkowych
   *[other] Wczytano { $count } linii środkowej
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] Znaleziono 1 zapisaną linię środkową
    [few] Znaleziono { $count } zapisane linie środkowe
    [many] Znaleziono { $count } zapisanych linii środkowych
   *[other] Znaleziono { $count } zapisanej linii środkowej
}
# A centreline without a name, by its place in the list.
cpr-centreline = Linia środkowa { $number }
cpr-new = Nowa linia środkowa
cpr-rename = Zmień nazwę
cpr-name = Nazwa linii środkowej
cpr-delete = Usuń
# $name: the centreline's name.
cpr-delete-title = Usunąć linię środkową „{ $name }”?
cpr-delete-detail = Jej punkty znikną. Pliki zapisane na dysku pozostaną.
cpr-deleted = Usunięto „{ $name }”
cpr-load-failed = Nie można wczytać krzywej: { $reason }
cpr-load-not-json = plik nie jest w formacie JSON
cpr-load-not-curves = plik nie jest krzywą programu Specular
cpr-load-empty = plik nie zawiera krzywej z co najmniej dwóch punktów
cpr-panoramic = Pantomogram
cpr-panoramic-slab = Pantomogram, warstwa { $thickness } mm
cpr-panoramic-thin = Pantomogram, cienki

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Stomatologia
dental-not-for-diagnosis = Nie do diagnostyki ani planowania leczenia
dental-nerve = Kanał żuchwy
dental-nerve-hint = Klikaj punkty wzdłuż kanału żuchwy na pantomogramie lub przekroju poprzecznym
dental-nerve-done = Wyznaczanie kanału żuchwy wyłączone
dental-spacing = Co { $spacing } mm
dental-section = Przekrój { $number }
dental-off = Najpierw otwórz układ stomatologiczny
dental-suggesting = Wyszukiwanie łuku zębowego…
dental-suggested = Zaproponowano łuk: przeciągnij jego punkty, aby go dopasować
dental-no-arch = Nie znaleziono łuku: klikaj punkty wzdłuż niego w widoku osiowym
dental-vendor = Stomatologiczna CBCT { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Eksportuj segmenty
seg-stl-detail = Segmenty: { $count ->
        [one] { $count } trójkąt
        [few] { $count } trójkąty
        [many] { $count } trójkątów
       *[other] { $count } trójkąta
    }
seg-stl-detail-full = Segmenty: pełna szczegółowość
seg-stl-none = Brak segmentów do eksportu
seg-stl-not-labels = ten wolumen nie jest mapą etykiet
seg-stl-exporting = Budowanie powierzchni segmentów…
seg-stl-saved = { $count ->
    [one] Zapisano { $count } segment w { $folder }
    [few] Zapisano { $count } segmenty w { $folder }
    [many] Zapisano { $count } segmentów w { $folder }
   *[other] Zapisano { $count } segmentu w { $folder }
}
seg-stl-open = Niezamknięte: { $count }.
seg-stl-empty = Segmenty są puste: nic nie zapisano
seg-stl-failed = Nie można zapisać segmentów: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Trzy płaszczyzny (MPR)
toolbar-mpr-close = Zamknij trzy płaszczyzny (MPR)
settings-viewing-toolbar-items-mpr = Trzy płaszczyzny (MPR)
