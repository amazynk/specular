# Specular — Export slideshow as video (DESIGN.md §6.5). Danish. Machine-drafted, needs native review.

palette-export-slideshow = Eksportér diasshow som video…

video-export-title = Eksportér diasshow som video
video-export-resolution = Opløsning
# $width and $height in pixels.
video-export-resolution-window = Vindue ({ $width } × { $height })
video-export-fps = Billedhastighed
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Kvalitet
video-export-quality-high = Høj
video-export-quality-standard = Standard
video-export-quality-compact = Kompakt
video-export-images = { $count ->
    [one] Ét billede, med diasshowets rækkefølge, overgang og billedtekster
   *[other] { $count } billeder, med diasshowets rækkefølge, overgang og billedtekster
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Omkring { $length } lang, omkring { $size }
video-export-export = Eksportér…
video-export-replace = Erstat
# $file a file name.
video-export-replace-question = { $file } findes allerede. Vil du erstatte den?
# $percent a formatted percentage ("42%").
video-export-progress = Eksporterer… { $percent }
video-export-cancelling = Annullerer…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Gemte { $file } ({ $size })
video-export-skipped = { $count ->
    [one] Ét billede kunne ikke læses og blev udeladt.
   *[other] { $count } billeder kunne ikke læses og blev udeladt.
}
video-export-failed = Videoen kunne ikke eksporteres: { $reason }
video-export-cancelled = Eksport annulleret
video-export-no-images = Ingen af billederne kunne læses.
video-export-show-in-folder = Vis i mappe

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Eksportér diasshow som video
video-export-compatibility = Kompatibilitet
video-export-compatibility-current = Nuværende afspillere
video-export-compatibility-all = Alle afspillere
