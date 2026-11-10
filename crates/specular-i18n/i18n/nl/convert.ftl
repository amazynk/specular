# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Converteren…

## Settings → Convert
settings-section-convert = Converteren
settings-convert-format = Formaat
settings-convert-format-help = Het formaat dat Converteren… schrijft. PNG en TIFF zijn verliesvrij; WebP en JPEG XL kunnen allebei.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = JPEG-kwaliteit
settings-convert-jpeg-quality-help = Hoger bewaart meer detail in grotere bestanden. Bij 90 is het verschil met het origineel moeilijk te zien.
settings-convert-jpeg-full-chroma = Kleur op volledige resolutie in JPEG
settings-convert-jpeg-full-chroma-help = Bewaart kleur op volledige resolutie (4:4:4) in plaats van de helft (4:2:0): scherpere kleurranden, grotere bestanden.
settings-convert-png-effort = PNG-compressie
settings-convert-png-effort-help = Hoger maakt kleinere bestanden, maar trager. 1 is het snelst.
settings-convert-jxl-effort = JPEG XL-inspanning
settings-convert-jxl-effort-help = Hoger maakt kleinere bestanden, maar trager.
settings-convert-tiff-compression = TIFF-compressie
settings-convert-tiff-compression-help = Deflate is verliesvrij en wordt door elk programma gelezen dat TIFF leest.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Geen
settings-convert-depth = Bitdiepte
settings-convert-depth-help = Automatisch schrijft 16 bits per sample als de afbeelding er meer dan 8 heeft en het formaat ze kan bevatten (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automatisch
settings-convert-depth-8 = 8 bits
settings-convert-depth-16 = 16 bits
settings-convert-resize = Formaat wijzigen
settings-convert-resize-help = Afbeeldingen worden in lineair licht verkleind met een filter van hoge kwaliteit.
settings-convert-resize-none = Oorspronkelijke grootte
settings-convert-resize-fit = Passend binnen
settings-convert-resize-long-edge = Lange zijde
settings-convert-resize-percent = Percentage
settings-convert-width = Breedte
settings-convert-width-help = De grootste breedte die een afbeelding mag krijgen bij Passend binnen.
settings-convert-height = Hoogte
settings-convert-height-help = De grootste hoogte die een afbeelding mag krijgen bij Passend binnen.
settings-convert-long-edge = Lange zijde
settings-convert-long-edge-help = De lengte van de langste zijde van een afbeelding bij Lange zijde.
settings-convert-percent = Percentage
settings-convert-percent-help = De grootte ten opzichte van het origineel bij Percentage.
settings-convert-upscale = Kleine afbeeldingen vergroten
settings-convert-upscale-help = Uit: afbeeldingen die al kleiner zijn dan de gevraagde grootte, houden hun grootte.
settings-convert-colour = Kleur
settings-convert-colour-help = Behouden slaat het kleurprofiel van de afbeelding mee op. Naar sRGB converteren is geschikt voor het web en apparaten zonder kleurbeheer.
settings-convert-colour-keep = Profiel behouden
settings-convert-colour-srgb = Naar sRGB converteren
settings-convert-hdr = HDR-afbeeldingen
settings-convert-hdr-help = PNG en JPEG XL behouden HDR zoals het is; andere formaten worden altijd met tonemapping naar SDR omgezet.
settings-convert-hdr-keep = Behouden waar mogelijk
settings-convert-hdr-tone-map = Tonemapping naar SDR
settings-convert-metadata = Metagegevens
settings-convert-metadata-help = Exif en XMP, zoals de camera, de opnamedatum en de locatie. Het kleurprofiel blijft altijd behouden.
settings-convert-metadata-all = Alles behouden
settings-convert-metadata-no-location = Alles behalve locatie
settings-convert-metadata-strip = Verwijderen
settings-convert-folder = Uitvoermap
settings-convert-folder-help = Waar geconverteerde bestanden terechtkomen. Niet ingesteld: in een submap naast elke afbeelding.
settings-convert-folder-unset = Submap naast de afbeeldingen
settings-convert-subfolder = Naam van submap
settings-convert-subfolder-help = De submap die wordt gebruikt als er geen uitvoermap is ingesteld. Leeg: de eigen map van de afbeeldingen.
settings-convert-pattern = Bestandsnamen
settings-convert-pattern-help = {"{"}name{"}"} is de oorspronkelijke naam, {"{"}n:03{"}"} het nummer in de reeks, {"{"}date:%Y-%m-%d{"}"} de opnamedatum, {"{"}w{"}"} en {"{"}h{"}"} de nieuwe grootte, {"{"}ext{"}"} de extensie van het formaat.
settings-convert-conflicts = Als een naam bezet is
settings-convert-conflicts-help = Bestaande bestanden worden nooit vervangen.
settings-convert-conflicts-suffix = Een nummer toevoegen
settings-convert-conflicts-skip = De afbeelding overslaan

## The dialog
convert-title = Converteren
convert-images = Afbeeldingen
convert-images-current = Deze afbeelding
convert-images-selection = Selectie ({ $count })
convert-images-folder = Map ({ $count })
convert-images-picked = Keuzes ({ $count })
convert-quality = Kwaliteit
convert-chroma = Kleurdetail
convert-chroma-half = 4:2:0 (kleiner)
convert-chroma-full = 4:4:4 (scherper)
convert-effort = Inspanning
convert-on = Aan
convert-off = Uit
convert-pixels = { $value } px
convert-effort-fastest = { $value } (snelst)
convert-folder-subfolder = Submap ‘{ $name }’
convert-folder-same = Dezelfde map als de afbeelding
convert-choose-folder = Map kiezen…
convert-name = Naam
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Een {"{"} wordt niet gesloten.
convert-name-error-stray = Een {"}"} heeft geen {"{"}. Schrijf {"}}"} voor een accolade.
convert-name-error-unknown = { $token } is geen naamdeel. Gebruik name, n, date, w, h of ext.
convert-name-error-format = { $token } heeft een ongeldige opmaak.
convert-name-error-empty = Voer een naampatroon in.
convert-start = Converteren
convert-stop = Stoppen
convert-hide = Verbergen
convert-close = Sluiten
convert-progress = Bezig met converteren: { $done } van { $total }
convert-progress-eta = Bezig met converteren: { $done } van { $total } · nog ongeveer { $time }
convert-eta-seconds = { $count ->
    [one] { $count } seconde
   *[other] { $count } seconden
}
convert-eta-minutes = { $count ->
    [one] { $count } minuut
   *[other] { $count } minuten
}
convert-done = { $count ->
    [one] { $count } afbeelding geconverteerd
   *[other] { $count } afbeeldingen geconverteerd
}
convert-done-skipped = { $count } overgeslagen: de naam was bezet
convert-done-failed = { $count } mislukt
convert-stopped = Gestopt na { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = kan niet worden geopend ({ $error })
convert-failure-decode = kan niet worden gelezen ({ $error })
convert-failure-encode = kan niet in dit formaat worden geschreven ({ $error })
convert-failure-write = kan niet worden opgeslagen ({ $error })
convert-failure-same = het nieuwe bestand zou het origineel vervangen
convert-report-more = en nog { $count }
convert-running = Er loopt al een conversie.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = WebP verliesvrij
settings-convert-webp-lossless-help = Behoudt elke pixel exact. Uitgeschakeld zijn WebP-bestanden veel kleiner bij de WebP-kwaliteit.
settings-convert-webp-quality = WebP-kwaliteit
settings-convert-webp-quality-help = Hoger bewaart meer detail in grotere bestanden. Gebruikt als WebP niet verliesvrij is.
settings-convert-webp-effort = WebP-inspanning
settings-convert-webp-effort-help = Hoger maakt kleinere bestanden, maar trager. Gebruikt als WebP niet verliesvrij is.
settings-convert-jxl-lossless = JPEG XL verliesvrij
settings-convert-jxl-lossless-help = Behoudt elke pixel exact. Uitgeschakeld zijn JPEG XL-bestanden veel kleiner bij de JPEG XL-kwaliteit.
settings-convert-jxl-quality = JPEG XL-kwaliteit
settings-convert-jxl-quality-help = Hoger bewaart meer detail in grotere bestanden. Bij 90 is het verschil met het origineel moeilijk te zien. Gebruikt als JPEG XL niet verliesvrij is.
settings-convert-avif-quality = AVIF-kwaliteit
settings-convert-avif-quality-help = Hoger bewaart meer detail in grotere bestanden.
settings-convert-avif-effort = AVIF-inspanning
settings-convert-avif-effort-help = Hoger maakt kleinere bestanden, maar trager. 1 is het snelst.
convert-lossless = Verliesvrij
