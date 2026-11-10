# machine-drafted, needs native review
# Afspilning af animationer og læsetilstand (arkiver, tegneserier). Dansk.

disabled-not-animated = Billedet er ikke animeret
disabled-in-archive = Sider i et arkiv kan ikke flyttes til papirkurven

## Kommandopalet og genvejsoversigt

palette-play-pause = Afspil eller sæt animation på pause
palette-next-frame = Næste billede i animationen
palette-prev-frame = Forrige billede i animationen
palette-faster = Afspil hurtigere
palette-slower = Afspil langsommere
palette-normal-speed = Normal afspilningshastighed
palette-loop-forever = Gentag animationen uendeligt
palette-right-to-left = Siderækkefølge fra højre mod venstre
# machine-drafted, needs native review
palette-contents = Indholdsfortegnelse
palette-spreads = Dobbeltsideopslag

## Animationsknapper (linjen over værktøjslinjen)

# Tilgængelighedsnavn for gruppen af afspilningsknapper.
transport-region = Animationsknapper
transport-play = Afspil
transport-pause = Pause
transport-prev-frame = Forrige billede
transport-next-frame = Næste billede
transport-slower = Langsommere
transport-faster = Hurtigere
# $index er det viste billede, $count antallet af billeder.
transport-frame = Billede { $index } / { $count }
# Antallet af billeder kendes først, når animationen er afspillet én gang.
transport-frame-unknown = Billede { $index }
# $speed er et formateret tal, f.eks. 1,5.
transport-speed = { $speed }×
transport-speed-name = Afspilningshastighed { $speed }×
# machine-drafted, needs native review
motion-badge = Bevægelse
motion-badge-live = Live
motion-play = Afspil bevægelsesklippet
motion-play-live = Afspil Live Photo
motion-stop = Tilbage til stillbilledet
toast-motion-failed = Kan ikke afspille bevægelsesklippet: { $reason }

## Meddelelser

toast-speed = Afspilningshastighed { $speed }×
toast-loop-forever-on = Gentages uendeligt
toast-loop-forever-off = Gentages som angivet i filen
toast-right-to-left-on = Sider læses fra højre mod venstre
toast-right-to-left-off = Sider læses fra venstre mod højre
toast-spreads-on = Dobbeltsideopslag
toast-spreads-off = Én side ad gangen

## Arkiver

# Vinduestitel for en side i et arkiv.
title-archive-page = { $page } — { $archive }
# Placeringsvisning, mens to sider vises.
readout-position-spread = { $first }–{ $last } / { $count }
# $format er RAR.
archive-needs-sandbox = { $format }-arkiver kan kun åbnes af den sandkassede afkodervært, som denne version endnu ikke indeholder.
archive-empty = Arkivet indeholder ingen billeder.
archive-failed = Arkivet kunne ikke åbnes: { $error }
archive-encrypted = Arkivet er krypteret. Åbn det igen for at indtaste adgangskoden.
archive-wrong-password = Adgangskoden til arkivet er forkert.
password-title = Adgangskode påkrævet
password-archive = ”{ $archive }” er krypteret.
password-label = Adgangskode
password-show = Vis adgangskode
password-wrong = Forkert adgangskode. Prøv igen.
password-open = Åbn
password-cancel = Annuller

## Indstillinger

settings-viewing-animation-autoplay = Afspil animationer automatisk
settings-viewing-animation-autoplay-help = Start animerede billeder (GIF, APNG, WebP…), så snart de åbnes.
settings-viewing-animation-loop = Gentagelse af animationer
settings-viewing-animation-loop-help = Hvor ofte animationer gentages.
settings-viewing-animation-loop-file = Som angivet i filen
settings-viewing-animation-loop-forever = Uendeligt
settings-viewing-animation-loop-once = Afspil én gang
settings-viewing-animation-short-delays = Meget korte billedforsinkelser
settings-viewing-animation-short-delays-help = Billeder, der skal vises i 10 ms eller mindre: sænk dem til 100 ms som webbrowsere, eller afspil dem som gemt.
settings-viewing-animation-short-delays-browser = Som webbrowsere (100 ms)
settings-viewing-animation-short-delays-as-stored = Som gemt
settings-browsing-reading-order = Siderækkefølge i arkiver
settings-browsing-reading-order-help = Tegneseriearkiver læses fra højre mod venstre, når deres ComicInfo.xml angiver det (manga), medmindre andet er angivet her.
settings-browsing-reading-order-auto = Fra arkivet
settings-browsing-reading-order-left-to-right = Venstre mod højre
settings-browsing-reading-order-right-to-left = Højre mod venstre
settings-browsing-page-layout = Sider i arkiver
settings-browsing-page-layout-help = Vis én side eller to side om side; forsiden vises altid alene.
settings-browsing-page-layout-auto = Fra arkivet
settings-browsing-page-layout-single = Én side
settings-browsing-page-layout-spreads = Dobbeltsideopslag

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Næste side
palette-prev-page = Forrige side
palette-first-page = Første side
palette-last-page = Sidste side
disabled-one-page = Billedet har kun én side
page-indicator = Side { $index } af { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Lag { $index }
page-label-face = Flade { $face }
page-label-sphere = Kuglekort
page-label-slice = Snit { $index }
page-label-join = { $a } · { $b }
page-label-slide = Objektglas
page-label-slide-label = Etiket
page-label-slide-macro = Oversigt
page-label-slide-thumbnail = Miniature
# machine-drafted, needs native review
page-label-thermal = Termisk billede
page-label-visible = Synligt foto
# machine-drafted, needs native review
page-label-left-view = Venstre visning
# machine-drafted, needs native review
page-label-right-view = Højre visning
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } af { $count }
page-label-t = T { $index } af { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Standardbånd
page-label-product-bands = Alle bånd
page-label-product-true-colour = Ægte farver
page-label-product-false-colour = Falske farver (nær-infrarød)
page-label-product-swir = Kortbølget infrarød
page-label-product-agriculture = Landbrug
page-label-product-scene-class = Sceneklassificering
page-label-product-cloud = Skysandsynlighed
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Side
info-page-position = Placering
info-page-contents = Indhold

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO bruges ikke: { $reason }
ocio-builtin-config = $OCIO peger på en indbygget konfiguration (ocio://); gem den som en fil, og vælg den fil.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits }-bit

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Samlet billede
page-label-from-layers = Bygget af lag
page-label-layer-hidden = { $name } (skjult)
page-label-layer-unnamed = Unavngivet lag { $index }
