# machine-drafted, needs native review
# Animaties afspelen en leesmodus (archieven, stripboeken). Nederlands.

disabled-not-animated = Deze afbeelding is niet geanimeerd
disabled-in-archive = Pagina’s in een archief kunnen niet naar de prullenbak worden verplaatst

## Opdrachtenpalet en sneltoetsenoverzicht

palette-play-pause = Animatie afspelen of pauzeren
palette-next-frame = Volgend frame
palette-prev-frame = Vorig frame
palette-faster = Sneller afspelen
palette-slower = Langzamer afspelen
palette-normal-speed = Normale afspeelsnelheid
palette-loop-forever = Animatie eindeloos herhalen
palette-right-to-left = Paginavolgorde van rechts naar links
# machine-drafted, needs native review
palette-contents = Inhoudsopgave
palette-spreads = Dubbele pagina’s

## Animatiebediening (balk boven de werkbalk)

# Toegankelijkheidsnaam van de groep afspeelknoppen.
transport-region = Animatiebediening
transport-play = Afspelen
transport-pause = Pauzeren
transport-prev-frame = Vorig frame
transport-next-frame = Volgend frame
transport-slower = Langzamer
transport-faster = Sneller
# $index is het getoonde frame, $count het aantal frames.
transport-frame = Frame { $index } / { $count }
# Het aantal frames is pas bekend als de animatie één keer is afgespeeld.
transport-frame-unknown = Frame { $index }
# $speed is een opgemaakt getal zoals 1,5.
transport-speed = { $speed }×
transport-speed-name = Afspeelsnelheid { $speed }×
# machine-drafted, needs native review
motion-badge = Beweging
motion-badge-live = Live
motion-play = Bewegingsclip afspelen
motion-play-live = Live Photo afspelen
motion-stop = Terug naar de foto
toast-motion-failed = Kan de bewegingsclip niet afspelen: { $reason }

## Meldingen

toast-speed = Afspeelsnelheid { $speed }×
toast-loop-forever-on = Eindeloos herhalen
toast-loop-forever-off = Herhalen zoals in het bestand
toast-right-to-left-on = Pagina’s van rechts naar links
toast-right-to-left-off = Pagina’s van links naar rechts
toast-spreads-on = Dubbele pagina’s
toast-spreads-off = Eén pagina tegelijk

## Archieven

# Venstertitel voor een pagina uit een archief.
title-archive-page = { $page } — { $archive }
# Positie terwijl twee pagina’s worden getoond.
readout-position-spread = { $first }–{ $last } / { $count }
# $format is RAR.
archive-needs-sandbox = { $format }-archieven kunnen alleen worden geopend door de decoderhost in de sandbox, die deze build nog niet bevat.
archive-empty = Dit archief bevat geen afbeeldingen.
archive-failed = Het archief kan niet worden geopend: { $error }
archive-encrypted = Dit archief is versleuteld. Open het opnieuw om het wachtwoord in te voeren.
archive-wrong-password = Het wachtwoord voor dit archief is onjuist.
password-title = Wachtwoord vereist
password-archive = ‘{ $archive }’ is versleuteld.
password-label = Wachtwoord
password-show = Wachtwoord tonen
password-wrong = Onjuist wachtwoord. Probeer het opnieuw.
password-open = Openen
password-cancel = Annuleren

## Instellingen

settings-viewing-animation-autoplay = Animaties automatisch afspelen
settings-viewing-animation-autoplay-help = Start geanimeerde afbeeldingen (GIF, APNG, WebP…) zodra ze openen.
settings-viewing-animation-loop = Animaties herhalen
settings-viewing-animation-loop-help = Hoe vaak animaties worden herhaald.
settings-viewing-animation-loop-file = Zoals in het bestand
settings-viewing-animation-loop-forever = Eindeloos
settings-viewing-animation-loop-once = Eén keer afspelen
settings-viewing-animation-short-delays = Zeer korte framevertragingen
settings-viewing-animation-short-delays-help = Frames van 10 ms of korter: vertraag ze tot 100 ms zoals webbrowsers doen, of speel ze af zoals opgeslagen.
settings-viewing-animation-short-delays-browser = Zoals webbrowsers (100 ms)
settings-viewing-animation-short-delays-as-stored = Zoals opgeslagen
settings-browsing-reading-order = Paginavolgorde in archieven
settings-browsing-reading-order-help = Stripboekarchieven lezen van rechts naar links als hun ComicInfo.xml dat aangeeft (manga), tenzij je het hier instelt.
settings-browsing-reading-order-auto = Uit het archief
settings-browsing-reading-order-left-to-right = Van links naar rechts
settings-browsing-reading-order-right-to-left = Van rechts naar links
settings-browsing-page-layout = Pagina’s in archieven
settings-browsing-page-layout-help = Toon één pagina of twee naast elkaar; de omslag wordt altijd los getoond.
settings-browsing-page-layout-auto = Uit het archief
settings-browsing-page-layout-single = Eén pagina
settings-browsing-page-layout-spreads = Dubbele pagina’s

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Volgende pagina
palette-prev-page = Vorige pagina
palette-first-page = Eerste pagina
palette-last-page = Laatste pagina
disabled-one-page = De afbeelding heeft maar één pagina
page-indicator = Pagina { $index } van { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Laag { $index }
page-label-face = Vlak { $face }
page-label-sphere = Sferische map
page-label-slice = Plak { $index }
page-label-join = { $a } · { $b }
page-label-slide = Objectglaasje
page-label-slide-label = Etiket
page-label-slide-macro = Overzicht
page-label-slide-thumbnail = Miniatuur
# machine-drafted, needs native review
page-label-thermal = Warmtebeeld
page-label-visible = Zichtbare foto
# machine-drafted, needs native review
page-label-left-view = Linkeraanzicht
# machine-drafted, needs native review
page-label-right-view = Rechteraanzicht
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } van { $count }
page-label-t = T { $index } van { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Standaardbanden
page-label-product-bands = Alle banden
page-label-product-true-colour = Ware kleuren
page-label-product-false-colour = Valse kleuren (nabij-infrarood)
page-label-product-swir = Kortegolfinfrarood
page-label-product-agriculture = Landbouw
page-label-product-scene-class = Scèneclassificatie
page-label-product-cloud = Wolkenwaarschijnlijkheid
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Pagina
info-page-position = Positie
info-page-contents = Inhoud

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO wordt niet gebruikt: { $reason }
ocio-builtin-config = $OCIO verwijst naar een ingebouwde configuratie (ocio://); sla die op als bestand en kies dat bestand.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits }-bits

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Samengevoegde afbeelding
page-label-from-layers = Opgebouwd uit lagen
page-label-layer-hidden = { $name } (verborgen)
page-label-layer-unnamed = Naamloze laag { $index }
