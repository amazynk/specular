# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Muunna…

## Settings → Convert
settings-section-convert = Muunnos
settings-convert-format = Muoto
settings-convert-format-help = Muoto, johon Muunna… kirjoittaa. PNG ja TIFF ovat häviöttömiä; WebP ja JPEG XL voivat olla kumpaa tahansa.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = JPEG-laatu
settings-convert-jpeg-quality-help = Suurempi arvo säilyttää enemmän yksityiskohtia suuremmissa tiedostoissa. Arvolla 90 eroa alkuperäiseen on vaikea huomata.
settings-convert-jpeg-full-chroma = Värit täydellä tarkkuudella JPEGissä
settings-convert-jpeg-full-chroma-help = Säilyttää värit täydellä tarkkuudella (4:4:4) puolikkaan (4:2:0) sijaan: terävämmät värireunat, suuremmat tiedostot.
settings-convert-png-effort = PNG-pakkaus
settings-convert-png-effort-help = Suurempi arvo tekee pienempiä tiedostoja hitaammin. 1 on nopein.
settings-convert-jxl-effort = JPEG XL -työmäärä
settings-convert-jxl-effort-help = Suurempi arvo tekee pienempiä tiedostoja hitaammin.
settings-convert-tiff-compression = TIFF-pakkaus
settings-convert-tiff-compression-help = Deflate on häviötön, ja kaikki TIFF-lukijat lukevat sitä.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Ei mitään
settings-convert-depth = Bittisyvyys
settings-convert-depth-help = Automaattinen kirjoittaa 16 bittiä näytettä kohden, kun kuvassa on enemmän kuin 8 ja muoto tukee niitä (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automaattinen
settings-convert-depth-8 = 8 bittiä
settings-convert-depth-16 = 16 bittiä
settings-convert-resize = Koon muutos
settings-convert-resize-help = Kuvat pienennetään lineaarisessa valossa laadukkaalla suodattimella.
settings-convert-resize-none = Alkuperäinen koko
settings-convert-resize-fit = Sovita rajoihin
settings-convert-resize-long-edge = Pitkä sivu
settings-convert-resize-percent = Prosentti
settings-convert-width = Leveys
settings-convert-width-help = Suurin leveys, jonka kuva voi saada asetuksella Sovita rajoihin.
settings-convert-height = Korkeus
settings-convert-height-help = Suurin korkeus, jonka kuva voi saada asetuksella Sovita rajoihin.
settings-convert-long-edge = Pitkä sivu
settings-convert-long-edge-help = Kuvan pidemmän sivun pituus asetuksella Pitkä sivu.
settings-convert-percent = Prosentti
settings-convert-percent-help = Koko suhteessa alkuperäiseen asetuksella Prosentti.
settings-convert-upscale = Suurenna pienet kuvat
settings-convert-upscale-help = Pois päältä: jo pyydettyä kokoa pienemmät kuvat säilyttävät kokonsa.
settings-convert-colour = Väri
settings-convert-colour-help = Säilytä tallentaa kuvan väriprofiilin sen mukana. Muunna sRGB:ksi sopii verkkoon ja laitteille, joissa ei ole värinhallintaa.
settings-convert-colour-keep = Säilytä profiili
settings-convert-colour-srgb = Muunna sRGB:ksi
settings-convert-hdr = HDR-kuvat
settings-convert-hdr-help = PNG ja JPEG XL säilyttävät HDR:n sellaisenaan; muut muodot sävykartoitetaan aina SDR:ksi.
settings-convert-hdr-keep = Säilytä, jos mahdollista
settings-convert-hdr-tone-map = Sävykartoita SDR:ksi
settings-convert-metadata = Metatiedot
settings-convert-metadata-help = Exif ja XMP, kuten kamera, kuvauspäivä ja sijainti. Väriprofiili säilytetään aina.
settings-convert-metadata-all = Säilytä kaikki
settings-convert-metadata-no-location = Kaikki paitsi sijainti
settings-convert-metadata-strip = Poista
settings-convert-folder = Kohdekansio
settings-convert-folder-help = Minne muunnetut tiedostot menevät. Jos sitä ei ole asetettu, ne menevät alikansioon kunkin kuvan viereen.
settings-convert-folder-unset = Alikansio kuvien vieressä
settings-convert-subfolder = Alikansion nimi
settings-convert-subfolder-help = Alikansio, jota käytetään, kun kohdekansiota ei ole asetettu. Tyhjä: kuvien oma kansio.
settings-convert-pattern = Tiedostonimet
settings-convert-pattern-help = {"{"}name{"}"} on alkuperäinen nimi, {"{"}n:03{"}"} numero erässä, {"{"}date:%Y-%m-%d{"}"} kuvauspäivä, {"{"}w{"}"} ja {"{"}h{"}"} uusi koko, {"{"}ext{"}"} muodon tiedostopääte.
settings-convert-conflicts = Kun nimi on varattu
settings-convert-conflicts-help = Olemassa olevia tiedostoja ei koskaan korvata.
settings-convert-conflicts-suffix = Lisää numero
settings-convert-conflicts-skip = Ohita kuva

## The dialog
convert-title = Muunna
convert-images = Kuvat
convert-images-current = Tämä kuva
convert-images-selection = Valinta ({ $count })
convert-images-folder = Kansio ({ $count })
convert-images-picked = Poiminnat ({ $count })
convert-quality = Laatu
convert-chroma = Värien tarkkuus
convert-chroma-half = 4:2:0 (pienempi)
convert-chroma-full = 4:4:4 (terävämpi)
convert-effort = Työmäärä
convert-on = Päällä
convert-off = Pois
convert-pixels = { $value } px
convert-effort-fastest = { $value } (nopein)
convert-folder-subfolder = Alikansio ”{ $name }”
convert-folder-same = Sama kansio kuin kuvalla
convert-choose-folder = Valitse kansio…
convert-name = Nimi
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = {"{"} on sulkematta.
convert-name-error-stray = Merkillä {"}"} ei ole paria {"{"}. Kirjoita {"}}"}, jos haluat aaltosulkeen.
convert-name-error-unknown = { $token } ei ole nimen osa. Käytä osia name, n, date, w, h tai ext.
convert-name-error-format = Kohteen { $token } muoto on virheellinen.
convert-name-error-empty = Kirjoita nimimalli.
convert-start = Muunna
convert-stop = Pysäytä
convert-hide = Piilota
convert-close = Sulje
convert-progress = Muunnetaan { $done }/{ $total }
convert-progress-eta = Muunnetaan { $done }/{ $total } · noin { $time } jäljellä
convert-eta-seconds = { $count ->
    [one] { $count } sekunti
   *[other] { $count } sekuntia
}
convert-eta-minutes = { $count ->
    [one] { $count } minuutti
   *[other] { $count } minuuttia
}
convert-done = { $count ->
    [one] { $count } kuva muunnettu
   *[other] { $count } kuvaa muunnettu
}
convert-done-skipped = { $count } ohitettu: nimi oli varattu
convert-done-failed = { $count } epäonnistui
convert-stopped = Pysäytetty { $count } kuvan jälkeen
convert-failure = { $file }: { $reason }
convert-failure-open = ei voi avata ({ $error })
convert-failure-decode = ei voi lukea ({ $error })
convert-failure-encode = ei voi kirjoittaa tähän muotoon ({ $error })
convert-failure-write = ei voi tallentaa ({ $error })
convert-failure-same = uusi tiedosto korvaisi alkuperäisen
convert-report-more = ja { $count } muuta
convert-running = Muunnos on jo käynnissä.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = Häviötön WebP
settings-convert-webp-lossless-help = Säilyttää jokaisen pikselin tarkasti. Pois päältä WebP-tiedostot ovat paljon pienempiä WebP-laadulla.
settings-convert-webp-quality = WebP-laatu
settings-convert-webp-quality-help = Suurempi arvo säilyttää enemmän yksityiskohtia suuremmissa tiedostoissa. Käytetään, kun WebP ei ole häviötön.
settings-convert-webp-effort = WebP-työmäärä
settings-convert-webp-effort-help = Suurempi arvo tekee pienempiä tiedostoja hitaammin. Käytetään, kun WebP ei ole häviötön.
settings-convert-jxl-lossless = Häviötön JPEG XL
settings-convert-jxl-lossless-help = Säilyttää jokaisen pikselin tarkasti. Pois päältä JPEG XL -tiedostot ovat paljon pienempiä JPEG XL -laadulla.
settings-convert-jxl-quality = JPEG XL -laatu
settings-convert-jxl-quality-help = Suurempi arvo säilyttää enemmän yksityiskohtia suuremmissa tiedostoissa. Arvolla 90 eroa alkuperäiseen on vaikea huomata. Käytetään, kun JPEG XL ei ole häviötön.
settings-convert-avif-quality = AVIF-laatu
settings-convert-avif-quality-help = Suurempi arvo säilyttää enemmän yksityiskohtia suuremmissa tiedostoissa.
settings-convert-avif-effort = AVIF-työmäärä
settings-convert-avif-effort-help = Suurempi arvo tekee pienempiä tiedostoja hitaammin. 1 on nopein.
convert-lossless = Häviötön
