# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Porovnat s…
palette-compare-with-help = Porovnat tento obrázek s jiným, který vyberete.
palette-compare-side = Porovnat vedle sebe
palette-compare-side-help = Zobrazit porovnávané obrázky vedle sebe.
palette-compare-split = Porovnat s dělicí čarou
palette-compare-split-help = Zobrazit druhý obrázek přes první, oříznutý u dělicí čáry, kterou lze táhnout.
palette-compare-split-direction = Otočit dělicí čáru
palette-compare-split-direction-help = Dát dělicí čáru vodorovně místo svisle, nebo zpět.
palette-compare-overlay = Porovnat jako překryv
palette-compare-overlay-help = Zobrazit druhý obrázek přes první, prolnutý.
palette-compare-difference = Porovnat rozdíl
palette-compare-difference-help = Ukázat, kde se obrázky liší; znovu pro další druh.
palette-compare-flicker = Blikat mezi porovnávanými obrázky
palette-compare-flicker-help = Ihned zobrazit druhý obrázek.
palette-compare-auto-flicker = Blikat automaticky
palette-compare-auto-flicker-help = Samočinně přepínat mezi oběma obrázky, nebo zastavit.
palette-compare-align = Zarovnat porovnávané obrázky
palette-compare-align-help = Zarovnat obrázky různých velikostí: vlevo nahoře, na střed, se změnou měřítka nebo automaticky.
palette-compare-metric-space = Přepnout prostor měření
palette-compare-metric-space-help = Měřit v lineárním světle nebo v kódovaném sRGB.
palette-compare-whole-image = Měřit celý obrázek
palette-compare-whole-image-help = Měřit celý obrázek místo viditelné části, nebo zpět.
palette-compare-probe = Pixelová sonda porovnání
palette-compare-probe-help = Zobrazit hodnoty obou obrázků pod ukazatelem.
palette-compare-link = Propojit nebo odpojit porovnávané obrázky
palette-compare-link-help = Přibližovat a posouvat každý ze dvou obrázků zvlášť, nebo znovu společně.
palette-compare-close = Zavřít porovnání
palette-compare-close-help = Vrátit se do galerie.
disabled-compare-closed = Nejprve otevřete porovnání
disabled-compare-not-two = Funguje se dvěma obrázky

## Cheat sheet
cheatsheet-compare-split = Dělicí čára nebo vedle sebe
cheatsheet-compare-split-direction = Dělicí čára vodorovně nebo svisle
cheatsheet-compare-auto-flicker = Blikat automaticky
cheatsheet-compare-align = Další zarovnání
cheatsheet-compare-space = Prostor měření
cheatsheet-compare-whole = Celý obrázek nebo viditelná část
cheatsheet-compare-probe = Pixelová sonda
cheatsheet-compare-link = Propojit nebo odpojit dva obrázky

## Settings
settings-gallery-compare-metric-space = Prostor měření při porovnání
settings-gallery-compare-metric-space-help = Kde porovnání měří MAE, MSE, PSNR a SSIM. Lineární světlo odpovídá zobrazení. sRGB se shoduje s jinými nástroji u 8-bit souborů.
settings-gallery-compare-metric-space-linear = Lineární světlo
settings-gallery-compare-metric-space-srgb = Kódované sRGB
settings-gallery-compare-flicker-limit = Nejrychlejší automatické blikání
settings-gallery-compare-flicker-limit-help = Nejvýše tolik změn obrázku za sekundu. Nad 3 může blikání vyvolat záchvaty.

## Toolbar and captions
compare-split = Dělení
compare-split-down = Dělicí čára svisle
compare-split-across = Dělicí čára vodorovně
compare-link = Propojeno
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } změn za sekundu
compare-gain = Zesílení ×{ $gain }
compare-probe = Sonda
compare-kind-signed = Chyba se znaménkem
compare-kind-on-image = Chyba přes obrázek
compare-align-top-left = Vlevo nahoře
compare-align-centre = Na střed
compare-align-scaled = B v měřítku A
compare-align-auto = Automatické zarovnání
compare-space-linear = Lineární světlo
compare-space-srgb = sRGB
compare-space-help-linear = Měřeno v lineárním světle, jak se zobrazuje po mapování tónů. PSNR zde není obvyklé 8-bit sRGB PSNR.
compare-space-help-srgb = Měřeno na kódovaných hodnotách sRGB, jako to dělají jiné nástroje u 8-bit souborů. HDR nad bílou SDR se počítá jako bílá.
compare-space-help-common = ΔE2000 je vždy v CIELAB (D65), bílá SDR jako L* 100, HDR oříznuté. Obrázky větší než { $size } pixelů se měří na kopii o { $size } pixelech.
compare-region-view = Viditelná část
compare-region-whole = Celý obrázek
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM potřebuje 11 pixelů
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, max. { $max })
compare-delta-e-hdr = HDR oříznuto pro ΔE
compare-aligned-centre = Na střed, porovnáno { $width } × { $height }
compare-aligned-scaled = B v měřítku { $scale }×, porovnáno { $width } × { $height }
compare-aligned-auto = Zarovnáno { $angle }° { $scale }×, porovnáno { $width } × { $height }
compare-aligning = Zarovnávání…
compare-align-failed = Tyto obrázky nelze zarovnat. Zpět na vlevo nahoře.
compare-split-label = { $b } přes { $a }, dělicí čára na { PERCENT($percent) }
compare-showing-which = Zobrazeno { $which }: { $name }
compare-auto-flicker-on = Bliká, { $rate } změn za sekundu
compare-auto-flicker-off = Blikání zastaveno
compare-linked = Obrázky propojeny
compare-unlinked = Obrázky odpojeny: každý se posouvá zvlášť
compare-with-title = Porovnat s
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Dělicí čára
compare-a11y-space = Prostor měření
compare-a11y-align = Zarovnání
