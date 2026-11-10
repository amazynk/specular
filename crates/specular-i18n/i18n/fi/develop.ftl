# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Suomi.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Tämän muokkauksen yksi moduuli päivitettiin uudempaan versioon. Tulos voi näyttää hieman erilaiselta.
   *[other] Tämän muokkauksen { $count } moduulia päivitettiin uudempaan versioon. Tulos voi näyttää hieman erilaiselta.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Tämän muokkauksen yksi moduuli vaatii uudemman Specular-version. Se säilytetään, mutta sitä ei näytetä.
   *[other] Tämän muokkauksen { $count } moduulia vaatii uudemman Specular-version. Ne säilytetään, mutta niitä ei näytetä.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Piste { $index }/{ $count }: sisään { $input }, ulos { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Nuolinäppäimet siirtävät pistettä. Page Up ja Page Down valitsevat toisen. Insert lisää pisteen, Delete poistaa sen.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutraali
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Värisävy { $hue }°, voimakkuus { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Nuolinäppäimet siirtävät väriä. Delete palauttaa sen.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Kehitä
# machine-drafted, needs native review
develop-undo = Kumoa
# machine-drafted, needs native review
develop-redo = Tee uudelleen
# machine-drafted, needs native review
develop-before-after = Ennen ja jälkeen
# machine-drafted, needs native review
develop-close = Poistu Kehitä-tilasta
# machine-drafted, needs native review
develop-history = Historia
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } päällä
develop-card-menu = { $module }: valinnat
develop-card-changed = Muutettu
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Filminauha

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Valotus
develop-module-light = Valo
develop-look-profile = Profiili
develop-look-standard = Vakio
develop-look-film = Filmi
develop-look-natural = Luonnollinen
develop-module-grading = Värisävytys
develop-add-tool = Lisää työkalu
# machine-drafted, needs native review
develop-module-white-balance = Valkotasapaino
# machine-drafted, needs native review
develop-module-tone = Sävyt
# machine-drafted, needs native review
develop-module-contrast = Kontrasti ja alue
# machine-drafted, needs native review
develop-module-colour = Väri
# machine-drafted, needs native review
develop-module-curve = Käyrä
# machine-drafted, needs native review
develop-module-crop = Rajaus ja suoristus
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Muu moduuli
# machine-drafted, needs native review
develop-exposure = Valotus
# machine-drafted, needs native review
develop-black-level = Mustan taso
# machine-drafted, needs native review
develop-temperature = Lämpötila
# machine-drafted, needs native review
develop-tint = Sävy
# machine-drafted, needs native review
develop-highlights = Kirkkaat alueet
# machine-drafted, needs native review
develop-shadows = Varjot
# machine-drafted, needs native review
develop-contrast = Kontrasti
# machine-drafted, needs native review
develop-whites = Valkoiset
# machine-drafted, needs native review
develop-blacks = Mustat
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = HDR-raja
# machine-drafted, needs native review
develop-hdr-preview = Näytä
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Eloisuus
# machine-drafted, needs native review
develop-saturation = Kylläisyys
# machine-drafted, needs native review
develop-colour-wheels = Värikiekot
# machine-drafted, needs native review
develop-curve = Sävykäyrä
# machine-drafted, needs native review
develop-module-local-contrast = Paikallinen kontrasti
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Automaattivalotus
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Pehmennys
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Tekstuuri
# machine-drafted, needs native review
develop-clarity = Selkeys
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Mustapiste
# machine-drafted, needs native review
develop-levels-mid = Keskipiste
# machine-drafted, needs native review
develop-levels-white = Valkopiste
# machine-drafted, needs native review
develop-module-detail = Yksityiskohdat
# machine-drafted, needs native review
develop-module-denoise = Kohinanpoisto
# machine-drafted, needs native review
develop-module-sharpen = Terävöinti
# machine-drafted, needs native review
develop-noise-luminance = Luminanssikohina
# machine-drafted, needs native review
develop-noise-detail = Kohinan yksityiskohdat
# machine-drafted, needs native review
develop-noise-colour = Värikohina
# machine-drafted, needs native review
develop-sharpen-amount = Terävöinnin määrä
# machine-drafted, needs native review
develop-sharpen-radius = Terävöinnin säde
# machine-drafted, needs native review
develop-sharpen-detail = Terävöinnin yksityiskohdat
# machine-drafted, needs native review
develop-sharpen-masking = Maskaus
# machine-drafted, needs native review
develop-capture-amount = Kuvausterävöinti
# machine-drafted, needs native review
develop-capture-radius = Kuvaussäde
# machine-drafted, needs native review
develop-straighten = Suorista
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Rajaa
# machine-drafted, needs native review
develop-crop-turn = Käännä
# machine-drafted, needs native review
develop-crop-flip = Peilaa
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Objektiivikorjaukset
develop-module-optics = Optiikka
# machine-drafted, needs native review
develop-lens-distortion = Vääristymä
# machine-drafted, needs native review
develop-lens-vignette = Vinjetointi
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Punainen värireunus
# machine-drafted, needs native review
develop-lens-tca-blue = Sininen värireunus
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Objektiiviprofiili
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automaattinen
# machine-drafted, needs native review
develop-lensfun-off = Ei mitään
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Ei objektiiviprofiilia.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Objektiiville { $lens } ei ole Lensfun-profiilia.
# machine-drafted, needs native review
develop-lensfun-no-profile = Tälle objektiiville ei ole Lensfun-profiilia.
# machine-drafted, needs native review
develop-lensfun-no-name = Tiedosto ei kerro objektiivin nimeä. Valitse se yltä.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } objektiivia sopii. Valitse yksi yltä.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Rajaus. Vedä reunoja tai Ctrl+vedä pitkin viivaa, jonka pitää olla vaakasuorassa. Enter säilyttää, Esc palaa.
# machine-drafted, needs native review
develop-crop-kept = Rajaus säilytetty
# machine-drafted, needs native review
develop-crop-cancelled = Rajaus kumottu
# machine-drafted, needs native review
develop-crop-reset = Rajaus ja suoristus palautettu
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Suoristettu kulmaan { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Kuvasuhde { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = vapaa
# machine-drafted, needs native review
develop-crop-aspect-original = alkuperäinen
# machine-drafted, needs native review
develop-crop-turned-right = Käännetty oikealle
# machine-drafted, needs native review
develop-crop-turned-left = Käännetty vasemmalle
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Peilattu vaakasuunnassa
# machine-drafted, needs native review
develop-crop-flipped-vertical = Peilattu pystysuunnassa
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Kuvasuhde { $aspect } · Apuviivat: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A kuvasuhde · X vaihda · O apuviivat · [ ] käännä · H V peilaa · Enter säilytä · Esc takaisin
# machine-drafted, needs native review
develop-crop-guide-thirds = kolmannekset
# machine-drafted, needs native review
develop-crop-guide-golden = kultainen leikkaus
# machine-drafted, needs native review
develop-crop-guide-diagonal = diagonaalit
# machine-drafted, needs native review
develop-crop-guide-grid = ruudukko

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspektiivi
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Oikaisu
# machine-drafted, needs native review
develop-persp-mode-off = Pois
# machine-drafted, needs native review
develop-persp-mode-auto = Automaattinen
# machine-drafted, needs native review
develop-persp-mode-level = Vaaka
# machine-drafted, needs native review
develop-persp-mode-vertical = Pysty
# machine-drafted, needs native review
develop-persp-mode-full = Täysi
# machine-drafted, needs native review
develop-persp-mode-guided = Ohjattu
# machine-drafted, needs native review
develop-persp-vertical = Pysty
# machine-drafted, needs native review
develop-persp-horizontal = Vaaka
# machine-drafted, needs native review
develop-persp-rotate = Kierto
# machine-drafted, needs native review
develop-persp-aspect = Kuvasuhde
# machine-drafted, needs native review
develop-persp-scale = Skaalaus
# machine-drafted, needs native review
develop-persp-offset-x = Sivusuunta
# machine-drafted, needs native review
develop-persp-offset-y = Ylös tai alas
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Vältä tyhjiä reunoja
# machine-drafted, needs native review
develop-persp-guides = Apuviivat
# machine-drafted, needs native review
develop-persp-focal = Polttoväli
# machine-drafted, needs native review
develop-persp-draw-guides = Piirrä apuviivat
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Piirrä apuviivat ({ $n }/{ $max })
# machine-drafted, needs native review
develop-persp-reset = Palauta perspektiivi
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Oikaisu: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Etsitään viivoja…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Palautetaanko perspektiivi? Apuviivasi ({ $n }) poistetaan. Kumoa tuo ne takaisin.
# machine-drafted, needs native review
develop-persp-reset-ok = Palauta
# machine-drafted, needs native review
develop-persp-reset-done = Perspektiivi palautettu
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Poistetaanko apuviiva { $n }? Kumoa tuo sen takaisin.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Poista
# machine-drafted, needs native review
develop-persp-guide-removed = Apuviiva poistettu
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Pysty { $vertical }, vaaka { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Oikaisu epäonnistui: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: kallistus { $vertical }°, käännös { $horizontal }°, kierto { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } tarvitsee vähintään kaksi erotettavaa suoraa viivaa. Kokeile toista tilaa tai piirrä apuviivat.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Enintään { $n } apuviivaa. Siirrä apuviivaa vetämällä sen päätä.
# machine-drafted, needs native review
develop-persp-guides-on = Apuviivat. Vedä pitkin viivaa, jonka pitää olla pystysuorassa tai vaakasuorassa.
# machine-drafted, needs native review
develop-persp-guides-off = Rajaus
# machine-drafted, needs native review
develop-persp-guide-moved = Apuviiva siirretty
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Apuviiva { $n } lisätty
# machine-drafted, needs native review
develop-persp-guides-keys = Vedä lisätäksesi · Vedä päätä siirtääksesi · Delete poistaa · Shift+R rajaus
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Apuviivat: { $n }/{ $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Varjot
# machine-drafted, needs native review
develop-wheel-midtones = Keskisävyt
# machine-drafted, needs native review
develop-wheel-highlights = Kirkkaat alueet
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Varjojen luminanssi
# machine-drafted, needs native review
develop-luminance-midtones = Keskisävyjen luminanssi
# machine-drafted, needs native review
develop-luminance-highlights = Kirkkaiden alueiden luminanssi
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Yleinen
# machine-drafted, needs native review
develop-luminance-global = Yleinen luminanssi
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Värien voimakkuus
# machine-drafted, needs native review
develop-brilliance = Värien kirkkaus

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Värimikseri
# machine-drafted, needs native review
develop-colour-red = Punainen
# machine-drafted, needs native review
develop-colour-orange = Oranssi
# machine-drafted, needs native review
develop-colour-yellow = Keltainen
# machine-drafted, needs native review
develop-colour-green = Vihreä
# machine-drafted, needs native review
develop-colour-aqua = Akvamariini
# machine-drafted, needs native review
develop-colour-blue = Sininen
# machine-drafted, needs native review
develop-colour-purple = Violetti
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Värisävy – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Kylläisyys – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminanssi – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Pisteväri
# machine-drafted, needs native review
develop-point-pick = Valitse väri
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Valitse toinen väri
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Napsauta muutettavaa väriä. Esc peruuttaa.
# machine-drafted, needs native review
develop-point-picked = Väri valittu
# machine-drafted, needs native review
develop-point-cancelled = Valinta peruttu
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Alue
# machine-drafted, needs native review
develop-point-hue-range = Sävyalue
# machine-drafted, needs native review
develop-point-saturation-range = Kylläisyysalue
# machine-drafted, needs native review
develop-point-lightness-range = Vaaleusalue
# machine-drafted, needs native review
develop-point-hue = Värisävyn siirto
# machine-drafted, needs native review
develop-point-saturation = Kylläisyyden siirto
# machine-drafted, needs native review
develop-point-lightness = Vaaleuden siirto

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Mustavalkoinen
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminanssi – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Valitse LUT-tiedosto…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Tiedosto: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Tiedostoa { $name } ei voi lukea
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Valitse LUT-tiedosto
# machine-drafted, needs native review
develop-lut-kind = LUT-tiedostot
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Kohdeväriavaruus
# machine-drafted, needs native review
develop-lut-space-linear = Lineaarinen (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES lineaarinen (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Valokuva (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Video (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Menetelmä
# machine-drafted, needs native review
develop-lut-tetrahedral = Tarkka
# machine-drafted, needs native review
develop-lut-trilinear = Yksinkertainen
# machine-drafted, needs native review
develop-lut-strength = Voimakkuus

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Alkuperäinen
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } päällä
# machine-drafted, needs native review
develop-history-off = { $module } pois
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Takaisin alkuperäiseen
# machine-drafted, needs native review
develop-at-step = Takaisin: { $step }
# machine-drafted, needs native review
develop-undone = Kumottu: { $step }
# machine-drafted, needs native review
develop-redone = Tehty uudelleen: { $step }
# machine-drafted, needs native review
develop-entered = Kehitä. Muokkaukset eivät koskaan muuta tiedostoa.
# machine-drafted, needs native review
develop-left = Poistuttiin Kehitä-tilasta
# machine-drafted, needs native review
develop-split-on = Ennen ja jälkeen rinnakkain
# machine-drafted, needs native review
develop-split-off = Vain muokkaus
# machine-drafted, needs native review
develop-showing-before = Näytetään alkuperäinen
# machine-drafted, needs native review
develop-showing-after = Näytetään muokkaus
# machine-drafted, needs native review
# Labels over the image.
develop-before = Ennen
# machine-drafted, needs native review
develop-after = Jälkeen
# machine-drafted, needs native review
develop-working = Kehitetään…
# machine-drafted, needs native review
develop-preview-failed = Muokkausta ei voi näyttää tälle tiedostolle

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Kehitä
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Pikamuokkaus: pidä painettuna ja vieritä
# machine-drafted, needs native review
palette-develop = Kehitä
# machine-drafted, needs native review
palette-develop-help = Muokkaa tätä kuvaa muuttamatta tiedostoa. Uudelleen poistuaksesi.
# machine-drafted, needs native review
palette-develop-before-after = Ennen ja jälkeen
# machine-drafted, needs native review
palette-develop-before-after-help = Näytä alkuperäinen tai taas muokkaus.
# machine-drafted, needs native review
palette-develop-split = Ennen ja jälkeen jaettuna
# machine-drafted, needs native review
palette-develop-split-help = Näytä alkuperäinen ja muokkaus rinnakkain.
# machine-drafted, needs native review
palette-develop-panel = Siirry Kehitä-paneeliin
# machine-drafted, needs native review
palette-develop-panel-help = Siirrä näppäimistö paneeliin. Esc palaa.
# machine-drafted, needs native review
palette-develop-speed-exposure = Pikamuokkaa valotusta
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Pidä painettuna ja vieritä tai paina nuolinäppäimiä muuttaaksesi valotusta.
# machine-drafted, needs native review
palette-develop-speed-contrast = Pikamuokkaa kontrastia
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Pidä painettuna ja vieritä tai paina nuolinäppäimiä muuttaaksesi kontrastia.
# machine-drafted, needs native review
palette-develop-speed-highlights = Pikamuokkaa kirkkaita alueita
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Pidä painettuna ja vieritä tai paina nuolinäppäimiä muuttaaksesi kirkkaita alueita.
# machine-drafted, needs native review
palette-develop-speed-shadows = Pikamuokkaa varjoja
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Pidä painettuna ja vieritä tai paina nuolinäppäimiä muuttaaksesi varjoja.
# machine-drafted, needs native review
palette-develop-speed-whites = Pikamuokkaa valkoisia
# machine-drafted, needs native review
palette-develop-speed-whites-help = Pidä painettuna ja vieritä tai paina nuolinäppäimiä muuttaaksesi valkoisia.
# machine-drafted, needs native review
palette-develop-speed-blacks = Pikamuokkaa mustia
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Pidä painettuna ja vieritä tai paina nuolinäppäimiä muuttaaksesi mustia.
# machine-drafted, needs native review
palette-develop-speed-temperature = Pikamuokkaa lämpötilaa
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Pidä painettuna ja vieritä tai paina nuolinäppäimiä muuttaaksesi valkotasapainon lämpötilaa.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Pikamuokkaa eloisuutta
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Pidä painettuna ja vieritä tai paina nuolinäppäimiä muuttaaksesi eloisuutta.
# machine-drafted, needs native review
palette-develop-crop = Rajaa ja suorista
# machine-drafted, needs native review
palette-develop-crop-help = Rajaa, suorista, käännä tai peilaa kuva. Enter säilyttää, Esc palaa.
# machine-drafted, needs native review
disabled-not-photo = Vain valokuville
# machine-drafted, needs native review
disabled-develop-off = Kehitä on pois päältä Lääketiede- ja Tiede-profiileissa
# machine-drafted, needs native review
develop-module-corrections = Korjaukset
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Pois
# machine-drafted, needs native review
develop-on = Päällä
# machine-drafted, needs native review
develop-highlight-method = Puhki palaneet kohdat
# machine-drafted, needs native review
develop-highlight-opposed = Palauta väri
# machine-drafted, needs native review
develop-highlight-rebuild = Palauta suuret alueet
# machine-drafted, needs native review
develop-highlight-clip = Leikkaa valkoiseksi
# machine-drafted, needs native review
develop-highlight-threshold = Leikkaustaso
# machine-drafted, needs native review
develop-module-hot-pixels = Kuumat pikselit
# machine-drafted, needs native review
develop-hot-threshold = Kuumien pikselien kynnys
# machine-drafted, needs native review
develop-module-ca = Kromaattinen aberraatio
# machine-drafted, needs native review
develop-ca-mode = Värireunat
# machine-drafted, needs native review
develop-ca-auto = Automaattinen
# machine-drafted, needs native review
develop-ca-manual = Käsin
# machine-drafted, needs native review
develop-ca-red = Punaisen siirto
# machine-drafted, needs native review
develop-ca-blue = Sinisen siirto
# machine-drafted, needs native review
develop-module-defringe = Värireunojen poisto
# machine-drafted, needs native review
develop-defringe = Värireunojen poisto
# machine-drafted, needs native review
develop-defringe-purple = Violetti sävy
# machine-drafted, needs native review
develop-defringe-green = Vihreä sävy
# machine-drafted, needs native review
develop-defringe-range = Sävyalue
# machine-drafted, needs native review
develop-defringe-edge = Reunakontrasti
# machine-drafted, needs native review
develop-module-flat-field = Kalibrointikuvat
# machine-drafted, needs native review
develop-calibration = Kalibrointi
# machine-drafted, needs native review
develop-calibration-auto = Kansiosta
# machine-drafted, needs native review
develop-calibration-chosen = Valitut tiedostot
# machine-drafted, needs native review
develop-flat-choose = Valitse flat-kuva…
# machine-drafted, needs native review
develop-dark-choose = Valitse dark-kuva…
# machine-drafted, needs native review
develop-flat-file = Flat-kuva: { $name }
# machine-drafted, needs native review
develop-dark-file = Dark-kuva: { $name }
# machine-drafted, needs native review
develop-flat-use = Flat-kuva
# machine-drafted, needs native review
develop-flat-use-full = Vinjetointi ja pöly
# machine-drafted, needs native review
develop-flat-use-vignetting = Vain vinjetointi
# machine-drafted, needs native review
develop-flat-dialog = Valitse flat-kuva
# machine-drafted, needs native review
develop-dark-dialog = Valitse dark-kuva
# machine-drafted, needs native review
develop-raw-kind = Raw-tiedostot
# machine-drafted, needs native review
develop-flat-frame = Flat-kuva
# machine-drafted, needs native review
develop-dark-frame = Dark-kuva

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Lisää maski
# machine-drafted, needs native review
develop-mask-kind-ellipse = Ellipsi
# machine-drafted, needs native review
develop-mask-kind-rectangle = Suorakulmio
# machine-drafted, needs native review
develop-mask-kind-polygon = Monikulmio
# machine-drafted, needs native review
develop-mask-kind-linear = Lineaarinen liukuma
# machine-drafted, needs native review
develop-mask-kind-radial = Säteittäinen liukuma
# machine-drafted, needs native review
develop-mask-kind-brush = Sivellin
# machine-drafted, needs native review
develop-mask-kind-luminance = Luminanssialue
# machine-drafted, needs native review
develop-mask-kind-hue = Sävyalue
# machine-drafted, needs native review
develop-mask-kind-chroma = Kylläisyysalue
# machine-drafted, needs native review
develop-mask-kind-colour = Värialue
# machine-drafted, needs native review
develop-mask-kind-other = Muu maski
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Sekoitus
# machine-drafted, needs native review
develop-blend-normal = Normaali
# machine-drafted, needs native review
develop-blend-multiply = Kerro
# machine-drafted, needs native review
develop-blend-screen = Rasteri
# machine-drafted, needs native review
develop-blend-overlay = Sulautus
# machine-drafted, needs native review
develop-blend-soft-light = Pehmeä valo
# machine-drafted, needs native review
develop-blend-lightness = Vain vaaleus
# machine-drafted, needs native review
develop-blend-colour = Vain väri
# machine-drafted, needs native review
develop-mask-amount = Maskin voimakkuus
# machine-drafted, needs native review
develop-mask-invert = Käänteinen maski
# machine-drafted, needs native review
develop-mask-off = Pois
# machine-drafted, needs native review
develop-mask-on = Päällä
# machine-drafted, needs native review
develop-mask-refine = Tarkenna reunat
# machine-drafted, needs native review
develop-mask-show = Näytä maski
# machine-drafted, needs native review
develop-mask-show-red = Punainen
# machine-drafted, needs native review
develop-mask-show-grey = Harmaa
# machine-drafted, needs native review
develop-mask-remove = Poista maski
# machine-drafted, needs native review
develop-mask-combine-add = Lisää
# machine-drafted, needs native review
develop-mask-combine-subtract = Vähennä
# machine-drafted, needs native review
develop-mask-combine-intersect = Leikkaus
# machine-drafted, needs native review
develop-mask-row-opacity = Peittävyys
# machine-drafted, needs native review
develop-mask-row-invert = Käänteinen
# machine-drafted, needs native review
develop-mask-feather = Pehmeä reuna
# machine-drafted, needs native review
develop-mask-roundness = Pyöristys
# machine-drafted, needs native review
develop-mask-brush-size = Siveltimen koko
# machine-drafted, needs native review
develop-mask-brush-feather = Siveltimen reuna
# machine-drafted, needs native review
develop-mask-brush-flow = Virtaus
# machine-drafted, needs native review
develop-mask-brush-mode = Sivellin
# machine-drafted, needs native review
develop-mask-brush-paint = Maalaa
# machine-drafted, needs native review
develop-mask-brush-erase = Pyyhi
# machine-drafted, needs native review
develop-mask-range-from = Alkaen
# machine-drafted, needs native review
develop-mask-range-full-from = Täysi alkaen
# machine-drafted, needs native review
develop-mask-range-full-to = Täysi asti
# machine-drafted, needs native review
develop-mask-range-to = Asti
# machine-drafted, needs native review
develop-mask-range-test = Testaa
# machine-drafted, needs native review
develop-mask-range-input = Syöte
# machine-drafted, needs native review
develop-mask-range-output = Tulos
# machine-drafted, needs native review
develop-mask-colour-pick = Valitse väri
# machine-drafted, needs native review
develop-mask-colour-pick-more = Valitse toinen väri
# machine-drafted, needs native review
develop-mask-colour-range = Alue
# machine-drafted, needs native review
develop-mask-colour-falloff = Hiipuma
# machine-drafted, needs native review
develop-mask-edit = Muokkaa kuvassa
# machine-drafted, needs native review
develop-mask-row-remove = Poista { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Poistetaanko moduulin { $module } maski? Sen muodot poistuvat. Levyllä oleva kuva säilyy.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Poistetaanko { $name } moduulista { $module }? Muut maskit ja levyllä oleva kuva säilyvät.
# machine-drafted, needs native review
develop-mask-remove-ok = Poista
# machine-drafted, needs native review
develop-mask-removed = Maski poistettu.
# machine-drafted, needs native review
develop-mask-added = { $name } lisätty.
# machine-drafted, needs native review
develop-mask-full = Maskissa on jo niin monta osaa kuin siihen mahtuu.
# machine-drafted, needs native review
develop-mask-editing = Muokataan: { $name }. Vedä kahvoja, tai paina sarkainta ja nuolinäppäimiä.
# machine-drafted, needs native review
develop-mask-painting = Maalataan: { $name }. Vedä kuvassa.
# machine-drafted, needs native review
develop-mask-picking = Valitse väri napsauttamalla kuvaa.
# machine-drafted, needs native review
develop-mask-picked = Väri valittu.
# machine-drafted, needs native review
develop-mask-done = Maskin muokkaus valmis.
# machine-drafted, needs native review
develop-mask-handles = Maskin kahvat
# machine-drafted, needs native review
develop-handle-centre = Keskikohta
# machine-drafted, needs native review
develop-handle-width = Leveys
# machine-drafted, needs native review
develop-handle-height = Korkeus
# machine-drafted, needs native review
develop-handle-rotate = Kierrä
# machine-drafted, needs native review
develop-handle-feather = Pehmeä reuna
# machine-drafted, needs native review
develop-handle-inner = Sisäreuna
# machine-drafted, needs native review
develop-handle-start = Alku
# machine-drafted, needs native review
develop-handle-end = Loppu
# machine-drafted, needs native review
develop-handle-point = Piste { $n }
# machine-drafted, needs native review
develop-instance = Taso
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Taso { $n }.
# machine-drafted, needs native review
develop-instance-new = Uusi taso
# machine-drafted, needs native review
develop-instance-duplicate = Monista taso
# machine-drafted, needs native review
develop-instance-delete = Poista taso
# machine-drafted, needs native review
develop-instance-earlier = Siirrä aiemmaksi
# machine-drafted, needs native review
develop-instance-later = Siirrä myöhemmäksi
# machine-drafted, needs native review
develop-instance-delete-confirm = Poistetaanko taso { $n } moduulista { $module }? Sen asetukset ja maskit poistuvat. Levyllä oleva kuva säilyy.
# machine-drafted, needs native review
develop-instance-delete-ok = Poista
# machine-drafted, needs native review
develop-instance-deleted = Taso poistettu.
# machine-drafted, needs native review
develop-history-mask = Maski: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Maskin muodot: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Uusi: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Poistettu: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Järjestys: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Muut ohjelmat
# machine-drafted, needs native review
develop-darktable-edited = Muokattu darktablessa:
# machine-drafted, needs native review
develop-darktable-open = Avaa darktablessa
# machine-drafted, needs native review
develop-darktable-opening = Avataan darktablessa
# machine-drafted, needs native review
develop-darktable-failed = darktable ei käynnistynyt: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Tuo Lightroomista
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] Tuotiin 1 Lightroomin asetus. Tulos on likimääräinen.
   *[other] Tuotiin { $count } Lightroomin asetusta. Tulos on likimääräinen.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Ei tuotavaa: nämä asetukset eivät muuta tässä mitään.
# machine-drafted, needs native review
develop-history-lightroom = Tuotu Lightroomista (likimäärin)
# machine-drafted, needs native review
develop-conflict-title = Kuvan { $file } muokkaus muuttui Specularin ulkopuolella
# machine-drafted, needs native review
develop-conflict-file = Sivutiedosto: tallennettu { $time }
# machine-drafted, needs native review
develop-conflict-library = Kirjasto: tallennettu { $time }
# machine-drafted, needs native review
develop-conflict-writer = Sivutiedoston kirjoitti { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Pidä yksi. Toinen korvataan.
# machine-drafted, needs native review
develop-conflict-use-file = Käytä tiedoston
# machine-drafted, needs native review
develop-conflict-keep-library = Pidä kirjaston
# machine-drafted, needs native review
develop-conflict-used-file = Käytetään sivutiedoston muokkausta
# machine-drafted, needs native review
develop-conflict-kept-library = Pidetään kirjaston muokkaus
# machine-drafted, needs native review
toast-develop-edit-newer-process = Muokkaus tehtiin uudemmalla Specularilla. Se voi näyttää täällä erilaiselta.
# machine-drafted, needs native review
info-darktable-edited = Muokattu darktablessa
# machine-drafted, needs native review
info-darktable-no-modules = Ei moduuleja päällä
# machine-drafted, needs native review
info-darktable-group = Tiedosto
# machine-drafted, needs native review
toast-develop-edit-partial = Muokkaus tehtiin uudemmalla Specularilla. Osat, joita tämä versio ei osaa näyttää, jätetään pois. Muutoksia ei tallenneta.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Kuvan vieressä olevaa muokkaustiedostoa ei voi lukea. Se säilytetään ennallaan. Muutoksia ei tallenneta.
# machine-drafted, needs native review
develop-demosaic = Raw-yksityiskohdat
develop-demosaic-directional = Vakio
develop-demosaic-dual = Pehmeät tasaiset alueet
develop-demosaic-fast = Nopea
develop-dual-threshold = Tasaisten alueiden kontrasti
