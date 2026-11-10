# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Konvertér…

## Settings → Convert
settings-section-convert = Konvertering
settings-convert-format = Format
settings-convert-format-help = Det format, Konvertér… skriver. PNG og TIFF er tabsfri; WebP og JPEG XL kan være begge dele.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = JPEG-kvalitet
settings-convert-jpeg-quality-help = Højere bevarer flere detaljer i større filer. Ved 90 er det svært at se forskel på originalen.
settings-convert-jpeg-full-chroma = Farver i fuld opløsning i JPEG
settings-convert-jpeg-full-chroma-help = Bevarer farverne i fuld opløsning (4:4:4) i stedet for halv (4:2:0): skarpere farvekanter, større filer.
settings-convert-png-effort = PNG-komprimering
settings-convert-png-effort-help = Højere giver mindre filer, men langsommere. 1 er hurtigst.
settings-convert-jxl-effort = JPEG XL-indsats
settings-convert-jxl-effort-help = Højere giver mindre filer, men langsommere.
settings-convert-tiff-compression = TIFF-komprimering
settings-convert-tiff-compression-help = Deflate er tabsfri og kan læses af alle TIFF-læsere.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Ingen
settings-convert-depth = Bitdybde
settings-convert-depth-help = Automatisk skriver 16 bit pr. sample, når billedet har mere end 8, og formatet kan rumme dem (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automatisk
settings-convert-depth-8 = 8 bit
settings-convert-depth-16 = 16 bit
settings-convert-resize = Skift størrelse
settings-convert-resize-help = Billeder skaleres ned i lineært lys med et filter af høj kvalitet.
settings-convert-resize-none = Original størrelse
settings-convert-resize-fit = Tilpas inden for
settings-convert-resize-long-edge = Lange side
settings-convert-resize-percent = Procent
settings-convert-width = Bredde
settings-convert-width-help = Den største bredde, et billede må få med Tilpas inden for.
settings-convert-height = Højde
settings-convert-height-help = Den største højde, et billede må få med Tilpas inden for.
settings-convert-long-edge = Lange side
settings-convert-long-edge-help = Længden af billedets længste side med Lange side.
settings-convert-percent = Procent
settings-convert-percent-help = Størrelsen i forhold til originalen med Procent.
settings-convert-upscale = Forstør små billeder
settings-convert-upscale-help = Når den er slået fra, beholder billeder, der allerede er mindre end den ønskede størrelse, deres størrelse.
settings-convert-colour = Farve
settings-convert-colour-help = Behold gemmer billedets farveprofil sammen med det. Konvertér til sRGB passer til nettet og enheder uden farvestyring.
settings-convert-colour-keep = Behold profilen
settings-convert-colour-srgb = Konvertér til sRGB
settings-convert-hdr = HDR-billeder
settings-convert-hdr-help = PNG og JPEG XL bevarer HDR som det er; andre formater tonemappes altid til SDR.
settings-convert-hdr-keep = Behold, hvor det er muligt
settings-convert-hdr-tone-map = Tonemap til SDR
settings-convert-metadata = Metadata
settings-convert-metadata-help = Exif og XMP, f.eks. kameraet, optagelsesdatoen og placeringen. Farveprofilen bevares altid.
settings-convert-metadata-all = Behold alt
settings-convert-metadata-no-location = Behold alt undtagen placering
settings-convert-metadata-strip = Fjern
settings-convert-folder = Destinationsmappe
settings-convert-folder-help = Hvor konverterede filer havner. Hvis den ikke er valgt, havner de i en undermappe ved siden af hvert billede.
settings-convert-folder-unset = Undermappe ved siden af billederne
settings-convert-subfolder = Navn på undermappe
settings-convert-subfolder-help = Den undermappe, der bruges, når der ikke er valgt en destinationsmappe. Tom: billedernes egen mappe.
settings-convert-pattern = Filnavne
settings-convert-pattern-help = {"{"}name{"}"} er det oprindelige navn, {"{"}n:03{"}"} nummeret i serien, {"{"}date:%Y-%m-%d{"}"} optagelsesdatoen, {"{"}w{"}"} og {"{"}h{"}"} den nye størrelse, {"{"}ext{"}"} formatets filendelse.
settings-convert-conflicts = Når et navn er optaget
settings-convert-conflicts-help = Eksisterende filer erstattes aldrig.
settings-convert-conflicts-suffix = Tilføj et nummer
settings-convert-conflicts-skip = Spring billedet over

## The dialog
convert-title = Konvertér
convert-images = Billeder
convert-images-current = Dette billede
convert-images-selection = Markering ({ $count })
convert-images-folder = Mappe ({ $count })
convert-images-picked = Valgte ({ $count })
convert-quality = Kvalitet
convert-chroma = Farvedetaljer
convert-chroma-half = 4:2:0 (mindre)
convert-chroma-full = 4:4:4 (skarpere)
convert-effort = Indsats
convert-on = Til
convert-off = Fra
convert-pixels = { $value } px
convert-effort-fastest = { $value } (hurtigst)
convert-folder-subfolder = Undermappen ”{ $name }”
convert-folder-same = Samme mappe som billedet
convert-choose-folder = Vælg mappe…
convert-name = Navn
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = En {"{"} lukkes ikke.
convert-name-error-stray = En {"}"} har ingen {"{"}. Skriv {"}}"} for en krøllet parentes.
convert-name-error-unknown = { $token } er ikke en navnedel. Brug name, n, date, w, h eller ext.
convert-name-error-format = { $token } har et ugyldigt format.
convert-name-error-empty = Angiv et navnemønster.
convert-start = Konvertér
convert-stop = Stop
convert-hide = Skjul
convert-close = Luk
convert-progress = Konverterer { $done } af { $total }
convert-progress-eta = Konverterer { $done } af { $total } · ca. { $time } tilbage
convert-eta-seconds = { $count ->
    [one] { $count } sekund
   *[other] { $count } sekunder
}
convert-eta-minutes = { $count ->
    [one] { $count } minut
   *[other] { $count } minutter
}
convert-done = { $count ->
    [one] { $count } billede konverteret
   *[other] { $count } billeder konverteret
}
convert-done-skipped = { $count } sprunget over: navnet var optaget
convert-done-failed = { $count } mislykkedes
convert-stopped = Stoppet efter { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = kan ikke åbnes ({ $error })
convert-failure-decode = kan ikke læses ({ $error })
convert-failure-encode = kan ikke skrives i dette format ({ $error })
convert-failure-write = kan ikke gemmes ({ $error })
convert-failure-same = den nye fil ville erstatte originalen
convert-report-more = og { $count } mere
convert-running = En konvertering kører allerede.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = Tabsfri WebP
settings-convert-webp-lossless-help = Bevarer hver pixel nøjagtigt. Slået fra bliver WebP-filer meget mindre ved WebP-kvaliteten.
settings-convert-webp-quality = WebP-kvalitet
settings-convert-webp-quality-help = Højere bevarer flere detaljer i større filer. Bruges, når WebP ikke er tabsfri.
settings-convert-webp-effort = WebP-indsats
settings-convert-webp-effort-help = Højere giver mindre filer, men langsommere. Bruges, når WebP ikke er tabsfri.
settings-convert-jxl-lossless = Tabsfri JPEG XL
settings-convert-jxl-lossless-help = Bevarer hver pixel nøjagtigt. Slået fra bliver JPEG XL-filer meget mindre ved JPEG XL-kvaliteten.
settings-convert-jxl-quality = JPEG XL-kvalitet
settings-convert-jxl-quality-help = Højere bevarer flere detaljer i større filer. Ved 90 er det svært at se forskel på originalen. Bruges, når JPEG XL ikke er tabsfri.
settings-convert-avif-quality = AVIF-kvalitet
settings-convert-avif-quality-help = Højere bevarer flere detaljer i større filer.
settings-convert-avif-effort = AVIF-indsats
settings-convert-avif-effort-help = Højere giver mindre filer, men langsommere. 1 er hurtigst.
convert-lossless = Tabsfri
