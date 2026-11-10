# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Jämför med…
palette-compare-with-help = Jämför den här bilden med en annan som du väljer.
palette-compare-side = Jämför sida vid sida
palette-compare-side-help = Visa de jämförda bilderna bredvid varandra.
palette-compare-split = Jämför med delning
palette-compare-split-help = Visa den andra bilden över den första, delad vid en avdelare som du kan dra.
palette-compare-split-direction = Vrid avdelaren
palette-compare-split-direction-help = Lägg avdelaren vågrätt i stället för lodrätt, eller tillbaka.
palette-compare-overlay = Jämför som överlagring
palette-compare-overlay-help = Visa den andra bilden över den första, blandad.
palette-compare-difference = Jämför skillnad
palette-compare-difference-help = Visa var bilderna skiljer sig; igen för nästa typ.
palette-compare-flicker = Blinka mellan jämförda bilder
palette-compare-flicker-help = Visa den andra bilden direkt.
palette-compare-auto-flicker = Blinka automatiskt
palette-compare-auto-flicker-help = Växla mellan de två bilderna av sig själv, eller stoppa.
palette-compare-align = Justera jämförda bilder
palette-compare-align-help = Passa ihop bilder av olika storlek: uppe till vänster, centrerat, skalat eller hittat automatiskt.
palette-compare-metric-space = Byt mätrymd
palette-compare-metric-space-help = Mät i linjärt ljus eller i kodad sRGB.
palette-compare-whole-image = Mät hela bilden
palette-compare-whole-image-help = Mät hela bilden i stället för det som syns, eller tillbaka.
palette-compare-probe = Pixelsond vid jämförelse
palette-compare-probe-help = Visa båda bildernas värden under pekaren.
palette-compare-link = Länka eller avlänka jämförda bilder
palette-compare-link-help = Zooma och panorera var och en av två bilder för sig, eller tillsammans igen.
palette-compare-close = Stäng jämförelsen
palette-compare-close-help = Gå tillbaka till galleriet.
disabled-compare-closed = Öppna jämförelsen först
disabled-compare-not-two = Fungerar med två bilder

## Cheat sheet
cheatsheet-compare-split = Delning eller sida vid sida
cheatsheet-compare-split-direction = Avdelare vågrätt eller lodrätt
cheatsheet-compare-auto-flicker = Blinka automatiskt
cheatsheet-compare-align = Nästa justering
cheatsheet-compare-space = Mätrymd
cheatsheet-compare-whole = Hela bilden eller det som syns
cheatsheet-compare-probe = Pixelsond
cheatsheet-compare-link = Länka eller avlänka två bilder

## Settings
settings-gallery-compare-metric-space = Mätrymd vid jämförelse
settings-gallery-compare-metric-space-help = Var jämförelsen mäter MAE, MSE, PSNR och SSIM. Linjärt ljus är det skärmen visar. sRGB stämmer med andra verktyg för 8-bit-filer.
settings-gallery-compare-metric-space-linear = Linjärt ljus
settings-gallery-compare-metric-space-srgb = Kodad sRGB
settings-gallery-compare-flicker-limit = Snabbaste automatiska blinkning
settings-gallery-compare-flicker-limit-help = Högst så många bildbyten per sekund. Över 3 kan blinkande utlösa anfall.

## Toolbar and captions
compare-split = Delning
compare-split-down = Avdelare lodrätt
compare-split-across = Avdelare vågrätt
compare-link = Länkade
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } byten per sekund
compare-gain = Förstärkning ×{ $gain }
compare-probe = Sond
compare-kind-signed = Fel med tecken
compare-kind-on-image = Fel över bilden
compare-align-top-left = Uppe till vänster
compare-align-centre = Centrerat
compare-align-scaled = B skalad till A
compare-align-auto = Automatisk justering
compare-space-linear = Linjärt ljus
compare-space-srgb = sRGB
compare-space-help-linear = Mätt i linjärt ljus, som det visas efter tonmappning. PSNR här är inte det vanliga 8-bit-sRGB-PSNR.
compare-space-help-srgb = Mätt på kodade sRGB-värden, som andra verktyg gör för 8-bit-filer. HDR över SDR-vitt räknas som vitt.
compare-space-help-common = ΔE2000 är alltid CIELAB (D65), med SDR-vitt som L* 100 och HDR klippt. Bilder större än { $size } pixlar mäts på en kopia med { $size } pixlar.
compare-region-view = Det som syns
compare-region-whole = Hela bilden
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM behöver 11 pixlar
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, max { $max })
compare-delta-e-hdr = HDR klippt för ΔE
compare-aligned-centre = Centrerat, { $width } × { $height } jämfört
compare-aligned-scaled = B skalad { $scale }×, { $width } × { $height } jämfört
compare-aligned-auto = Justerat { $angle }° { $scale }×, { $width } × { $height } jämfört
compare-aligning = Justerar…
compare-align-failed = Det gick inte att justera de här bilderna. Tillbaka till uppe till vänster.
compare-split-label = { $b } över { $a }, avdelare vid { PERCENT($percent) }
compare-showing-which = Visar { $which }: { $name }
compare-auto-flicker-on = Blinkar, { $rate } byten per sekund
compare-auto-flicker-off = Blinkning stoppad
compare-linked = Bilderna länkade
compare-unlinked = Bilderna avlänkade: var och en panoreras för sig
compare-with-title = Jämför med
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Avdelare
compare-a11y-space = Mätrymd
compare-a11y-align = Justering
