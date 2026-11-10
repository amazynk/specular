# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Figuur exporteren…
# machine-drafted, needs native review
palette-export-figure-help = De weergave opslaan als figuur met schaalbalk, kleurenbalk en labels.
# machine-drafted, needs native review
palette-toggle-key-image = Sleutelbeeld markeren
# machine-drafted, needs native review
palette-toggle-key-image-help = Dit beeld als sleutelbeeld markeren, of de markering weghalen.
# machine-drafted, needs native review
palette-copy-key-images = Sleutelbeelden kopiëren
# machine-drafted, needs native review
palette-copy-key-images-help = De sleutelbeelden als één afbeelding kopiëren.
# machine-drafted, needs native review
palette-export-key-images = Sleutelbeelden exporteren…
# machine-drafted, needs native review
palette-export-key-images-help = De sleutelbeelden opslaan als dia’s of als map met PNG’s.
# machine-drafted, needs native review
palette-export-teaching-file = Lesbestand exporteren…
# machine-drafted, needs native review
palette-export-teaching-file-help = De sleutelbeelden zonder patiëntgegevens opslaan, voor onderwijs.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Figuur exporteren
# machine-drafted, needs native review
figure-format = Indeling
# machine-drafted, needs native review
figure-format-png = PNG, 16-bits
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16-bits
# machine-drafted, needs native review
figure-region = Gebied
# machine-drafted, needs native review
figure-region-visible = Zichtbaar gebied
# machine-drafted, needs native review
figure-region-whole = Hele afbeelding
# machine-drafted, needs native review
figure-size = Grootte
# machine-drafted, needs native review
figure-size-pixels = Afbeeldingspixels (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = { $width } px breed
# machine-drafted, needs native review
figure-scale-bar = Schaalbalk
# machine-drafted, needs native review
figure-scale-bar-none = Geen kalibratie
# machine-drafted, needs native review
figure-colour-bar = Kleurenbalk
# machine-drafted, needs native review
figure-colour-bar-none = Geen kleurenkaart of venster
# machine-drafted, needs native review
figure-inset = Uitsnede
# machine-drafted, needs native review
figure-inset-rectangle = Laatste rechthoek
# machine-drafted, needs native review
figure-inset-none = Teken eerst een rechthoek
# machine-drafted, needs native review
figure-channels = Kanalen
# machine-drafted, needs native review
figure-channels-merged = Samengevoegd
# machine-drafted, needs native review
figure-channels-split = Apart en samengevoegd
# machine-drafted, needs native review
figure-channels-one = Eén kanaal
# machine-drafted, needs native review
figure-measurements = Metingen
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Aan ({ $count })
   *[other] Aan ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Geen
# machine-drafted, needs native review
figure-title-row = Titel
# machine-drafted, needs native review
figure-provenance = Herkomst
# machine-drafted, needs native review
figure-provenance-metadata = In het bestand
# machine-drafted, needs native review
figure-provenance-drawn = In het bestand en getekend
# machine-drafted, needs native review
figure-file = Bestand
# machine-drafted, needs native review
figure-choose = Kiezen…
# machine-drafted, needs native review
figure-on = Aan
# machine-drafted, needs native review
figure-off = Uit
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Aan · { $source }
# machine-drafted, needs native review
figure-export = Exporteren
# machine-drafted, needs native review
figure-working = Exporteren…
# machine-drafted, needs native review
figure-not-full-res = Pixels met volledige resolutie zijn niet geladen. Zoom in tot { PERCENT(100) } of kies een grootte.
# machine-drafted, needs native review
figure-channel-red = Rood
# machine-drafted, needs native review
figure-channel-green = Groen
# machine-drafted, needs native review
figure-channel-blue = Blauw
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, { $lo } tot { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Venster { $lo } tot { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Kanaalkleuren { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Kanaal { $channel }
# machine-drafted, needs native review
figure-display-hilo = HiLo-markeringen
# machine-drafted, needs native review
figure-display-as-stored = Zoals opgeslagen
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Kalibratie: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Weergave: { $value }
# machine-drafted, needs native review
figure-not-image = Figuren worden gemaakt van een afbeelding in de viewer.
# machine-drafted, needs native review
figure-not-ready = De afbeelding wordt nog geladen.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Te groot: { $width } × { $height } px. Kies het zichtbare gebied of een grootte.

## Key images
# machine-drafted, needs native review
figure-keys-title = Sleutelbeelden exporteren
# machine-drafted, needs native review
figure-teaching-title = Lesbestand exporteren
# machine-drafted, needs native review
figure-keys-images = Afbeeldingen
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } sleutelbeeld
   *[other] { $count } sleutelbeelden
}
# machine-drafted, needs native review
figure-keys-pptx = Dia’s (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Map met PNG’s
# machine-drafted, needs native review
figure-keys-preset = Voorinstelling
# machine-drafted, needs native review
figure-keys-standard = Standaard
# machine-drafted, needs native review
figure-keys-teaching = Lesbestand
# machine-drafted, needs native review
figure-keys-overlays-on = Getekend en in de notities
# machine-drafted, needs native review
figure-keys-masks = Ingebrande tekst
# machine-drafted, needs native review
figure-keys-masks-auto = Zwartgemaakt
# machine-drafted, needs native review
figure-keys-notes = Casusnotities
# machine-drafted, needs native review
figure-keys-destination = Opslaan in
# machine-drafted, needs native review
figure-keys-empty = Nog geen sleutelbeelden. Markeer eerst een afbeelding als sleutelbeeld.
# machine-drafted, needs native review
figure-keys-teaching-note = Patiëntgegevens worden weggelaten en ingebrande tekst wordt zwartgemaakt. Controleer elke afbeelding voordat je deelt.
# machine-drafted, needs native review
figure-keys-none-left = Er kon geen afbeelding zonder patiëntgegevens worden geëxporteerd.
# machine-drafted, needs native review
figure-keys-copied = Sleutelbeelden gekopieerd
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Gemarkeerd als sleutelbeeld ({ $count })
   *[other] Gemarkeerd als sleutelbeeld ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Geen sleutelbeeld meer ({ $count } over)
   *[other] Geen sleutelbeeld meer ({ $count } over)
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Afbeelding { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Sleutelbeeld { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } opgeslagen
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } opgeslagen. { $count } afbeelding is weggelaten: de ingebrande tekst is niet gevonden.
   *[other] { $name } opgeslagen. { $count } afbeeldingen zijn weggelaten: hun ingebrande tekst is niet gevonden.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Exporteren is mislukt: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } staat al in die map. Kies een lege map.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Sleutelbeeld
