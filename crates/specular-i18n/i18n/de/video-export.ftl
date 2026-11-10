# Specular — Export slideshow as video (DESIGN.md §6.5). German. Machine-drafted, needs native review.

palette-export-slideshow = Diashow als Video exportieren…

video-export-title = Diashow als Video exportieren
video-export-resolution = Auflösung
# $width and $height in pixels.
video-export-resolution-window = Fenster ({ $width } × { $height })
video-export-fps = Bildrate
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Qualität
video-export-quality-high = Hoch
video-export-quality-standard = Standard
video-export-quality-compact = Kompakt
video-export-images = { $count ->
    [one] Ein Bild, mit Reihenfolge, Übergang und Beschriftungen der Diashow
   *[other] { $count } Bilder, mit Reihenfolge, Übergang und Beschriftungen der Diashow
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Etwa { $length } lang, etwa { $size }
video-export-export = Exportieren…
video-export-replace = Ersetzen
# $file a file name.
video-export-replace-question = { $file } ist bereits vorhanden. Ersetzen?
# $percent a formatted percentage ("42%").
video-export-progress = Wird exportiert… { $percent }
video-export-cancelling = Wird abgebrochen…
# $file a file name, $size a file size ("38 MB").
video-export-saved = { $file } gespeichert ({ $size })
video-export-skipped = { $count ->
    [one] Ein Bild konnte nicht gelesen werden und wurde ausgelassen.
   *[other] { $count } Bilder konnten nicht gelesen werden und wurden ausgelassen.
}
video-export-failed = Das Video konnte nicht exportiert werden: { $reason }
video-export-cancelled = Export abgebrochen
video-export-no-images = Keines der Bilder konnte gelesen werden.
video-export-show-in-folder = Im Ordner anzeigen

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Diashow als Video exportieren
video-export-compatibility = Kompatibilität
video-export-compatibility-current = Aktuelle Player
video-export-compatibility-all = Alle Player
