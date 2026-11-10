# machine-drafted, needs native review
# Specular — Pathologie: Färbungstrennung, Export und Import von Annotationen, Labelvolumen über Volumen. Deutsch.

## Palette
palette-stain-separation = Färbungen trennen
palette-stain-1 = Färbung 1 ein- oder ausblenden
palette-stain-2 = Färbung 2 ein- oder ausblenden
palette-stain-3 = Färbung 3 ein- oder ausblenden
palette-stain-mode = Färbungen als Licht oder optische Dichte
palette-stain-estimate = Färbungen aus Region schätzen
palette-stain-normalise = Färbungsfarben normalisieren
palette-stain-reference = Region als Färbungsreferenz verwenden
palette-stain-reset = Veröffentlichte Färbungsvektoren
palette-export-annotations = Annotationen exportieren…
palette-import-annotations = Annotationen importieren…
palette-label-volume = Labelvolumen überlagern…

## Gründe für deaktivierte Befehle
disabled-not-colour-image = Benötigt ein Farbbild
disabled-stains-off = Schalten Sie zuerst die Färbungstrennung ein
disabled-no-region = Zeichnen Sie zuerst eine Region
disabled-no-slide-annotations = Keine Annotationen, Regionen oder Punkte zum Exportieren
disabled-no-label-target = Öffnen Sie ein Volumen in den multiplanaren Ansichten oder ein medizinisches Bild

## Namen der Färbungen
stain-hematoxylin = Hämatoxylin
stain-eosin = Eosin
stain-dab = DAB
stain-residual = Rest
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Meldungen und Legende der Färbungen
stain-off = Färbungstrennung aus
# { $set }: H&E, H-DAB oder H&E-DAB.
stain-on = Färbungen: { $set }
stain-shown = { $stain } eingeblendet
stain-hidden = { $stain } ausgeblendet
stain-mode-light = Färbungen als Licht
stain-mode-od = Färbungen als optische Dichte
# Überschrift der Legende: „H&E · Licht“, „H-DAB · optische Dichte“.
stain-legend-light = { $set } · Licht
stain-legend-od = { $set } · optische Dichte
stain-legend-estimated = Aus Region geschätzt
stain-legend-normalised = Normalisiert
stain-legend-published = Veröffentlichte Vektoren
# { $pixels }: verwendete gefärbte Pixel.
stain-estimated = Färbungen aus { $pixels } Pixeln geschätzt
stain-estimate-failed = Färbungen nicht schätzbar: { $reason }
stain-estimate-background = die Region ist größtenteils Hintergrund
stain-estimate-one-colour = die Region enthält nur eine Farbe
stain-estimate-three = Schätzung braucht H&E oder H-DAB
stain-estimate-no-pixels = die Pixel der Region sind noch nicht geladen
stain-reference-set = Region als Färbungsreferenz gesetzt
stain-normalise-on = Färbungsfarben normalisiert
stain-normalise-off = Färbungsfarben wie gescannt
stain-reset-done = Veröffentlichte Färbungsvektoren

## Dialog: Annotationen exportieren
annot-export-title = Annotationen exportieren
annot-export-format = Format
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Labelmasken
annot-export-output = Masken als
annot-export-output-png = PNG-Kacheln
annot-export-output-tiff = TIFF-Kacheln
annot-export-output-ome = OME-TIFF-Pyramide
annot-export-region = Region
annot-export-region-slide = Ganzer Objektträger
annot-export-region-view = Sichtbarer Bereich
annot-export-region-drawn = Zuletzt gezeichnete Region
annot-export-level = Ebene
# { $level }: 0 für volle Auflösung; { $width } × { $height } ihre Größe in Pixeln.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Kachelgröße
annot-export-empty = Leere Kacheln
annot-export-empty-skip = Überspringen
annot-export-empty-keep = Schreiben
annot-export-what = { $count ->
    [one] 1 Form
   *[other] { $count } Formen
}, { $classes ->
    [one] 1 Klasse
   *[other] { $classes } Klassen
}
annot-export-lines = Linien fehlen in COCO und Masken.
annot-export-export = Exportieren…
annot-export-writing = Wird geschrieben…
# { $files }: geschriebene Dateien.
annot-export-done = { $files ->
    [one] 1 Datei exportiert
   *[other] { $files } Dateien exportiert
}
annot-export-failed = Export fehlgeschlagen: { $reason }
annot-import-done = { $count ->
    [one] 1 Annotation importiert
   *[other] { $count } Annotationen importiert
}
annot-import-failed = Import nicht möglich: { $reason }
annot-import-skipped = { $count ->
    [one] 1 Form hatte keinen brauchbaren Umriss
   *[other] { $count } Formen hatten keinen brauchbaren Umriss
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Labelvolumen
label-volume-file = Labelvolumen
label-volume-table = Farbtabelle
# { $file }: Dateiname des Labelvolumens; { $count }: seine Labels.
label-volume-loaded = { $count ->
    [one] { $file }: 1 Label
   *[other] { $file }: { $count } Labels
}
label-volume-failed = { $file } kann nicht überlagert werden: { $reason }
label-volume-outside = { $file } überlappt dieses Volumen nicht
label-volume-not-whole = seine Werte sind keine ganzen Zahlen
label-volume-colour = es enthält Farben, keine Labels
label-volume-empty = es hat keine Labels
label-volume-too-many = es hat zu viele Labels
label-volume-too-large = es ist zu groß zum Neuabtasten
label-volume-degenerate = seine Ausrichtung ist ungültig
# Ein Label ohne Namen in der Farbtabelle.
label-volume-unnamed = Label { $value }
