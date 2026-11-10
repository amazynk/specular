# Specular — Messwerkzeug (Längen, Winkel, Regionen und ihre Statistik auf jedem Bild). Maschinell erstellt, muss muttersprachlich geprüft werden.

palette-measure = Messen
palette-measure-copy-csv = Messungen als CSV kopieren
palette-measure-export = Messungen exportieren…
toast-copied-measurements = Messungen als CSV kopiert

measure-tool-title = Messen
measure-tool-list = Messungen
measure-tool-count = { $count ->
    [0] Keine Messungen
    [one] Eine Messung
   *[other] { $count } Messungen
}
measure-tool-empty = Werkzeug wählen und auf dem Bild zeichnen.

measure-tool-point = Punkt
measure-tool-line = Linie
measure-tool-polyline = Polylinie
measure-tool-angle = Winkel
measure-tool-rect = Rechteck
measure-tool-ellipse = Ellipse
measure-tool-polygon = Polygon
measure-tool-freehand = Freihand
measure-tool-calibrate = Kalibrieren
# machine-drafted, needs native review
measure-tool-spot = Messpunkt

measure-tool-undo = Rückgängig
measure-tool-delete = Löschen
measure-tool-copy = CSV kopieren
measure-tool-export = Exportieren…
measure-tool-import = Importieren…
measure-tool-done = Fertig
measure-tool-unit = Einheit: { $unit }
measure-tool-apply = Anwenden

measure-tool-hint-line = Ziehen oder zwei Punkte anklicken. Pfeiltasten bewegen das Fadenkreuz, Leertaste setzt einen Punkt.
measure-tool-hint-angle = Drei Punkte anklicken; der zweite ist der Scheitel.
measure-tool-hint-rect = Ziehen oder zwei Ecken anklicken.
measure-tool-hint-ellipse = Ziehen oder zwei Ecken des umgebenden Rechtecks anklicken.
measure-tool-hint-polygon = Ecken anklicken; Eingabe oder ein Klick auf die erste Ecke schließt es.
measure-tool-hint-calibrate = Eine Linie über etwas mit bekannter Länge ziehen.
# machine-drafted, needs native review
measure-tool-hint-spot = Auf einen Punkt klicken, um seine Temperatur abzulesen.
measure-tool-known-length = Bekannte Länge: { $value } { $unit } (Zahl eingeben, U wechselt die Einheit, Eingabe übernimmt)
measure-tool-calibrated = Kalibriert: ein Pixel ist { $size } { $unit }
measure-tool-bad-length = Eine Länge größer als null eingeben.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Fläche { $value }
measure-tool-mean = Mittelwert { $mean } ± { $sd }
measure-tool-means = Mittelwert { $values }
measure-tool-range = Bereich { $min } – { $max } über { $count } px
measure-tool-pixels = Über { $count } px
measure-tool-stats-pending = Pixel werden gelesen…
measure-tool-stats-none = Hier keine Pixelwerte
measure-tool-approximate = Aus einer verkleinerten Kopie (ungefähr)

measure-tool-deleted = Messung gelöscht
measure-tool-undone = Rückgängig gemacht
measure-tool-nothing = Keine Messungen auf diesem Bild
measure-tool-exported = Messungen in { $name } gespeichert
measure-tool-export-failed = Messungen konnten nicht gespeichert werden: { $reason }
measure-tool-imported = { $count ->
    [one] Eine Messung importiert.
   *[other] { $count } Messungen importiert.
}
measure-tool-skipped = { $count ->
    [one] Eine Form konnte nicht gelesen werden.
   *[other] { $count } Formen konnten nicht gelesen werden.
}
measure-tool-import-failed = Messungen konnten nicht gelesen werden: { $reason }
measure-tool-too-large = Die Datei ist zu groß.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR erfordert ein DICOM-Bild. Speichern Sie stattdessen als CSV oder GeoJSON.

measure-source-uncalibrated = Pixel (nicht kalibriert)
measure-source-dicom-pixel-spacing = DICOM-Pixelabstand
measure-source-dicom-calibrated = DICOM-kalibrierter Abstand
measure-source-dicom-imager-pixel-spacing = DICOM-Detektorabstand (am Detektor)
measure-source-dicom-magnification = DICOM-Abstand, um Vergrößerung korrigiert
measure-source-dicom-ultrasound-region = DICOM-Ultraschallregion
measure-source-tiff-resolution = TIFF-Auflösung
measure-source-exif-resolution = EXIF-Auflösung
measure-source-ome-physical-size = OME-physische Größe
measure-source-fits-wcs = FITS-Weltkoordinaten
measure-source-vendor-tag = Herstellermetadaten
measure-source-drawing-units = Zeichnungseinheiten
# machine-drafted, needs native review
measure-source-drawing-scale = Aus dem Zeichnungsmaßstab
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Aus der Zeichnung (Maßstab { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Aus der Zeichnung (Maßstab { $scale }, Ansichtsfenster { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Kartenmaßstab in der Mitte (ungefähr; die Karten-Werkzeuge messen auf der Erde)
measure-source-user = Ihre Kalibrierung
