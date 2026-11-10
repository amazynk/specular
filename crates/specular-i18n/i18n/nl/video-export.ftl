# Specular — Export slideshow as video (DESIGN.md §6.5). Dutch. Machine-drafted, needs native review.

palette-export-slideshow = Diavoorstelling als video exporteren…

video-export-title = Diavoorstelling als video exporteren
video-export-resolution = Resolutie
# $width and $height in pixels.
video-export-resolution-window = Venster ({ $width } × { $height })
video-export-fps = Framerate
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Kwaliteit
video-export-quality-high = Hoog
video-export-quality-standard = Standaard
video-export-quality-compact = Compact
video-export-images = { $count ->
    [one] Eén afbeelding, met de volgorde, overgang en onderschriften van de diavoorstelling
   *[other] { $count } afbeeldingen, met de volgorde, overgang en onderschriften van de diavoorstelling
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Ongeveer { $length } lang, ongeveer { $size }
video-export-export = Exporteren…
video-export-replace = Vervangen
# $file a file name.
video-export-replace-question = { $file } bestaat al. Vervangen?
# $percent a formatted percentage ("42%").
video-export-progress = Exporteren… { $percent }
video-export-cancelling = Annuleren…
# $file a file name, $size a file size ("38 MB").
video-export-saved = { $file } opgeslagen ({ $size })
video-export-skipped = { $count ->
    [one] Eén afbeelding kon niet worden gelezen en is weggelaten.
   *[other] { $count } afbeeldingen konden niet worden gelezen en zijn weggelaten.
}
video-export-failed = Kan de video niet exporteren: { $reason }
video-export-cancelled = Exporteren geannuleerd
video-export-no-images = Geen van de afbeeldingen kon worden gelezen.
video-export-show-in-folder = Tonen in map

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Diavoorstelling als video exporteren
video-export-compatibility = Compatibiliteit
video-export-compatibility-current = Huidige spelers
video-export-compatibility-all = Alle spelers
