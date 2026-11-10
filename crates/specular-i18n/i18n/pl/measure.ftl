# Specular — Narzędzie pomiaru (długości, kąty, obszary i ich statystyki na dowolnym obrazie). Tłumaczenie maszynowe, wymaga weryfikacji przez native speakera.

palette-measure = Mierz
palette-measure-copy-csv = Kopiuj pomiary jako CSV
palette-measure-export = Eksportuj pomiary…
toast-copied-measurements = Skopiowano pomiary jako CSV

measure-tool-title = Pomiar
measure-tool-list = Pomiary
measure-tool-count = { $count ->
    [0] Brak pomiarów
    [one] Jeden pomiar
    [few] { $count } pomiary
    [many] { $count } pomiarów
   *[other] { $count } pomiaru
}
measure-tool-empty = Wybierz narzędzie i rysuj na obrazie.

measure-tool-point = Punkt
measure-tool-line = Linia
measure-tool-polyline = Łamana
measure-tool-angle = Kąt
measure-tool-rect = Prostokąt
measure-tool-ellipse = Elipsa
measure-tool-polygon = Wielokąt
measure-tool-freehand = Odręcznie
measure-tool-calibrate = Kalibruj
# machine-drafted, needs native review
measure-tool-spot = Punkt pomiarowy

measure-tool-undo = Cofnij
measure-tool-delete = Usuń
measure-tool-copy = Kopiuj CSV
measure-tool-export = Eksportuj…
measure-tool-import = Importuj…
measure-tool-done = Gotowe
measure-tool-unit = Jednostka: { $unit }
measure-tool-apply = Zastosuj

measure-tool-hint-line = Przeciągnij lub kliknij dwa punkty. Strzałki przesuwają celownik, Spacja stawia punkt.
measure-tool-hint-angle = Kliknij trzy punkty; drugi to wierzchołek.
measure-tool-hint-rect = Przeciągnij lub kliknij dwa narożniki.
measure-tool-hint-ellipse = Przeciągnij lub kliknij dwa narożniki jej ramki.
measure-tool-hint-polygon = Klikaj narożniki; Enter lub kliknięcie pierwszego narożnika zamyka kształt.
measure-tool-hint-calibrate = Narysuj linię wzdłuż czegoś o znanej długości.
# machine-drafted, needs native review
measure-tool-hint-spot = Kliknij punkt, aby odczytać jego temperaturę.
measure-tool-known-length = Znana długość: { $value } { $unit } (wpisz liczbę, U zmienia jednostkę, Enter zatwierdza)
measure-tool-calibrated = Skalibrowano: jeden piksel to { $size } { $unit }
measure-tool-bad-length = Wpisz długość większą od zera.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Pole { $value }
measure-tool-mean = Średnia { $mean } ± { $sd }
measure-tool-means = Średnia { $values }
measure-tool-range = Zakres { $min } – { $max } z { $count } px
measure-tool-pixels = Z { $count } px
measure-tool-stats-pending = Odczyt pikseli…
measure-tool-stats-none = Brak wartości pikseli
measure-tool-approximate = Z pomniejszonej kopii (przybliżone)

measure-tool-deleted = Usunięto pomiar
measure-tool-undone = Cofnięto
measure-tool-nothing = Brak pomiarów na tym obrazie
measure-tool-exported = Zapisano pomiary do { $name }
measure-tool-export-failed = Nie można zapisać pomiarów: { $reason }
measure-tool-imported = { $count ->
    [one] Zaimportowano jeden pomiar.
    [few] Zaimportowano { $count } pomiary.
    [many] Zaimportowano { $count } pomiarów.
   *[other] Zaimportowano { $count } pomiaru.
}
measure-tool-skipped = { $count ->
    [one] Nie można odczytać jednego kształtu.
    [few] Nie można odczytać { $count } kształtów.
    [many] Nie można odczytać { $count } kształtów.
   *[other] Nie można odczytać { $count } kształtu.
}
measure-tool-import-failed = Nie można odczytać pomiarów: { $reason }
measure-tool-too-large = Plik jest za duży.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR wymaga obrazu DICOM. Zapisz zamiast tego jako CSV lub GeoJSON.

measure-source-uncalibrated = Piksele (bez kalibracji)
measure-source-dicom-pixel-spacing = Odstęp pikseli DICOM
measure-source-dicom-calibrated = Skalibrowany odstęp DICOM
measure-source-dicom-imager-pixel-spacing = Odstęp DICOM na detektorze
measure-source-dicom-magnification = Odstęp DICOM z korekcją powiększenia
measure-source-dicom-ultrasound-region = Region USG DICOM
measure-source-tiff-resolution = Rozdzielczość TIFF
measure-source-exif-resolution = Rozdzielczość EXIF
measure-source-ome-physical-size = Rozmiar fizyczny OME
measure-source-fits-wcs = Współrzędne świata FITS
measure-source-vendor-tag = Metadane producenta
measure-source-drawing-units = Jednostki rysunku
# machine-drafted, needs native review
measure-source-drawing-scale = Ze skali rysunku
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Z rysunku (skala { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Z rysunku (skala { $scale }, rzutnia { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Skala mapy w środku (przybliżona; Mapy mierzą na Ziemi)
measure-source-user = Twoja kalibracja
