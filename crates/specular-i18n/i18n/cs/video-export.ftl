# Specular — Export slideshow as video (DESIGN.md §6.5). Czech. Machine-drafted, needs native review.

palette-export-slideshow = Exportovat prezentaci jako video…

video-export-title = Exportovat prezentaci jako video
video-export-resolution = Rozlišení
# $width and $height in pixels.
video-export-resolution-window = Okno ({ $width } × { $height })
video-export-fps = Snímková frekvence
# $fps frames per second, a number.
video-export-fps-value = { $fps } snímků/s
video-export-quality = Kvalita
video-export-quality-high = Vysoká
video-export-quality-standard = Standardní
video-export-quality-compact = Úsporná
video-export-images = { $count ->
    [one] Jeden obrázek, v pořadí, s přechodem a popisky prezentace
    [few] { $count } obrázky, v pořadí, s přechodem a popisky prezentace
    [many] { $count } obrázku, v pořadí, s přechodem a popisky prezentace
   *[other] { $count } obrázků, v pořadí, s přechodem a popisky prezentace
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Délka asi { $length }, velikost asi { $size }
video-export-export = Exportovat…
video-export-replace = Nahradit
# $file a file name.
video-export-replace-question = { $file } už existuje. Nahradit?
# $percent a formatted percentage ("42%").
video-export-progress = Exportuje se… { $percent }
video-export-cancelling = Ruší se…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Uloženo { $file } ({ $size })
video-export-skipped = { $count ->
    [one] Jeden obrázek nešlo načíst a byl vynechán.
    [few] { $count } obrázky nešlo načíst a byly vynechány.
    [many] { $count } obrázku nešlo načíst a bylo vynecháno.
   *[other] { $count } obrázků nešlo načíst a bylo vynecháno.
}
video-export-failed = Video nelze exportovat: { $reason }
video-export-cancelled = Export zrušen
video-export-no-images = Žádný z obrázků nešlo načíst.
video-export-show-in-folder = Zobrazit ve složce

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Exportovat prezentaci jako video
video-export-compatibility = Kompatibilita
video-export-compatibility-current = Současné přehrávače
video-export-compatibility-all = Všechny přehrávače
