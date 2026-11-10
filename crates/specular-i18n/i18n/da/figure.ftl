# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Eksportér figur…
# machine-drafted, needs native review
palette-export-figure-help = Gem visningen som en figur med målestok, farveskala og etiketter.
# machine-drafted, needs native review
palette-toggle-key-image = Markér nøglebillede
# machine-drafted, needs native review
palette-toggle-key-image-help = Markér dette billede som nøglebillede, eller fjern markeringen.
# machine-drafted, needs native review
palette-copy-key-images = Kopiér nøglebilleder
# machine-drafted, needs native review
palette-copy-key-images-help = Kopiér nøglebillederne som ét billede.
# machine-drafted, needs native review
palette-export-key-images = Eksportér nøglebilleder…
# machine-drafted, needs native review
palette-export-key-images-help = Gem nøglebillederne som slides eller en mappe med PNG-filer.
# machine-drafted, needs native review
palette-export-teaching-file = Eksportér undervisningsfil…
# machine-drafted, needs native review
palette-export-teaching-file-help = Gem nøglebillederne uden patientoplysninger, til undervisning.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Eksportér figur
# machine-drafted, needs native review
figure-format = Format
# machine-drafted, needs native review
figure-format-png = PNG, 16 bit
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16 bit
# machine-drafted, needs native review
figure-region = Område
# machine-drafted, needs native review
figure-region-visible = Synligt område
# machine-drafted, needs native review
figure-region-whole = Hele billedet
# machine-drafted, needs native review
figure-size = Størrelse
# machine-drafted, needs native review
figure-size-pixels = Billedpixel (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = { $width } px bred
# machine-drafted, needs native review
figure-scale-bar = Målestok
# machine-drafted, needs native review
figure-scale-bar-none = Ingen kalibrering
# machine-drafted, needs native review
figure-colour-bar = Farveskala
# machine-drafted, needs native review
figure-colour-bar-none = Intet farvekort eller vindue
# machine-drafted, needs native review
figure-inset = Udsnit
# machine-drafted, needs native review
figure-inset-rectangle = Sidste rektangel
# machine-drafted, needs native review
figure-inset-none = Tegn først et rektangel
# machine-drafted, needs native review
figure-channels = Kanaler
# machine-drafted, needs native review
figure-channels-merged = Sammenlagt
# machine-drafted, needs native review
figure-channels-split = Opdelt og sammenlagt
# machine-drafted, needs native review
figure-channels-one = Én kanal
# machine-drafted, needs native review
figure-measurements = Målinger
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Til ({ $count })
   *[other] Til ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Ingen
# machine-drafted, needs native review
figure-title-row = Titel
# machine-drafted, needs native review
figure-provenance = Oprindelse
# machine-drafted, needs native review
figure-provenance-metadata = I filen
# machine-drafted, needs native review
figure-provenance-drawn = I filen og tegnet
# machine-drafted, needs native review
figure-file = Fil
# machine-drafted, needs native review
figure-choose = Vælg…
# machine-drafted, needs native review
figure-on = Til
# machine-drafted, needs native review
figure-off = Fra
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Til · { $source }
# machine-drafted, needs native review
figure-export = Eksportér
# machine-drafted, needs native review
figure-working = Eksporterer…
# machine-drafted, needs native review
figure-not-full-res = Pixel i fuld opløsning er ikke indlæst. Zoom til { PERCENT(100) }, eller vælg en størrelse.
# machine-drafted, needs native review
figure-channel-red = Rød
# machine-drafted, needs native review
figure-channel-green = Grøn
# machine-drafted, needs native review
figure-channel-blue = Blå
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, { $lo } til { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Vindue { $lo } til { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Kanalfarver { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Kanal { $channel }
# machine-drafted, needs native review
figure-display-hilo = HiLo-markeringer
# machine-drafted, needs native review
figure-display-as-stored = Som gemt
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Kalibrering: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Visning: { $value }
# machine-drafted, needs native review
figure-not-image = Figurer laves ud fra et billede i fremviseren.
# machine-drafted, needs native review
figure-not-ready = Billedet indlæses stadig.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = For stort: { $width } × { $height } px. Vælg det synlige område eller en størrelse.

## Key images
# machine-drafted, needs native review
figure-keys-title = Eksportér nøglebilleder
# machine-drafted, needs native review
figure-teaching-title = Eksportér undervisningsfil
# machine-drafted, needs native review
figure-keys-images = Billeder
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } nøglebillede
   *[other] { $count } nøglebilleder
}
# machine-drafted, needs native review
figure-keys-pptx = Slides (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Mappe med PNG-filer
# machine-drafted, needs native review
figure-keys-preset = Forudindstilling
# machine-drafted, needs native review
figure-keys-standard = Standard
# machine-drafted, needs native review
figure-keys-teaching = Undervisningsfil
# machine-drafted, needs native review
figure-keys-overlays-on = Tegnet og i noterne
# machine-drafted, needs native review
figure-keys-masks = Indbrændt tekst
# machine-drafted, needs native review
figure-keys-masks-auto = Sort overstreget
# machine-drafted, needs native review
figure-keys-notes = Sagsnoter
# machine-drafted, needs native review
figure-keys-destination = Gem i
# machine-drafted, needs native review
figure-keys-empty = Ingen nøglebilleder endnu. Markér først et billede som nøglebillede.
# machine-drafted, needs native review
figure-keys-teaching-note = Patientoplysninger udelades, og indbrændt tekst overstreges med sort. Tjek hvert billede, før du deler.
# machine-drafted, needs native review
figure-keys-none-left = Intet billede kunne eksporteres uden patientoplysninger.
# machine-drafted, needs native review
figure-keys-copied = Nøglebilleder kopieret
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Markeret som nøglebillede ({ $count })
   *[other] Markeret som nøglebillede ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Ikke længere nøglebillede ({ $count } tilbage)
   *[other] Ikke længere nøglebillede ({ $count } tilbage)
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Billede { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Nøglebillede { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } gemt
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } gemt. { $count } billede blev udeladt: dets indbrændte tekst blev ikke fundet.
   *[other] { $name } gemt. { $count } billeder blev udeladt: deres indbrændte tekst blev ikke fundet.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Kunne ikke eksportere: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } findes allerede i den mappe. Vælg en tom mappe.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Nøglebillede
