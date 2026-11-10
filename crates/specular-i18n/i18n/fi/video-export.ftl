# Specular — Export slideshow as video (DESIGN.md §6.5). Finnish. Machine-drafted, needs native review.

palette-export-slideshow = Vie diaesitys videona…

video-export-title = Vie diaesitys videona
video-export-resolution = Tarkkuus
# $width and $height in pixels.
video-export-resolution-window = Ikkuna ({ $width } × { $height })
video-export-fps = Kuvataajuus
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Laatu
video-export-quality-high = Korkea
video-export-quality-standard = Tavallinen
video-export-quality-compact = Tiivis
video-export-images = { $count ->
    [one] Yksi kuva, diaesityksen järjestyksessä, siirtymällä ja kuvateksteillä
   *[other] { $count } kuvaa, diaesityksen järjestyksessä, siirtymällä ja kuvateksteillä
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Pituus noin { $length }, koko noin { $size }
video-export-export = Vie…
video-export-replace = Korvaa
# $file a file name.
video-export-replace-question = { $file } on jo olemassa. Korvataanko se?
# $percent a formatted percentage ("42%").
video-export-progress = Viedään… { $percent }
video-export-cancelling = Peruutetaan…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Tallennettu { $file } ({ $size })
video-export-skipped = { $count ->
    [one] Yhtä kuvaa ei voitu lukea, ja se jätettiin pois.
   *[other] { $count } kuvaa ei voitu lukea, ja ne jätettiin pois.
}
video-export-failed = Videon vienti epäonnistui: { $reason }
video-export-cancelled = Vienti peruutettu
video-export-no-images = Mitään kuvista ei voitu lukea.
video-export-show-in-folder = Näytä kansiossa

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Vie diaesitys videona
video-export-compatibility = Yhteensopivuus
video-export-compatibility-current = Nykyiset soittimet
video-export-compatibility-all = Kaikki soittimet
