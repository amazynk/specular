# machine-drafted, needs native review
# Specular — de galerij (miniatuurweergave, DESIGN §10) en Instellingen → Galerij. Nederlands.

## Palet en menu’s

palette-gallery = Galerij
gallery-palette-view = Galerijweergave…
gallery-palette-layout = Indeling
gallery-palette-sort = Sorteren op
gallery-palette-group = Groeperen op
gallery-palette-filter = Filteren…
gallery-palette-quick-filters = Snelfilters
gallery-palette-details = Detailweergave
gallery-palette-info-overlay = Info op miniaturen
gallery-palette-stacks = RAW+JPEG en reeksen stapelen
gallery-palette-subfolders = Submappen meenemen
gallery-palette-compare = Selectie vergelijken
gallery-palette-rate = Beoordelen…
gallery-palette-settings = Galerij-instellingen…
gallery-view-menu = Weergave
gallery-more-menu = Meer
gallery-open = Openen
gallery-new-window = Openen in nieuw venster
gallery-reveal = Tonen in map
gallery-properties = Eigenschappen
gallery-copy-paths = Paden kopiëren
gallery-select-all = Alles selecteren
gallery-select-none = Niets selecteren
gallery-select-invert = Selectie omkeren
gallery-next-unrated = Volgende zonder beoordeling
gallery-stack-toggle = Stapel uit- of samenvouwen
gallery-run-on = { $name } ({ $count } bestanden)
gallery-depth-off = Alleen deze map
gallery-depth-all = Alle submappen
gallery-depth-levels = { $depth ->
    [one] { $depth } niveau submappen
   *[other] { $depth } niveaus submappen
}

## Status en lege toestanden

gallery-items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
gallery-selected = { $count } van { $total } geselecteerd
gallery-shown-of = { $count } van { $total } getoond
gallery-listing = Map lezen… { $count }
gallery-empty = Deze map bevat geen afbeeldingen.
gallery-empty-filter = Geen afbeeldingen voldoen aan het filter.
gallery-list-error = Kan deze map niet lezen: { $reason }
gallery-no-folder = Geen map geopend
gallery-kind-folder = Map
gallery-drag-count = { $count ->
    [one] { $count } bestand
   *[other] { $count } bestanden
}
gallery-badge-pages = { $count } pagina’s

## Filterbalk

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Naam, of rating>=3 camera:X-T5 …
gallery-filter-clear = Filter wissen
gallery-filter-rating = { $stars ->
    [one] ★ { $stars } of meer
   *[other] ★ { $stars } of meer
}
gallery-filter-label = Label: { $label }
gallery-filter-picks = Keuzes
gallery-filter-hide-rejects = Afgewezen verbergen
gallery-filter-unrated = Zonder beoordeling
gallery-filter-raw = RAW
gallery-filter-animated = Geanimeerd
gallery-filter-hdr = HDR
gallery-filter-landscape = Liggend
gallery-filter-portrait = Staand

## Query errors and facets

gallery-query-unknown-field = Onbekend veld ‘{ $field }’
gallery-query-unknown-field-suggest = Onbekend veld ‘{ $field }’: bedoelde u ‘{ $suggestion }’?
gallery-query-missing-value = ‘{ $field }’ heeft een waarde nodig
gallery-query-bad-number = ‘{ $value }’ is geen getal ({ $field })
gallery-query-bad-date = ‘{ $value }’ is geen datum ({ $field }): gebruik 2024, 2024-05 of 2024-05-12
gallery-query-bad-word = ‘{ $value }’ past niet bij { $field }: gebruik { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = ‘{ $value }’ is geen Exif-tagnaam
gallery-query-no-compare = { $field } kan hier niet met < of > worden vergeleken
gallery-query-unterminated = Er ontbreekt een sluitend aanhalingsteken
gallery-query-unavailable = ‘{ $field }’ vereist de bibliotheek (hier niet beschikbaar)
gallery-facets = Facetten
gallery-facets-counting = Tellen…
gallery-facets-empty = Niets te tellen
gallery-facets-search = Facetten doorzoeken
gallery-facets-no-match = Geen overeenkomende waarden
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Startscherm
gallery-facet-format = Formaat
gallery-facet-camera = Camera
gallery-facet-lens = Objectief
gallery-facet-rating = Beoordeling
gallery-facet-year = Jaar
gallery-facet-orientation = Oriëntatie
gallery-facet-label = Label
gallery-facet-properties = Eigenschappen
gallery-facet-square = Vierkant
gallery-facet-alpha = Transparantie
gallery-facet-gps = Locatie
gallery-facet-multipage = Meerdere pagina’s
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } bestand
   *[other] { $count } bestanden
}

## Groepen

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Deze map
gallery-group-unknown = Onbekend
gallery-group-unrated = Zonder beoordeling

## Schiften

gallery-stars = { $stars ->
    [one] { $stars } ster
   *[other] { $stars } sterren
}
gallery-rate-none = Geen beoordeling
gallery-pick = Kiezen
gallery-reject = Afwijzen
gallery-unflag = Geen markering
gallery-label-red = Rood
gallery-label-yellow = Geel
gallery-label-green = Groen
gallery-label-blue = Blauw
gallery-label-none = Geen label
gallery-compare-hint = Selecteer 2 tot 4 afbeeldingen om ze te vergelijken.

## Bestanden

gallery-trash-count = { $count ->
    [one] { $count } bestand naar prullenbak verplaatsen
   *[other] { $count } bestanden naar prullenbak verplaatsen
}
gallery-moved-to = { $count ->
    [one] { $count } bestand verplaatst naar { $folder }
   *[other] { $count } bestanden verplaatst naar { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } bestand gekopieerd naar { $folder }
   *[other] { $count } bestanden gekopieerd naar { $folder }
}
gallery-move-failed = Kan { $file } niet verplaatsen: { $reason }
gallery-drop-move = Verplaatsen naar { $folder }
gallery-drop-copy = Kopiëren naar { $folder }
gallery-move-here = Selectie hierheen verplaatsen
gallery-copy-here = Selectie hierheen kopiëren
gallery-not-an-image = { $file } is geen afbeelding die { -app-name } kan openen.
gallery-cache-cleared = Miniaturencache gewist
# machine-drafted, needs native review
gallery-thumb-failed = Geen miniatuur: { $reason }
gallery-cache-clear-failed = Kan de miniaturencache niet wissen: { $reason }

## Eigenschappen

gallery-prop-name = Naam
gallery-prop-folder = Map
gallery-prop-size = Bestandsgrootte
gallery-prop-dimensions = Afmetingen
gallery-prop-format = Indeling
gallery-prop-date = Datum
gallery-prop-camera = Camera
gallery-prop-lens = Objectief
gallery-prop-rating = Beoordeling
gallery-prop-label = Label
gallery-prop-items = Items

## Schermlezers

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } item
   *[other] { $count } items
}
gallery-a11y-dims = { $width } bij { $height }
gallery-a11y-folder = map
gallery-a11y-up = Omhoog naar { $folder }
gallery-a11y-archive = archief
gallery-a11y-stack = stapel van { $count }
gallery-a11y-filter-bar = Filter
gallery-a11y-filter-field = Filteren op naam of veld
gallery-a11y-chip = Filterterm { $term }
gallery-a11y-chip-remove = Druk om te verwijderen
gallery-a11y-results = { $count ->
    [0] Geen overeenkomende items
    [one] { $count } overeenkomend item
   *[other] { $count } overeenkomende items
}

## Instellingen → Galerij

settings-section-gallery = Galerij
settings-gallery-layout = Indeling
settings-gallery-layout-help = Vierkanten snijden miniaturen bij tot een raster; uitgevulde rijen behouden de vorm van elke afbeelding; onbijgesneden past hele afbeeldingen in gelijke vakken.
settings-gallery-layout-squares = Vierkanten
settings-gallery-layout-justified = Uitgevulde rijen
settings-gallery-layout-uncropped = Onbijgesneden
settings-gallery-layout-details = Details
settings-gallery-thumbnail-size = Miniatuurgrootte
settings-gallery-thumbnail-size-help = Ook met Ctrl+muiswiel, knijpen, of Ctrl+plus en min in de galerij.
settings-gallery-captions = Bestandsnamen tonen
settings-gallery-captions-help = Een bijschrift onder elke miniatuur.
settings-gallery-badges = Badges tonen
settings-gallery-badges-help = Kleine markeringen voor RAW, geanimeerd, HDR, meerdere pagina’s, beoordeling en label.
settings-gallery-sort-order = Sorteren op
settings-gallery-sort-order-help = De volgorde van de galerij. Mappen komen altijd eerst.
settings-gallery-sort-order-name = Naam
settings-gallery-sort-order-date-taken = Opnamedatum
settings-gallery-sort-order-modified = Wijzigingsdatum
settings-gallery-sort-order-size = Bestandsgrootte
settings-gallery-sort-order-type = Bestandstype
settings-gallery-sort-order-dimensions = Afmetingen
settings-gallery-sort-descending = Omgekeerde volgorde
settings-gallery-sort-descending-help = Sorteren van laatste naar eerste.
settings-gallery-group-by = Groeperen op
settings-gallery-group-by-help = Sectiekoppen die bovenaan blijven terwijl je scrolt.
settings-gallery-group-by-none = Niets
settings-gallery-group-by-day = Dag
settings-gallery-group-by-month = Maand
settings-gallery-group-by-year = Jaar
settings-gallery-group-by-event = Gebeurtenis
settings-gallery-group-by-folder = Map
settings-gallery-group-by-camera = Camera
settings-gallery-group-by-format = Indeling
settings-gallery-group-by-rating = Beoordeling
settings-gallery-group-by-target = Object
settings-gallery-group-by-filter = Filter
settings-gallery-per-folder-view = Weergave per map onthouden
settings-gallery-per-folder-view-help = Indeling, grootte, sortering en groepering worden per map bewaard; anders geldt overal één weergave.
settings-gallery-open-folders-in-gallery = Mappen openen in de galerij
settings-gallery-open-folders-in-gallery-help = Begin bij het openen van een map met de miniaturen in plaats van de eerste afbeelding.
settings-gallery-escape-opens-gallery = Esc opent de galerij
settings-gallery-escape-opens-gallery-help = Als er niets anders te sluiten is, gaat Esc in de viewer naar de galerij in plaats van niets te doen.
settings-gallery-wrap-around = Pijltoetsen gaan rond
settings-gallery-wrap-around-help = Aan het eind van de lijst gaan de pijltoetsen naar het begin, en van het begin naar het eind. Een ingedrukt gehouden toets stopt nog steeds aan het eind.
settings-gallery-show-hidden = Verborgen bestanden tonen
settings-gallery-show-hidden-help = Bestanden en mappen meenemen waarvan de naam met een punt begint.
settings-gallery-show-other-files = Andere bestanden tonen
settings-gallery-show-other-files-help = Bestanden meenemen die { -app-name } niet kan openen, met een algemeen pictogram.
settings-gallery-show-archives = Archieven tonen
settings-gallery-show-archives-help = Strips en andere archieven (zip, cbz, rar, 7z…) als tegels meenemen. Een archief rechtstreeks openen werkt nog steeds.
settings-gallery-subfolder-depth = Niveaus submappen
settings-gallery-subfolder-depth-help = Toon ook afbeeldingen uit submappen, tot zoveel niveaus diep (0: alleen deze map).
settings-gallery-stacks = Verwante bestanden stapelen
settings-gallery-stacks-help = Toon RAW+JPEG-paren en reeksen als één tegel. Druk op S om een stapel uit te vouwen.
settings-gallery-info-overlay = Info op miniaturen
settings-gallery-info-overlay-help = Afmetingen en belichting over elke miniatuur.
settings-gallery-prefer-embedded = Ingesloten voorbeelden gebruiken
settings-gallery-prefer-embedded-help = Gebruik het voorbeeld uit het bestand als het groot genoeg is. Veel sneller; iets minder exact.
settings-gallery-sharper-raw-thumbnails = Scherpere RAW-miniaturen
settings-gallery-sharper-raw-thumbnails-help = Bewerkte RAW-foto’s worden voor hun miniaturen ontwikkeld op 1024 px. Scherper, iets trager. Uit: 512 px.
settings-gallery-rating-sidecars = Beoordelingen in XMP-sidecars schrijven
settings-gallery-rating-sidecars-help = Beoordelingen, labels en markeringen gaan ook in een .xmp-bestand naast de afbeelding, dat andere fotoprogramma’s lezen.
settings-gallery-cache-size = Grootte miniaturencache
settings-gallery-cache-size-help = De oudste miniaturen worden verwijderd als de cache vol is.
settings-gallery-cache-location = Locatie miniaturencache
settings-gallery-cache-location-help = Een map op een snelle schijf werkt het best.
settings-gallery-clear-cache = Miniaturencache wissen
settings-gallery-clear-cache-help = Verwijdert de opgeslagen miniaturen. Ze worden opnieuw gemaakt terwijl je bladert.

## Compare
# machine-drafted, needs native review

compare-side = Naast elkaar
compare-overlay = Overlay
compare-difference = Verschil
compare-flicker = Knipperen
compare-blend = Overvloeien
compare-kind-absolute = Absolute fout
compare-kind-squared = Kwadratische fout
compare-kind-relative = Relatieve fout
compare-scale-down = Minder gevoelig
compare-scale-up = Gevoeliger
compare-scale = Volle schaal { $value }
compare-space = Lineair licht, zoals weergegeven
compare-space-help = Fouten en metingen worden gemeten in lineair licht zoals de afbeelding wordt weergegeven, na tonemapping, per kanaal inclusief alfa.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = bemonsterd op { $width } × { $height }
compare-aligned = Linksboven uitgelijnd, { $width } × { $height } vergeleken
compare-vs = { $b } tegenover { $a }
compare-overlay-label = { $b } over { $a } op { PERCENT($percent) }
compare-pending = Meten…
# machine-drafted, needs native review
compare-diff-failed = Kan het verschil niet tonen: { $reason }
compare-no-overlap = Niets te vergelijken in beeld
compare-zoom-fit = Passend
compare-zoom = { PERCENT($percent) }
compare-showing = Toont { $name }
compare-a11y-group = Vergelijken
compare-a11y-blend = Overvloeien tussen de afbeeldingen
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Naast elkaar vergelijken
gallery-palette-compare-overlay = Als overlay vergelijken
gallery-palette-compare-difference = Verschil vergelijken: { $kind }
gallery-palette-compare-flicker = Knipperen tussen vergeleken afbeeldingen
gallery-palette-compare-actual = Vergelijken op { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Spatie of Ctrl+Enter selecteert meerdere

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Kleurprofiel
gallery-facet-date = Opnamedatum
gallery-facet-picked = Keuzes
gallery-facet-rejected = Afgewezen
gallery-facet-flagged = Gekozen of afgewezen
gallery-facet-rated = Beoordeeld
gallery-facet-labelled = Met label

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Bitdiepte
gallery-facet-pages = Pagina’s
gallery-facet-width = Breedte
gallery-facet-height = Hoogte
gallery-facet-megapixels = Megapixels
gallery-picker-bits = { $bits }-bits
gallery-picker-pages = { $pages ->
    [one] { $pages } pagina
   *[other] { $pages } pagina’s
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = Elk bestand in een groep
gallery-picker-group-keep = Voorstel: behouden
gallery-picker-group-discard = Voorstel: weggooien
gallery-picker-ops = Vergelijken
gallery-picker-op-eq = Gelijk aan
gallery-picker-op-ge = Minstens
gallery-picker-op-le = Hoogstens
gallery-picker-ge = minstens { $value }
gallery-picker-gt = meer dan { $value }
gallery-picker-le = hoogstens { $value }
gallery-picker-lt = minder dan { $value }

# Map view.
gallery-facet-place = Plaats
gallery-palette-map = Kaart
map-title = Kaart
map-summary = { $count ->
    [one] { $count } foto met locatie
   *[other] { $count } foto’s met locatie
}
map-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } foto’s
}
map-zoom-in = Inzoomen
map-zoom-out = Uitzoomen
map-show-all = Alles tonen
map-done = Klaar
map-credit = Kaartgegevens: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Kaart: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Online kaarttegels
map-basemap-consent-server = Kaarttegels komen van { $host }.
map-basemap-consent-sees = Die server ziet je internetadres en de gebieden die je bekijkt.
map-basemap-consent-private = Je foto’s, hun namen en hun locaties worden niet verstuurd.
map-basemap-consent-cache = Tegels worden op deze computer bewaard, tot { $size }.
map-basemap-allow = Online kaart gebruiken
map-basemap-decline = Offline kaart houden
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Achtergrondkaart niet getoond: { $reason }
map-basemap-bad-address = het adres van de tegelserver moet {"{"}z{"}"}, {"{"}x{"}"} en {"{"}y{"}"} bevatten.
map-basemap-unknown-tiles = de tegels van het kaartbestand zijn van een onbekende soort.
map-loading = Fotolocaties lezen…
map-empty = Geen van deze foto’s heeft een locatie
map-hidden = Fotolocaties zijn verborgen (Instellingen › Metagegevens en privacy)
map-a11y-group = Kaart van fotolocaties
map-a11y-status = { $count ->
    [one] { $count } plaats getoond, zoomniveau { $zoom }
   *[other] { $count } plaatsen getoond, zoomniveau { $zoom }
}
