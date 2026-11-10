# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Abbildung exportieren…
# machine-drafted, needs native review
palette-export-figure-help = Die Ansicht als Abbildung mit Maßstabsbalken, Farbskala und Beschriftungen speichern.
# machine-drafted, needs native review
palette-toggle-key-image = Schlüsselbild markieren
# machine-drafted, needs native review
palette-toggle-key-image-help = Dieses Bild als Schlüsselbild markieren oder die Markierung entfernen.
# machine-drafted, needs native review
palette-copy-key-images = Schlüsselbilder kopieren
# machine-drafted, needs native review
palette-copy-key-images-help = Die Schlüsselbilder als ein Bild kopieren.
# machine-drafted, needs native review
palette-export-key-images = Schlüsselbilder exportieren…
# machine-drafted, needs native review
palette-export-key-images-help = Die Schlüsselbilder als Folien oder als Ordner mit PNGs speichern.
# machine-drafted, needs native review
palette-export-teaching-file = Lehrfall exportieren…
# machine-drafted, needs native review
palette-export-teaching-file-help = Die Schlüsselbilder ohne Patientendaten für die Lehre speichern.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Abbildung exportieren
# machine-drafted, needs native review
figure-format = Format
# machine-drafted, needs native review
figure-format-png = PNG, 16 Bit
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16 Bit
# machine-drafted, needs native review
figure-region = Bereich
# machine-drafted, needs native review
figure-region-visible = Sichtbarer Bereich
# machine-drafted, needs native review
figure-region-whole = Ganzes Bild
# machine-drafted, needs native review
figure-size = Größe
# machine-drafted, needs native review
figure-size-pixels = Bildpixel (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = { $width } px breit
# machine-drafted, needs native review
figure-scale-bar = Maßstabsbalken
# machine-drafted, needs native review
figure-scale-bar-none = Keine Kalibrierung
# machine-drafted, needs native review
figure-colour-bar = Farbskala
# machine-drafted, needs native review
figure-colour-bar-none = Keine Farbtabelle oder Fensterung
# machine-drafted, needs native review
figure-inset = Ausschnittsvergrößerung
# machine-drafted, needs native review
figure-inset-rectangle = Letztes Rechteck
# machine-drafted, needs native review
figure-inset-none = Zuerst ein Rechteck zeichnen
# machine-drafted, needs native review
figure-channels = Kanäle
# machine-drafted, needs native review
figure-channels-merged = Zusammengeführt
# machine-drafted, needs native review
figure-channels-split = Einzeln und zusammengeführt
# machine-drafted, needs native review
figure-channels-one = Ein Kanal
# machine-drafted, needs native review
figure-measurements = Messungen
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] An ({ $count })
   *[other] An ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Keine
# machine-drafted, needs native review
figure-title-row = Titel
# machine-drafted, needs native review
figure-provenance = Herkunft
# machine-drafted, needs native review
figure-provenance-metadata = In der Datei
# machine-drafted, needs native review
figure-provenance-drawn = In der Datei und eingezeichnet
# machine-drafted, needs native review
figure-file = Datei
# machine-drafted, needs native review
figure-choose = Auswählen…
# machine-drafted, needs native review
figure-on = An
# machine-drafted, needs native review
figure-off = Aus
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = An · { $source }
# machine-drafted, needs native review
figure-export = Exportieren
# machine-drafted, needs native review
figure-working = Wird exportiert…
# machine-drafted, needs native review
figure-not-full-res = Die Pixel in voller Auflösung sind nicht geladen. Auf { PERCENT(100) } zoomen oder eine Größe wählen.
# machine-drafted, needs native review
figure-channel-red = Rot
# machine-drafted, needs native review
figure-channel-green = Grün
# machine-drafted, needs native review
figure-channel-blue = Blau
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, { $lo } bis { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Fenster { $lo } bis { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Kanalfarben { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Kanal { $channel }
# machine-drafted, needs native review
figure-display-hilo = HiLo-Markierung
# machine-drafted, needs native review
figure-display-as-stored = Wie gespeichert
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Kalibrierung: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Anzeige: { $value }
# machine-drafted, needs native review
figure-not-image = Abbildungen werden aus einem Bild im Viewer erstellt.
# machine-drafted, needs native review
figure-not-ready = Das Bild wird noch geladen.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Zu groß: { $width } × { $height } px. Den sichtbaren Bereich oder eine Größe wählen.

## Key images
# machine-drafted, needs native review
figure-keys-title = Schlüsselbilder exportieren
# machine-drafted, needs native review
figure-teaching-title = Lehrfall exportieren
# machine-drafted, needs native review
figure-keys-images = Bilder
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } Schlüsselbild
   *[other] { $count } Schlüsselbilder
}
# machine-drafted, needs native review
figure-keys-pptx = Folien (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Ordner mit PNGs
# machine-drafted, needs native review
figure-keys-preset = Voreinstellung
# machine-drafted, needs native review
figure-keys-standard = Standard
# machine-drafted, needs native review
figure-keys-teaching = Lehrfall
# machine-drafted, needs native review
figure-keys-overlays-on = Eingezeichnet und in den Notizen
# machine-drafted, needs native review
figure-keys-masks = Eingebrannter Text
# machine-drafted, needs native review
figure-keys-masks-auto = Geschwärzt
# machine-drafted, needs native review
figure-keys-notes = Fallnotizen
# machine-drafted, needs native review
figure-keys-destination = Speichern unter
# machine-drafted, needs native review
figure-keys-empty = Noch keine Schlüsselbilder. Markieren Sie zuerst ein Bild als Schlüsselbild.
# machine-drafted, needs native review
figure-keys-teaching-note = Patientendaten werden weggelassen und eingebrannter Text wird geschwärzt. Prüfen Sie jedes Bild, bevor Sie es weitergeben.
# machine-drafted, needs native review
figure-keys-none-left = Kein Bild konnte ohne Patientendaten exportiert werden.
# machine-drafted, needs native review
figure-keys-copied = Schlüsselbilder kopiert
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Als Schlüsselbild markiert ({ $count })
   *[other] Als Schlüsselbild markiert ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Kein Schlüsselbild mehr ({ $count } übrig)
   *[other] Kein Schlüsselbild mehr ({ $count } übrig)
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Bild { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Schlüsselbild { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } gespeichert
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } gespeichert. { $count } Bild wurde ausgelassen: Der eingebrannte Text wurde nicht gefunden.
   *[other] { $name } gespeichert. { $count } Bilder wurden ausgelassen: Der eingebrannte Text wurde nicht gefunden.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Export nicht möglich: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } ist bereits in diesem Ordner. Wählen Sie einen leeren Ordner.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Schlüsselbild
