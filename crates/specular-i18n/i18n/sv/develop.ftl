# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Svenska.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] En modul i den här redigeringen har uppdaterats till en nyare version. Resultatet kan se något annorlunda ut.
   *[other] { $count } moduler i den här redigeringen har uppdaterats till en nyare version. Resultatet kan se något annorlunda ut.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] En modul i den här redigeringen kräver en nyare version av Specular. Den behålls men visas inte.
   *[other] { $count } moduler i den här redigeringen kräver en nyare version av Specular. De behålls men visas inte.
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
develop-curve-point = Punkt { $index } av { $count }: in { $input }, ut { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Piltangenterna flyttar punkten. Page Up och Page Down väljer en annan. Insert lägger till en punkt, Delete tar bort den.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutral
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Nyans { $hue }°, styrka { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Piltangenterna flyttar färgen. Delete återställer den.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Framkalla
# machine-drafted, needs native review
develop-undo = Ångra
# machine-drafted, needs native review
develop-redo = Gör om
# machine-drafted, needs native review
develop-before-after = Före och efter
# machine-drafted, needs native review
develop-close = Lämna Framkalla
# machine-drafted, needs native review
develop-history = Historik
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } på
develop-card-menu = Alternativ för { $module }
develop-card-changed = Ändrad
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Filmremsa

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Exponering
develop-module-light = Ljus
develop-look-profile = Profil
develop-look-standard = Standard
develop-look-film = Film
develop-look-natural = Naturlig
develop-module-grading = Färggradering
develop-add-tool = Lägg till ett verktyg
# machine-drafted, needs native review
develop-module-white-balance = Vitbalans
# machine-drafted, needs native review
develop-module-tone = Ton
# machine-drafted, needs native review
develop-module-contrast = Kontrast och omfång
# machine-drafted, needs native review
develop-module-colour = Färg
# machine-drafted, needs native review
develop-module-curve = Kurva
# machine-drafted, needs native review
develop-module-crop = Beskär och räta upp
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Annan modul
# machine-drafted, needs native review
develop-exposure = Exponering
# machine-drafted, needs native review
develop-black-level = Svartnivå
# machine-drafted, needs native review
develop-temperature = Temperatur
# machine-drafted, needs native review
develop-tint = Färgton
# machine-drafted, needs native review
develop-highlights = Högdagrar
# machine-drafted, needs native review
develop-shadows = Skuggor
# machine-drafted, needs native review
develop-contrast = Kontrast
# machine-drafted, needs native review
develop-whites = Vitt
# machine-drafted, needs native review
develop-blacks = Svart
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = HDR-gräns
# machine-drafted, needs native review
develop-hdr-preview = Visa
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Lyster
# machine-drafted, needs native review
develop-saturation = Mättnad
# machine-drafted, needs native review
develop-colour-wheels = Färghjul
# machine-drafted, needs native review
develop-curve = Tonkurva
# machine-drafted, needs native review
develop-module-local-contrast = Lokal kontrast
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Automatisk exponering
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Utjämning
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Textur
# machine-drafted, needs native review
develop-clarity = Klarhet
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Svartpunkt
# machine-drafted, needs native review
develop-levels-mid = Mittpunkt
# machine-drafted, needs native review
develop-levels-white = Vitpunkt
# machine-drafted, needs native review
develop-module-detail = Detaljer
# machine-drafted, needs native review
develop-module-denoise = Brusreducering
# machine-drafted, needs native review
develop-module-sharpen = Skärpa
# machine-drafted, needs native review
develop-noise-luminance = Luminansbrus
# machine-drafted, needs native review
develop-noise-detail = Brusdetaljer
# machine-drafted, needs native review
develop-noise-colour = Färgbrus
# machine-drafted, needs native review
develop-sharpen-amount = Skärpningsmängd
# machine-drafted, needs native review
develop-sharpen-radius = Skärperadie
# machine-drafted, needs native review
develop-sharpen-detail = Skärpedetaljer
# machine-drafted, needs native review
develop-sharpen-masking = Maskering
# machine-drafted, needs native review
develop-capture-amount = Tagningsskärpning
# machine-drafted, needs native review
develop-capture-radius = Tagningsradie
# machine-drafted, needs native review
develop-straighten = Räta upp
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Beskär
# machine-drafted, needs native review
develop-crop-turn = Rotera
# machine-drafted, needs native review
develop-crop-flip = Spegelvänd
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Objektivkorrigeringar
develop-module-optics = Optik
# machine-drafted, needs native review
develop-lens-distortion = Distorsion
# machine-drafted, needs native review
develop-lens-vignette = Vinjettering
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Röd färgkant
# machine-drafted, needs native review
develop-lens-tca-blue = Blå färgkant
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
develop-lensfun-no-match = Ingen Lensfun-profil för { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Ingen Lensfun-profil för det här objektivet.
# machine-drafted, needs native review
develop-lensfun-no-name = Filen anger inte objektivet. Välj det ovan.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } objektiv passar. Välj ett ovan.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Beskär. Dra i kanterna, eller Ctrl+dra längs en linje som ska vara vågrät. Enter behåller, Esc går tillbaka.
# machine-drafted, needs native review
develop-crop-kept = Beskärning behållen
# machine-drafted, needs native review
develop-crop-cancelled = Beskärning ångrad
# machine-drafted, needs native review
develop-crop-reset = Beskärning och upprätning återställda
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Uppriktad till { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Format { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = fritt
# machine-drafted, needs native review
develop-crop-aspect-original = original
# machine-drafted, needs native review
develop-crop-turned-right = Roterad åt höger
# machine-drafted, needs native review
develop-crop-turned-left = Roterad åt vänster
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Spegelvänd från vänster till höger
# machine-drafted, needs native review
develop-crop-flipped-vertical = Spegelvänd uppifrån och ned
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Format { $aspect } · Stödlinjer: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A format · X byt · O stödlinjer · [ ] rotera · H V spegelvänd · Enter behåll · Esc tillbaka
# machine-drafted, needs native review
develop-crop-guide-thirds = tredjedelar
# machine-drafted, needs native review
develop-crop-guide-golden = gyllene snittet
# machine-drafted, needs native review
develop-crop-guide-diagonal = diagonaler
# machine-drafted, needs native review
develop-crop-guide-grid = rutnät

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspektiv
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Perspektivrätning
# machine-drafted, needs native review
develop-persp-mode-off = Av
# machine-drafted, needs native review
develop-persp-mode-auto = Auto
# machine-drafted, needs native review
develop-persp-mode-level = Våg
# machine-drafted, needs native review
develop-persp-mode-vertical = Lodrät
# machine-drafted, needs native review
develop-persp-mode-full = Fullständig
# machine-drafted, needs native review
develop-persp-mode-guided = Styrd
# machine-drafted, needs native review
develop-persp-vertical = Lodrät
# machine-drafted, needs native review
develop-persp-horizontal = Vågrät
# machine-drafted, needs native review
develop-persp-rotate = Rotera
# machine-drafted, needs native review
develop-persp-aspect = Proportion
# machine-drafted, needs native review
develop-persp-scale = Skala
# machine-drafted, needs native review
develop-persp-offset-x = I sidled
# machine-drafted, needs native review
develop-persp-offset-y = Upp eller ner
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Undvik tomma kanter
# machine-drafted, needs native review
develop-persp-guides = Stödlinjer
# machine-drafted, needs native review
develop-persp-focal = Brännvidd
# machine-drafted, needs native review
develop-persp-draw-guides = Rita stödlinjer
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Rita stödlinjer ({ $n } av { $max })
# machine-drafted, needs native review
develop-persp-reset = Återställ perspektiv
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Perspektivrätning: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Söker linjer…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Återställa perspektivet? Dina { $n } stödlinjer tas bort. Ångra tar tillbaka dem.
# machine-drafted, needs native review
develop-persp-reset-ok = Återställ
# machine-drafted, needs native review
develop-persp-reset-done = Perspektivet återställt
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Ta bort stödlinje { $n }? Ångra tar tillbaka den.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Ta bort
# machine-drafted, needs native review
develop-persp-guide-removed = Stödlinje borttagen
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Lodrät { $vertical }, vågrät { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Perspektivrätning misslyckades: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: lutning { $vertical }°, vridning { $horizontal }°, rotation { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } behöver minst två raka linjer som går att skilja åt. Prova ett annat läge, eller rita stödlinjer.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Högst { $n } stödlinjer. Dra i en ände för att flytta en.
# machine-drafted, needs native review
develop-persp-guides-on = Stödlinjer. Dra längs en linje som ska vara lodrät eller vågrät.
# machine-drafted, needs native review
develop-persp-guides-off = Beskär
# machine-drafted, needs native review
develop-persp-guide-moved = Stödlinje flyttad
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Stödlinje { $n } tillagd
# machine-drafted, needs native review
develop-persp-guides-keys = Dra för att lägga till · Dra i en ände för att flytta · Delete tar bort · Shift+R beskär
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Stödlinjer: { $n } av { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Skuggor
# machine-drafted, needs native review
develop-wheel-midtones = Mellantoner
# machine-drafted, needs native review
develop-wheel-highlights = Högdagrar
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminans skuggor
# machine-drafted, needs native review
develop-luminance-midtones = Luminans mellantoner
# machine-drafted, needs native review
develop-luminance-highlights = Luminans högdagrar
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Global
# machine-drafted, needs native review
develop-luminance-global = Luminans global
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Färgintensitet
# machine-drafted, needs native review
develop-brilliance = Färgernas ljusstyrka

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Färgmixer
# machine-drafted, needs native review
develop-colour-red = Röd
# machine-drafted, needs native review
develop-colour-orange = Orange
# machine-drafted, needs native review
develop-colour-yellow = Gul
# machine-drafted, needs native review
develop-colour-green = Grön
# machine-drafted, needs native review
develop-colour-aqua = Akvamarin
# machine-drafted, needs native review
develop-colour-blue = Blå
# machine-drafted, needs native review
develop-colour-purple = Lila
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Nyans { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Mättnad { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminans { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Punktfärg
# machine-drafted, needs native review
develop-point-pick = Välj en färg
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Välj en annan färg
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Klicka på färgen som ska ändras. Esc avbryter.
# machine-drafted, needs native review
develop-point-picked = Färg vald
# machine-drafted, needs native review
develop-point-cancelled = Färgval avbrutet
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Intervall
# machine-drafted, needs native review
develop-point-hue-range = Nyansintervall
# machine-drafted, needs native review
develop-point-saturation-range = Mättnadsomfång
# machine-drafted, needs native review
develop-point-lightness-range = Ljushetsomfång
# machine-drafted, needs native review
develop-point-hue = Nyansförskjutning
# machine-drafted, needs native review
develop-point-saturation = Mättnadsförskjutning
# machine-drafted, needs native review
develop-point-lightness = Ljushetsförskjutning

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Svartvitt
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminans { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Välj en LUT-fil…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Fil: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Kan inte läsa { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Välj en LUT-fil
# machine-drafted, needs native review
develop-lut-kind = LUT-filer
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Gjord för
# machine-drafted, needs native review
develop-lut-space-linear = Linjär (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES linjär (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Foto (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Video (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Metod
# machine-drafted, needs native review
develop-lut-tetrahedral = Exakt
# machine-drafted, needs native review
develop-lut-trilinear = Enkel
# machine-drafted, needs native review
develop-lut-strength = Styrka

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Original
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } på
# machine-drafted, needs native review
develop-history-off = { $module } av
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Tillbaka till originalet
# machine-drafted, needs native review
develop-at-step = Tillbaka till: { $step }
# machine-drafted, needs native review
develop-undone = Ångrat: { $step }
# machine-drafted, needs native review
develop-redone = Gjort om: { $step }
# machine-drafted, needs native review
develop-entered = Framkalla. Redigeringar ändrar aldrig filen.
# machine-drafted, needs native review
develop-left = Lämnade Framkalla
# machine-drafted, needs native review
develop-split-on = Före och efter sida vid sida
# machine-drafted, needs native review
develop-split-off = Bara redigeringen
# machine-drafted, needs native review
develop-showing-before = Visar originalet
# machine-drafted, needs native review
develop-showing-after = Visar redigeringen
# machine-drafted, needs native review
# Labels over the image.
develop-before = Före
# machine-drafted, needs native review
develop-after = Efter
# machine-drafted, needs native review
develop-working = Framkallar…
# machine-drafted, needs native review
develop-preview-failed = Kan inte visa redigeringen för den här filen

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Framkalla
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Snabbredigering: håll ned, rulla sedan
# machine-drafted, needs native review
palette-develop = Framkalla
# machine-drafted, needs native review
palette-develop-help = Redigera fotot utan att ändra filen. Igen för att lämna.
# machine-drafted, needs native review
palette-develop-before-after = Före och efter
# machine-drafted, needs native review
palette-develop-before-after-help = Visa originalet, eller redigeringen igen.
# machine-drafted, needs native review
palette-develop-split = Före och efter delat
# machine-drafted, needs native review
palette-develop-split-help = Visa originalet och redigeringen sida vid sida.
# machine-drafted, needs native review
palette-develop-panel = Gå till panelen Framkalla
# machine-drafted, needs native review
palette-develop-panel-help = Flytta tangentbordet till panelen. Esc går tillbaka.
# machine-drafted, needs native review
palette-develop-speed-exposure = Snabbredigera exponering
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Håll ned, rulla sedan eller tryck på piltangenterna för att ändra exponeringen.
# machine-drafted, needs native review
palette-develop-speed-contrast = Snabbredigera kontrast
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Håll ned, rulla sedan eller tryck på piltangenterna för att ändra kontrasten.
# machine-drafted, needs native review
palette-develop-speed-highlights = Snabbredigera högdagrar
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Håll ned, rulla sedan eller tryck på piltangenterna för att ändra högdagrarna.
# machine-drafted, needs native review
palette-develop-speed-shadows = Snabbredigera skuggor
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Håll ned, rulla sedan eller tryck på piltangenterna för att ändra skuggorna.
# machine-drafted, needs native review
palette-develop-speed-whites = Snabbredigera vitt
# machine-drafted, needs native review
palette-develop-speed-whites-help = Håll ned, rulla sedan eller tryck på piltangenterna för att ändra vitt.
# machine-drafted, needs native review
palette-develop-speed-blacks = Snabbredigera svart
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Håll ned, rulla sedan eller tryck på piltangenterna för att ändra svart.
# machine-drafted, needs native review
palette-develop-speed-temperature = Snabbredigera temperatur
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Håll ned, rulla sedan eller tryck på piltangenterna för att ändra vitbalansens temperatur.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Snabbredigera lyster
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Håll ned, rulla sedan eller tryck på piltangenterna för att ändra lystern.
# machine-drafted, needs native review
palette-develop-crop = Beskär och räta upp
# machine-drafted, needs native review
palette-develop-crop-help = Beskär, räta upp, rotera eller spegelvänd fotot. Enter behåller, Esc går tillbaka.
# machine-drafted, needs native review
disabled-not-photo = Bara för stillbilder
# machine-drafted, needs native review
disabled-develop-off = Framkalla är av i profilerna Medicin och Vetenskap
# machine-drafted, needs native review
develop-module-corrections = Korrigeringar
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Av
# machine-drafted, needs native review
develop-on = På
# machine-drafted, needs native review
develop-highlight-method = Utfrätta högdagrar
# machine-drafted, needs native review
develop-highlight-opposed = Återskapa färg
# machine-drafted, needs native review
develop-highlight-rebuild = Återskapa stora ytor
# machine-drafted, needs native review
develop-highlight-clip = Klipp till vitt
# machine-drafted, needs native review
develop-highlight-threshold = Klippnivå
# machine-drafted, needs native review
develop-module-hot-pixels = Heta pixlar
# machine-drafted, needs native review
develop-hot-threshold = Tröskel för heta pixlar
# machine-drafted, needs native review
develop-module-ca = Kromatisk aberration
# machine-drafted, needs native review
develop-ca-mode = Färgfransar
# machine-drafted, needs native review
develop-ca-auto = Automatisk
# machine-drafted, needs native review
develop-ca-manual = Manuell
# machine-drafted, needs native review
develop-ca-red = Rödförskjutning
# machine-drafted, needs native review
develop-ca-blue = Blåförskjutning
# machine-drafted, needs native review
develop-module-defringe = Ta bort färgfransar
# machine-drafted, needs native review
develop-defringe = Ta bort färgfransar
# machine-drafted, needs native review
develop-defringe-purple = Lila nyans
# machine-drafted, needs native review
develop-defringe-green = Grön nyans
# machine-drafted, needs native review
develop-defringe-range = Nyansintervall
# machine-drafted, needs native review
develop-defringe-edge = Kantkontrast
# machine-drafted, needs native review
develop-module-flat-field = Kalibreringsbilder
# machine-drafted, needs native review
develop-calibration = Kalibrering
# machine-drafted, needs native review
develop-calibration-auto = Från mapp
# machine-drafted, needs native review
develop-calibration-chosen = Valda filer
# machine-drafted, needs native review
develop-flat-choose = Välj en flat…
# machine-drafted, needs native review
develop-dark-choose = Välj en dark…
# machine-drafted, needs native review
develop-flat-file = Flatbild: { $name }
# machine-drafted, needs native review
develop-dark-file = Darkbild: { $name }
# machine-drafted, needs native review
develop-flat-use = Flatbild
# machine-drafted, needs native review
develop-flat-use-full = Vinjettering och damm
# machine-drafted, needs native review
develop-flat-use-vignetting = Endast vinjettering
# machine-drafted, needs native review
develop-flat-dialog = Välj en flat
# machine-drafted, needs native review
develop-dark-dialog = Välj en dark
# machine-drafted, needs native review
develop-raw-kind = Raw-filer
# machine-drafted, needs native review
develop-flat-frame = Flatbild
# machine-drafted, needs native review
develop-dark-frame = Darkbild

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Lägg till mask
# machine-drafted, needs native review
develop-mask-kind-ellipse = Ellips
# machine-drafted, needs native review
develop-mask-kind-rectangle = Rektangel
# machine-drafted, needs native review
develop-mask-kind-polygon = Polygon
# machine-drafted, needs native review
develop-mask-kind-linear = Linjär toning
# machine-drafted, needs native review
develop-mask-kind-radial = Radiell toning
# machine-drafted, needs native review
develop-mask-kind-brush = Pensel
# machine-drafted, needs native review
develop-mask-kind-luminance = Luminansintervall
# machine-drafted, needs native review
develop-mask-kind-hue = Nyansintervall
# machine-drafted, needs native review
develop-mask-kind-chroma = Mättnadsintervall
# machine-drafted, needs native review
develop-mask-kind-colour = Färgintervall
# machine-drafted, needs native review
develop-mask-kind-other = Annan mask
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Blandning
# machine-drafted, needs native review
develop-blend-normal = Normal
# machine-drafted, needs native review
develop-blend-multiply = Multiplicera
# machine-drafted, needs native review
develop-blend-screen = Raster
# machine-drafted, needs native review
develop-blend-overlay = Täcka över
# machine-drafted, needs native review
develop-blend-soft-light = Mjukt ljus
# machine-drafted, needs native review
develop-blend-lightness = Endast ljushet
# machine-drafted, needs native review
develop-blend-colour = Endast färg
# machine-drafted, needs native review
develop-mask-amount = Maskstyrka
# machine-drafted, needs native review
develop-mask-invert = Invertera mask
# machine-drafted, needs native review
develop-mask-off = Av
# machine-drafted, needs native review
develop-mask-on = På
# machine-drafted, needs native review
develop-mask-refine = Förfina kanter
# machine-drafted, needs native review
develop-mask-show = Visa mask
# machine-drafted, needs native review
develop-mask-show-red = Röd
# machine-drafted, needs native review
develop-mask-show-grey = Grå
# machine-drafted, needs native review
develop-mask-remove = Ta bort mask
# machine-drafted, needs native review
develop-mask-combine-add = Lägg till
# machine-drafted, needs native review
develop-mask-combine-subtract = Dra ifrån
# machine-drafted, needs native review
develop-mask-combine-intersect = Snitt
# machine-drafted, needs native review
develop-mask-row-opacity = Opacitet
# machine-drafted, needs native review
develop-mask-row-invert = Invertera
# machine-drafted, needs native review
develop-mask-feather = Mjuk kant
# machine-drafted, needs native review
develop-mask-roundness = Rundning
# machine-drafted, needs native review
develop-mask-brush-size = Penselstorlek
# machine-drafted, needs native review
develop-mask-brush-feather = Penselkant
# machine-drafted, needs native review
develop-mask-brush-flow = Flöde
# machine-drafted, needs native review
develop-mask-brush-mode = Pensel
# machine-drafted, needs native review
develop-mask-brush-paint = Måla
# machine-drafted, needs native review
develop-mask-brush-erase = Sudda
# machine-drafted, needs native review
develop-mask-range-from = Från
# machine-drafted, needs native review
develop-mask-range-full-from = Fullt från
# machine-drafted, needs native review
develop-mask-range-full-to = Fullt till
# machine-drafted, needs native review
develop-mask-range-to = Till
# machine-drafted, needs native review
develop-mask-range-test = Testa
# machine-drafted, needs native review
develop-mask-range-input = Indata
# machine-drafted, needs native review
develop-mask-range-output = Utdata
# machine-drafted, needs native review
develop-mask-colour-pick = Välj en färg
# machine-drafted, needs native review
develop-mask-colour-pick-more = Välj en färg till
# machine-drafted, needs native review
develop-mask-colour-range = Intervall
# machine-drafted, needs native review
develop-mask-colour-falloff = Avtoning
# machine-drafted, needs native review
develop-mask-edit = Redigera i bilden
# machine-drafted, needs native review
develop-mask-row-remove = Ta bort { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Ta bort masken för { $module }? Dess former försvinner. Fotot på disken finns kvar.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Ta bort { $name } från { $module }? De andra maskerna och fotot på disken finns kvar.
# machine-drafted, needs native review
develop-mask-remove-ok = Ta bort
# machine-drafted, needs native review
develop-mask-removed = Masken borttagen.
# machine-drafted, needs native review
develop-mask-added = { $name } tillagd.
# machine-drafted, needs native review
develop-mask-full = Masken har redan så många delar den rymmer.
# machine-drafted, needs native review
develop-mask-editing = Redigerar { $name }. Dra i handtagen, eller tryck Tab och piltangenterna.
# machine-drafted, needs native review
develop-mask-painting = Målar { $name }. Dra i bilden.
# machine-drafted, needs native review
develop-mask-picking = Klicka i bilden för att välja en färg.
# machine-drafted, needs native review
develop-mask-picked = Färg vald.
# machine-drafted, needs native review
develop-mask-done = Klar med masken.
# machine-drafted, needs native review
develop-mask-handles = Maskhandtag
# machine-drafted, needs native review
develop-handle-centre = Mitt
# machine-drafted, needs native review
develop-handle-width = Bredd
# machine-drafted, needs native review
develop-handle-height = Höjd
# machine-drafted, needs native review
develop-handle-rotate = Rotera
# machine-drafted, needs native review
develop-handle-feather = Mjuk kant
# machine-drafted, needs native review
develop-handle-inner = Innerkant
# machine-drafted, needs native review
develop-handle-start = Början
# machine-drafted, needs native review
develop-handle-end = Slut
# machine-drafted, needs native review
develop-handle-point = Punkt { $n }
# machine-drafted, needs native review
develop-instance = Lager
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Lager { $n }.
# machine-drafted, needs native review
develop-instance-new = Nytt lager
# machine-drafted, needs native review
develop-instance-duplicate = Duplicera lager
# machine-drafted, needs native review
develop-instance-delete = Ta bort lager
# machine-drafted, needs native review
develop-instance-earlier = Flytta tidigare
# machine-drafted, needs native review
develop-instance-later = Flytta senare
# machine-drafted, needs native review
develop-instance-delete-confirm = Ta bort lager { $n } av { $module }? Dess inställningar och masker försvinner. Fotot på disken finns kvar.
# machine-drafted, needs native review
develop-instance-delete-ok = Ta bort
# machine-drafted, needs native review
develop-instance-deleted = Lagret borttaget.
# machine-drafted, needs native review
develop-history-mask = Mask: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Maskformer: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Ny: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Borttagen: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Ordning: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Andra program
# machine-drafted, needs native review
develop-darktable-edited = Redigerad i darktable:
# machine-drafted, needs native review
develop-darktable-open = Öppna i darktable
# machine-drafted, needs native review
develop-darktable-opening = Öppnar i darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable startade inte: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importera från Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] 1 Lightroom-inställning importerad. Resultatet är ungefärligt.
   *[other] { $count } Lightroom-inställningar importerade. Resultatet är ungefärligt.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Inget att importera: inställningarna ändrar inget här.
# machine-drafted, needs native review
develop-history-lightroom = Importerad från Lightroom (ungefärligt)
# machine-drafted, needs native review
develop-conflict-title = Redigeringen av { $file } ändrades utanför Specular
# machine-drafted, needs native review
develop-conflict-file = Sidofil: sparad { $time }
# machine-drafted, needs native review
develop-conflict-library = Bibliotek: sparad { $time }
# machine-drafted, needs native review
develop-conflict-writer = Sidofilen skrevs av { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Behåll en. Den andra ersätts.
# machine-drafted, needs native review
develop-conflict-use-file = Använd filens
# machine-drafted, needs native review
develop-conflict-keep-library = Behåll bibliotekets
# machine-drafted, needs native review
develop-conflict-used-file = Använder redigeringen från sidofilen
# machine-drafted, needs native review
develop-conflict-kept-library = Behåller bibliotekets redigering
# machine-drafted, needs native review
toast-develop-edit-newer-process = Redigeringen gjordes i en nyare Specular. Den kan se annorlunda ut här.
# machine-drafted, needs native review
info-darktable-edited = Redigerad i darktable
# machine-drafted, needs native review
info-darktable-no-modules = Inga moduler på
# machine-drafted, needs native review
info-darktable-group = Fil
# machine-drafted, needs native review
toast-develop-edit-partial = Redigeringen gjordes i en nyare Specular. Delar som den här versionen inte kan visa utelämnas. Ändringar här sparas inte.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Redigeringsfilen bredvid fotot går inte att läsa. Den lämnas orörd. Ändringar här sparas inte.
# machine-drafted, needs native review
develop-demosaic = Raw-detaljer
develop-demosaic-directional = Standard
develop-demosaic-dual = Släta jämna ytor
develop-demosaic-fast = Snabb
develop-dual-threshold = Kontrast för jämna ytor
