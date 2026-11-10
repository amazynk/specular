# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Dansk.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Ét modul i denne redigering er opdateret til en nyere version. Resultatet kan se lidt anderledes ud.
   *[other] { $count } moduler i denne redigering er opdateret til en nyere version. Resultatet kan se lidt anderledes ud.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Ét modul i denne redigering kræver en nyere version af Specular. Det bevares, men vises ikke.
   *[other] { $count } moduler i denne redigering kræver en nyere version af Specular. De bevares, men vises ikke.
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
develop-curve-point = Punkt { $index } af { $count }: ind { $input }, ud { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Piletasterne flytter punktet. Page Up og Page Down vælger et andet. Insert tilføjer et punkt, Delete fjerner det.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutral
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Nuance { $hue }°, styrke { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Piletasterne flytter farven. Delete nulstiller den.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Fremkald
# machine-drafted, needs native review
develop-undo = Fortryd
# machine-drafted, needs native review
develop-redo = Annuller fortryd
# machine-drafted, needs native review
develop-before-after = Før og efter
# machine-drafted, needs native review
develop-close = Forlad Fremkald
# machine-drafted, needs native review
develop-history = Historik
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } til
develop-card-menu = Indstillinger for { $module }
develop-card-changed = Ændret
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Filmstrimmel

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Eksponering
develop-module-light = Lys
develop-look-profile = Profil
develop-look-standard = Standard
develop-look-film = Film
develop-look-natural = Naturlig
develop-module-grading = Farvegradering
develop-add-tool = Tilføj et værktøj
# machine-drafted, needs native review
develop-module-white-balance = Hvidbalance
# machine-drafted, needs native review
develop-module-tone = Tone
# machine-drafted, needs native review
develop-module-contrast = Kontrast og omfang
# machine-drafted, needs native review
develop-module-colour = Farve
# machine-drafted, needs native review
develop-module-curve = Kurve
# machine-drafted, needs native review
develop-module-crop = Beskær og ret op
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Andet modul
# machine-drafted, needs native review
develop-exposure = Eksponering
# machine-drafted, needs native review
develop-black-level = Sortniveau
# machine-drafted, needs native review
develop-temperature = Temperatur
# machine-drafted, needs native review
develop-tint = Farvetone
# machine-drafted, needs native review
develop-highlights = Højlys
# machine-drafted, needs native review
develop-shadows = Skygger
# machine-drafted, needs native review
develop-contrast = Kontrast
# machine-drafted, needs native review
develop-whites = Hvide
# machine-drafted, needs native review
develop-blacks = Sorte
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = HDR-grænse
# machine-drafted, needs native review
develop-hdr-preview = Vis
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Livlighed
# machine-drafted, needs native review
develop-saturation = Mætning
# machine-drafted, needs native review
develop-colour-wheels = Farvehjul
# machine-drafted, needs native review
develop-curve = Tonekurve
# machine-drafted, needs native review
develop-module-local-contrast = Lokal kontrast
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Automatisk eksponering
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Udjævning
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Tekstur
# machine-drafted, needs native review
develop-clarity = Klarhed
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Sortpunkt
# machine-drafted, needs native review
develop-levels-mid = Midtpunkt
# machine-drafted, needs native review
develop-levels-white = Hvidpunkt
# machine-drafted, needs native review
develop-module-detail = Detaljer
# machine-drafted, needs native review
develop-module-denoise = Støjreduktion
# machine-drafted, needs native review
develop-module-sharpen = Skærpning
# machine-drafted, needs native review
develop-noise-luminance = Luminansstøj
# machine-drafted, needs native review
develop-noise-detail = Støjdetaljer
# machine-drafted, needs native review
develop-noise-colour = Farvestøj
# machine-drafted, needs native review
develop-sharpen-amount = Skærpningsstyrke
# machine-drafted, needs native review
develop-sharpen-radius = Skærpningsradius
# machine-drafted, needs native review
develop-sharpen-detail = Skærpningsdetaljer
# machine-drafted, needs native review
develop-sharpen-masking = Maskering
# machine-drafted, needs native review
develop-capture-amount = Optagelsesskærpning
# machine-drafted, needs native review
develop-capture-radius = Optagelsesradius
# machine-drafted, needs native review
develop-straighten = Ret op
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Beskær
# machine-drafted, needs native review
develop-crop-turn = Drej
# machine-drafted, needs native review
develop-crop-flip = Spejlvend
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Objektivkorrektioner
develop-module-optics = Optik
# machine-drafted, needs native review
develop-lens-distortion = Forvrængning
# machine-drafted, needs native review
develop-lens-vignette = Vignettering
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Rød farvekant
# machine-drafted, needs native review
develop-lens-tca-blue = Blå farvekant
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Objektivprofil
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automatisk
# machine-drafted, needs native review
develop-lensfun-off = Ingen
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Ingen objektivprofil.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Ingen Lensfun-profil for { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Ingen Lensfun-profil for dette objektiv.
# machine-drafted, needs native review
develop-lensfun-no-name = Filen angiver ikke objektivet. Vælg det ovenfor.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } objektiver passer. Vælg et ovenfor.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Beskær. Træk i kanterne, eller Ctrl+træk langs en linje, der skal være vandret. Enter beholder, Esc går tilbage.
# machine-drafted, needs native review
develop-crop-kept = Beskæring beholdt
# machine-drafted, needs native review
develop-crop-cancelled = Beskæring fortrudt
# machine-drafted, needs native review
develop-crop-reset = Beskæring og opretning nulstillet
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Rettet op til { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Format { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = frit
# machine-drafted, needs native review
develop-crop-aspect-original = originalt
# machine-drafted, needs native review
develop-crop-turned-right = Drejet til højre
# machine-drafted, needs native review
develop-crop-turned-left = Drejet til venstre
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Spejlvendt fra venstre til højre
# machine-drafted, needs native review
develop-crop-flipped-vertical = Spejlvendt fra top til bund
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Format { $aspect } · Hjælpelinjer: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A format · X byt · O hjælpelinjer · [ ] drej · H V spejlvend · Enter behold · Esc tilbage
# machine-drafted, needs native review
develop-crop-guide-thirds = tredjedele
# machine-drafted, needs native review
develop-crop-guide-golden = det gyldne snit
# machine-drafted, needs native review
develop-crop-guide-diagonal = diagonaler
# machine-drafted, needs native review
develop-crop-guide-grid = gitter

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspektiv
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Perspektivretning
# machine-drafted, needs native review
develop-persp-mode-off = Fra
# machine-drafted, needs native review
develop-persp-mode-auto = Auto
# machine-drafted, needs native review
develop-persp-mode-level = Vater
# machine-drafted, needs native review
develop-persp-mode-vertical = Lodret
# machine-drafted, needs native review
develop-persp-mode-full = Fuld
# machine-drafted, needs native review
develop-persp-mode-guided = Styret
# machine-drafted, needs native review
develop-persp-vertical = Lodret
# machine-drafted, needs native review
develop-persp-horizontal = Vandret
# machine-drafted, needs native review
develop-persp-rotate = Roter
# machine-drafted, needs native review
develop-persp-aspect = Forhold
# machine-drafted, needs native review
develop-persp-scale = Skalér
# machine-drafted, needs native review
develop-persp-offset-x = Sidelæns
# machine-drafted, needs native review
develop-persp-offset-y = Op eller ned
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Undgå tomme kanter
# machine-drafted, needs native review
develop-persp-guides = Hjælpelinjer
# machine-drafted, needs native review
develop-persp-focal = Brændvidde
# machine-drafted, needs native review
develop-persp-draw-guides = Tegn hjælpelinjer
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Tegn hjælpelinjer ({ $n } af { $max })
# machine-drafted, needs native review
develop-persp-reset = Nulstil perspektiv
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Perspektivretning: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Finder linjer…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Nulstil perspektiv? Dine { $n } hjælpelinjer fjernes. Fortryd bringer dem tilbage.
# machine-drafted, needs native review
develop-persp-reset-ok = Nulstil
# machine-drafted, needs native review
develop-persp-reset-done = Perspektiv nulstillet
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Fjern hjælpelinje { $n }? Fortryd bringer den tilbage.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Fjern
# machine-drafted, needs native review
develop-persp-guide-removed = Hjælpelinje fjernet
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Lodret { $vertical }, vandret { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Perspektivretning mislykkedes: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: hældning { $vertical }°, drejning { $horizontal }°, rotation { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } kræver mindst to lige linjer, den kan skelne fra hinanden. Prøv en anden tilstand, eller tegn hjælpelinjer.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Højst { $n } hjælpelinjer. Træk i en ende for at flytte en.
# machine-drafted, needs native review
develop-persp-guides-on = Hjælpelinjer. Træk langs en linje, der skal være lodret eller vandret.
# machine-drafted, needs native review
develop-persp-guides-off = Beskær
# machine-drafted, needs native review
develop-persp-guide-moved = Hjælpelinje flyttet
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Hjælpelinje { $n } tilføjet
# machine-drafted, needs native review
develop-persp-guides-keys = Træk for at tilføje · Træk i en ende for at flytte · Delete fjerner · Shift+R beskær
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Hjælpelinjer: { $n } af { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Skygger
# machine-drafted, needs native review
develop-wheel-midtones = Mellemtoner
# machine-drafted, needs native review
develop-wheel-highlights = Højlys
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminans for skygger
# machine-drafted, needs native review
develop-luminance-midtones = Luminans for mellemtoner
# machine-drafted, needs native review
develop-luminance-highlights = Luminans for højlys
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Global
# machine-drafted, needs native review
develop-luminance-global = Global luminans
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Farveintensitet
# machine-drafted, needs native review
develop-brilliance = Farvernes lysstyrke

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Farvemixer
# machine-drafted, needs native review
develop-colour-red = Rød
# machine-drafted, needs native review
develop-colour-orange = Orange
# machine-drafted, needs native review
develop-colour-yellow = Gul
# machine-drafted, needs native review
develop-colour-green = Grøn
# machine-drafted, needs native review
develop-colour-aqua = Akvamarin
# machine-drafted, needs native review
develop-colour-blue = Blå
# machine-drafted, needs native review
develop-colour-purple = Lilla
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Nuance for { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Mætning for { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminans for { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Punktfarve
# machine-drafted, needs native review
develop-point-pick = Vælg en farve
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Vælg en anden farve
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Klik på den farve, der skal ændres. Esc annullerer.
# machine-drafted, needs native review
develop-point-picked = Farve valgt
# machine-drafted, needs native review
develop-point-cancelled = Farvevalg annulleret
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Område
# machine-drafted, needs native review
develop-point-hue-range = Nuanceområde
# machine-drafted, needs native review
develop-point-saturation-range = Mætningsområde
# machine-drafted, needs native review
develop-point-lightness-range = Lyshedsområde
# machine-drafted, needs native review
develop-point-hue = Nuanceforskydning
# machine-drafted, needs native review
develop-point-saturation = Mætningsforskydning
# machine-drafted, needs native review
develop-point-lightness = Lyshedsforskydning

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Sort-hvid
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminans for { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Vælg en LUT-fil…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Fil: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Kan ikke læse { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Vælg en LUT-fil
# machine-drafted, needs native review
develop-lut-kind = LUT-filer
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Lavet til
# machine-drafted, needs native review
develop-lut-space-linear = Lineær (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES lineær (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Foto (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Video (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Metode
# machine-drafted, needs native review
develop-lut-tetrahedral = Præcis
# machine-drafted, needs native review
develop-lut-trilinear = Enkel
# machine-drafted, needs native review
develop-lut-strength = Styrke

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Original
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } til
# machine-drafted, needs native review
develop-history-off = { $module } fra
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Tilbage til originalen
# machine-drafted, needs native review
develop-at-step = Tilbage til: { $step }
# machine-drafted, needs native review
develop-undone = Fortrudt: { $step }
# machine-drafted, needs native review
develop-redone = Gentaget: { $step }
# machine-drafted, needs native review
develop-entered = Fremkald. Redigeringer ændrer aldrig filen.
# machine-drafted, needs native review
develop-left = Forlod Fremkald
# machine-drafted, needs native review
develop-split-on = Før og efter side om side
# machine-drafted, needs native review
develop-split-off = Kun redigeringen
# machine-drafted, needs native review
develop-showing-before = Viser originalen
# machine-drafted, needs native review
develop-showing-after = Viser redigeringen
# machine-drafted, needs native review
# Labels over the image.
develop-before = Før
# machine-drafted, needs native review
develop-after = Efter
# machine-drafted, needs native review
develop-working = Fremkalder…
# machine-drafted, needs native review
develop-preview-failed = Kan ikke vise redigeringen for denne fil

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Fremkald
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Hurtigredigering: hold nede, og rul så
# machine-drafted, needs native review
palette-develop = Fremkald
# machine-drafted, needs native review
palette-develop-help = Rediger fotoet uden at ændre filen. Igen for at forlade.
# machine-drafted, needs native review
palette-develop-before-after = Før og efter
# machine-drafted, needs native review
palette-develop-before-after-help = Vis originalen eller redigeringen igen.
# machine-drafted, needs native review
palette-develop-split = Før og efter delt
# machine-drafted, needs native review
palette-develop-split-help = Vis originalen og redigeringen side om side.
# machine-drafted, needs native review
palette-develop-panel = Gå til panelet Fremkald
# machine-drafted, needs native review
palette-develop-panel-help = Flyt tastaturet til panelet. Esc går tilbage.
# machine-drafted, needs native review
palette-develop-speed-exposure = Hurtigrediger eksponering
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Hold nede, og rul så eller tryk på piletasterne for at ændre eksponeringen.
# machine-drafted, needs native review
palette-develop-speed-contrast = Hurtigrediger kontrast
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Hold nede, og rul så eller tryk på piletasterne for at ændre kontrasten.
# machine-drafted, needs native review
palette-develop-speed-highlights = Hurtigrediger højlys
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Hold nede, og rul så eller tryk på piletasterne for at ændre højlys.
# machine-drafted, needs native review
palette-develop-speed-shadows = Hurtigrediger skygger
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Hold nede, og rul så eller tryk på piletasterne for at ændre skyggerne.
# machine-drafted, needs native review
palette-develop-speed-whites = Hurtigrediger hvide
# machine-drafted, needs native review
palette-develop-speed-whites-help = Hold nede, og rul så eller tryk på piletasterne for at ændre de hvide.
# machine-drafted, needs native review
palette-develop-speed-blacks = Hurtigrediger sorte
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Hold nede, og rul så eller tryk på piletasterne for at ændre de sorte.
# machine-drafted, needs native review
palette-develop-speed-temperature = Hurtigrediger temperatur
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Hold nede, og rul så eller tryk på piletasterne for at ændre hvidbalancens temperatur.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Hurtigrediger livlighed
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Hold nede, og rul så eller tryk på piletasterne for at ændre livligheden.
# machine-drafted, needs native review
palette-develop-crop = Beskær og ret op
# machine-drafted, needs native review
palette-develop-crop-help = Beskær, ret op, drej eller spejlvend fotoet. Enter beholder, Esc går tilbage.
# machine-drafted, needs native review
disabled-not-photo = Kun for stillbilleder
# machine-drafted, needs native review
disabled-develop-off = Fremkald er slået fra i profilerne Medicin og Videnskab
# machine-drafted, needs native review
develop-module-corrections = Korrektioner
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Fra
# machine-drafted, needs native review
develop-on = Til
# machine-drafted, needs native review
develop-highlight-method = Udbrændte højlys
# machine-drafted, needs native review
develop-highlight-opposed = Genskab farve
# machine-drafted, needs native review
develop-highlight-rebuild = Genskab store flader
# machine-drafted, needs native review
develop-highlight-clip = Klip til hvid
# machine-drafted, needs native review
develop-highlight-threshold = Klipningsniveau
# machine-drafted, needs native review
develop-module-hot-pixels = Varme pixels
# machine-drafted, needs native review
develop-hot-threshold = Tærskel for varme pixels
# machine-drafted, needs native review
develop-module-ca = Kromatisk aberration
# machine-drafted, needs native review
develop-ca-mode = Farvefrynser
# machine-drafted, needs native review
develop-ca-auto = Automatisk
# machine-drafted, needs native review
develop-ca-manual = Manuel
# machine-drafted, needs native review
develop-ca-red = Rødforskydning
# machine-drafted, needs native review
develop-ca-blue = Blåforskydning
# machine-drafted, needs native review
develop-module-defringe = Fjern farvefrynser
# machine-drafted, needs native review
develop-defringe = Fjern farvefrynser
# machine-drafted, needs native review
develop-defringe-purple = Lilla nuance
# machine-drafted, needs native review
develop-defringe-green = Grøn nuance
# machine-drafted, needs native review
develop-defringe-range = Nuanceområde
# machine-drafted, needs native review
develop-defringe-edge = Kantkontrast
# machine-drafted, needs native review
develop-module-flat-field = Kalibreringsbilleder
# machine-drafted, needs native review
develop-calibration = Kalibrering
# machine-drafted, needs native review
develop-calibration-auto = Fra mappe
# machine-drafted, needs native review
develop-calibration-chosen = Valgte filer
# machine-drafted, needs native review
develop-flat-choose = Vælg et flat-billede…
# machine-drafted, needs native review
develop-dark-choose = Vælg et dark-billede…
# machine-drafted, needs native review
develop-flat-file = Flatbillede: { $name }
# machine-drafted, needs native review
develop-dark-file = Darkbillede: { $name }
# machine-drafted, needs native review
develop-flat-use = Flatbillede
# machine-drafted, needs native review
develop-flat-use-full = Vignettering og støv
# machine-drafted, needs native review
develop-flat-use-vignetting = Kun vignettering
# machine-drafted, needs native review
develop-flat-dialog = Vælg et flat-billede
# machine-drafted, needs native review
develop-dark-dialog = Vælg et dark-billede
# machine-drafted, needs native review
develop-raw-kind = Raw-filer
# machine-drafted, needs native review
develop-flat-frame = Flatbillede
# machine-drafted, needs native review
develop-dark-frame = Darkbillede

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Tilføj maske
# machine-drafted, needs native review
develop-mask-kind-ellipse = Ellipse
# machine-drafted, needs native review
develop-mask-kind-rectangle = Rektangel
# machine-drafted, needs native review
develop-mask-kind-polygon = Polygon
# machine-drafted, needs native review
develop-mask-kind-linear = Lineær overgang
# machine-drafted, needs native review
develop-mask-kind-radial = Radial overgang
# machine-drafted, needs native review
develop-mask-kind-brush = Pensel
# machine-drafted, needs native review
develop-mask-kind-luminance = Luminansområde
# machine-drafted, needs native review
develop-mask-kind-hue = Nuanceområde
# machine-drafted, needs native review
develop-mask-kind-chroma = Mætningsområde
# machine-drafted, needs native review
develop-mask-kind-colour = Farveområde
# machine-drafted, needs native review
develop-mask-kind-other = Anden maske
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Blanding
# machine-drafted, needs native review
develop-blend-normal = Normal
# machine-drafted, needs native review
develop-blend-multiply = Multiplicer
# machine-drafted, needs native review
develop-blend-screen = Skærm
# machine-drafted, needs native review
develop-blend-overlay = Overlejring
# machine-drafted, needs native review
develop-blend-soft-light = Blødt lys
# machine-drafted, needs native review
develop-blend-lightness = Kun lyshed
# machine-drafted, needs native review
develop-blend-colour = Kun farve
# machine-drafted, needs native review
develop-mask-amount = Maskestyrke
# machine-drafted, needs native review
develop-mask-invert = Vend maske om
# machine-drafted, needs native review
develop-mask-off = Fra
# machine-drafted, needs native review
develop-mask-on = Til
# machine-drafted, needs native review
develop-mask-refine = Forfin kanter
# machine-drafted, needs native review
develop-mask-show = Vis maske
# machine-drafted, needs native review
develop-mask-show-red = Rød
# machine-drafted, needs native review
develop-mask-show-grey = Grå
# machine-drafted, needs native review
develop-mask-remove = Fjern maske
# machine-drafted, needs native review
develop-mask-combine-add = Tilføj
# machine-drafted, needs native review
develop-mask-combine-subtract = Træk fra
# machine-drafted, needs native review
develop-mask-combine-intersect = Fællesmængde
# machine-drafted, needs native review
develop-mask-row-opacity = Uigennemsigtighed
# machine-drafted, needs native review
develop-mask-row-invert = Vend om
# machine-drafted, needs native review
develop-mask-feather = Blød kant
# machine-drafted, needs native review
develop-mask-roundness = Afrunding
# machine-drafted, needs native review
develop-mask-brush-size = Penselstørrelse
# machine-drafted, needs native review
develop-mask-brush-feather = Penselkant
# machine-drafted, needs native review
develop-mask-brush-flow = Flow
# machine-drafted, needs native review
develop-mask-brush-mode = Pensel
# machine-drafted, needs native review
develop-mask-brush-paint = Mal
# machine-drafted, needs native review
develop-mask-brush-erase = Visk ud
# machine-drafted, needs native review
develop-mask-range-from = Fra
# machine-drafted, needs native review
develop-mask-range-full-from = Fuld fra
# machine-drafted, needs native review
develop-mask-range-full-to = Fuld til
# machine-drafted, needs native review
develop-mask-range-to = Til
# machine-drafted, needs native review
develop-mask-range-test = Test
# machine-drafted, needs native review
develop-mask-range-input = Input
# machine-drafted, needs native review
develop-mask-range-output = Output
# machine-drafted, needs native review
develop-mask-colour-pick = Vælg en farve
# machine-drafted, needs native review
develop-mask-colour-pick-more = Vælg en farve mere
# machine-drafted, needs native review
develop-mask-colour-range = Område
# machine-drafted, needs native review
develop-mask-colour-falloff = Aftagning
# machine-drafted, needs native review
develop-mask-edit = Rediger på billedet
# machine-drafted, needs native review
develop-mask-row-remove = Fjern { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Fjern masken for { $module }? Dens former forsvinder. Fotoet på disken bliver.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Fjern { $name } fra { $module }? De andre masker og fotoet på disken bliver.
# machine-drafted, needs native review
develop-mask-remove-ok = Fjern
# machine-drafted, needs native review
develop-mask-removed = Masken er fjernet.
# machine-drafted, needs native review
develop-mask-added = { $name } tilføjet.
# machine-drafted, needs native review
develop-mask-full = Masken har allerede så mange dele, den kan rumme.
# machine-drafted, needs native review
develop-mask-editing = Redigerer { $name }. Træk i håndtagene, eller tryk Tab og piletasterne.
# machine-drafted, needs native review
develop-mask-painting = Maler { $name }. Træk på billedet.
# machine-drafted, needs native review
develop-mask-picking = Klik på billedet for at vælge en farve.
# machine-drafted, needs native review
develop-mask-picked = Farve valgt.
# machine-drafted, needs native review
develop-mask-done = Færdig med masken.
# machine-drafted, needs native review
develop-mask-handles = Maskehåndtag
# machine-drafted, needs native review
develop-handle-centre = Midte
# machine-drafted, needs native review
develop-handle-width = Bredde
# machine-drafted, needs native review
develop-handle-height = Højde
# machine-drafted, needs native review
develop-handle-rotate = Drej
# machine-drafted, needs native review
develop-handle-feather = Blød kant
# machine-drafted, needs native review
develop-handle-inner = Inderkant
# machine-drafted, needs native review
develop-handle-start = Start
# machine-drafted, needs native review
develop-handle-end = Slut
# machine-drafted, needs native review
develop-handle-point = Punkt { $n }
# machine-drafted, needs native review
develop-instance = Lag
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Lag { $n }.
# machine-drafted, needs native review
develop-instance-new = Nyt lag
# machine-drafted, needs native review
develop-instance-duplicate = Dupliker lag
# machine-drafted, needs native review
develop-instance-delete = Slet lag
# machine-drafted, needs native review
develop-instance-earlier = Flyt tidligere
# machine-drafted, needs native review
develop-instance-later = Flyt senere
# machine-drafted, needs native review
develop-instance-delete-confirm = Slet lag { $n } af { $module }? Dets indstillinger og masker forsvinder. Fotoet på disken bliver.
# machine-drafted, needs native review
develop-instance-delete-ok = Slet
# machine-drafted, needs native review
develop-instance-deleted = Laget er slettet.
# machine-drafted, needs native review
develop-history-mask = Maske: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Maskeformer: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Ny: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Slettet: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Rækkefølge: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Andre programmer
# machine-drafted, needs native review
develop-darktable-edited = Redigeret i darktable:
# machine-drafted, needs native review
develop-darktable-open = Åbn i darktable
# machine-drafted, needs native review
develop-darktable-opening = Åbner i darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable startede ikke: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importér fra Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] 1 Lightroom-indstilling importeret. Resultatet er omtrentligt.
   *[other] { $count } Lightroom-indstillinger importeret. Resultatet er omtrentligt.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Intet at importere: indstillingerne ændrer intet her.
# machine-drafted, needs native review
develop-history-lightroom = Importeret fra Lightroom (omtrentligt)
# machine-drafted, needs native review
develop-conflict-title = Redigeringen af { $file } blev ændret uden for Specular
# machine-drafted, needs native review
develop-conflict-file = Sidefil: gemt { $time }
# machine-drafted, needs native review
develop-conflict-library = Bibliotek: gemt { $time }
# machine-drafted, needs native review
develop-conflict-writer = Sidefilen blev skrevet af { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Behold én. Den anden erstattes.
# machine-drafted, needs native review
develop-conflict-use-file = Brug filens
# machine-drafted, needs native review
develop-conflict-keep-library = Behold bibliotekets
# machine-drafted, needs native review
develop-conflict-used-file = Bruger redigeringen fra sidefilen
# machine-drafted, needs native review
develop-conflict-kept-library = Beholder bibliotekets redigering
# machine-drafted, needs native review
toast-develop-edit-newer-process = Redigeringen er lavet i en nyere Specular. Den kan se anderledes ud her.
# machine-drafted, needs native review
info-darktable-edited = Redigeret i darktable
# machine-drafted, needs native review
info-darktable-no-modules = Ingen moduler slået til
# machine-drafted, needs native review
info-darktable-group = Fil
# machine-drafted, needs native review
toast-develop-edit-partial = Redigeringen er lavet i en nyere Specular. Dele, denne version ikke kan vise, er udeladt. Ændringer her gemmes ikke.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Redigeringsfilen ved siden af fotoet kan ikke læses. Den bevares uændret. Ændringer her gemmes ikke.
# machine-drafted, needs native review
develop-demosaic = Raw-detaljer
develop-demosaic-directional = Standard
develop-demosaic-dual = Glatte flader
develop-demosaic-fast = Hurtig
develop-dual-threshold = Fladekontrast
