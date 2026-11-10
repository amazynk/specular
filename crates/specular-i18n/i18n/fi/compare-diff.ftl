# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Vertaa kuvaan…
palette-compare-with-help = Vertaa tätä kuvaa toiseen valitsemaasi kuvaan.
palette-compare-side = Vertaa rinnakkain
palette-compare-side-help = Näytä verratut kuvat vierekkäin.
palette-compare-split = Vertaa jakoviivalla
palette-compare-split-help = Näytä toinen kuva ensimmäisen päällä, rajattuna vedettävään jakoviivaan.
palette-compare-split-direction = Käännä jakoviiva
palette-compare-split-direction-help = Aseta jakoviiva vaakaan pystyn sijaan, tai takaisin.
palette-compare-overlay = Vertaa päällekkäin
palette-compare-overlay-help = Näytä toinen kuva ensimmäisen päällä sekoitettuna.
palette-compare-difference = Vertaa eroa
palette-compare-difference-help = Näytä, missä kuvat eroavat; uudelleen seuraavaa lajia varten.
palette-compare-flicker = Vilkuta verrattuja kuvia
palette-compare-flicker-help = Näytä toinen kuva heti.
palette-compare-auto-flicker = Vilkuta automaattisesti
palette-compare-auto-flicker-help = Vaihda kahden kuvan välillä automaattisesti, tai pysäytä.
palette-compare-align = Kohdista verratut kuvat
palette-compare-align-help = Kohdista erikokoiset kuvat: vasempaan yläkulmaan, keskelle, skaalaten tai automaattisesti.
palette-compare-metric-space = Vaihda mittausavaruutta
palette-compare-metric-space-help = Mittaa lineaarisessa valossa tai koodatussa sRGB:ssä.
palette-compare-whole-image = Mittaa koko kuva
palette-compare-whole-image-help = Mittaa koko kuva näkyvän osan sijaan, tai takaisin.
palette-compare-probe = Vertailun pikselianturi
palette-compare-probe-help = Näytä molempien kuvien arvot osoittimen kohdalla.
palette-compare-link = Linkitä tai irrota verratut kuvat
palette-compare-link-help = Zoomaa ja panoroi kumpaakin kuvaa erikseen, tai taas yhdessä.
palette-compare-close = Sulje vertailu
palette-compare-close-help = Palaa galleriaan.
disabled-compare-closed = Avaa ensin vertailu
disabled-compare-not-two = Toimii kahdella kuvalla

## Cheat sheet
cheatsheet-compare-split = Jakoviiva tai rinnakkain
cheatsheet-compare-split-direction = Jakoviiva vaakaan tai pystyyn
cheatsheet-compare-auto-flicker = Vilkuta automaattisesti
cheatsheet-compare-align = Seuraava kohdistus
cheatsheet-compare-space = Mittausavaruus
cheatsheet-compare-whole = Koko kuva tai näkyvä osa
cheatsheet-compare-probe = Pikselianturi
cheatsheet-compare-link = Linkitä tai irrota kaksi kuvaa

## Settings
settings-gallery-compare-metric-space = Vertailun mittausavaruus
settings-gallery-compare-metric-space-help = Missä vertailu mittaa MAE:n, MSE:n, PSNR:n ja SSIM:n. Lineaarinen valo on se, mitä näyttö näyttää. sRGB vastaa muita työkaluja 8-bit-tiedostoissa.
settings-gallery-compare-metric-space-linear = Lineaarinen valo
settings-gallery-compare-metric-space-srgb = Koodattu sRGB
settings-gallery-compare-flicker-limit = Nopein automaattinen vilkutus
settings-gallery-compare-flicker-limit-help = Kuvanvaihtoja sekunnissa enintään. Yli 3 vilkkuminen voi laukaista kohtauksia.

## Toolbar and captions
compare-split = Jakoviiva
compare-split-down = Jakoviiva pystyyn
compare-split-across = Jakoviiva vaakaan
compare-link = Linkitetty
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } vaihtoa sekunnissa
compare-gain = Vahvistus ×{ $gain }
compare-probe = Anturi
compare-kind-signed = Etumerkillinen virhe
compare-kind-on-image = Virhe kuvan päällä
compare-align-top-left = Vasen yläkulma
compare-align-centre = Keskitetty
compare-align-scaled = B skaalattu A:han
compare-align-auto = Automaattinen kohdistus
compare-space-linear = Lineaarinen valo
compare-space-srgb = sRGB
compare-space-help-linear = Mitattu lineaarisessa valossa, kuten näytetään sävykartoituksen jälkeen. PSNR ei tässä ole tavallinen 8-bit-sRGB:n PSNR.
compare-space-help-srgb = Mitattu koodatuista sRGB-arvoista, kuten muut työkalut tekevät 8-bit-tiedostoille. SDR-valkoisen yli menevä HDR lasketaan valkoiseksi.
compare-space-help-common = ΔE2000 on aina CIELAB (D65), SDR-valkoinen on L* 100 ja HDR leikataan. Yli { $size } pikselin kuvat mitataan { $size } pikselin kopiosta.
compare-region-view = Näkyvä osa
compare-region-whole = Koko kuva
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM tarvitsee 11 pikseliä
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, enint. { $max })
compare-delta-e-hdr = HDR leikattu ΔE:tä varten
compare-aligned-centre = Keskitetty, { $width } × { $height } verrattu
compare-aligned-scaled = B skaalattu { $scale }×, { $width } × { $height } verrattu
compare-aligned-auto = Kohdistettu { $angle }° { $scale }×, { $width } × { $height } verrattu
compare-aligning = Kohdistetaan…
compare-align-failed = Kuvia ei voitu kohdistaa. Palattiin vasempaan yläkulmaan.
compare-split-label = { $b } kuvan { $a } päällä, jakoviiva kohdassa { PERCENT($percent) }
compare-showing-which = Näytetään { $which }: { $name }
compare-auto-flicker-on = Vilkutetaan, { $rate } vaihtoa sekunnissa
compare-auto-flicker-off = Vilkutus pysäytetty
compare-linked = Kuvat linkitetty
compare-unlinked = Kuvien linkitys irrotettu: kumpaakin panoroidaan erikseen
compare-with-title = Vertaa kuvaan
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Jakoviiva
compare-a11y-space = Mittausavaruus
compare-a11y-align = Kohdistus
