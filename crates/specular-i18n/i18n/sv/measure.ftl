# Specular — Mätverktyget (längder, vinklar, områden och deras statistik på valfri bild). Maskinöversatt, behöver granskas av en infödd talare.

palette-measure = Mät
palette-measure-copy-csv = Kopiera mätningar som CSV
palette-measure-export = Exportera mätningar…
toast-copied-measurements = Mätningarna kopierades som CSV

measure-tool-title = Mät
measure-tool-list = Mätningar
measure-tool-count = { $count ->
    [0] Inga mätningar
    [one] En mätning
   *[other] { $count } mätningar
}
measure-tool-empty = Välj ett verktyg och rita på bilden.

measure-tool-point = Punkt
measure-tool-line = Linje
measure-tool-polyline = Polylinje
measure-tool-angle = Vinkel
measure-tool-rect = Rektangel
measure-tool-ellipse = Ellips
measure-tool-polygon = Polygon
measure-tool-freehand = Frihand
measure-tool-calibrate = Kalibrera
# machine-drafted, needs native review
measure-tool-spot = Mätpunkt

measure-tool-undo = Ångra
measure-tool-delete = Ta bort
measure-tool-copy = Kopiera CSV
measure-tool-export = Exportera…
measure-tool-import = Importera…
measure-tool-done = Klar
measure-tool-unit = Enhet: { $unit }
measure-tool-apply = Verkställ

measure-tool-hint-line = Dra eller klicka på två punkter. Piltangenterna flyttar hårkorset, Blanksteg sätter en punkt.
measure-tool-hint-angle = Klicka på tre punkter; den andra är spetsen.
measure-tool-hint-rect = Dra eller klicka på två hörn.
measure-tool-hint-ellipse = Dra eller klicka på två hörn av dess ram.
measure-tool-hint-polygon = Klicka på hörnen; Retur eller ett klick på första hörnet stänger den.
measure-tool-hint-calibrate = Rita en linje över något med känd längd.
# machine-drafted, needs native review
measure-tool-hint-spot = Klicka på en punkt för att läsa av dess temperatur.
measure-tool-known-length = Känd längd: { $value } { $unit } (skriv talet, U byter enhet, Retur verkställer)
measure-tool-calibrated = Kalibrerad: en pixel är { $size } { $unit }
measure-tool-bad-length = Skriv en längd större än noll.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Area { $value }
measure-tool-mean = Medel { $mean } ± { $sd }
measure-tool-means = Medel { $values }
measure-tool-range = Intervall { $min } – { $max } över { $count } px
measure-tool-pixels = Över { $count } px
measure-tool-stats-pending = Läser pixlar…
measure-tool-stats-none = Inga pixelvärden här
measure-tool-approximate = Från en förminskad kopia (ungefärligt)

measure-tool-deleted = Mätningen togs bort
measure-tool-undone = Ångrat
measure-tool-nothing = Inga mätningar på den här bilden
measure-tool-exported = Mätningarna sparades i { $name }
measure-tool-export-failed = Det gick inte att spara mätningarna: { $reason }
measure-tool-imported = { $count ->
    [one] En mätning importerades.
   *[other] { $count } mätningar importerades.
}
measure-tool-skipped = { $count ->
    [one] En form kunde inte läsas.
   *[other] { $count } former kunde inte läsas.
}
measure-tool-import-failed = Det gick inte att läsa mätningarna: { $reason }
measure-tool-too-large = Filen är för stor.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR kräver en DICOM-bild. Spara som CSV eller GeoJSON i stället.

measure-source-uncalibrated = Pixlar (inte kalibrerat)
measure-source-dicom-pixel-spacing = DICOM-pixelavstånd
measure-source-dicom-calibrated = DICOM kalibrerat avstånd
measure-source-dicom-imager-pixel-spacing = DICOM-detektoravstånd (vid detektorn)
measure-source-dicom-magnification = DICOM-avstånd korrigerat för förstoring
measure-source-dicom-ultrasound-region = DICOM-ultraljudsområde
measure-source-tiff-resolution = TIFF-upplösning
measure-source-exif-resolution = EXIF-upplösning
measure-source-ome-physical-size = OME fysisk storlek
measure-source-fits-wcs = FITS-världskoordinater
measure-source-vendor-tag = Tillverkarens metadata
measure-source-drawing-units = Ritningsenheter
# machine-drafted, needs native review
measure-source-drawing-scale = Från ritningens skala
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Från ritningen (skala { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Från ritningen (skala { $scale }, vy { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Kartskala i mitten (ungefärlig; Kartor mäter på jorden)
measure-source-user = Din kalibrering
