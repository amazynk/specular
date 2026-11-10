# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Konvertera…

## Settings → Convert
settings-section-convert = Konvertering
settings-convert-format = Format
settings-convert-format-help = Formatet som Konvertera… skriver. PNG och TIFF är förlustfria; WebP och JPEG XL kan vara båda.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = JPEG-kvalitet
settings-convert-jpeg-quality-help = Högre behåller fler detaljer i större filer. Vid 90 är det svårt att se skillnad mot originalet.
settings-convert-jpeg-full-chroma = Färg i full upplösning i JPEG
settings-convert-jpeg-full-chroma-help = Behåller färgen i full upplösning (4:4:4) i stället för halv (4:2:0): skarpare färgkanter, större filer.
settings-convert-png-effort = PNG-komprimering
settings-convert-png-effort-help = Högre ger mindre filer, men långsammare. 1 är snabbast.
settings-convert-jxl-effort = JPEG XL-ansträngning
settings-convert-jxl-effort-help = Högre ger mindre filer, men långsammare.
settings-convert-tiff-compression = TIFF-komprimering
settings-convert-tiff-compression-help = Deflate är förlustfritt och kan läsas av alla TIFF-läsare.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Ingen
settings-convert-depth = Bitdjup
settings-convert-depth-help = Automatiskt skriver 16 bitar per sampel när bilden har fler än 8 och formatet kan lagra dem (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automatiskt
settings-convert-depth-8 = 8 bitar
settings-convert-depth-16 = 16 bitar
settings-convert-resize = Ändra storlek
settings-convert-resize-help = Bilder skalas ned i linjärt ljus med ett filter av hög kvalitet.
settings-convert-resize-none = Originalstorlek
settings-convert-resize-fit = Anpassa inom
settings-convert-resize-long-edge = Långsida
settings-convert-resize-percent = Procent
settings-convert-width = Bredd
settings-convert-width-help = Den största bredd en bild får ha med Anpassa inom.
settings-convert-height = Höjd
settings-convert-height-help = Den största höjd en bild får ha med Anpassa inom.
settings-convert-long-edge = Långsida
settings-convert-long-edge-help = Längden på bildens längsta sida med Långsida.
settings-convert-percent = Procent
settings-convert-percent-help = Storleken i förhållande till originalet med Procent.
settings-convert-upscale = Förstora små bilder
settings-convert-upscale-help = Av: bilder som redan är mindre än den begärda storleken behåller sin storlek.
settings-convert-colour = Färg
settings-convert-colour-help = Behåll sparar bildens färgprofil tillsammans med den. Konvertera till sRGB passar webben och enheter utan färghantering.
settings-convert-colour-keep = Behåll profilen
settings-convert-colour-srgb = Konvertera till sRGB
settings-convert-hdr = HDR-bilder
settings-convert-hdr-help = PNG och JPEG XL behåller HDR som det är; andra format tonmappas alltid till SDR.
settings-convert-hdr-keep = Behåll där det går
settings-convert-hdr-tone-map = Tonmappa till SDR
settings-convert-metadata = Metadata
settings-convert-metadata-help = Exif och XMP, till exempel kameran, fotograferingsdatumet och platsen. Färgprofilen behålls alltid.
settings-convert-metadata-all = Behåll allt
settings-convert-metadata-no-location = Behåll allt utom plats
settings-convert-metadata-strip = Ta bort
settings-convert-folder = Utdatamapp
settings-convert-folder-help = Var konverterade filer hamnar. Om den inte är vald hamnar de i en undermapp bredvid varje bild.
settings-convert-folder-unset = Undermapp bredvid bilderna
settings-convert-subfolder = Undermappens namn
settings-convert-subfolder-help = Undermappen som används när ingen utdatamapp är vald. Tom: bildernas egen mapp.
settings-convert-pattern = Filnamn
settings-convert-pattern-help = {"{"}name{"}"} är originalnamnet, {"{"}n:03{"}"} numret i omgången, {"{"}date:%Y-%m-%d{"}"} fotograferingsdatumet, {"{"}w{"}"} och {"{"}h{"}"} den nya storleken, {"{"}ext{"}"} formatets filändelse.
settings-convert-conflicts = När ett namn är upptaget
settings-convert-conflicts-help = Befintliga filer ersätts aldrig.
settings-convert-conflicts-suffix = Lägg till ett nummer
settings-convert-conflicts-skip = Hoppa över bilden

## The dialog
convert-title = Konvertera
convert-images = Bilder
convert-images-current = Den här bilden
convert-images-selection = Markering ({ $count })
convert-images-folder = Mapp ({ $count })
convert-images-picked = Val ({ $count })
convert-quality = Kvalitet
convert-chroma = Färgdetaljer
convert-chroma-half = 4:2:0 (mindre)
convert-chroma-full = 4:4:4 (skarpare)
convert-effort = Ansträngning
convert-on = På
convert-off = Av
convert-pixels = { $value } px
convert-effort-fastest = { $value } (snabbast)
convert-folder-subfolder = Undermappen ”{ $name }”
convert-folder-same = Samma mapp som bilden
convert-choose-folder = Välj mapp…
convert-name = Namn
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = En {"{"} stängs inte.
convert-name-error-stray = En {"}"} saknar {"{"}. Skriv {"}}"} för en klammerparentes.
convert-name-error-unknown = { $token } är inte en namndel. Använd name, n, date, w, h eller ext.
convert-name-error-format = { $token } har ett ogiltigt format.
convert-name-error-empty = Ange ett namnmönster.
convert-start = Konvertera
convert-stop = Stoppa
convert-hide = Dölj
convert-close = Stäng
convert-progress = Konverterar { $done } av { $total }
convert-progress-eta = Konverterar { $done } av { $total } · ungefär { $time } kvar
convert-eta-seconds = { $count ->
    [one] { $count } sekund
   *[other] { $count } sekunder
}
convert-eta-minutes = { $count ->
    [one] { $count } minut
   *[other] { $count } minuter
}
convert-done = { $count ->
    [one] { $count } bild konverterad
   *[other] { $count } bilder konverterade
}
convert-done-skipped = { $count } överhoppade: namnet var upptaget
convert-done-failed = { $count } misslyckades
convert-stopped = Stoppades efter { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = kan inte öppnas ({ $error })
convert-failure-decode = kan inte läsas ({ $error })
convert-failure-encode = kan inte skrivas i det här formatet ({ $error })
convert-failure-write = kan inte sparas ({ $error })
convert-failure-same = den nya filen skulle ersätta originalet
convert-report-more = och { $count } till
convert-running = En konvertering pågår redan.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = Förlustfri WebP
settings-convert-webp-lossless-help = Behåller varje pixel exakt. Avstängt blir WebP-filer mycket mindre vid WebP-kvaliteten.
settings-convert-webp-quality = WebP-kvalitet
settings-convert-webp-quality-help = Högre behåller fler detaljer i större filer. Används när WebP inte är förlustfri.
settings-convert-webp-effort = WebP-ansträngning
settings-convert-webp-effort-help = Högre ger mindre filer, men långsammare. Används när WebP inte är förlustfri.
settings-convert-jxl-lossless = Förlustfri JPEG XL
settings-convert-jxl-lossless-help = Behåller varje pixel exakt. Avstängt blir JPEG XL-filer mycket mindre vid JPEG XL-kvaliteten.
settings-convert-jxl-quality = JPEG XL-kvalitet
settings-convert-jxl-quality-help = Högre behåller fler detaljer i större filer. Vid 90 är det svårt att se skillnad mot originalet. Används när JPEG XL inte är förlustfri.
settings-convert-avif-quality = AVIF-kvalitet
settings-convert-avif-quality-help = Högre behåller fler detaljer i större filer.
settings-convert-avif-effort = AVIF-ansträngning
settings-convert-avif-effort-help = Högre ger mindre filer, men långsammare. 1 är snabbast.
convert-lossless = Förlustfri
