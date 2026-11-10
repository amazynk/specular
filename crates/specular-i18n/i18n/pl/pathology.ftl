# machine-drafted, needs native review
# Specular — Patologia: rozdzielanie barwników, eksport i import adnotacji, objętości etykiet nad objętościami. Polski.

## Paleta
palette-stain-separation = Rozdziel barwniki
palette-stain-1 = Pokaż lub ukryj barwnik 1
palette-stain-2 = Pokaż lub ukryj barwnik 2
palette-stain-3 = Pokaż lub ukryj barwnik 3
palette-stain-mode = Barwniki jako światło lub gęstość optyczna
palette-stain-estimate = Oszacuj barwniki z obszaru
palette-stain-normalise = Normalizuj kolory barwników
palette-stain-reference = Użyj obszaru jako wzorca barwników
palette-stain-reset = Opublikowane wektory barwników
palette-export-annotations = Eksportuj adnotacje…
palette-import-annotations = Importuj adnotacje…
palette-label-volume = Nałóż objętość etykiet…

## Powody niedostępności
disabled-not-colour-image = Wymaga obrazu kolorowego
disabled-stains-off = Najpierw włącz rozdzielanie barwników
disabled-no-region = Najpierw narysuj obszar
disabled-no-slide-annotations = Brak adnotacji, obszarów lub punktów do eksportu
disabled-no-label-target = Otwórz objętość w widokach wielopłaszczyznowych albo obraz medyczny

## Nazwy barwników
stain-hematoxylin = Hematoksylina
stain-eosin = Eozyna
stain-dab = DAB
stain-residual = Reszta
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Komunikaty i legenda barwników
stain-off = Rozdzielanie barwników wyłączone
# { $set }: H&E, H-DAB lub H&E-DAB.
stain-on = Barwniki: { $set }
stain-shown = Pokazano: { $stain }
stain-hidden = Ukryto: { $stain }
stain-mode-light = Barwniki jako światło
stain-mode-od = Barwniki jako gęstość optyczna
# Nagłówek legendy: „H&E · światło”, „H-DAB · gęstość optyczna”.
stain-legend-light = { $set } · światło
stain-legend-od = { $set } · gęstość optyczna
stain-legend-estimated = Oszacowane z obszaru
stain-legend-normalised = Znormalizowane
stain-legend-published = Opublikowane wektory
# { $pixels }: użyte zabarwione piksele.
stain-estimated = Barwniki oszacowane z { $pixels } pikseli
stain-estimate-failed = Nie można oszacować barwników: { $reason }
stain-estimate-background = obszar to głównie tło
stain-estimate-one-colour = obszar zawiera jeden kolor
stain-estimate-three = szacowanie wymaga H&E lub H-DAB
stain-estimate-no-pixels = piksele obszaru nie są jeszcze wczytane
stain-reference-set = Obszar ustawiony jako wzorzec barwników
stain-normalise-on = Kolory barwników znormalizowane
stain-normalise-off = Kolory barwników jak ze skanu
stain-reset-done = Opublikowane wektory barwników

## Okno eksportu adnotacji
annot-export-title = Eksportuj adnotacje
annot-export-format = Format
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Maski etykiet
annot-export-output = Maski jako
annot-export-output-png = Kafelki PNG
annot-export-output-tiff = Kafelki TIFF
annot-export-output-ome = Piramida OME-TIFF
annot-export-region = Obszar
annot-export-region-slide = Cały preparat
annot-export-region-view = Widoczny obszar
annot-export-region-drawn = Ostatnio narysowany obszar
annot-export-level = Poziom
# { $level }: 0 to pełna rozdzielczość; { $width } × { $height } to jego rozmiar w pikselach.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Rozmiar kafelka
annot-export-empty = Puste kafelki
annot-export-empty-skip = Pomiń
annot-export-empty-keep = Zapisz
annot-export-what = { $count ->
    [one] 1 kształt
    [few] { $count } kształty
    [many] { $count } kształtów
   *[other] { $count } kształtu
}, { $classes ->
    [one] 1 klasa
    [few] { $classes } klasy
    [many] { $classes } klas
   *[other] { $classes } klasy
}
annot-export-lines = Linie są pomijane w COCO i maskach.
annot-export-export = Eksportuj…
annot-export-writing = Zapisywanie…
# { $files }: zapisane pliki.
annot-export-done = { $files ->
    [one] Wyeksportowano 1 plik
    [few] Wyeksportowano { $files } pliki
    [many] Wyeksportowano { $files } plików
   *[other] Wyeksportowano { $files } pliku
}
annot-export-failed = Eksport nie powiódł się: { $reason }
annot-import-done = { $count ->
    [one] Zaimportowano 1 adnotację
    [few] Zaimportowano { $count } adnotacje
    [many] Zaimportowano { $count } adnotacji
   *[other] Zaimportowano { $count } adnotacji
}
annot-import-failed = Nie można zaimportować: { $reason }
annot-import-skipped = { $count ->
    [one] 1 kształt nie miał użytecznego obrysu
    [few] { $count } kształty nie miały użytecznego obrysu
    [many] { $count } kształtów nie miało użytecznego obrysu
   *[other] { $count } kształtu nie miało użytecznego obrysu
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Objętości etykiet
label-volume-file = Objętość etykiet
label-volume-table = Tabela kolorów
# { $file }: nazwa pliku objętości etykiet; { $count }: jej etykiety.
label-volume-loaded = { $count ->
    [one] { $file }: 1 etykieta
    [few] { $file }: { $count } etykiety
    [many] { $file }: { $count } etykiet
   *[other] { $file }: { $count } etykiety
}
label-volume-failed = Nie można nałożyć { $file }: { $reason }
label-volume-outside = { $file } nie pokrywa się z tą objętością
label-volume-not-whole = jej wartości nie są liczbami całkowitymi
label-volume-colour = zawiera kolor, a nie etykiety
label-volume-empty = nie ma etykiet
label-volume-too-many = ma za dużo etykiet
label-volume-too-large = jest za duża do przepróbkowania
label-volume-degenerate = jej orientacja jest nieprawidłowa
# Etykieta bez nazwy w tabeli kolorów.
label-volume-unnamed = Etykieta { $value }
