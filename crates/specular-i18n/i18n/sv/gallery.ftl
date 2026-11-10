# machine-drafted, needs native review
# Specular — galleriet (miniatyrläge, DESIGN §10) och Inställningar → Galleri. Svenska.

## Palett och menyer

palette-gallery = Galleri
gallery-palette-view = Gallerivy…
gallery-palette-layout = Layout
gallery-palette-sort = Sortera efter
gallery-palette-group = Gruppera efter
gallery-palette-filter = Filtrera…
gallery-palette-quick-filters = Snabbfilter
gallery-palette-details = Detaljvy
gallery-palette-info-overlay = Info på miniatyrer
gallery-palette-stacks = Stapla RAW+JPEG och bildserier
gallery-palette-subfolders = Ta med undermappar
gallery-palette-compare = Jämför markerade
gallery-palette-rate = Betygsätt…
gallery-palette-settings = Galleriinställningar…
gallery-view-menu = Visa
gallery-more-menu = Mer
gallery-open = Öppna
gallery-new-window = Öppna i nytt fönster
gallery-reveal = Visa i mapp
gallery-properties = Egenskaper
gallery-copy-paths = Kopiera sökvägar
gallery-select-all = Markera alla
gallery-select-none = Avmarkera alla
gallery-select-invert = Invertera markering
gallery-next-unrated = Nästa utan betyg
gallery-stack-toggle = Visa eller dölj stapel
gallery-run-on = { $name } ({ $count } filer)
gallery-depth-off = Endast den här mappen
gallery-depth-all = Alla undermappar
gallery-depth-levels = { $depth ->
    [one] { $depth } nivå av undermappar
   *[other] { $depth } nivåer av undermappar
}

## Status och tomma lägen

gallery-items = { $count ->
    [one] { $count } objekt
   *[other] { $count } objekt
}
gallery-selected = { $count } av { $total } markerade
gallery-shown-of = { $count } av { $total } visas
gallery-listing = Läser mappen… { $count }
gallery-empty = Mappen innehåller inga bilder.
gallery-empty-filter = Inga bilder matchar filtret.
gallery-list-error = Det går inte att läsa mappen: { $reason }
gallery-no-folder = Ingen mapp är öppen
gallery-kind-folder = Mapp
gallery-drag-count = { $count ->
    [one] { $count } fil
   *[other] { $count } filer
}
gallery-badge-pages = { $count } sidor

## Filterfält

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Namn, eller rating>=3 camera:X-T5 …
gallery-filter-clear = Rensa filter
gallery-filter-rating = { $stars ->
    [one] ★ { $stars } eller fler
   *[other] ★ { $stars } eller fler
}
gallery-filter-label = Etikett: { $label }
gallery-filter-picks = Valda
gallery-filter-hide-rejects = Dölj förkastade
gallery-filter-unrated = Utan betyg
gallery-filter-raw = RAW
gallery-filter-animated = Animerade
gallery-filter-hdr = HDR
gallery-filter-landscape = Liggande
gallery-filter-portrait = Stående

## Query errors and facets

gallery-query-unknown-field = Okänt fält ”{ $field }”
gallery-query-unknown-field-suggest = Okänt fält ”{ $field }”: menade du ”{ $suggestion }”?
gallery-query-missing-value = ”{ $field }” behöver ett värde
gallery-query-bad-number = ”{ $value }” är inget tal ({ $field })
gallery-query-bad-date = ”{ $value }” är inget datum ({ $field }): använd 2024, 2024-05 eller 2024-05-12
gallery-query-bad-word = ”{ $value }” passar inte { $field }: använd { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = ”{ $value }” är inget Exif-taggnamn
gallery-query-no-compare = { $field } kan inte jämföras med < eller > här
gallery-query-unterminated = Ett avslutande citattecken saknas
gallery-query-unavailable = ”{ $field }” kräver biblioteket (inte tillgängligt här)
gallery-facets = Fasetter
gallery-facets-counting = Räknar…
gallery-facets-empty = Inget att räkna
gallery-facets-search = Sök i fasetter
gallery-facets-no-match = Inga värden matchar
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Startskärm
gallery-facet-format = Format
gallery-facet-camera = Kamera
gallery-facet-lens = Objektiv
gallery-facet-rating = Betyg
gallery-facet-year = År
gallery-facet-orientation = Orientering
gallery-facet-label = Etikett
gallery-facet-properties = Egenskaper
gallery-facet-square = Kvadratisk
gallery-facet-alpha = Genomskinlighet
gallery-facet-gps = Plats
gallery-facet-multipage = Flera sidor
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } fil
   *[other] { $count } filer
}

## Grupper

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Den här mappen
gallery-group-unknown = Okänt
gallery-group-unrated = Utan betyg

## Gallring

gallery-stars = { $stars ->
    [one] { $stars } stjärna
   *[other] { $stars } stjärnor
}
gallery-rate-none = Inget betyg
gallery-pick = Välj
gallery-reject = Förkasta
gallery-unflag = Ingen flagga
gallery-label-red = Röd
gallery-label-yellow = Gul
gallery-label-green = Grön
gallery-label-blue = Blå
gallery-label-none = Ingen etikett
gallery-compare-hint = Markera 2 till 4 bilder för att jämföra dem.

## Filer

gallery-trash-count = { $count ->
    [one] Flytta { $count } fil till papperskorgen
   *[other] Flytta { $count } filer till papperskorgen
}
gallery-moved-to = { $count ->
    [one] { $count } fil flyttades till { $folder }
   *[other] { $count } filer flyttades till { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } fil kopierades till { $folder }
   *[other] { $count } filer kopierades till { $folder }
}
gallery-move-failed = Det gick inte att flytta { $file }: { $reason }
gallery-drop-move = Flytta till { $folder }
gallery-drop-copy = Kopiera till { $folder }
gallery-move-here = Flytta markerade hit
gallery-copy-here = Kopiera markerade hit
gallery-not-an-image = { $file } är inte en bild som { -app-name } kan öppna.
gallery-cache-cleared = Miniatyrcachen har rensats
# machine-drafted, needs native review
gallery-thumb-failed = Ingen miniatyr: { $reason }
gallery-cache-clear-failed = Det gick inte att rensa miniatyrcachen: { $reason }

## Egenskaper

gallery-prop-name = Namn
gallery-prop-folder = Mapp
gallery-prop-size = Filstorlek
gallery-prop-dimensions = Mått
gallery-prop-format = Format
gallery-prop-date = Datum
gallery-prop-camera = Kamera
gallery-prop-lens = Objektiv
gallery-prop-rating = Betyg
gallery-prop-label = Etikett
gallery-prop-items = Objekt

## Skärmläsare

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } objekt
   *[other] { $count } objekt
}
gallery-a11y-dims = { $width } gånger { $height }
gallery-a11y-folder = mapp
gallery-a11y-up = Upp till { $folder }
gallery-a11y-archive = arkiv
gallery-a11y-stack = stapel med { $count }
gallery-a11y-filter-bar = Filter
gallery-a11y-filter-field = Filtrera efter namn eller fält
gallery-a11y-chip = Filtervillkor { $term }
gallery-a11y-chip-remove = Tryck för att ta bort
gallery-a11y-results = { $count ->
    [0] Inga objekt matchar
    [one] { $count } objekt matchar
   *[other] { $count } objekt matchar
}

## Inställningar → Galleri

settings-section-gallery = Galleri
settings-gallery-layout = Layout
settings-gallery-layout-help = Kvadrater beskär miniatyrerna till ett rutnät; justerade rader behåller varje bilds form; obeskuret passar in hela bilder i lika stora celler.
settings-gallery-layout-squares = Kvadrater
settings-gallery-layout-justified = Justerade rader
settings-gallery-layout-uncropped = Obeskuret
settings-gallery-layout-details = Detaljer
settings-gallery-thumbnail-size = Miniatyrstorlek
settings-gallery-thumbnail-size-help = Även Ctrl+mushjul, nypning eller Ctrl+plus och minus i galleriet.
settings-gallery-captions = Visa filnamn
settings-gallery-captions-help = En bildtext under varje miniatyr.
settings-gallery-badges = Visa märken
settings-gallery-badges-help = Små markeringar för RAW, animerad, HDR, flera sidor, betyg och etikett.
settings-gallery-sort-order = Sortera efter
settings-gallery-sort-order-help = Galleriets ordning. Mappar kommer alltid först.
settings-gallery-sort-order-name = Namn
settings-gallery-sort-order-date-taken = Fotodatum
settings-gallery-sort-order-modified = Ändringsdatum
settings-gallery-sort-order-size = Filstorlek
settings-gallery-sort-order-type = Filtyp
settings-gallery-sort-order-dimensions = Mått
settings-gallery-sort-descending = Omvänd ordning
settings-gallery-sort-descending-help = Sortera från sist till först.
settings-gallery-group-by = Gruppera efter
settings-gallery-group-by-help = Avsnittsrubriker som stannar överst medan du rullar.
settings-gallery-group-by-none = Ingenting
settings-gallery-group-by-day = Dag
settings-gallery-group-by-month = Månad
settings-gallery-group-by-year = År
settings-gallery-group-by-event = Händelse
settings-gallery-group-by-folder = Mapp
settings-gallery-group-by-camera = Kamera
settings-gallery-group-by-format = Format
settings-gallery-group-by-rating = Betyg
settings-gallery-group-by-target = Objekt
settings-gallery-group-by-filter = Filter
settings-gallery-per-folder-view = Kom ihåg vyn för varje mapp
settings-gallery-per-folder-view-help = Layout, storlek, sortering och gruppering sparas per mapp; annars används en vy överallt.
settings-gallery-open-folders-in-gallery = Öppna mappar i galleriet
settings-gallery-open-folders-in-gallery-help = När du öppnar en mapp börjar du med dess miniatyrer i stället för den första bilden.
settings-gallery-escape-opens-gallery = Esc öppnar galleriet
settings-gallery-escape-opens-gallery-help = När det inte finns något annat att stänga går Esc i visaren till galleriet i stället för att inte göra något.
settings-gallery-wrap-around = Piltangenterna går runt
settings-gallery-wrap-around-help = I slutet av listan går piltangenterna till början, och från början till slutet. En nedhållen tangent stannar fortfarande vid slutet.
settings-gallery-show-hidden = Visa dolda filer
settings-gallery-show-hidden-help = Ta med filer och mappar vars namn börjar med en punkt.
settings-gallery-show-other-files = Visa andra filer
settings-gallery-show-other-files-help = Ta med filer som { -app-name } inte kan öppna, visade med en allmän ikon.
settings-gallery-show-archives = Visa arkiv
settings-gallery-show-archives-help = Ta med serier och andra arkiv (zip, cbz, rar, 7z…) som rutor. Det går fortfarande att öppna ett arkiv direkt.
settings-gallery-subfolder-depth = Nivåer av undermappar
settings-gallery-subfolder-depth-help = Visa även bilder från undermappar, ned till så här många nivåer (0: endast den här mappen).
settings-gallery-stacks = Stapla relaterade filer
settings-gallery-stacks-help = Visa RAW+JPEG-par och bildserier som en ruta. Tryck på S för att fälla ut en stapel.
settings-gallery-info-overlay = Info på miniatyrer
settings-gallery-info-overlay-help = Mått och exponering över varje miniatyr.
settings-gallery-prefer-embedded = Föredra inbäddade förhandsvisningar
settings-gallery-prefer-embedded-help = Använd förhandsvisningen som finns i filen när den är tillräckligt stor. Mycket snabbare; lite mindre exakt.
settings-gallery-sharper-raw-thumbnails = Skarpare RAW-miniatyrer
settings-gallery-sharper-raw-thumbnails-help = Redigerade RAW-foton framkallas i 1024 px för sina miniatyrer. Skarpare, lite långsammare. Av: 512 px.
settings-gallery-rating-sidecars = Skriv betyg till XMP-sidofiler
settings-gallery-rating-sidecars-help = Betyg, etiketter och flaggor sparas även i en .xmp-fil bredvid bilden, som andra fotoprogram läser.
settings-gallery-cache-size = Storlek på miniatyrcache
settings-gallery-cache-size-help = De äldsta miniatyrerna tas bort när cachen är full.
settings-gallery-cache-location = Plats för miniatyrcache
settings-gallery-cache-location-help = En mapp på en snabb enhet fungerar bäst.
settings-gallery-clear-cache = Rensa miniatyrcache
settings-gallery-clear-cache-help = Tar bort de sparade miniatyrerna. De skapas igen när du bläddrar.

## Compare
# machine-drafted, needs native review

compare-side = Sida vid sida
compare-overlay = Överlagring
compare-difference = Skillnad
compare-flicker = Blinka
compare-blend = Blanda
compare-kind-absolute = Absolut fel
compare-kind-squared = Kvadratiskt fel
compare-kind-relative = Relativt fel
compare-scale-down = Mindre känslig
compare-scale-up = Känsligare
compare-scale = Fullskala { $value }
compare-space = Linjärt ljus, som det visas
compare-space-help = Fel och mått mäts i linjärt ljus som bilden visas, efter tonmappning, per kanal inklusive alfa.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = samplat vid { $width } × { $height }
compare-aligned = Justerat uppe till vänster, { $width } × { $height } jämfört
compare-vs = { $b } mot { $a }
compare-overlay-label = { $b } över { $a } vid { PERCENT($percent) }
compare-pending = Mäter…
# machine-drafted, needs native review
compare-diff-failed = Det gick inte att visa skillnaden: { $reason }
compare-no-overlap = Inget att jämföra i vyn
compare-zoom-fit = Anpassa
compare-zoom = { PERCENT($percent) }
compare-showing = Visar { $name }
compare-a11y-group = Jämför
compare-a11y-blend = Blanda mellan bilderna
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Jämför sida vid sida
gallery-palette-compare-overlay = Jämför som överlagring
gallery-palette-compare-difference = Jämför skillnad: { $kind }
gallery-palette-compare-flicker = Blinka mellan jämförda bilder
gallery-palette-compare-actual = Jämför vid { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Blanksteg eller Ctrl+Retur väljer flera

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Färgprofil
gallery-facet-date = Fotodatum
gallery-facet-picked = Valda
gallery-facet-rejected = Förkastade
gallery-facet-flagged = Valda eller förkastade
gallery-facet-rated = Betygsatta
gallery-facet-labelled = Med etikett

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Bitdjup
gallery-facet-pages = Sidor
gallery-facet-width = Bredd
gallery-facet-height = Höjd
gallery-facet-megapixels = Megapixel
gallery-picker-bits = { $bits } bitar
gallery-picker-pages = { $pages ->
    [one] { $pages } sida
   *[other] { $pages } sidor
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = Alla i en grupp
gallery-picker-group-keep = Förslag: behåll
gallery-picker-group-discard = Förslag: kassera
gallery-picker-ops = Jämför
gallery-picker-op-eq = Lika med
gallery-picker-op-ge = Minst
gallery-picker-op-le = Högst
gallery-picker-ge = minst { $value }
gallery-picker-gt = mer än { $value }
gallery-picker-le = högst { $value }
gallery-picker-lt = mindre än { $value }

# Map view.
gallery-facet-place = Plats
gallery-palette-map = Karta
map-title = Karta
map-summary = { $count ->
    [one] { $count } foto med plats
   *[other] { $count } foton med plats
}
map-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } foton
}
map-zoom-in = Zooma in
map-zoom-out = Zooma ut
map-show-all = Visa alla
map-done = Klar
map-credit = Kartdata: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Karta: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Kartrutor online
map-basemap-consent-server = Kartrutorna kommer från { $host }.
map-basemap-consent-sees = Servern ser din internetadress och de områden du tittar på.
map-basemap-consent-private = Dina foton, deras namn och deras platser skickas inte.
map-basemap-consent-cache = Rutor sparas på den här datorn, upp till { $size }.
map-basemap-allow = Använd onlinekarta
map-basemap-decline = Behåll offlinekarta
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Bakgrundskartan visas inte: { $reason }
map-basemap-bad-address = ruttjänstens adress måste innehålla {"{"}z{"}"}, {"{"}x{"}"} och {"{"}y{"}"}.
map-basemap-unknown-tiles = kartfilens rutor är av okänd typ.
map-loading = Läser fotoplatser…
map-empty = Inget av dessa foton har en plats
map-hidden = Fotoplatser är dolda (Inställningar › Metadata och integritet)
map-a11y-group = Karta över fotoplatser
map-a11y-status = { $count ->
    [one] { $count } plats visas, zoomnivå { $zoom }
   *[other] { $count } platser visas, zoomnivå { $zoom }
}
