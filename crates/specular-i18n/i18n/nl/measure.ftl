# Specular — Meetgereedschap (lengtes, hoeken, gebieden en hun statistieken op elke afbeelding). Machinaal opgesteld, moet door een moedertaalspreker worden nagekeken.

palette-measure = Meten
palette-measure-copy-csv = Metingen kopiëren als CSV
palette-measure-export = Metingen exporteren…
toast-copied-measurements = Metingen gekopieerd als CSV

measure-tool-title = Meten
measure-tool-list = Metingen
measure-tool-count = { $count ->
    [0] Geen metingen
    [one] Eén meting
   *[other] { $count } metingen
}
measure-tool-empty = Kies een gereedschap en teken op de afbeelding.

measure-tool-point = Punt
measure-tool-line = Lijn
measure-tool-polyline = Polylijn
measure-tool-angle = Hoek
measure-tool-rect = Rechthoek
measure-tool-ellipse = Ellips
measure-tool-polygon = Veelhoek
measure-tool-freehand = Uit de vrije hand
measure-tool-calibrate = Kalibreren
# machine-drafted, needs native review
measure-tool-spot = Meetpunt

measure-tool-undo = Ongedaan maken
measure-tool-delete = Verwijderen
measure-tool-copy = CSV kopiëren
measure-tool-export = Exporteren…
measure-tool-import = Importeren…
measure-tool-done = Klaar
measure-tool-unit = Eenheid: { $unit }
measure-tool-apply = Toepassen

measure-tool-hint-line = Sleep of klik op twee punten. Pijltjestoetsen verplaatsen het dradenkruis, Spatie plaatst een punt.
measure-tool-hint-angle = Klik op drie punten; het tweede is het hoekpunt.
measure-tool-hint-rect = Sleep of klik op twee hoeken.
measure-tool-hint-ellipse = Sleep of klik op twee hoeken van het kader.
measure-tool-hint-polygon = Klik op de hoeken; Enter of een klik op de eerste hoek sluit de vorm.
measure-tool-hint-calibrate = Trek een lijn over iets met een bekende lengte.
# machine-drafted, needs native review
measure-tool-hint-spot = Klik op een punt om de temperatuur ervan af te lezen.
measure-tool-known-length = Bekende lengte: { $value } { $unit } (typ het getal, U wisselt de eenheid, Enter past toe)
measure-tool-calibrated = Gekalibreerd: één pixel is { $size } { $unit }
measure-tool-bad-length = Typ een lengte groter dan nul.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Oppervlakte { $value }
measure-tool-mean = Gemiddelde { $mean } ± { $sd }
measure-tool-means = Gemiddelde { $values }
measure-tool-range = Bereik { $min } – { $max } over { $count } px
measure-tool-pixels = Over { $count } px
measure-tool-stats-pending = Pixels lezen…
measure-tool-stats-none = Hier geen pixelwaarden
measure-tool-approximate = Uit een verkleinde kopie (bij benadering)

measure-tool-deleted = Meting verwijderd
measure-tool-undone = Ongedaan gemaakt
measure-tool-nothing = Geen metingen op deze afbeelding
measure-tool-exported = Metingen opgeslagen in { $name }
measure-tool-export-failed = Kan de metingen niet opslaan: { $reason }
measure-tool-imported = { $count ->
    [one] Eén meting geïmporteerd.
   *[other] { $count } metingen geïmporteerd.
}
measure-tool-skipped = { $count ->
    [one] Eén vorm kon niet worden gelezen.
   *[other] { $count } vormen konden niet worden gelezen.
}
measure-tool-import-failed = Kan de metingen niet lezen: { $reason }
measure-tool-too-large = Het bestand is te groot.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR vereist een DICOM-afbeelding. Sla in plaats daarvan op als CSV of GeoJSON.

measure-source-uncalibrated = Pixels (niet gekalibreerd)
measure-source-dicom-pixel-spacing = DICOM-pixelafstand
measure-source-dicom-calibrated = DICOM gekalibreerde afstand
measure-source-dicom-imager-pixel-spacing = DICOM-detectorafstand (bij de detector)
measure-source-dicom-magnification = DICOM-afstand gecorrigeerd voor vergroting
measure-source-dicom-ultrasound-region = DICOM-echogebied
measure-source-tiff-resolution = TIFF-resolutie
measure-source-exif-resolution = EXIF-resolutie
measure-source-ome-physical-size = OME fysieke grootte
measure-source-fits-wcs = FITS-wereldcoördinaten
measure-source-vendor-tag = Metadata van de fabrikant
measure-source-drawing-units = Tekeneenheden
# machine-drafted, needs native review
measure-source-drawing-scale = Uit de tekenschaal
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Uit de tekening (schaal { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Uit de tekening (schaal { $scale }, viewport { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Kaartschaal in het midden (bij benadering; Kaarten meet op de aarde)
measure-source-user = Uw kalibratie
