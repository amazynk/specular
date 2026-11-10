# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Nederlands.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Eén module van deze bewerking is bijgewerkt naar een nieuwere versie. Het resultaat kan er iets anders uitzien.
   *[other] { $count } modules van deze bewerking zijn bijgewerkt naar een nieuwere versie. Het resultaat kan er iets anders uitzien.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Eén module van deze bewerking vereist een nieuwere versie van Specular. Die blijft behouden, maar wordt niet getoond.
   *[other] { $count } modules van deze bewerking vereisen een nieuwere versie van Specular. Ze blijven behouden, maar worden niet getoond.
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
develop-curve-point = Punt { $index } van { $count }: in { $input }, uit { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Pijltjes verplaatsen het punt. Page Up en Page Down kiezen een ander. Insert voegt een punt toe, Delete verwijdert het.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutraal
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Tint { $hue }°, sterkte { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Pijltjes verplaatsen de kleur. Delete zet hem terug.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Ontwikkelen
# machine-drafted, needs native review
develop-undo = Ongedaan maken
# machine-drafted, needs native review
develop-redo = Opnieuw
# machine-drafted, needs native review
develop-before-after = Voor en na
# machine-drafted, needs native review
develop-close = Ontwikkelen verlaten
# machine-drafted, needs native review
develop-history = Geschiedenis
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } aan
develop-card-menu = Opties voor { $module }
develop-card-changed = Gewijzigd
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Filmstrip

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Belichting
develop-module-light = Licht
develop-look-profile = Profiel
develop-look-standard = Standaard
develop-look-film = Film
develop-look-natural = Natuurlijk
develop-module-grading = Kleurgradatie
develop-add-tool = Gereedschap toevoegen
# machine-drafted, needs native review
develop-module-white-balance = Witbalans
# machine-drafted, needs native review
develop-module-tone = Toon
# machine-drafted, needs native review
develop-module-contrast = Contrast en bereik
# machine-drafted, needs native review
develop-module-colour = Kleur
# machine-drafted, needs native review
develop-module-curve = Curve
# machine-drafted, needs native review
develop-module-crop = Uitsnijden en rechttrekken
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Andere module
# machine-drafted, needs native review
develop-exposure = Belichting
# machine-drafted, needs native review
develop-black-level = Zwartniveau
# machine-drafted, needs native review
develop-temperature = Temperatuur
# machine-drafted, needs native review
develop-tint = Kleurtint
# machine-drafted, needs native review
develop-highlights = Hooglichten
# machine-drafted, needs native review
develop-shadows = Schaduwen
# machine-drafted, needs native review
develop-contrast = Contrast
# machine-drafted, needs native review
develop-whites = Witte tinten
# machine-drafted, needs native review
develop-blacks = Zwarte tinten
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = HDR-limiet
# machine-drafted, needs native review
develop-hdr-preview = Tonen
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Levendigheid
# machine-drafted, needs native review
develop-saturation = Verzadiging
# machine-drafted, needs native review
develop-colour-wheels = Kleurwielen
# machine-drafted, needs native review
develop-curve = Tooncurve
# machine-drafted, needs native review
develop-module-local-contrast = Lokaal contrast
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Automatische belichting
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Afvlakking
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Textuur
# machine-drafted, needs native review
develop-clarity = Duidelijkheid
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Zwartpunt
# machine-drafted, needs native review
develop-levels-mid = Middelpunt
# machine-drafted, needs native review
develop-levels-white = Witpunt
# machine-drafted, needs native review
develop-module-detail = Detail
# machine-drafted, needs native review
develop-module-denoise = Ruisreductie
# machine-drafted, needs native review
develop-module-sharpen = Verscherpen
# machine-drafted, needs native review
develop-noise-luminance = Luminantieruis
# machine-drafted, needs native review
develop-noise-detail = Ruisdetail
# machine-drafted, needs native review
develop-noise-colour = Kleurruis
# machine-drafted, needs native review
develop-sharpen-amount = Verscherpingssterkte
# machine-drafted, needs native review
develop-sharpen-radius = Verscherpingsstraal
# machine-drafted, needs native review
develop-sharpen-detail = Verscherpingsdetail
# machine-drafted, needs native review
develop-sharpen-masking = Maskeren
# machine-drafted, needs native review
develop-capture-amount = Opnameverscherping
# machine-drafted, needs native review
develop-capture-radius = Opnamestraal
# machine-drafted, needs native review
develop-straighten = Rechttrekken
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Uitsnijden
# machine-drafted, needs native review
develop-crop-turn = Draaien
# machine-drafted, needs native review
develop-crop-flip = Spiegelen
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Lenscorrecties
develop-module-optics = Optiek
# machine-drafted, needs native review
develop-lens-distortion = Vervorming
# machine-drafted, needs native review
develop-lens-vignette = Vignettering
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Rode kleurrand
# machine-drafted, needs native review
develop-lens-tca-blue = Blauwe kleurrand
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Lensprofiel
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automatisch
# machine-drafted, needs native review
develop-lensfun-off = Geen
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Geen lensprofiel.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Geen Lensfun-profiel voor { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Geen Lensfun-profiel voor deze lens.
# machine-drafted, needs native review
develop-lensfun-no-name = Het bestand noemt de lens niet. Kies hem hierboven.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } lenzen komen overeen. Kies er hierboven een.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Uitsnijden. Sleep de randen, of Ctrl+sleep langs een lijn die waterpas moet zijn. Enter behoudt, Esc gaat terug.
# machine-drafted, needs native review
develop-crop-kept = Uitsnede behouden
# machine-drafted, needs native review
develop-crop-cancelled = Uitsnede ongedaan gemaakt
# machine-drafted, needs native review
develop-crop-reset = Uitsnede en rechttrekken teruggezet
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Rechtgetrokken naar { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Verhouding { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = vrij
# machine-drafted, needs native review
develop-crop-aspect-original = origineel
# machine-drafted, needs native review
develop-crop-turned-right = Rechtsom gedraaid
# machine-drafted, needs native review
develop-crop-turned-left = Linksom gedraaid
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Van links naar rechts gespiegeld
# machine-drafted, needs native review
develop-crop-flipped-vertical = Van boven naar onder gespiegeld
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Verhouding { $aspect } · Hulplijnen: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A verhouding · X omwisselen · O hulplijnen · [ ] draaien · H V spiegelen · Enter behouden · Esc terug
# machine-drafted, needs native review
develop-crop-guide-thirds = derden
# machine-drafted, needs native review
develop-crop-guide-golden = gulden snede
# machine-drafted, needs native review
develop-crop-guide-diagonal = diagonalen
# machine-drafted, needs native review
develop-crop-guide-grid = raster

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspectief
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Rechtzetten
# machine-drafted, needs native review
develop-persp-mode-off = Uit
# machine-drafted, needs native review
develop-persp-mode-auto = Automatisch
# machine-drafted, needs native review
develop-persp-mode-level = Waterpas
# machine-drafted, needs native review
develop-persp-mode-vertical = Verticaal
# machine-drafted, needs native review
develop-persp-mode-full = Volledig
# machine-drafted, needs native review
develop-persp-mode-guided = Begeleid
# machine-drafted, needs native review
develop-persp-vertical = Verticaal
# machine-drafted, needs native review
develop-persp-horizontal = Horizontaal
# machine-drafted, needs native review
develop-persp-rotate = Draaien
# machine-drafted, needs native review
develop-persp-aspect = Verhouding
# machine-drafted, needs native review
develop-persp-scale = Schaal
# machine-drafted, needs native review
develop-persp-offset-x = Zijwaarts
# machine-drafted, needs native review
develop-persp-offset-y = Omhoog of omlaag
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Lege randen vermijden
# machine-drafted, needs native review
develop-persp-guides = Hulplijnen
# machine-drafted, needs native review
develop-persp-focal = Brandpuntsafstand
# machine-drafted, needs native review
develop-persp-draw-guides = Hulplijnen tekenen
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Hulplijnen tekenen ({ $n } van { $max })
# machine-drafted, needs native review
develop-persp-reset = Perspectief terugzetten
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Rechtzetten: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Lijnen zoeken…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Perspectief terugzetten? Je { $n } hulplijnen worden verwijderd. Ongedaan maken haalt ze terug.
# machine-drafted, needs native review
develop-persp-reset-ok = Terugzetten
# machine-drafted, needs native review
develop-persp-reset-done = Perspectief teruggezet
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Hulplijn { $n } verwijderen? Ongedaan maken haalt hem terug.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Verwijderen
# machine-drafted, needs native review
develop-persp-guide-removed = Hulplijn verwijderd
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Verticaal { $vertical }, horizontaal { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Rechtzetten mislukt: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: kanteling { $vertical }°, zwenking { $horizontal }°, rotatie { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } heeft minstens twee rechte lijnen nodig die het kan onderscheiden. Probeer een andere modus, of teken hulplijnen.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Hoogstens { $n } hulplijnen. Sleep een uiteinde om er een te verplaatsen.
# machine-drafted, needs native review
develop-persp-guides-on = Hulplijnen. Sleep langs een lijn die loodrecht of waterpas moet zijn.
# machine-drafted, needs native review
develop-persp-guides-off = Uitsnijden
# machine-drafted, needs native review
develop-persp-guide-moved = Hulplijn verplaatst
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Hulplijn { $n } toegevoegd
# machine-drafted, needs native review
develop-persp-guides-keys = Slepen voegt toe · Uiteinde slepen verplaatst · Delete verwijdert · Shift+R uitsnijden
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Hulplijnen: { $n } van { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Schaduwen
# machine-drafted, needs native review
develop-wheel-midtones = Middentonen
# machine-drafted, needs native review
develop-wheel-highlights = Hooglichten
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminantie schaduwen
# machine-drafted, needs native review
develop-luminance-midtones = Luminantie middentonen
# machine-drafted, needs native review
develop-luminance-highlights = Luminantie hooglichten
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Globaal
# machine-drafted, needs native review
develop-luminance-global = Luminantie globaal
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Kleurintensiteit
# machine-drafted, needs native review
develop-brilliance = Kleurhelderheid

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Kleurmixer
# machine-drafted, needs native review
develop-colour-red = Rood
# machine-drafted, needs native review
develop-colour-orange = Oranje
# machine-drafted, needs native review
develop-colour-yellow = Geel
# machine-drafted, needs native review
develop-colour-green = Groen
# machine-drafted, needs native review
develop-colour-aqua = Aquablauw
# machine-drafted, needs native review
develop-colour-blue = Blauw
# machine-drafted, needs native review
develop-colour-purple = Paars
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Tint { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Verzadiging { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminantie { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Puntkleur
# machine-drafted, needs native review
develop-point-pick = Kies een kleur
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Kies een andere kleur
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Klik op de kleur die je wilt wijzigen. Esc annuleert.
# machine-drafted, needs native review
develop-point-picked = Kleur gekozen
# machine-drafted, needs native review
develop-point-cancelled = Kiezen geannuleerd
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Bereik
# machine-drafted, needs native review
develop-point-hue-range = Tintbereik
# machine-drafted, needs native review
develop-point-saturation-range = Verzadigingsbereik
# machine-drafted, needs native review
develop-point-lightness-range = Lichtheidsbereik
# machine-drafted, needs native review
develop-point-hue = Tintverschuiving
# machine-drafted, needs native review
develop-point-saturation = Verzadigingsverschuiving
# machine-drafted, needs native review
develop-point-lightness = Lichtheidsverschuiving

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Zwart-wit
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminantie { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Kies een LUT-bestand…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Bestand: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Kan { $name } niet lezen
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Kies een LUT-bestand
# machine-drafted, needs native review
develop-lut-kind = LUT-bestanden
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Gemaakt voor
# machine-drafted, needs native review
develop-lut-space-linear = Lineair (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES lineair (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Foto (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Video (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Methode
# machine-drafted, needs native review
develop-lut-tetrahedral = Nauwkeurig
# machine-drafted, needs native review
develop-lut-trilinear = Eenvoudig
# machine-drafted, needs native review
develop-lut-strength = Sterkte

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Origineel
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } aan
# machine-drafted, needs native review
develop-history-off = { $module } uit
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Terug naar het origineel
# machine-drafted, needs native review
develop-at-step = Terug naar: { $step }
# machine-drafted, needs native review
develop-undone = Ongedaan gemaakt: { $step }
# machine-drafted, needs native review
develop-redone = Opnieuw gedaan: { $step }
# machine-drafted, needs native review
develop-entered = Ontwikkelen. Bewerkingen veranderen het bestand nooit.
# machine-drafted, needs native review
develop-left = Ontwikkelen verlaten
# machine-drafted, needs native review
develop-split-on = Voor en na naast elkaar
# machine-drafted, needs native review
develop-split-off = Alleen de bewerking
# machine-drafted, needs native review
develop-showing-before = Het origineel wordt getoond
# machine-drafted, needs native review
develop-showing-after = De bewerking wordt getoond
# machine-drafted, needs native review
# Labels over the image.
develop-before = Voor
# machine-drafted, needs native review
develop-after = Na
# machine-drafted, needs native review
develop-working = Ontwikkelen…
# machine-drafted, needs native review
develop-preview-failed = Kan de bewerking voor dit bestand niet tonen

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Ontwikkelen
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Snel bewerken: ingedrukt houden, dan scrollen
# machine-drafted, needs native review
palette-develop = Ontwikkelen
# machine-drafted, needs native review
palette-develop-help = Deze foto bewerken zonder het bestand te veranderen. Nogmaals om te verlaten.
# machine-drafted, needs native review
palette-develop-before-after = Voor en na
# machine-drafted, needs native review
palette-develop-before-after-help = Het origineel tonen, of weer de bewerking.
# machine-drafted, needs native review
palette-develop-split = Voor en na gesplitst
# machine-drafted, needs native review
palette-develop-split-help = Het origineel en de bewerking naast elkaar tonen.
# machine-drafted, needs native review
palette-develop-panel = Naar het paneel Ontwikkelen
# machine-drafted, needs native review
palette-develop-panel-help = Het toetsenbord naar het paneel verplaatsen. Esc gaat terug.
# machine-drafted, needs native review
palette-develop-speed-exposure = Snel bewerken: belichting
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Ingedrukt houden, dan scrollen of pijltjes indrukken om de belichting te veranderen.
# machine-drafted, needs native review
palette-develop-speed-contrast = Snel bewerken: contrast
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Ingedrukt houden, dan scrollen of pijltjes indrukken om het contrast te veranderen.
# machine-drafted, needs native review
palette-develop-speed-highlights = Snel bewerken: hooglichten
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Ingedrukt houden, dan scrollen of pijltjes indrukken om de hooglichten te veranderen.
# machine-drafted, needs native review
palette-develop-speed-shadows = Snel bewerken: schaduwen
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Ingedrukt houden, dan scrollen of pijltjes indrukken om de schaduwen te veranderen.
# machine-drafted, needs native review
palette-develop-speed-whites = Snel bewerken: witte tinten
# machine-drafted, needs native review
palette-develop-speed-whites-help = Ingedrukt houden, dan scrollen of pijltjes indrukken om de witte tinten te veranderen.
# machine-drafted, needs native review
palette-develop-speed-blacks = Snel bewerken: zwarte tinten
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Ingedrukt houden, dan scrollen of pijltjes indrukken om de zwarte tinten te veranderen.
# machine-drafted, needs native review
palette-develop-speed-temperature = Snel bewerken: temperatuur
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Ingedrukt houden, dan scrollen of pijltjes indrukken om de temperatuur van de witbalans te veranderen.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Snel bewerken: levendigheid
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Ingedrukt houden, dan scrollen of pijltjes indrukken om de levendigheid te veranderen.
# machine-drafted, needs native review
palette-develop-crop = Uitsnijden en rechttrekken
# machine-drafted, needs native review
palette-develop-crop-help = De foto uitsnijden, rechttrekken, draaien of spiegelen. Enter behoudt, Esc gaat terug.
# machine-drafted, needs native review
disabled-not-photo = Alleen voor foto’s
# machine-drafted, needs native review
disabled-develop-off = Ontwikkelen staat uit in de profielen Medisch en Wetenschap
# machine-drafted, needs native review
develop-module-corrections = Correcties
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Uit
# machine-drafted, needs native review
develop-on = Aan
# machine-drafted, needs native review
develop-highlight-method = Uitgebeten hooglichten
# machine-drafted, needs native review
develop-highlight-opposed = Kleur herstellen
# machine-drafted, needs native review
develop-highlight-rebuild = Grote vlakken herstellen
# machine-drafted, needs native review
develop-highlight-clip = Afkappen naar wit
# machine-drafted, needs native review
develop-highlight-threshold = Afkapniveau
# machine-drafted, needs native review
develop-module-hot-pixels = Hete pixels
# machine-drafted, needs native review
develop-hot-threshold = Drempel hete pixels
# machine-drafted, needs native review
develop-module-ca = Chromatische aberratie
# machine-drafted, needs native review
develop-ca-mode = Kleurranden
# machine-drafted, needs native review
develop-ca-auto = Automatisch
# machine-drafted, needs native review
develop-ca-manual = Handmatig
# machine-drafted, needs native review
develop-ca-red = Roodverschuiving
# machine-drafted, needs native review
develop-ca-blue = Blauwverschuiving
# machine-drafted, needs native review
develop-module-defringe = Kleurranden verwijderen
# machine-drafted, needs native review
develop-defringe = Kleurranden verwijderen
# machine-drafted, needs native review
develop-defringe-purple = Paarse tint
# machine-drafted, needs native review
develop-defringe-green = Groene tint
# machine-drafted, needs native review
develop-defringe-range = Tintbereik
# machine-drafted, needs native review
develop-defringe-edge = Randcontrast
# machine-drafted, needs native review
develop-module-flat-field = Kalibratiebeelden
# machine-drafted, needs native review
develop-calibration = Kalibratie
# machine-drafted, needs native review
develop-calibration-auto = Uit map
# machine-drafted, needs native review
develop-calibration-chosen = Gekozen bestanden
# machine-drafted, needs native review
develop-flat-choose = Flatframe kiezen…
# machine-drafted, needs native review
develop-dark-choose = Darkframe kiezen…
# machine-drafted, needs native review
develop-flat-file = Flatframe: { $name }
# machine-drafted, needs native review
develop-dark-file = Darkframe: { $name }
# machine-drafted, needs native review
develop-flat-use = Flatframe
# machine-drafted, needs native review
develop-flat-use-full = Vignettering en stof
# machine-drafted, needs native review
develop-flat-use-vignetting = Alleen vignettering
# machine-drafted, needs native review
develop-flat-dialog = Flatframe kiezen
# machine-drafted, needs native review
develop-dark-dialog = Darkframe kiezen
# machine-drafted, needs native review
develop-raw-kind = Raw-bestanden
# machine-drafted, needs native review
develop-flat-frame = Flatframe
# machine-drafted, needs native review
develop-dark-frame = Darkframe

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Masker toevoegen
# machine-drafted, needs native review
develop-mask-kind-ellipse = Ellips
# machine-drafted, needs native review
develop-mask-kind-rectangle = Rechthoek
# machine-drafted, needs native review
develop-mask-kind-polygon = Veelhoek
# machine-drafted, needs native review
develop-mask-kind-linear = Lineair verloop
# machine-drafted, needs native review
develop-mask-kind-radial = Radiaal verloop
# machine-drafted, needs native review
develop-mask-kind-brush = Penseel
# machine-drafted, needs native review
develop-mask-kind-luminance = Luminantiebereik
# machine-drafted, needs native review
develop-mask-kind-hue = Tintbereik
# machine-drafted, needs native review
develop-mask-kind-chroma = Verzadigingsbereik
# machine-drafted, needs native review
develop-mask-kind-colour = Kleurbereik
# machine-drafted, needs native review
develop-mask-kind-other = Ander masker
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Overvloeien
# machine-drafted, needs native review
develop-blend-normal = Normaal
# machine-drafted, needs native review
develop-blend-multiply = Vermenigvuldigen
# machine-drafted, needs native review
develop-blend-screen = Bleken
# machine-drafted, needs native review
develop-blend-overlay = Bedekken
# machine-drafted, needs native review
develop-blend-soft-light = Zacht licht
# machine-drafted, needs native review
develop-blend-lightness = Alleen helderheid
# machine-drafted, needs native review
develop-blend-colour = Alleen kleur
# machine-drafted, needs native review
develop-mask-amount = Maskersterkte
# machine-drafted, needs native review
develop-mask-invert = Masker omkeren
# machine-drafted, needs native review
develop-mask-off = Uit
# machine-drafted, needs native review
develop-mask-on = Aan
# machine-drafted, needs native review
develop-mask-refine = Randen verfijnen
# machine-drafted, needs native review
develop-mask-show = Masker tonen
# machine-drafted, needs native review
develop-mask-show-red = Rood
# machine-drafted, needs native review
develop-mask-show-grey = Grijs
# machine-drafted, needs native review
develop-mask-remove = Masker verwijderen
# machine-drafted, needs native review
develop-mask-combine-add = Toevoegen
# machine-drafted, needs native review
develop-mask-combine-subtract = Aftrekken
# machine-drafted, needs native review
develop-mask-combine-intersect = Doorsnede
# machine-drafted, needs native review
develop-mask-row-opacity = Dekking
# machine-drafted, needs native review
develop-mask-row-invert = Omkeren
# machine-drafted, needs native review
develop-mask-feather = Zachte rand
# machine-drafted, needs native review
develop-mask-roundness = Afronding
# machine-drafted, needs native review
develop-mask-brush-size = Penseelgrootte
# machine-drafted, needs native review
develop-mask-brush-feather = Penseelrand
# machine-drafted, needs native review
develop-mask-brush-flow = Stroom
# machine-drafted, needs native review
develop-mask-brush-mode = Penseel
# machine-drafted, needs native review
develop-mask-brush-paint = Schilderen
# machine-drafted, needs native review
develop-mask-brush-erase = Gummen
# machine-drafted, needs native review
develop-mask-range-from = Van
# machine-drafted, needs native review
develop-mask-range-full-from = Vol vanaf
# machine-drafted, needs native review
develop-mask-range-full-to = Vol tot
# machine-drafted, needs native review
develop-mask-range-to = Tot
# machine-drafted, needs native review
develop-mask-range-test = Testen
# machine-drafted, needs native review
develop-mask-range-input = Invoer
# machine-drafted, needs native review
develop-mask-range-output = Uitvoer
# machine-drafted, needs native review
develop-mask-colour-pick = Kleur kiezen
# machine-drafted, needs native review
develop-mask-colour-pick-more = Nog een kleur kiezen
# machine-drafted, needs native review
develop-mask-colour-range = Bereik
# machine-drafted, needs native review
develop-mask-colour-falloff = Afname
# machine-drafted, needs native review
develop-mask-edit = Bewerken op de foto
# machine-drafted, needs native review
develop-mask-row-remove = { $name } verwijderen
# machine-drafted, needs native review
develop-mask-remove-confirm = Het masker van { $module } verwijderen? De vormen verdwijnen. De foto op schijf blijft.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = { $name } uit { $module } verwijderen? De andere maskers en de foto op schijf blijven.
# machine-drafted, needs native review
develop-mask-remove-ok = Verwijderen
# machine-drafted, needs native review
develop-mask-removed = Masker verwijderd.
# machine-drafted, needs native review
develop-mask-added = { $name } toegevoegd.
# machine-drafted, needs native review
develop-mask-full = Dit masker heeft al zoveel delen als het aankan.
# machine-drafted, needs native review
develop-mask-editing = { $name } bewerken. Sleep de grepen, of druk op Tab en de pijltjes.
# machine-drafted, needs native review
develop-mask-painting = { $name } schilderen. Sleep over de foto.
# machine-drafted, needs native review
develop-mask-picking = Klik op de foto om een kleur te kiezen.
# machine-drafted, needs native review
develop-mask-picked = Kleur gekozen.
# machine-drafted, needs native review
develop-mask-done = Klaar met het masker.
# machine-drafted, needs native review
develop-mask-handles = Maskergrepen
# machine-drafted, needs native review
develop-handle-centre = Midden
# machine-drafted, needs native review
develop-handle-width = Breedte
# machine-drafted, needs native review
develop-handle-height = Hoogte
# machine-drafted, needs native review
develop-handle-rotate = Draaien
# machine-drafted, needs native review
develop-handle-feather = Zachte rand
# machine-drafted, needs native review
develop-handle-inner = Binnenrand
# machine-drafted, needs native review
develop-handle-start = Begin
# machine-drafted, needs native review
develop-handle-end = Eind
# machine-drafted, needs native review
develop-handle-point = Punt { $n }
# machine-drafted, needs native review
develop-instance = Laag
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Laag { $n }.
# machine-drafted, needs native review
develop-instance-new = Nieuwe laag
# machine-drafted, needs native review
develop-instance-duplicate = Laag dupliceren
# machine-drafted, needs native review
develop-instance-delete = Laag verwijderen
# machine-drafted, needs native review
develop-instance-earlier = Eerder
# machine-drafted, needs native review
develop-instance-later = Later
# machine-drafted, needs native review
develop-instance-delete-confirm = Laag { $n } van { $module } verwijderen? De instellingen en maskers verdwijnen. De foto op schijf blijft.
# machine-drafted, needs native review
develop-instance-delete-ok = Verwijderen
# machine-drafted, needs native review
develop-instance-deleted = Laag verwijderd.
# machine-drafted, needs native review
develop-history-mask = Masker: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Maskervormen: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Nieuw: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Verwijderd: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Volgorde: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Andere programma’s
# machine-drafted, needs native review
develop-darktable-edited = Bewerkt in darktable:
# machine-drafted, needs native review
develop-darktable-open = Openen in darktable
# machine-drafted, needs native review
develop-darktable-opening = Openen in darktable…
# machine-drafted, needs native review
develop-darktable-failed = darktable startte niet: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importeren uit Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] 1 Lightroom-instelling geïmporteerd. Het resultaat is bij benadering.
   *[other] { $count } Lightroom-instellingen geïmporteerd. Het resultaat is bij benadering.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Niets te importeren: deze Lightroom-instellingen veranderen hier niets.
# machine-drafted, needs native review
develop-history-lightroom = Geïmporteerd uit Lightroom (bij benadering)
# machine-drafted, needs native review
develop-conflict-title = De bewerking van { $file } is buiten Specular gewijzigd
# machine-drafted, needs native review
develop-conflict-file = Sidecarbestand: bewaard { $time }
# machine-drafted, needs native review
develop-conflict-library = Bibliotheek: bewaard { $time }
# machine-drafted, needs native review
develop-conflict-writer = Het sidecarbestand is geschreven door { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Houd er één. De andere wordt vervangen.
# machine-drafted, needs native review
develop-conflict-use-file = Die van het bestand
# machine-drafted, needs native review
develop-conflict-keep-library = Die van de bibliotheek houden
# machine-drafted, needs native review
develop-conflict-used-file = De bewerking uit het sidecarbestand wordt gebruikt
# machine-drafted, needs native review
develop-conflict-kept-library = De bewerking van de bibliotheek blijft
# machine-drafted, needs native review
toast-develop-edit-newer-process = Deze bewerking komt uit een nieuwere Specular. Ze kan er hier anders uitzien.
# machine-drafted, needs native review
info-darktable-edited = Bewerkt in darktable
# machine-drafted, needs native review
info-darktable-no-modules = Geen modules aan
# machine-drafted, needs native review
info-darktable-group = Bestand
# machine-drafted, needs native review
toast-develop-edit-partial = Deze bewerking komt uit een nieuwere Specular. Delen die deze versie niet kan tonen, ontbreken. Wijzigingen hier worden niet bewaard.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Het bewerkingsbestand naast deze foto is onleesbaar. Het blijft zoals het is. Wijzigingen hier worden niet bewaard.
# machine-drafted, needs native review
develop-demosaic = Raw-details
develop-demosaic-directional = Standaard
develop-demosaic-dual = Vlakke delen glad
develop-demosaic-fast = Snel
develop-dual-threshold = Contrast vlakke delen
