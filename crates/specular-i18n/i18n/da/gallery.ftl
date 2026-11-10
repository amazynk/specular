# machine-drafted, needs native review
# Specular — galleriet (miniaturetilstand, DESIGN §10) og Indstillinger → Galleri. Dansk.

## Palet og menuer

palette-gallery = Galleri
gallery-palette-view = Gallerivisning…
gallery-palette-layout = Layout
gallery-palette-sort = Sortér efter
gallery-palette-group = Gruppér efter
gallery-palette-filter = Filtrer…
gallery-palette-quick-filters = Hurtigfiltre
gallery-palette-details = Detaljevisning
gallery-palette-info-overlay = Oplysninger på miniaturer
gallery-palette-stacks = Stak RAW+JPEG og serier
gallery-palette-subfolders = Medtag undermapper
gallery-palette-compare = Sammenlign valgte
gallery-palette-rate = Bedøm…
gallery-palette-settings = Galleriindstillinger…
gallery-view-menu = Vis
gallery-more-menu = Mere
gallery-open = Åbn
gallery-new-window = Åbn i nyt vindue
gallery-reveal = Vis i mappe
gallery-properties = Egenskaber
gallery-copy-paths = Kopiér stier
gallery-select-all = Markér alt
gallery-select-none = Fjern markering
gallery-select-invert = Inverter markering
gallery-next-unrated = Næste ubedømte
gallery-stack-toggle = Udvid eller skjul stak
gallery-run-on = { $name } ({ $count } filer)
gallery-depth-off = Kun denne mappe
gallery-depth-all = Alle undermapper
gallery-depth-levels = { $depth ->
    [one] { $depth } niveau af undermapper
   *[other] { $depth } niveauer af undermapper
}

## Status og tomme tilstande

gallery-items = { $count ->
    [one] { $count } element
   *[other] { $count } elementer
}
gallery-selected = { $count } af { $total } valgt
gallery-shown-of = { $count } af { $total } vist
gallery-listing = Læser mappe… { $count }
gallery-empty = Mappen indeholder ingen billeder.
gallery-empty-filter = Ingen billeder matcher filteret.
gallery-list-error = Kan ikke læse mappen: { $reason }
gallery-no-folder = Ingen mappe åben
gallery-kind-folder = Mappe
gallery-drag-count = { $count ->
    [one] { $count } fil
   *[other] { $count } filer
}
gallery-badge-pages = { $count } sider

## Filterlinje

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Navn, eller rating>=3 camera:X-T5 …
gallery-filter-clear = Ryd filter
gallery-filter-rating = { $stars ->
    [one] ★ { $stars } eller flere
   *[other] ★ { $stars } eller flere
}
gallery-filter-label = Etiket: { $label }
gallery-filter-picks = Udvalgte
gallery-filter-hide-rejects = Skjul afviste
gallery-filter-unrated = Ubedømte
gallery-filter-raw = RAW
gallery-filter-animated = Animerede
gallery-filter-hdr = HDR
gallery-filter-landscape = Liggende
gallery-filter-portrait = Stående

## Query errors and facets

gallery-query-unknown-field = Ukendt felt »{ $field }«
gallery-query-unknown-field-suggest = Ukendt felt »{ $field }«: mente du »{ $suggestion }«?
gallery-query-missing-value = »{ $field }« skal have en værdi
gallery-query-bad-number = »{ $value }« er ikke et tal ({ $field })
gallery-query-bad-date = »{ $value }« er ikke en dato ({ $field }): brug 2024, 2024-05 eller 2024-05-12
gallery-query-bad-word = »{ $value }« passer ikke til { $field }: brug { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = »{ $value }« er ikke et Exif-tagnavn
gallery-query-no-compare = { $field } kan ikke sammenlignes med < eller > her
gallery-query-unterminated = Der mangler et afsluttende anførselstegn
gallery-query-unavailable = »{ $field }« kræver biblioteket (ikke tilgængeligt her)
gallery-facets = Facetter
gallery-facets-counting = Tæller…
gallery-facets-empty = Intet at tælle
gallery-facets-search = Søg i facetter
gallery-facets-no-match = Ingen værdier passer
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Startskærm
gallery-facet-format = Format
gallery-facet-camera = Kamera
gallery-facet-lens = Objektiv
gallery-facet-rating = Bedømmelse
gallery-facet-year = År
gallery-facet-orientation = Retning
gallery-facet-label = Etiket
gallery-facet-properties = Egenskaber
gallery-facet-square = Kvadratisk
gallery-facet-alpha = Gennemsigtighed
gallery-facet-gps = Placering
gallery-facet-multipage = Flere sider
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } fil
   *[other] { $count } filer
}

## Grupper

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Denne mappe
gallery-group-unknown = Ukendt
gallery-group-unrated = Ubedømte

## Sortering

gallery-stars = { $stars ->
    [one] { $stars } stjerne
   *[other] { $stars } stjerner
}
gallery-rate-none = Ingen bedømmelse
gallery-pick = Udvælg
gallery-reject = Afvis
gallery-unflag = Intet flag
gallery-label-red = Rød
gallery-label-yellow = Gul
gallery-label-green = Grøn
gallery-label-blue = Blå
gallery-label-none = Ingen etiket
gallery-compare-hint = Vælg 2 til 4 billeder for at sammenligne dem.

## Filer

gallery-trash-count = { $count ->
    [one] Flyt { $count } fil til papirkurven
   *[other] Flyt { $count } filer til papirkurven
}
gallery-moved-to = { $count ->
    [one] { $count } fil flyttet til { $folder }
   *[other] { $count } filer flyttet til { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } fil kopieret til { $folder }
   *[other] { $count } filer kopieret til { $folder }
}
gallery-move-failed = Kunne ikke flytte { $file }: { $reason }
gallery-drop-move = Flyt til { $folder }
gallery-drop-copy = Kopiér til { $folder }
gallery-move-here = Flyt valgte hertil
gallery-copy-here = Kopiér valgte hertil
gallery-not-an-image = { $file } er ikke et billede, som { -app-name } kan åbne.
gallery-cache-cleared = Miniaturecachen er ryddet
# machine-drafted, needs native review
gallery-thumb-failed = Ingen miniature: { $reason }
gallery-cache-clear-failed = Kunne ikke rydde miniaturecachen: { $reason }

## Egenskaber

gallery-prop-name = Navn
gallery-prop-folder = Mappe
gallery-prop-size = Filstørrelse
gallery-prop-dimensions = Mål
gallery-prop-format = Format
gallery-prop-date = Dato
gallery-prop-camera = Kamera
gallery-prop-lens = Objektiv
gallery-prop-rating = Bedømmelse
gallery-prop-label = Etiket
gallery-prop-items = Elementer

## Skærmlæsere

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } element
   *[other] { $count } elementer
}
gallery-a11y-dims = { $width } gange { $height }
gallery-a11y-folder = mappe
gallery-a11y-up = Op til { $folder }
gallery-a11y-archive = arkiv
gallery-a11y-stack = stak med { $count }
gallery-a11y-filter-bar = Filter
gallery-a11y-filter-field = Filtrér efter navn eller felt
gallery-a11y-chip = Filterudtryk { $term }
gallery-a11y-chip-remove = Tryk for at fjerne
gallery-a11y-results = { $count ->
    [0] Ingen elementer matcher
    [one] { $count } element matcher
   *[other] { $count } elementer matcher
}

## Indstillinger → Galleri

settings-section-gallery = Galleri
settings-gallery-layout = Layout
settings-gallery-layout-help = Kvadrater beskærer miniaturer til et gitter; justerede rækker bevarer hvert billedes form; ubeskåret tilpasser hele billeder i lige store celler.
settings-gallery-layout-squares = Kvadrater
settings-gallery-layout-justified = Justerede rækker
settings-gallery-layout-uncropped = Ubeskåret
settings-gallery-layout-details = Detaljer
settings-gallery-thumbnail-size = Miniaturestørrelse
settings-gallery-thumbnail-size-help = Også Ctrl+musehjul, knib eller Ctrl+plus og minus i galleriet.
settings-gallery-captions = Vis filnavne
settings-gallery-captions-help = En billedtekst under hver miniature.
settings-gallery-badges = Vis mærker
settings-gallery-badges-help = Små mærker for RAW, animeret, HDR, flere sider, bedømmelse og etiket.
settings-gallery-sort-order = Sortér efter
settings-gallery-sort-order-help = Rækkefølgen i galleriet. Mapper kommer altid først.
settings-gallery-sort-order-name = Navn
settings-gallery-sort-order-date-taken = Optagelsesdato
settings-gallery-sort-order-modified = Ændringsdato
settings-gallery-sort-order-size = Filstørrelse
settings-gallery-sort-order-type = Filtype
settings-gallery-sort-order-dimensions = Mål
settings-gallery-sort-descending = Omvendt rækkefølge
settings-gallery-sort-descending-help = Sortér fra sidste til første.
settings-gallery-group-by = Gruppér efter
settings-gallery-group-by-help = Sektionsoverskrifter, der bliver øverst, mens du ruller.
settings-gallery-group-by-none = Intet
settings-gallery-group-by-day = Dag
settings-gallery-group-by-month = Måned
settings-gallery-group-by-year = År
settings-gallery-group-by-event = Begivenhed
settings-gallery-group-by-folder = Mappe
settings-gallery-group-by-camera = Kamera
settings-gallery-group-by-format = Format
settings-gallery-group-by-rating = Bedømmelse
settings-gallery-group-by-target = Objekt
settings-gallery-group-by-filter = Filter
settings-gallery-per-folder-view = Husk visningen for hver mappe
settings-gallery-per-folder-view-help = Layout, størrelse, sortering og gruppering gemmes pr. mappe; ellers bruges én visning overalt.
settings-gallery-open-folders-in-gallery = Åbn mapper i galleriet
settings-gallery-open-folders-in-gallery-help = Når du åbner en mappe, vises dens miniaturer først i stedet for det første billede.
settings-gallery-escape-opens-gallery = Esc åbner galleriet
settings-gallery-escape-opens-gallery-help = Når der ikke er andet at lukke, går Esc i fremviseren til galleriet i stedet for ikke at gøre noget.
settings-gallery-wrap-around = Piletasterne går rundt
settings-gallery-wrap-around-help = Ved slutningen af listen går piletasterne videre til begyndelsen og fra begyndelsen til slutningen. Holder du en tast nede, stopper den stadig ved slutningen.
settings-gallery-show-hidden = Vis skjulte filer
settings-gallery-show-hidden-help = Medtag filer og mapper, hvis navne starter med et punktum.
settings-gallery-show-other-files = Vis andre filer
settings-gallery-show-other-files-help = Medtag filer, som { -app-name } ikke kan åbne, vist med et generisk ikon.
settings-gallery-show-archives = Vis arkiver
settings-gallery-show-archives-help = Medtag tegneserier og andre arkiver (zip, cbz, rar, 7z…) som felter. Arkiver kan stadig åbnes direkte.
settings-gallery-subfolder-depth = Undermappeniveauer
settings-gallery-subfolder-depth-help = Vis også billeder fra undermapper, ned til så mange niveauer (0: kun denne mappe).
settings-gallery-stacks = Stak relaterede filer
settings-gallery-stacks-help = Vis RAW+JPEG-par og serier som ét felt. Tryk på S for at udvide en stak.
settings-gallery-info-overlay = Oplysninger på miniaturer
settings-gallery-info-overlay-help = Mål og eksponering over hver miniature.
settings-gallery-prefer-embedded = Foretræk indlejrede eksempler
settings-gallery-prefer-embedded-help = Brug det eksempel, der er gemt i filen, når det er stort nok. Meget hurtigere; lidt mindre nøjagtigt.
settings-gallery-sharper-raw-thumbnails = Skarpere RAW-miniaturer
settings-gallery-sharper-raw-thumbnails-help = Redigerede RAW-fotos fremkaldes ved 1024 px til deres miniaturer. Skarpere, lidt langsommere. Fra: 512 px.
settings-gallery-rating-sidecars = Skriv bedømmelser til XMP-sidevognsfiler
settings-gallery-rating-sidecars-help = Bedømmelser, etiketter og flag gemmes også i en .xmp-fil ved siden af billedet, som andre fotoprogrammer kan læse.
settings-gallery-cache-size = Størrelse på miniaturecache
settings-gallery-cache-size-help = De ældste miniaturer fjernes, når cachen er fuld.
settings-gallery-cache-location = Placering af miniaturecache
settings-gallery-cache-location-help = En mappe på et hurtigt drev fungerer bedst.
settings-gallery-clear-cache = Ryd miniaturecache
settings-gallery-clear-cache-help = Sletter de gemte miniaturer. De laves igen, når du gennemser.

## Compare
# machine-drafted, needs native review

compare-side = Side om side
compare-overlay = Overlejring
compare-difference = Forskel
compare-flicker = Blink
compare-blend = Bland
compare-kind-absolute = Absolut fejl
compare-kind-squared = Kvadratisk fejl
compare-kind-relative = Relativ fejl
compare-scale-down = Mindre følsom
compare-scale-up = Mere følsom
compare-scale = Fuld skala { $value }
compare-space = Lineært lys, som vist
compare-space-help = Fejl og målinger måles i lineært lys, som billedet vises, efter tonemapping, pr. kanal inklusive alfa.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = samplet ved { $width } × { $height }
compare-aligned = Justeret øverst til venstre, { $width } × { $height } sammenlignet
compare-vs = { $b } mod { $a }
compare-overlay-label = { $b } over { $a } ved { PERCENT($percent) }
compare-pending = Måler…
# machine-drafted, needs native review
compare-diff-failed = Kan ikke vise forskellen: { $reason }
compare-no-overlap = Intet at sammenligne i visningen
compare-zoom-fit = Tilpas
compare-zoom = { PERCENT($percent) }
compare-showing = Viser { $name }
compare-a11y-group = Sammenlign
compare-a11y-blend = Bland mellem billederne
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Sammenlign side om side
gallery-palette-compare-overlay = Sammenlign som overlejring
gallery-palette-compare-difference = Sammenlign forskel: { $kind }
gallery-palette-compare-flicker = Blink mellem sammenlignede billeder
gallery-palette-compare-actual = Sammenlign ved { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Mellemrum eller Ctrl+Enter vælger flere

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Farveprofil
gallery-facet-date = Optagelsesdato
gallery-facet-picked = Udvalgte
gallery-facet-rejected = Afviste
gallery-facet-flagged = Udvalgte eller afviste
gallery-facet-rated = Bedømte
gallery-facet-labelled = Med etiket

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Bitdybde
gallery-facet-pages = Sider
gallery-facet-width = Bredde
gallery-facet-height = Højde
gallery-facet-megapixels = Megapixel
gallery-picker-bits = { $bits }-bit
gallery-picker-pages = { $pages ->
    [one] { $pages } side
   *[other] { $pages } sider
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = Alle i en gruppe
gallery-picker-group-keep = Foreslået at beholde
gallery-picker-group-discard = Foreslået at kassere
gallery-picker-ops = Sammenlign
gallery-picker-op-eq = Lig med
gallery-picker-op-ge = Mindst
gallery-picker-op-le = Højst
gallery-picker-ge = mindst { $value }
gallery-picker-gt = mere end { $value }
gallery-picker-le = højst { $value }
gallery-picker-lt = mindre end { $value }

# Map view.
gallery-facet-place = Sted
gallery-palette-map = Kort
map-title = Kort
map-summary = { $count ->
    [one] { $count } foto med sted
   *[other] { $count } fotos med sted
}
map-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } fotos
}
map-zoom-in = Zoom ind
map-zoom-out = Zoom ud
map-show-all = Vis alle
map-done = Færdig
map-credit = Kortdata: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Kort: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Onlinekortfliser
map-basemap-consent-server = Kortfliser kommer fra { $host }.
map-basemap-consent-sees = Serveren ser din internetadresse og de områder, du ser på.
map-basemap-consent-private = Dine fotos, deres navne og deres steder sendes ikke.
map-basemap-consent-cache = Fliser gemmes på denne computer, op til { $size }.
map-basemap-allow = Brug onlinekort
map-basemap-decline = Behold offlinekort
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Baggrundskort vises ikke: { $reason }
map-basemap-bad-address = fliseserverens adresse skal indeholde {"{"}z{"}"}, {"{"}x{"}"} og {"{"}y{"}"}.
map-basemap-unknown-tiles = kortfilens fliser er af en ukendt type.
map-loading = Læser fotosteder…
map-empty = Ingen af disse fotos har et sted
map-hidden = Fotosteder er skjult (Indstillinger › Metadata og privatliv)
map-a11y-group = Kort over fotosteder
map-a11y-status = { $count ->
    [one] { $count } sted vist, zoomniveau { $zoom }
   *[other] { $count } steder vist, zoomniveau { $zoom }
}
