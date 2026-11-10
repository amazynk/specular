# Specular — Export slideshow as video (DESIGN.md §6.5). Swedish. Machine-drafted, needs native review.

palette-export-slideshow = Exportera bildspel som video…

video-export-title = Exportera bildspel som video
video-export-resolution = Upplösning
# $width and $height in pixels.
video-export-resolution-window = Fönster ({ $width } × { $height })
video-export-fps = Bildfrekvens
# $fps frames per second, a number.
video-export-fps-value = { $fps } bilder/s
video-export-quality = Kvalitet
video-export-quality-high = Hög
video-export-quality-standard = Standard
video-export-quality-compact = Kompakt
video-export-images = { $count ->
    [one] En bild, med bildspelets ordning, övergång och bildtexter
   *[other] { $count } bilder, med bildspelets ordning, övergång och bildtexter
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Cirka { $length } lång, cirka { $size }
video-export-export = Exportera…
video-export-replace = Ersätt
# $file a file name.
video-export-replace-question = { $file } finns redan. Vill du ersätta den?
# $percent a formatted percentage ("42%").
video-export-progress = Exporterar… { $percent }
video-export-cancelling = Avbryter…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Sparade { $file } ({ $size })
video-export-skipped = { $count ->
    [one] En bild kunde inte läsas och utelämnades.
   *[other] { $count } bilder kunde inte läsas och utelämnades.
}
video-export-failed = Det gick inte att exportera videon: { $reason }
video-export-cancelled = Exporten avbröts
video-export-no-images = Ingen av bilderna kunde läsas.
video-export-show-in-folder = Visa i mapp

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Exportera bildspel som video
video-export-compatibility = Kompatibilitet
video-export-compatibility-current = Nuvarande spelare
video-export-compatibility-all = Alla spelare
