# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Vergelijken met…
palette-compare-with-help = Deze afbeelding vergelijken met een andere die u kiest.
palette-compare-side = Naast elkaar vergelijken
palette-compare-side-help = De vergeleken afbeeldingen naast elkaar tonen.
palette-compare-split = Vergelijken met een scheidingslijn
palette-compare-split-help = De tweede afbeelding over de eerste tonen, gesneden bij een scheidingslijn die u kunt slepen.
palette-compare-split-direction = Scheidingslijn draaien
palette-compare-split-direction-help = De scheidingslijn horizontaal in plaats van verticaal leggen, of terug.
palette-compare-overlay = Vergelijken als overlay
palette-compare-overlay-help = De tweede afbeelding over de eerste tonen, overvloeiend.
palette-compare-difference = Verschil vergelijken
palette-compare-difference-help = Tonen waar de afbeeldingen verschillen; nogmaals voor de volgende soort.
palette-compare-flicker = Knipperen tussen vergeleken afbeeldingen
palette-compare-flicker-help = Meteen de andere afbeelding tonen.
palette-compare-auto-flicker = Automatisch knipperen
palette-compare-auto-flicker-help = Vanzelf tussen de twee afbeeldingen wisselen, of stoppen.
palette-compare-align = Vergeleken afbeeldingen uitlijnen
palette-compare-align-help = Afbeeldingen van verschillende grootte uitlijnen: linksboven, gecentreerd, geschaald of automatisch gevonden.
palette-compare-metric-space = Meetruimte wisselen
palette-compare-metric-space-help = Meten in lineair licht of in gecodeerd sRGB.
palette-compare-whole-image = Hele afbeelding meten
palette-compare-whole-image-help = De hele afbeelding meten in plaats van wat in beeld is, of terug.
palette-compare-probe = Pixelsonde bij vergelijken
palette-compare-probe-help = De waarden van beide afbeeldingen onder de aanwijzer tonen.
palette-compare-link = Vergeleken afbeeldingen koppelen of ontkoppelen
palette-compare-link-help = Elk van twee afbeeldingen apart zoomen en verschuiven, of weer samen.
palette-compare-close = Vergelijken sluiten
palette-compare-close-help = Terug naar de galerij.
disabled-compare-closed = Open eerst vergelijken
disabled-compare-not-two = Werkt met twee afbeeldingen

## Cheat sheet
cheatsheet-compare-split = Scheidingslijn of naast elkaar
cheatsheet-compare-split-direction = Scheidingslijn horizontaal of verticaal
cheatsheet-compare-auto-flicker = Automatisch knipperen
cheatsheet-compare-align = Volgende uitlijning
cheatsheet-compare-space = Meetruimte
cheatsheet-compare-whole = Hele afbeelding of in beeld
cheatsheet-compare-probe = Pixelsonde
cheatsheet-compare-link = Twee afbeeldingen koppelen of ontkoppelen

## Settings
settings-gallery-compare-metric-space = Meetruimte bij vergelijken
settings-gallery-compare-metric-space-help = Waarin vergelijken MAE, MSE, PSNR en SSIM meet. Lineair licht is wat het scherm toont. sRGB komt overeen met andere programma’s bij 8-bit-bestanden.
settings-gallery-compare-metric-space-linear = Lineair licht
settings-gallery-compare-metric-space-srgb = Gecodeerd sRGB
settings-gallery-compare-flicker-limit = Snelste automatisch knipperen
settings-gallery-compare-flicker-limit-help = Hoogstens zoveel beeldwisselingen per seconde. Boven 3 kan knipperen aanvallen uitlokken.

## Toolbar and captions
compare-split = Scheidingslijn
compare-split-down = Scheidingslijn verticaal
compare-split-across = Scheidingslijn horizontaal
compare-link = Gekoppeld
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } wisselingen per seconde
compare-gain = Versterking ×{ $gain }
compare-probe = Sonde
compare-kind-signed = Fout met teken
compare-kind-on-image = Fout over afbeelding
compare-align-top-left = Linksboven
compare-align-centre = Gecentreerd
compare-align-scaled = B geschaald naar A
compare-align-auto = Automatisch uitlijnen
compare-space-linear = Lineair licht
compare-space-srgb = sRGB
compare-space-help-linear = Gemeten in lineair licht, zoals weergegeven na tone mapping. PSNR is hier niet de gebruikelijke 8-bit-sRGB-PSNR.
compare-space-help-srgb = Gemeten op gecodeerde sRGB-waarden, zoals andere programma’s dat bij 8-bit-bestanden doen. HDR boven SDR-wit telt als wit.
compare-space-help-common = ΔE2000 is altijd CIELAB (D65), met SDR-wit als L* 100 en HDR afgekapt. Afbeeldingen groter dan { $size } pixels worden gemeten op een kopie van { $size } pixels.
compare-region-view = In beeld
compare-region-whole = Hele afbeelding
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM heeft 11 pixels nodig
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, max. { $max })
compare-delta-e-hdr = HDR afgekapt voor ΔE
compare-aligned-centre = Gecentreerd, { $width } × { $height } vergeleken
compare-aligned-scaled = B { $scale }× geschaald, { $width } × { $height } vergeleken
compare-aligned-auto = Uitgelijnd { $angle }° { $scale }×, { $width } × { $height } vergeleken
compare-aligning = Uitlijnen…
compare-align-failed = Kan deze afbeeldingen niet uitlijnen. Terug naar linksboven.
compare-split-label = { $b } over { $a }, scheidingslijn op { PERCENT($percent) }
compare-showing-which = Toont { $which }: { $name }
compare-auto-flicker-on = Knippert, { $rate } wisselingen per seconde
compare-auto-flicker-off = Knipperen gestopt
compare-linked = Afbeeldingen gekoppeld
compare-unlinked = Afbeeldingen ontkoppeld: elk verschuift apart
compare-with-title = Vergelijken met
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Scheidingslijn
compare-a11y-space = Meetruimte
compare-a11y-align = Uitlijning
