# machine-drafted, needs native review
# Specular — Patologia: värjäysten erottelu, annotaatioiden vienti ja tuonti, nimiötilavuudet tilavuuksien päällä. Suomi.

## Paletti
palette-stain-separation = Erottele värjäykset
palette-stain-1 = Näytä tai piilota värjäys 1
palette-stain-2 = Näytä tai piilota värjäys 2
palette-stain-3 = Näytä tai piilota värjäys 3
palette-stain-mode = Värjäykset valona tai optisena tiheytenä
palette-stain-estimate = Arvioi värjäykset alueesta
palette-stain-normalise = Normalisoi värjäysten värit
palette-stain-reference = Käytä aluetta värjäysten referenssinä
palette-stain-reset = Julkaistut värjäysvektorit
palette-export-annotations = Vie annotaatiot…
palette-import-annotations = Tuo annotaatiot…
palette-label-volume = Aseta nimiötilavuus päälle…

## Syyt poistetuille komennoille
disabled-not-colour-image = Vaatii värikuvan
disabled-stains-off = Ota ensin värjäysten erottelu käyttöön
disabled-no-region = Piirrä ensin alue
disabled-no-slide-annotations = Ei vietäviä annotaatioita, alueita tai pisteitä
disabled-no-label-target = Avaa tilavuus monitasonäkymissä tai lääketieteellinen kuva

## Värjäysten nimet
stain-hematoxylin = Hematoksyliini
stain-eosin = Eosiini
stain-dab = DAB
stain-residual = Jäännös
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Värjäysten ilmoitukset ja selite
stain-off = Värjäysten erottelu pois
# { $set }: H&E, H-DAB tai H&E-DAB.
stain-on = Värjäykset: { $set }
stain-shown = { $stain } näkyvissä
stain-hidden = { $stain } piilotettu
stain-mode-light = Värjäykset valona
stain-mode-od = Värjäykset optisena tiheytenä
# Selitteen otsikko: ”H&E · valo”, ”H-DAB · optinen tiheys”.
stain-legend-light = { $set } · valo
stain-legend-od = { $set } · optinen tiheys
stain-legend-estimated = Arvioitu alueesta
stain-legend-normalised = Normalisoitu
stain-legend-published = Julkaistut vektorit
# { $pixels }: käytetyt värjäytyneet pikselit.
stain-estimated = Värjäykset arvioitu { $pixels } pikselistä
stain-estimate-failed = Värjäyksiä ei voi arvioida: { $reason }
stain-estimate-background = alue on enimmäkseen taustaa
stain-estimate-one-colour = alueella on vain yksi väri
stain-estimate-three = arvio vaatii H&E- tai H-DAB-värjäyksen
stain-estimate-no-pixels = alueen pikseleitä ei ole vielä ladattu
stain-reference-set = Alue asetettu värjäysten referenssiksi
stain-normalise-on = Värjäysten värit normalisoitu
stain-normalise-off = Värjäysten värit skannatut
stain-reset-done = Julkaistut värjäysvektorit

## Annotaatioiden vienti -valintaikkuna
annot-export-title = Vie annotaatiot
annot-export-format = Muoto
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Nimiömaskit
annot-export-output = Maskit muodossa
annot-export-output-png = PNG-ruudut
annot-export-output-tiff = TIFF-ruudut
annot-export-output-ome = OME-TIFF-pyramidi
annot-export-region = Alue
annot-export-region-slide = Koko lasi
annot-export-region-view = Näkyvä alue
annot-export-region-drawn = Viimeksi piirretty alue
annot-export-level = Taso
# { $level }: 0 on täysi tarkkuus; { $width } × { $height } sen koko pikseleinä.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Ruudun koko
annot-export-empty = Tyhjät ruudut
annot-export-empty-skip = Ohita
annot-export-empty-keep = Kirjoita
annot-export-what = { $count ->
    [one] 1 muoto
   *[other] { $count } muotoa
}, { $classes ->
    [one] 1 luokka
   *[other] { $classes } luokkaa
}
annot-export-lines = Viivat jätetään pois COCOsta ja maskeista.
annot-export-export = Vie…
annot-export-writing = Kirjoitetaan…
# { $files }: kirjoitetut tiedostot.
annot-export-done = { $files ->
    [one] Vietiin 1 tiedosto
   *[other] Vietiin { $files } tiedostoa
}
annot-export-failed = Vienti epäonnistui: { $reason }
annot-import-done = { $count ->
    [one] Tuotiin 1 annotaatio
   *[other] Tuotiin { $count } annotaatiota
}
annot-import-failed = Tuonti ei onnistu: { $reason }
annot-import-skipped = { $count ->
    [one] 1 muodolla ei ollut käyttökelpoista ääriviivaa
   *[other] { $count } muodolla ei ollut käyttökelpoista ääriviivaa
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Nimiötilavuudet
label-volume-file = Nimiötilavuus
label-volume-table = Väritaulukko
# { $file }: nimiötilavuuden tiedostonimi; { $count }: sen nimiöt.
label-volume-loaded = { $count ->
    [one] { $file }: 1 nimiö
   *[other] { $file }: { $count } nimiötä
}
label-volume-failed = Tiedostoa { $file } ei voi asettaa päälle: { $reason }
label-volume-outside = { $file } ei osu tämän tilavuuden kohdalle
label-volume-not-whole = sen arvot eivät ole kokonaislukuja
label-volume-colour = se sisältää värejä, ei nimiöitä
label-volume-empty = siinä ei ole nimiöitä
label-volume-too-many = siinä on liikaa nimiöitä
label-volume-too-large = se on liian suuri uudelleennäytteistettäväksi
label-volume-degenerate = sen suunta on virheellinen
# Nimiö, jolla ei ole nimeä väritaulukossa.
label-volume-unnamed = Nimiö { $value }
