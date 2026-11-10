# Specular — Måleværktøj (længder, vinkler, områder og deres statistik på ethvert billede). Maskinoversat, skal gennemses af en indfødt.

palette-measure = Mål
palette-measure-copy-csv = Kopiér målinger som CSV
palette-measure-export = Eksportér målinger…
toast-copied-measurements = Målinger kopieret som CSV

measure-tool-title = Mål
measure-tool-list = Målinger
measure-tool-count = { $count ->
    [0] Ingen målinger
    [one] Én måling
   *[other] { $count } målinger
}
measure-tool-empty = Vælg et værktøj, og tegn på billedet.

measure-tool-point = Punkt
measure-tool-line = Linje
measure-tool-polyline = Polylinje
measure-tool-angle = Vinkel
measure-tool-rect = Rektangel
measure-tool-ellipse = Ellipse
measure-tool-polygon = Polygon
measure-tool-freehand = Frihånd
measure-tool-calibrate = Kalibrér
# machine-drafted, needs native review
measure-tool-spot = Målepunkt

measure-tool-undo = Fortryd
measure-tool-delete = Slet
measure-tool-copy = Kopiér CSV
measure-tool-export = Eksportér…
measure-tool-import = Importér…
measure-tool-done = Færdig
measure-tool-unit = Enhed: { $unit }
measure-tool-apply = Anvend

measure-tool-hint-line = Træk, eller klik på to punkter. Pilene flytter trådkorset, Mellemrum sætter et punkt.
measure-tool-hint-angle = Klik på tre punkter; det andet er toppunktet.
measure-tool-hint-rect = Træk, eller klik på to hjørner.
measure-tool-hint-ellipse = Træk, eller klik på to hjørner af dens ramme.
measure-tool-hint-polygon = Klik på hjørnerne; Enter eller et klik på det første hjørne lukker den.
measure-tool-hint-calibrate = Tegn en linje hen over noget med kendt længde.
# machine-drafted, needs native review
measure-tool-hint-spot = Klik på et punkt for at aflæse dets temperatur.
measure-tool-known-length = Kendt længde: { $value } { $unit } (skriv tallet, U skifter enhed, Enter anvender)
measure-tool-calibrated = Kalibreret: én pixel er { $size } { $unit }
measure-tool-bad-length = Skriv en længde større end nul.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Areal { $value }
measure-tool-mean = Middel { $mean } ± { $sd }
measure-tool-means = Middel { $values }
measure-tool-range = Interval { $min } – { $max } over { $count } px
measure-tool-pixels = Over { $count } px
measure-tool-stats-pending = Læser pixels…
measure-tool-stats-none = Ingen pixelværdier her
measure-tool-approximate = Fra en formindsket kopi (omtrentlig)

measure-tool-deleted = Måling slettet
measure-tool-undone = Fortrudt
measure-tool-nothing = Ingen målinger på dette billede
measure-tool-exported = Målinger gemt i { $name }
measure-tool-export-failed = Kunne ikke gemme målingerne: { $reason }
measure-tool-imported = { $count ->
    [one] Én måling importeret.
   *[other] { $count } målinger importeret.
}
measure-tool-skipped = { $count ->
    [one] Én figur kunne ikke læses.
   *[other] { $count } figurer kunne ikke læses.
}
measure-tool-import-failed = Kunne ikke læse målingerne: { $reason }
measure-tool-too-large = Filen er for stor.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR kræver et DICOM-billede. Gem som CSV eller GeoJSON i stedet.

measure-source-uncalibrated = Pixels (ikke kalibreret)
measure-source-dicom-pixel-spacing = DICOM-pixelafstand
measure-source-dicom-calibrated = DICOM kalibreret afstand
measure-source-dicom-imager-pixel-spacing = DICOM-detektorafstand (ved detektoren)
measure-source-dicom-magnification = DICOM-afstand korrigeret for forstørrelse
measure-source-dicom-ultrasound-region = DICOM-ultralydsområde
measure-source-tiff-resolution = TIFF-opløsning
measure-source-exif-resolution = EXIF-opløsning
measure-source-ome-physical-size = OME fysisk størrelse
measure-source-fits-wcs = FITS-verdenskoordinater
measure-source-vendor-tag = Producentmetadata
measure-source-drawing-units = Tegningsenheder
# machine-drafted, needs native review
measure-source-drawing-scale = Fra tegningens målestok
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Fra tegningen (målestok { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Fra tegningen (målestok { $scale }, visningsområde { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Kortets målestok i midten (omtrentlig; kortværktøjerne måler på Jorden)
measure-source-user = Din kalibrering
