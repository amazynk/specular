# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Vie kuva…
# machine-drafted, needs native review
palette-export-figure-help = Tallenna näkymä kuvaksi, jossa on mittakaavapalkki, väripalkki ja nimiöt.
# machine-drafted, needs native review
palette-toggle-key-image = Merkitse avainkuvaksi
# machine-drafted, needs native review
palette-toggle-key-image-help = Merkitse tämä kuva avainkuvaksi tai poista merkintä.
# machine-drafted, needs native review
palette-copy-key-images = Kopioi avainkuvat
# machine-drafted, needs native review
palette-copy-key-images-help = Kopioi avainkuvat yhtenä kuvana.
# machine-drafted, needs native review
palette-export-key-images = Vie avainkuvat…
# machine-drafted, needs native review
palette-export-key-images-help = Tallenna avainkuvat dioina tai PNG-tiedostojen kansiona.
# machine-drafted, needs native review
palette-export-teaching-file = Vie opetustiedosto…
# machine-drafted, needs native review
palette-export-teaching-file-help = Tallenna avainkuvat ilman potilastietoja opetusta varten.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Vie kuva
# machine-drafted, needs native review
figure-format = Muoto
# machine-drafted, needs native review
figure-format-png = PNG, 16-bittinen
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16-bittinen
# machine-drafted, needs native review
figure-region = Alue
# machine-drafted, needs native review
figure-region-visible = Näkyvä alue
# machine-drafted, needs native review
figure-region-whole = Koko kuva
# machine-drafted, needs native review
figure-size = Koko
# machine-drafted, needs native review
figure-size-pixels = Kuvan pikselit (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = { $width } px leveä
# machine-drafted, needs native review
figure-scale-bar = Mittakaavapalkki
# machine-drafted, needs native review
figure-scale-bar-none = Ei kalibrointia
# machine-drafted, needs native review
figure-colour-bar = Väripalkki
# machine-drafted, needs native review
figure-colour-bar-none = Ei värikarttaa eikä ikkunaa
# machine-drafted, needs native review
figure-inset = Tarkennus
# machine-drafted, needs native review
figure-inset-rectangle = Viimeisin suorakulmio
# machine-drafted, needs native review
figure-inset-none = Piirrä ensin suorakulmio
# machine-drafted, needs native review
figure-channels = Kanavat
# machine-drafted, needs native review
figure-channels-merged = Yhdistetty
# machine-drafted, needs native review
figure-channels-split = Eroteltu ja yhdistetty
# machine-drafted, needs native review
figure-channels-one = Yksi kanava
# machine-drafted, needs native review
figure-measurements = Mittaukset
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Päällä ({ $count })
   *[other] Päällä ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Ei mitään
# machine-drafted, needs native review
figure-title-row = Otsikko
# machine-drafted, needs native review
figure-provenance = Alkuperä
# machine-drafted, needs native review
figure-provenance-metadata = Tiedostossa
# machine-drafted, needs native review
figure-provenance-drawn = Tiedostossa ja piirrettynä
# machine-drafted, needs native review
figure-file = Tiedosto
# machine-drafted, needs native review
figure-choose = Valitse…
# machine-drafted, needs native review
figure-on = Päällä
# machine-drafted, needs native review
figure-off = Pois
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Päällä · { $source }
# machine-drafted, needs native review
figure-export = Vie
# machine-drafted, needs native review
figure-working = Viedään…
# machine-drafted, needs native review
figure-not-full-res = Täyden resoluution pikseleitä ei ole ladattu. Zoomaa { PERCENT(100) }:iin tai valitse koko.
# machine-drafted, needs native review
figure-channel-red = Punainen
# machine-drafted, needs native review
figure-channel-green = Vihreä
# machine-drafted, needs native review
figure-channel-blue = Sininen
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, { $lo }–{ $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Ikkuna { $lo }–{ $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Kanavavärit { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Kanava { $channel }
# machine-drafted, needs native review
figure-display-hilo = HiLo-merkinnät
# machine-drafted, needs native review
figure-display-as-stored = Tallennetulla tavalla
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Kalibrointi: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Näyttö: { $value }
# machine-drafted, needs native review
figure-not-image = Kuvat tehdään katseluohjelmassa olevasta kuvasta.
# machine-drafted, needs native review
figure-not-ready = Kuvaa ladataan vielä.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Liian suuri: { $width } × { $height } px. Valitse näkyvä alue tai koko.

## Key images
# machine-drafted, needs native review
figure-keys-title = Vie avainkuvat
# machine-drafted, needs native review
figure-teaching-title = Vie opetustiedosto
# machine-drafted, needs native review
figure-keys-images = Kuvat
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } avainkuva
   *[other] { $count } avainkuvaa
}
# machine-drafted, needs native review
figure-keys-pptx = Diat (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Kansio, jossa on PNG-tiedostoja
# machine-drafted, needs native review
figure-keys-preset = Esiasetus
# machine-drafted, needs native review
figure-keys-standard = Vakio
# machine-drafted, needs native review
figure-keys-teaching = Opetustiedosto
# machine-drafted, needs native review
figure-keys-overlays-on = Piirrettyinä ja muistiinpanoissa
# machine-drafted, needs native review
figure-keys-masks = Kuvaan poltettu teksti
# machine-drafted, needs native review
figure-keys-masks-auto = Peitetty mustalla
# machine-drafted, needs native review
figure-keys-notes = Tapausmuistiinpanot
# machine-drafted, needs native review
figure-keys-destination = Tallenna kohteeseen
# machine-drafted, needs native review
figure-keys-empty = Avainkuvia ei ole vielä. Merkitse ensin kuva avainkuvaksi.
# machine-drafted, needs native review
figure-keys-teaching-note = Potilastiedot jätetään pois ja kuvaan poltettu teksti peitetään mustalla. Tarkista jokainen kuva ennen jakamista.
# machine-drafted, needs native review
figure-keys-none-left = Yhtäkään kuvaa ei voitu viedä ilman potilastietoja.
# machine-drafted, needs native review
figure-keys-copied = Avainkuvat kopioitu
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Merkitty avainkuvaksi ({ $count })
   *[other] Merkitty avainkuvaksi ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Ei enää avainkuva ({ $count } jäljellä)
   *[other] Ei enää avainkuva ({ $count } jäljellä)
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Kuva { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Avainkuva { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } tallennettu
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } tallennettu. { $count } kuva jätettiin pois: sen kuvaan poltettua tekstiä ei löytynyt.
   *[other] { $name } tallennettu. { $count } kuvaa jätettiin pois: niiden kuvaan poltettua tekstiä ei löytynyt.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Vienti epäonnistui: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } on jo tässä kansiossa. Valitse tyhjä kansio.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Avainkuva
