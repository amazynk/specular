# machine-drafted, needs native review
# Animationsuppspelning och läsläge (arkiv, serietidningar). Svenska.

disabled-not-animated = Bilden är inte animerad
disabled-in-archive = Sidor i ett arkiv kan inte flyttas till papperskorgen

## Kommandopalett och lathund

palette-play-pause = Spela upp eller pausa animationen
palette-next-frame = Nästa bildruta
palette-prev-frame = Föregående bildruta
palette-faster = Spela upp snabbare
palette-slower = Spela upp långsammare
palette-normal-speed = Normal uppspelningshastighet
palette-loop-forever = Upprepa animationen i oändlighet
palette-right-to-left = Sidordning höger till vänster
# machine-drafted, needs native review
palette-contents = Innehållsförteckning
palette-spreads = Uppslag med två sidor

## Animationskontroller (fältet ovanför verktygsfältet)

# Tillgänglighetsnamn för gruppen med uppspelningskontroller.
transport-region = Animationskontroller
transport-play = Spela upp
transport-pause = Pausa
transport-prev-frame = Föregående bildruta
transport-next-frame = Nästa bildruta
transport-slower = Långsammare
transport-faster = Snabbare
# $index är den visade bildrutan, $count antalet bildrutor.
transport-frame = Bildruta { $index } / { $count }
# Antalet bildrutor är okänt tills animationen har spelats en gång.
transport-frame-unknown = Bildruta { $index }
# $speed är ett formaterat tal, t.ex. 1,5.
transport-speed = { $speed }×
transport-speed-name = Uppspelningshastighet { $speed }×
# machine-drafted, needs native review
motion-badge = Rörelse
motion-badge-live = Live
motion-play = Spela upp rörelseklippet
motion-play-live = Spela upp Live Photo
motion-stop = Tillbaka till stillbilden
toast-motion-failed = Kan inte spela upp rörelseklippet: { $reason }

## Aviseringar

toast-speed = Uppspelningshastighet { $speed }×
toast-loop-forever-on = Upprepas i oändlighet
toast-loop-forever-off = Upprepas enligt filen
toast-right-to-left-on = Sidor läses från höger till vänster
toast-right-to-left-off = Sidor läses från vänster till höger
toast-spreads-on = Uppslag med två sidor
toast-spreads-off = En sida i taget

## Arkiv

# Fönsterrubrik för en sida i ett arkiv.
title-archive-page = { $page } — { $archive }
# Positionsavläsning när två sidor visas.
readout-position-spread = { $first }–{ $last } / { $count }
# $format är RAR.
archive-needs-sandbox = { $format }-arkiv kan bara öppnas av den sandlådade avkodningsvärden, som inte ingår i den här versionen än.
archive-empty = Arkivet innehåller inga bilder.
archive-failed = Arkivet kunde inte öppnas: { $error }
archive-encrypted = Arkivet är krypterat. Öppna det igen för att ange lösenordet.
archive-wrong-password = Lösenordet för arkivet är fel.
password-title = Lösenord krävs
password-archive = ”{ $archive }” är krypterat.
password-label = Lösenord
password-show = Visa lösenord
password-wrong = Fel lösenord. Försök igen.
password-open = Öppna
password-cancel = Avbryt

## Inställningar

settings-viewing-animation-autoplay = Spela upp animationer automatiskt
settings-viewing-animation-autoplay-help = Starta animerade bilder (GIF, APNG, WebP…) så fort de öppnas.
settings-viewing-animation-loop = Upprepning av animationer
settings-viewing-animation-loop-help = Hur ofta animationer upprepas.
settings-viewing-animation-loop-file = Enligt filen
settings-viewing-animation-loop-forever = I oändlighet
settings-viewing-animation-loop-once = Spela upp en gång
settings-viewing-animation-short-delays = Mycket korta bildrutefördröjningar
settings-viewing-animation-short-delays-help = Bildrutor som ska visas i 10 ms eller kortare: sakta ned dem till 100 ms som webbläsare gör, eller spela upp dem som de är lagrade.
settings-viewing-animation-short-delays-browser = Som webbläsare (100 ms)
settings-viewing-animation-short-delays-as-stored = Som lagrat
settings-browsing-reading-order = Sidordning i arkiv
settings-browsing-reading-order-help = Seriearkiv läses från höger till vänster när deras ComicInfo.xml anger det (manga), om inget annat anges här.
settings-browsing-reading-order-auto = Från arkivet
settings-browsing-reading-order-left-to-right = Vänster till höger
settings-browsing-reading-order-right-to-left = Höger till vänster
settings-browsing-page-layout = Sidor i arkiv
settings-browsing-page-layout-help = Visa en sida eller två sida vid sida; omslaget visas alltid ensamt.
settings-browsing-page-layout-auto = Från arkivet
settings-browsing-page-layout-single = En sida
settings-browsing-page-layout-spreads = Uppslag med två sidor

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Nästa sida
palette-prev-page = Föregående sida
palette-first-page = Första sidan
palette-last-page = Sista sidan
disabled-one-page = Bilden har bara en sida
page-indicator = Sida { $index } av { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Lager { $index }
page-label-face = Yta { $face }
page-label-sphere = Sfärkarta
page-label-slice = Snitt { $index }
page-label-join = { $a } · { $b }
page-label-slide = Objektglas
page-label-slide-label = Etikett
page-label-slide-macro = Översikt
page-label-slide-thumbnail = Miniatyr
# machine-drafted, needs native review
page-label-thermal = Värmebild
page-label-visible = Synligt foto
# machine-drafted, needs native review
page-label-left-view = Vänster vy
# machine-drafted, needs native review
page-label-right-view = Höger vy
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } av { $count }
page-label-t = T { $index } av { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Standardband
page-label-product-bands = Alla band
page-label-product-true-colour = Sanna färger
page-label-product-false-colour = Falska färger (nära infrarött)
page-label-product-swir = Kortvågigt infrarött
page-label-product-agriculture = Jordbruk
page-label-product-scene-class = Scenklassificering
page-label-product-cloud = Molnsannolikhet
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Sida
info-page-position = Position
info-page-contents = Innehåll

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO används inte: { $reason }
ocio-builtin-config = $OCIO pekar på en inbyggd konfiguration (ocio://); spara den som en fil och välj den filen.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits }-bitars

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Sammanslagen bild
page-label-from-layers = Byggd av lager
page-label-layer-hidden = { $name } (dold)
page-label-layer-unnamed = Namnlöst lager { $index }
