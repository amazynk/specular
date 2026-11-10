# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Čeština.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Jeden modul této úpravy byl aktualizován na novější verzi. Výsledek může vypadat trochu jinak.
    [few] { $count } moduly této úpravy byly aktualizovány na novější verzi. Výsledek může vypadat trochu jinak.
    [many] { $count } modulu této úpravy bylo aktualizováno na novější verzi. Výsledek může vypadat trochu jinak.
   *[other] { $count } modulů této úpravy bylo aktualizováno na novější verzi. Výsledek může vypadat trochu jinak.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Jeden modul této úpravy vyžaduje novější verzi aplikace Specular. Zůstává zachován, ale nezobrazuje se.
    [few] { $count } moduly této úpravy vyžadují novější verzi aplikace Specular. Zůstávají zachovány, ale nezobrazují se.
    [many] { $count } modulu této úpravy vyžaduje novější verzi aplikace Specular. Zůstávají zachovány, ale nezobrazují se.
   *[other] { $count } modulů této úpravy vyžaduje novější verzi aplikace Specular. Zůstávají zachovány, ale nezobrazují se.
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
develop-curve-point = Bod { $index } z { $count }: vstup { $input }, výstup { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Šipky posouvají bod. Page Up a Page Down vyberou jiný. Insert přidá bod, Delete ho odebere.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutrální
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Odstín { $hue }°, síla { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Šipky posouvají barvu. Delete ji vynuluje.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Vyvolání
# machine-drafted, needs native review
develop-undo = Zpět
# machine-drafted, needs native review
develop-redo = Znovu
# machine-drafted, needs native review
develop-before-after = Před a po
# machine-drafted, needs native review
develop-close = Opustit vyvolání
# machine-drafted, needs native review
develop-history = Historie
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module }: zapnuto
develop-card-menu = { $module }: možnosti
develop-card-changed = Změněno
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Filmový pás

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Expozice
develop-module-light = Světlo
develop-look-profile = Profil
develop-look-standard = Standardní
develop-look-film = Film
develop-look-natural = Přirozený
develop-module-grading = Barevné ladění
develop-add-tool = Přidat nástroj
# machine-drafted, needs native review
develop-module-white-balance = Vyvážení bílé
# machine-drafted, needs native review
develop-module-tone = Tóny
# machine-drafted, needs native review
develop-module-contrast = Kontrast a rozsah
# machine-drafted, needs native review
develop-module-colour = Barva
# machine-drafted, needs native review
develop-module-curve = Křivka
# machine-drafted, needs native review
develop-module-crop = Oříznutí a narovnání
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Jiný modul
# machine-drafted, needs native review
develop-exposure = Expozice
# machine-drafted, needs native review
develop-black-level = Úroveň černé
# machine-drafted, needs native review
develop-temperature = Teplota
# machine-drafted, needs native review
develop-tint = Nádech
# machine-drafted, needs native review
develop-highlights = Světla
# machine-drafted, needs native review
develop-shadows = Stíny
# machine-drafted, needs native review
develop-contrast = Kontrast
# machine-drafted, needs native review
develop-whites = Bílé
# machine-drafted, needs native review
develop-blacks = Černé
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = Limit HDR
# machine-drafted, needs native review
develop-hdr-preview = Zobrazit
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Živost
# machine-drafted, needs native review
develop-saturation = Sytost
# machine-drafted, needs native review
develop-colour-wheels = Barevná kola
# machine-drafted, needs native review
develop-curve = Tónová křivka
# machine-drafted, needs native review
develop-module-local-contrast = Místní kontrast
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Automatická expozice
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Vyhlazení
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Textura
# machine-drafted, needs native review
develop-clarity = Zřetelnost
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Černý bod
# machine-drafted, needs native review
develop-levels-mid = Střední bod
# machine-drafted, needs native review
develop-levels-white = Bílý bod
# machine-drafted, needs native review
develop-module-detail = Detaily
# machine-drafted, needs native review
develop-module-denoise = Redukce šumu
# machine-drafted, needs native review
develop-module-sharpen = Doostření
# machine-drafted, needs native review
develop-noise-luminance = Jasový šum
# machine-drafted, needs native review
develop-noise-detail = Detail šumu
# machine-drafted, needs native review
develop-noise-colour = Barevný šum
# machine-drafted, needs native review
develop-sharpen-amount = Síla doostření
# machine-drafted, needs native review
develop-sharpen-radius = Poloměr doostření
# machine-drafted, needs native review
develop-sharpen-detail = Detail doostření
# machine-drafted, needs native review
develop-sharpen-masking = Maskování
# machine-drafted, needs native review
develop-capture-amount = Vstupní doostření
# machine-drafted, needs native review
develop-capture-radius = Vstupní poloměr
# machine-drafted, needs native review
develop-straighten = Narovnat
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Oříznutí
# machine-drafted, needs native review
develop-crop-turn = Otočit
# machine-drafted, needs native review
develop-crop-flip = Převrátit
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Korekce objektivu
develop-module-optics = Optika
# machine-drafted, needs native review
develop-lens-distortion = Zkreslení
# machine-drafted, needs native review
develop-lens-vignette = Vinětace
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Červený lem
# machine-drafted, needs native review
develop-lens-tca-blue = Modrý lem
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Profil objektivu
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automaticky
# machine-drafted, needs native review
develop-lensfun-off = Žádný
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Žádný profil objektivu.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Pro { $lens } není profil Lensfun.
# machine-drafted, needs native review
develop-lensfun-no-profile = Pro tento objektiv není profil Lensfun.
# machine-drafted, needs native review
develop-lensfun-no-name = Soubor neuvádí název objektivu. Vyberte ho výše.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = Odpovídající objektivy: { $count }. Vyberte jeden výše.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Oříznutí. Táhněte okraje, nebo Ctrl+tažením podél čáry, která má být vodorovná. Enter ponechá, Esc vrátí zpět.
# machine-drafted, needs native review
develop-crop-kept = Oříznutí ponecháno
# machine-drafted, needs native review
develop-crop-cancelled = Oříznutí vráceno
# machine-drafted, needs native review
develop-crop-reset = Oříznutí a narovnání vynulováno
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Narovnáno na { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Poměr stran { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = volný
# machine-drafted, needs native review
develop-crop-aspect-original = původní
# machine-drafted, needs native review
develop-crop-turned-right = Otočeno doprava
# machine-drafted, needs native review
develop-crop-turned-left = Otočeno doleva
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Převráceno zleva doprava
# machine-drafted, needs native review
develop-crop-flipped-vertical = Převráceno shora dolů
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Poměr stran { $aspect } · Vodítka: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A poměr stran · X prohodit · O vodítka · [ ] otočit · H V převrátit · Enter ponechat · Esc zpět
# machine-drafted, needs native review
develop-crop-guide-thirds = třetiny
# machine-drafted, needs native review
develop-crop-guide-golden = zlatý řez
# machine-drafted, needs native review
develop-crop-guide-diagonal = úhlopříčky
# machine-drafted, needs native review
develop-crop-guide-grid = mřížka

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspektiva
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Vzpřímení
# machine-drafted, needs native review
develop-persp-mode-off = Vyp.
# machine-drafted, needs native review
develop-persp-mode-auto = Automaticky
# machine-drafted, needs native review
develop-persp-mode-level = Vodorovně
# machine-drafted, needs native review
develop-persp-mode-vertical = Svisle
# machine-drafted, needs native review
develop-persp-mode-full = Úplné
# machine-drafted, needs native review
develop-persp-mode-guided = S vodítky
# machine-drafted, needs native review
develop-persp-vertical = Svislá
# machine-drafted, needs native review
develop-persp-horizontal = Vodorovná
# machine-drafted, needs native review
develop-persp-rotate = Otočení
# machine-drafted, needs native review
develop-persp-aspect = Poměr stran
# machine-drafted, needs native review
develop-persp-scale = Měřítko
# machine-drafted, needs native review
develop-persp-offset-x = Do strany
# machine-drafted, needs native review
develop-persp-offset-y = Nahoru nebo dolů
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Bez prázdných okrajů
# machine-drafted, needs native review
develop-persp-guides = Vodítka
# machine-drafted, needs native review
develop-persp-focal = Ohnisková vzdálenost
# machine-drafted, needs native review
develop-persp-draw-guides = Kreslit vodítka
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Kreslit vodítka ({ $n } z { $max })
# machine-drafted, needs native review
develop-persp-reset = Vynulovat perspektivu
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Vzpřímení: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Hledání čar…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Vynulovat perspektivu? Nakreslená vodítka ({ $n }) budou odebrána. Zpět je vrátí.
# machine-drafted, needs native review
develop-persp-reset-ok = Vynulovat
# machine-drafted, needs native review
develop-persp-reset-done = Perspektiva vynulována
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Odebrat vodítko { $n }? Zpět ho vrátí.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Odebrat
# machine-drafted, needs native review
develop-persp-guide-removed = Vodítko odebráno
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Svislá { $vertical }, vodorovná { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Vzpřímení selhalo: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: náklon { $vertical }°, natočení { $horizontal }°, otočení { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } potřebuje aspoň dvě rovné čáry, které rozliší. Zkuste jiný režim, nebo nakreslete vodítka.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Nejvyšší počet vodítek: { $n }. Tažením konce jedno přesunete.
# machine-drafted, needs native review
develop-persp-guides-on = Vodítka. Táhněte podél čáry, která má být svislá nebo vodorovná.
# machine-drafted, needs native review
develop-persp-guides-off = Oříznutí
# machine-drafted, needs native review
develop-persp-guide-moved = Vodítko přesunuto
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Vodítko { $n } přidáno
# machine-drafted, needs native review
develop-persp-guides-keys = Tažením přidáte · Tažením konce přesunete · Delete odebere · Shift+R oříznutí
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Vodítka: { $n } z { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Stíny
# machine-drafted, needs native review
develop-wheel-midtones = Střední tóny
# machine-drafted, needs native review
develop-wheel-highlights = Světla
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Jas stínů
# machine-drafted, needs native review
develop-luminance-midtones = Jas středních tónů
# machine-drafted, needs native review
develop-luminance-highlights = Jas světel
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Celek
# machine-drafted, needs native review
develop-luminance-global = Jas celku
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Intenzita barev
# machine-drafted, needs native review
develop-brilliance = Jas barev

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Mixér barev
# machine-drafted, needs native review
develop-colour-red = Červená
# machine-drafted, needs native review
develop-colour-orange = Oranžová
# machine-drafted, needs native review
develop-colour-yellow = Žlutá
# machine-drafted, needs native review
develop-colour-green = Zelená
# machine-drafted, needs native review
develop-colour-aqua = Akvamarínová
# machine-drafted, needs native review
develop-colour-blue = Modrá
# machine-drafted, needs native review
develop-colour-purple = Fialová
# machine-drafted, needs native review
develop-colour-magenta = Purpurová
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Odstín – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Sytost – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Jas – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Bodová barva
# machine-drafted, needs native review
develop-point-pick = Vybrat barvu
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Vybrat jinou barvu
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Klikněte na barvu, kterou chcete změnit. Esc ruší.
# machine-drafted, needs native review
develop-point-picked = Barva vybrána
# machine-drafted, needs native review
develop-point-cancelled = Výběr zrušen
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Rozsah
# machine-drafted, needs native review
develop-point-hue-range = Rozsah odstínu
# machine-drafted, needs native review
develop-point-saturation-range = Rozsah sytosti
# machine-drafted, needs native review
develop-point-lightness-range = Rozsah světlosti
# machine-drafted, needs native review
develop-point-hue = Posun odstínu
# machine-drafted, needs native review
develop-point-saturation = Posun sytosti
# machine-drafted, needs native review
develop-point-lightness = Posun světlosti

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Černobílá
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Jas – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Vybrat soubor LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Soubor: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Nelze přečíst { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Vybrat soubor LUT
# machine-drafted, needs native review
develop-lut-kind = Soubory LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Určeno pro
# machine-drafted, needs native review
develop-lut-space-linear = Lineární (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES lineární (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Fotografie (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Video (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Metoda
# machine-drafted, needs native review
develop-lut-tetrahedral = Přesná
# machine-drafted, needs native review
develop-lut-trilinear = Jednoduchá
# machine-drafted, needs native review
develop-lut-strength = Síla

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Originál
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module }: zapnuto
# machine-drafted, needs native review
develop-history-off = { $module }: vypnuto
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Zpět na originál
# machine-drafted, needs native review
develop-at-step = Zpět na: { $step }
# machine-drafted, needs native review
develop-undone = Vráceno zpět: { $step }
# machine-drafted, needs native review
develop-redone = Znovu provedeno: { $step }
# machine-drafted, needs native review
develop-entered = Vyvolání. Úpravy nikdy nemění soubor.
# machine-drafted, needs native review
develop-left = Vyvolání opuštěno
# machine-drafted, needs native review
develop-split-on = Před a po vedle sebe
# machine-drafted, needs native review
develop-split-off = Jen úprava
# machine-drafted, needs native review
develop-showing-before = Zobrazuje se originál
# machine-drafted, needs native review
develop-showing-after = Zobrazuje se úprava
# machine-drafted, needs native review
# Labels over the image.
develop-before = Před
# machine-drafted, needs native review
develop-after = Po
# machine-drafted, needs native review
develop-working = Vyvolává se…
# machine-drafted, needs native review
develop-preview-failed = Úpravu tohoto souboru nelze zobrazit

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Vyvolání
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Rychlá úprava: podržet a posunout kolečkem
# machine-drafted, needs native review
palette-develop = Vyvolání
# machine-drafted, needs native review
palette-develop-help = Upravit tuto fotku beze změny souboru. Znovu pro odchod.
# machine-drafted, needs native review
palette-develop-before-after = Před a po
# machine-drafted, needs native review
palette-develop-before-after-help = Zobrazit originál, nebo znovu úpravu.
# machine-drafted, needs native review
palette-develop-split = Před a po rozděleně
# machine-drafted, needs native review
palette-develop-split-help = Zobrazit originál a úpravu vedle sebe.
# machine-drafted, needs native review
palette-develop-panel = Přejít na panel vyvolání
# machine-drafted, needs native review
palette-develop-panel-help = Přesunout klávesnici do panelu. Esc se vrátí.
# machine-drafted, needs native review
palette-develop-speed-exposure = Rychlá úprava expozice
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Podržet, pak posunout kolečkem nebo stisknout šipky a změnit expozici.
# machine-drafted, needs native review
palette-develop-speed-contrast = Rychlá úprava kontrastu
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Podržet, pak posunout kolečkem nebo stisknout šipky a změnit kontrast.
# machine-drafted, needs native review
palette-develop-speed-highlights = Rychlá úprava světel
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Podržet, pak posunout kolečkem nebo stisknout šipky a změnit světla.
# machine-drafted, needs native review
palette-develop-speed-shadows = Rychlá úprava stínů
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Podržet, pak posunout kolečkem nebo stisknout šipky a změnit stíny.
# machine-drafted, needs native review
palette-develop-speed-whites = Rychlá úprava bílých
# machine-drafted, needs native review
palette-develop-speed-whites-help = Podržet, pak posunout kolečkem nebo stisknout šipky a změnit bílé.
# machine-drafted, needs native review
palette-develop-speed-blacks = Rychlá úprava černých
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Podržet, pak posunout kolečkem nebo stisknout šipky a změnit černé.
# machine-drafted, needs native review
palette-develop-speed-temperature = Rychlá úprava teploty
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Podržet, pak posunout kolečkem nebo stisknout šipky a změnit teplotu vyvážení bílé.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Rychlá úprava živosti
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Podržet, pak posunout kolečkem nebo stisknout šipky a změnit živost.
# machine-drafted, needs native review
palette-develop-crop = Oříznout a narovnat
# machine-drafted, needs native review
palette-develop-crop-help = Oříznutí, narovnání, otočení nebo převrácení fotografie. Enter ponechá, Esc vrátí zpět.
# machine-drafted, needs native review
disabled-not-photo = Jen pro fotografie
# machine-drafted, needs native review
disabled-develop-off = Vyvolání je v profilech Medicína a Věda vypnuté
# machine-drafted, needs native review
develop-module-corrections = Korekce
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Vyp.
# machine-drafted, needs native review
develop-on = Zap.
# machine-drafted, needs native review
develop-highlight-method = Přepálená světla
# machine-drafted, needs native review
develop-highlight-opposed = Obnovit barvu
# machine-drafted, needs native review
develop-highlight-rebuild = Obnovit velké plochy
# machine-drafted, needs native review
develop-highlight-clip = Oříznout do bílé
# machine-drafted, needs native review
develop-highlight-threshold = Úroveň přepalu
# machine-drafted, needs native review
develop-module-hot-pixels = Horké pixely
# machine-drafted, needs native review
develop-hot-threshold = Práh horkých pixelů
# machine-drafted, needs native review
develop-module-ca = Chromatická vada
# machine-drafted, needs native review
develop-ca-mode = Barevné lemy
# machine-drafted, needs native review
develop-ca-auto = Automaticky
# machine-drafted, needs native review
develop-ca-manual = Ručně
# machine-drafted, needs native review
develop-ca-red = Posun červené
# machine-drafted, needs native review
develop-ca-blue = Posun modré
# machine-drafted, needs native review
develop-module-defringe = Odstranit lemy
# machine-drafted, needs native review
develop-defringe = Odstranit lemy
# machine-drafted, needs native review
develop-defringe-purple = Fialový odstín
# machine-drafted, needs native review
develop-defringe-green = Zelený odstín
# machine-drafted, needs native review
develop-defringe-range = Rozsah odstínu
# machine-drafted, needs native review
develop-defringe-edge = Kontrast hran
# machine-drafted, needs native review
develop-module-flat-field = Kalibrační snímky
# machine-drafted, needs native review
develop-calibration = Kalibrace
# machine-drafted, needs native review
develop-calibration-auto = Ze složky
# machine-drafted, needs native review
develop-calibration-chosen = Vybrané soubory
# machine-drafted, needs native review
develop-flat-choose = Vybrat flat snímek…
# machine-drafted, needs native review
develop-dark-choose = Vybrat dark snímek…
# machine-drafted, needs native review
develop-flat-file = Flat snímek: { $name }
# machine-drafted, needs native review
develop-dark-file = Dark snímek: { $name }
# machine-drafted, needs native review
develop-flat-use = Flat snímek
# machine-drafted, needs native review
develop-flat-use-full = Vinětace i prach
# machine-drafted, needs native review
develop-flat-use-vignetting = Jen vinětace
# machine-drafted, needs native review
develop-flat-dialog = Vybrat flat snímek
# machine-drafted, needs native review
develop-dark-dialog = Vybrat dark snímek
# machine-drafted, needs native review
develop-raw-kind = Soubory raw
# machine-drafted, needs native review
develop-flat-frame = Flat snímek
# machine-drafted, needs native review
develop-dark-frame = Dark snímek

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Přidat masku
# machine-drafted, needs native review
develop-mask-kind-ellipse = Elipsa
# machine-drafted, needs native review
develop-mask-kind-rectangle = Obdélník
# machine-drafted, needs native review
develop-mask-kind-polygon = Mnohoúhelník
# machine-drafted, needs native review
develop-mask-kind-linear = Lineární přechod
# machine-drafted, needs native review
develop-mask-kind-radial = Radiální přechod
# machine-drafted, needs native review
develop-mask-kind-brush = Štětec
# machine-drafted, needs native review
develop-mask-kind-luminance = Rozsah jasu
# machine-drafted, needs native review
develop-mask-kind-hue = Rozsah odstínu
# machine-drafted, needs native review
develop-mask-kind-chroma = Rozsah sytosti
# machine-drafted, needs native review
develop-mask-kind-colour = Rozsah barvy
# machine-drafted, needs native review
develop-mask-kind-other = Jiná maska
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Prolnutí
# machine-drafted, needs native review
develop-blend-normal = Normální
# machine-drafted, needs native review
develop-blend-multiply = Násobit
# machine-drafted, needs native review
develop-blend-screen = Závoj
# machine-drafted, needs native review
develop-blend-overlay = Překrýt
# machine-drafted, needs native review
develop-blend-soft-light = Měkké světlo
# machine-drafted, needs native review
develop-blend-lightness = Jen světlost
# machine-drafted, needs native review
develop-blend-colour = Jen barva
# machine-drafted, needs native review
develop-mask-amount = Síla masky
# machine-drafted, needs native review
develop-mask-invert = Obrátit masku
# machine-drafted, needs native review
develop-mask-off = Vyp.
# machine-drafted, needs native review
develop-mask-on = Zap.
# machine-drafted, needs native review
develop-mask-refine = Zpřesnit okraje
# machine-drafted, needs native review
develop-mask-show = Zobrazit masku
# machine-drafted, needs native review
develop-mask-show-red = Červená
# machine-drafted, needs native review
develop-mask-show-grey = Šedá
# machine-drafted, needs native review
develop-mask-remove = Odebrat masku
# machine-drafted, needs native review
develop-mask-combine-add = Přidat
# machine-drafted, needs native review
develop-mask-combine-subtract = Odečíst
# machine-drafted, needs native review
develop-mask-combine-intersect = Průnik
# machine-drafted, needs native review
develop-mask-row-opacity = Krytí
# machine-drafted, needs native review
develop-mask-row-invert = Obrátit
# machine-drafted, needs native review
develop-mask-feather = Prolnutí okraje
# machine-drafted, needs native review
develop-mask-roundness = Zaoblení
# machine-drafted, needs native review
develop-mask-brush-size = Velikost štětce
# machine-drafted, needs native review
develop-mask-brush-feather = Měkkost štětce
# machine-drafted, needs native review
develop-mask-brush-flow = Tok
# machine-drafted, needs native review
develop-mask-brush-mode = Štětec
# machine-drafted, needs native review
develop-mask-brush-paint = Malovat
# machine-drafted, needs native review
develop-mask-brush-erase = Gumovat
# machine-drafted, needs native review
develop-mask-range-from = Od
# machine-drafted, needs native review
develop-mask-range-full-from = Plně od
# machine-drafted, needs native review
develop-mask-range-full-to = Plně do
# machine-drafted, needs native review
develop-mask-range-to = Do
# machine-drafted, needs native review
develop-mask-range-test = Testovat
# machine-drafted, needs native review
develop-mask-range-input = Vstup
# machine-drafted, needs native review
develop-mask-range-output = Výstup
# machine-drafted, needs native review
develop-mask-colour-pick = Vybrat barvu
# machine-drafted, needs native review
develop-mask-colour-pick-more = Vybrat další barvu
# machine-drafted, needs native review
develop-mask-colour-range = Rozsah
# machine-drafted, needs native review
develop-mask-colour-falloff = Útlum
# machine-drafted, needs native review
develop-mask-edit = Upravit na snímku
# machine-drafted, needs native review
develop-mask-row-remove = Odebrat { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Odebrat masku modulu { $module }? Její tvary zmizí. Fotografie na disku zůstane.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Odebrat { $name } z modulu { $module }? Ostatní masky a fotografie na disku zůstanou.
# machine-drafted, needs native review
develop-mask-remove-ok = Odebrat
# machine-drafted, needs native review
develop-mask-removed = Maska odebrána.
# machine-drafted, needs native review
develop-mask-added = Přidáno: { $name }.
# machine-drafted, needs native review
develop-mask-full = Tato maska už má tolik částí, kolik pojme.
# machine-drafted, needs native review
develop-mask-editing = Úprava: { $name }. Táhněte úchyty, nebo použijte Tab a šipky.
# machine-drafted, needs native review
develop-mask-painting = Malování: { $name }. Táhněte po snímku.
# machine-drafted, needs native review
develop-mask-picking = Klepněte na snímek a vyberte barvu.
# machine-drafted, needs native review
develop-mask-picked = Barva vybrána.
# machine-drafted, needs native review
develop-mask-done = Úprava masky skončena.
# machine-drafted, needs native review
develop-mask-handles = Úchyty masky
# machine-drafted, needs native review
develop-handle-centre = Střed
# machine-drafted, needs native review
develop-handle-width = Šířka
# machine-drafted, needs native review
develop-handle-height = Výška
# machine-drafted, needs native review
develop-handle-rotate = Otočit
# machine-drafted, needs native review
develop-handle-feather = Prolnutí okraje
# machine-drafted, needs native review
develop-handle-inner = Vnitřní okraj
# machine-drafted, needs native review
develop-handle-start = Začátek
# machine-drafted, needs native review
develop-handle-end = Konec
# machine-drafted, needs native review
develop-handle-point = Bod { $n }
# machine-drafted, needs native review
develop-instance = Vrstva
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Vrstva { $n }.
# machine-drafted, needs native review
develop-instance-new = Nová vrstva
# machine-drafted, needs native review
develop-instance-duplicate = Duplikovat vrstvu
# machine-drafted, needs native review
develop-instance-delete = Smazat vrstvu
# machine-drafted, needs native review
develop-instance-earlier = Posunout dřív
# machine-drafted, needs native review
develop-instance-later = Posunout později
# machine-drafted, needs native review
develop-instance-delete-confirm = Smazat vrstvu { $n } modulu { $module }? Její nastavení a masky zmizí. Fotografie na disku zůstane.
# machine-drafted, needs native review
develop-instance-delete-ok = Smazat
# machine-drafted, needs native review
develop-instance-deleted = Vrstva smazána.
# machine-drafted, needs native review
develop-history-mask = Maska: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Tvary masky: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Nový: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Smazáno: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Pořadí: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Jiné programy
# machine-drafted, needs native review
develop-darktable-edited = Upraveno v darktable:
# machine-drafted, needs native review
develop-darktable-open = Otevřít v darktable
# machine-drafted, needs native review
develop-darktable-opening = Otevírá se v darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable se nespustil: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importovat z Lightroomu
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] Importována nastavení Lightroomu: 1. Výsledek je přibližný.
    [few] Importována nastavení Lightroomu: { $count }. Výsledek je přibližný.
    [many] Importována nastavení Lightroomu: { $count }. Výsledek je přibližný.
   *[other] Importována nastavení Lightroomu: { $count }. Výsledek je přibližný.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Nic k importu: tato nastavení zde nic nemění.
# machine-drafted, needs native review
develop-history-lightroom = Importováno z Lightroomu (přibližně)
# machine-drafted, needs native review
develop-conflict-title = Úprava souboru { $file } se změnila mimo Specular
# machine-drafted, needs native review
develop-conflict-file = Doprovodný soubor: uložen { $time }
# machine-drafted, needs native review
develop-conflict-library = Knihovna: uložena { $time }
# machine-drafted, needs native review
develop-conflict-writer = Doprovodný soubor zapsal { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Ponechte jednu. Druhá se nahradí.
# machine-drafted, needs native review
develop-conflict-use-file = Použít ze souboru
# machine-drafted, needs native review
develop-conflict-keep-library = Ponechat z knihovny
# machine-drafted, needs native review
develop-conflict-used-file = Použita úprava z doprovodného souboru
# machine-drafted, needs native review
develop-conflict-kept-library = Ponechána úprava z knihovny
# machine-drafted, needs native review
toast-develop-edit-newer-process = Úprava vznikla v novějším Specularu. Zde může vypadat jinak.
# machine-drafted, needs native review
info-darktable-edited = Upraveno v darktable
# machine-drafted, needs native review
info-darktable-no-modules = Žádné zapnuté moduly
# machine-drafted, needs native review
info-darktable-group = Soubor
# machine-drafted, needs native review
toast-develop-edit-partial = Úprava vznikla v novějším Specularu. Části, které tato verze neumí zobrazit, jsou vynechány. Změny zde se neukládají.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Soubor úprav vedle fotky nelze přečíst. Zůstává beze změny. Změny zde se neukládají.
# machine-drafted, needs native review
develop-demosaic = Detaily raw
develop-demosaic-directional = Standardní
develop-demosaic-dual = Hladké plochy
develop-demosaic-fast = Rychlé
develop-dual-threshold = Kontrast ploch
