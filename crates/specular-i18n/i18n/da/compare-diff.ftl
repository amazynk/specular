# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Sammenlign med…
palette-compare-with-help = Sammenlign dette billede med et andet, du vælger.
palette-compare-side = Sammenlign side om side
palette-compare-side-help = Vis de sammenlignede billeder ved siden af hinanden.
palette-compare-split = Sammenlign med deling
palette-compare-split-help = Vis det andet billede over det første, delt ved en skillelinje, du kan trække.
palette-compare-split-direction = Drej skillelinjen
palette-compare-split-direction-help = Læg skillelinjen vandret i stedet for lodret, eller tilbage.
palette-compare-overlay = Sammenlign som overlejring
palette-compare-overlay-help = Vis det andet billede over det første, blandet.
palette-compare-difference = Sammenlign forskel
palette-compare-difference-help = Vis, hvor billederne er forskellige; igen for næste slags.
palette-compare-flicker = Blink mellem sammenlignede billeder
palette-compare-flicker-help = Vis det andet billede med det samme.
palette-compare-auto-flicker = Blink automatisk
palette-compare-auto-flicker-help = Skift automatisk mellem de to billeder, eller stop.
palette-compare-align = Justér sammenlignede billeder
palette-compare-align-help = Ret billeder af forskellig størrelse ind: øverst til venstre, centreret, skaleret eller fundet automatisk.
palette-compare-metric-space = Skift målerum
palette-compare-metric-space-help = Mål i lineært lys eller i kodet sRGB.
palette-compare-whole-image = Mål hele billedet
palette-compare-whole-image-help = Mål hele billedet i stedet for det, der er i visningen, eller tilbage.
palette-compare-probe = Pixelsonde ved sammenligning
palette-compare-probe-help = Vis begge billeders værdier under markøren.
palette-compare-link = Kobl sammenlignede billeder til eller fra
palette-compare-link-help = Zoom og panorér hvert af to billeder for sig, eller sammen igen.
palette-compare-close = Luk sammenligning
palette-compare-close-help = Gå tilbage til galleriet.
disabled-compare-closed = Åbn først sammenligning
disabled-compare-not-two = Virker med to billeder

## Cheat sheet
cheatsheet-compare-split = Deling eller side om side
cheatsheet-compare-split-direction = Skillelinje vandret eller lodret
cheatsheet-compare-auto-flicker = Blink automatisk
cheatsheet-compare-align = Næste justering
cheatsheet-compare-space = Målerum
cheatsheet-compare-whole = Hele billedet eller i visningen
cheatsheet-compare-probe = Pixelsonde
cheatsheet-compare-link = Kobl to billeder til eller fra

## Settings
settings-gallery-compare-metric-space = Målerum for sammenligning
settings-gallery-compare-metric-space-help = Hvor sammenligning måler MAE, MSE, PSNR og SSIM. Lineært lys er det, skærmen viser. sRGB stemmer med andre værktøjer for 8-bit-filer.
settings-gallery-compare-metric-space-linear = Lineært lys
settings-gallery-compare-metric-space-srgb = Kodet sRGB
settings-gallery-compare-flicker-limit = Hurtigste automatiske blink
settings-gallery-compare-flicker-limit-help = Højst så mange billedskift i sekundet. Over 3 kan blink udløse anfald.

## Toolbar and captions
compare-split = Deling
compare-split-down = Skillelinje lodret
compare-split-across = Skillelinje vandret
compare-link = Koblet
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } skift i sekundet
compare-gain = Forstærkning ×{ $gain }
compare-probe = Sonde
compare-kind-signed = Fejl med fortegn
compare-kind-on-image = Fejl over billedet
compare-align-top-left = Øverst til venstre
compare-align-centre = Centreret
compare-align-scaled = B skaleret til A
compare-align-auto = Automatisk justering
compare-space-linear = Lineært lys
compare-space-srgb = sRGB
compare-space-help-linear = Målt i lineært lys, som vist efter tonemapping. PSNR er her ikke den sædvanlige 8-bit-sRGB-PSNR.
compare-space-help-srgb = Målt på kodede sRGB-værdier, som andre værktøjer gør for 8-bit-filer. HDR over SDR-hvid tæller som hvid.
compare-space-help-common = ΔE2000 er altid CIELAB (D65), med SDR-hvid som L* 100 og HDR klippet. Billeder større end { $size } pixel måles på en kopi på { $size } pixel.
compare-region-view = I visningen
compare-region-whole = Hele billedet
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM kræver 11 pixel
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, maks. { $max })
compare-delta-e-hdr = HDR klippet for ΔE
compare-aligned-centre = Centreret, { $width } × { $height } sammenlignet
compare-aligned-scaled = B skaleret { $scale }×, { $width } × { $height } sammenlignet
compare-aligned-auto = Justeret { $angle }° { $scale }×, { $width } × { $height } sammenlignet
compare-aligning = Justerer…
compare-align-failed = Kunne ikke justere disse billeder. Tilbage til øverst til venstre.
compare-split-label = { $b } over { $a }, skillelinje ved { PERCENT($percent) }
compare-showing-which = Viser { $which }: { $name }
compare-auto-flicker-on = Blinker, { $rate } skift i sekundet
compare-auto-flicker-off = Blink stoppet
compare-linked = Billeder koblet
compare-unlinked = Billeder frakoblet: hvert panoreres for sig
compare-with-title = Sammenlign med
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Skillelinje
compare-a11y-space = Målerum
compare-a11y-align = Justering
