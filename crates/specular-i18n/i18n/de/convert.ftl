# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Konvertieren…

## Settings → Convert
settings-section-convert = Konvertieren
settings-convert-format = Format
settings-convert-format-help = Das Format, in das „Konvertieren…“ schreibt. PNG und TIFF sind verlustfrei; WebP und JPEG XL können beides.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = JPEG-Qualität
settings-convert-jpeg-quality-help = Höhere Werte bewahren mehr Details in größeren Dateien. Bei 90 ist kaum ein Unterschied zum Original zu erkennen.
settings-convert-jpeg-full-chroma = Farbe in voller Auflösung bei JPEG
settings-convert-jpeg-full-chroma-help = Speichert die Farbe in voller Auflösung (4:4:4) statt in halber (4:2:0): schärfere Farbkanten, größere Dateien.
settings-convert-png-effort = PNG-Komprimierung
settings-convert-png-effort-help = Höhere Werte erzeugen kleinere Dateien, aber langsamer. 1 ist am schnellsten.
settings-convert-jxl-effort = JPEG-XL-Aufwand
settings-convert-jxl-effort-help = Höhere Werte erzeugen kleinere Dateien, aber langsamer.
settings-convert-tiff-compression = TIFF-Komprimierung
settings-convert-tiff-compression-help = Deflate ist verlustfrei und wird von jedem TIFF-Programm gelesen.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Keine
settings-convert-depth = Bittiefe
settings-convert-depth-help = „Automatisch“ schreibt 16 Bit pro Kanal, wenn das Bild mehr als 8 hat und das Format sie aufnehmen kann (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automatisch
settings-convert-depth-8 = 8 Bit
settings-convert-depth-16 = 16 Bit
settings-convert-resize = Größe ändern
settings-convert-resize-help = Bilder werden in linearem Licht mit einem hochwertigen Filter verkleinert.
settings-convert-resize-none = Originalgröße
settings-convert-resize-fit = Einpassen in
settings-convert-resize-long-edge = Lange Seite
settings-convert-resize-percent = Prozent
settings-convert-width = Breite
settings-convert-width-help = Die größte Breite, die ein Bild bei „Einpassen in“ erhalten darf.
settings-convert-height = Höhe
settings-convert-height-help = Die größte Höhe, die ein Bild bei „Einpassen in“ erhalten darf.
settings-convert-long-edge = Lange Seite
settings-convert-long-edge-help = Die Länge der längeren Bildseite bei „Lange Seite“.
settings-convert-percent = Prozent
settings-convert-percent-help = Die Größe im Verhältnis zum Original bei „Prozent“.
settings-convert-upscale = Kleine Bilder vergrößern
settings-convert-upscale-help = Wenn aus, behalten Bilder, die schon kleiner als die gewünschte Größe sind, ihre Größe.
settings-convert-colour = Farbe
settings-convert-colour-help = „Profil behalten“ speichert das Farbprofil des Bildes mit. „In sRGB konvertieren“ eignet sich für das Web und für Geräte ohne Farbmanagement.
settings-convert-colour-keep = Profil behalten
settings-convert-colour-srgb = In sRGB konvertieren
settings-convert-hdr = HDR-Bilder
settings-convert-hdr-help = PNG und JPEG XL behalten HDR bei; andere Formate werden immer per Tone-Mapping auf SDR gebracht.
settings-convert-hdr-keep = Wenn möglich behalten
settings-convert-hdr-tone-map = Tone-Mapping auf SDR
settings-convert-metadata = Metadaten
settings-convert-metadata-help = Exif und XMP, etwa Kamera, Aufnahmedatum und Standort. Das Farbprofil bleibt immer erhalten.
settings-convert-metadata-all = Alle behalten
settings-convert-metadata-no-location = Alle außer Standort behalten
settings-convert-metadata-strip = Entfernen
settings-convert-folder = Zielordner
settings-convert-folder-help = Wohin die konvertierten Dateien kommen. Ist keiner festgelegt, landen sie in einem Unterordner neben jedem Bild.
settings-convert-folder-unset = Unterordner neben den Bildern
settings-convert-subfolder = Name des Unterordners
settings-convert-subfolder-help = Der Unterordner, der verwendet wird, wenn kein Zielordner festgelegt ist. Leer: der Ordner der Bilder selbst.
settings-convert-pattern = Dateinamen
settings-convert-pattern-help = {"{"}name{"}"} ist der ursprüngliche Name, {"{"}n:03{"}"} die Nummer im Stapel, {"{"}date:%Y-%m-%d{"}"} das Aufnahmedatum, {"{"}w{"}"} und {"{"}h{"}"} die neue Größe, {"{"}ext{"}"} die Dateiendung des Formats.
settings-convert-conflicts = Wenn ein Name vergeben ist
settings-convert-conflicts-help = Vorhandene Dateien werden nie ersetzt.
settings-convert-conflicts-suffix = Eine Nummer anhängen
settings-convert-conflicts-skip = Das Bild überspringen

## The dialog
convert-title = Konvertieren
convert-images = Bilder
convert-images-current = Dieses Bild
convert-images-selection = Auswahl ({ $count })
convert-images-folder = Ordner ({ $count })
convert-images-picked = Ausgewählte ({ $count })
convert-quality = Qualität
convert-chroma = Farbdetails
convert-chroma-half = 4:2:0 (kleiner)
convert-chroma-full = 4:4:4 (schärfer)
convert-effort = Aufwand
convert-on = Ein
convert-off = Aus
convert-pixels = { $value } px
convert-effort-fastest = { $value } (am schnellsten)
convert-folder-subfolder = Unterordner „{ $name }“
convert-folder-same = Derselbe Ordner wie das Bild
convert-choose-folder = Ordner wählen…
convert-name = Name
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Eine {"{"} ist nicht geschlossen.
convert-name-error-stray = Zu einer {"}"} gehört keine {"{"}. Schreiben Sie {"}}"} für eine geschweifte Klammer.
convert-name-error-unknown = { $token } ist kein Namensbestandteil. Verwenden Sie name, n, date, w, h oder ext.
convert-name-error-format = { $token } hat ein ungültiges Format.
convert-name-error-empty = Geben Sie ein Namensmuster ein.
convert-start = Konvertieren
convert-stop = Anhalten
convert-hide = Ausblenden
convert-close = Schließen
convert-progress = { $done } von { $total } wird konvertiert
convert-progress-eta = { $done } von { $total } wird konvertiert · noch etwa { $time }
convert-eta-seconds = { $count ->
    [one] { $count } Sekunde
   *[other] { $count } Sekunden
}
convert-eta-minutes = { $count ->
    [one] { $count } Minute
   *[other] { $count } Minuten
}
convert-done = { $count ->
    [one] { $count } Bild konvertiert
   *[other] { $count } Bilder konvertiert
}
convert-done-skipped = { $count } übersprungen: Name bereits vergeben
convert-done-failed = { $count } fehlgeschlagen
convert-stopped = Nach { $count } angehalten
convert-failure = { $file }: { $reason }
convert-failure-open = lässt sich nicht öffnen ({ $error })
convert-failure-decode = lässt sich nicht lesen ({ $error })
convert-failure-encode = lässt sich in diesem Format nicht schreiben ({ $error })
convert-failure-write = lässt sich nicht speichern ({ $error })
convert-failure-same = die neue Datei würde das Original ersetzen
convert-report-more = und { $count } weitere
convert-running = Es läuft bereits eine Konvertierung.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = Verlustfreies WebP
settings-convert-webp-lossless-help = Bewahrt jedes Pixel exakt. Ausgeschaltet werden WebP-Dateien bei der WebP-Qualität deutlich kleiner.
settings-convert-webp-quality = WebP-Qualität
settings-convert-webp-quality-help = Höhere Werte bewahren mehr Details in größeren Dateien. Gilt, wenn WebP nicht verlustfrei ist.
settings-convert-webp-effort = WebP-Aufwand
settings-convert-webp-effort-help = Höhere Werte erzeugen kleinere Dateien, aber langsamer. Gilt, wenn WebP nicht verlustfrei ist.
settings-convert-jxl-lossless = Verlustfreies JPEG XL
settings-convert-jxl-lossless-help = Bewahrt jedes Pixel exakt. Ausgeschaltet werden JPEG-XL-Dateien bei der JPEG-XL-Qualität deutlich kleiner.
settings-convert-jxl-quality = JPEG-XL-Qualität
settings-convert-jxl-quality-help = Höhere Werte bewahren mehr Details in größeren Dateien. Bei 90 ist kaum ein Unterschied zum Original zu erkennen. Gilt, wenn JPEG XL nicht verlustfrei ist.
settings-convert-avif-quality = AVIF-Qualität
settings-convert-avif-quality-help = Höhere Werte bewahren mehr Details in größeren Dateien.
settings-convert-avif-effort = AVIF-Aufwand
settings-convert-avif-effort-help = Höhere Werte erzeugen kleinere Dateien, aber langsamer. 1 ist am schnellsten.
convert-lossless = Verlustfrei
