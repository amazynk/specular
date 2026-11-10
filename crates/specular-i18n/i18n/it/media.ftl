# machine-drafted, needs native review
# Riproduzione delle animazioni e modalità lettura (archivi, fumetti). Italiano.

disabled-not-animated = Questa immagine non è animata
disabled-in-archive = Le pagine di un archivio non possono essere spostate nel cestino

## Tavolozza dei comandi e scorciatoie

palette-play-pause = Riproduci o metti in pausa l’animazione
palette-next-frame = Frame successivo
palette-prev-frame = Frame precedente
palette-faster = Riproduci più veloce
palette-slower = Riproduci più lento
palette-normal-speed = Velocità di riproduzione normale
palette-loop-forever = Ripeti l’animazione all’infinito
palette-right-to-left = Ordine pagine da destra a sinistra
# machine-drafted, needs native review
palette-contents = Indice
palette-spreads = Doppia pagina

## Controlli dell’animazione (barra sopra la barra degli strumenti)

# Nome di accessibilità del gruppo di controlli di riproduzione.
transport-region = Controlli dell’animazione
transport-play = Riproduci
transport-pause = Pausa
transport-prev-frame = Frame precedente
transport-next-frame = Frame successivo
transport-slower = Più lento
transport-faster = Più veloce
# $index è il frame mostrato, $count il numero di frame.
transport-frame = Frame { $index } / { $count }
# Il numero di frame non è noto finché l’animazione non è stata riprodotta una volta.
transport-frame-unknown = Frame { $index }
# $speed è un numero formattato, ad es. 1,5.
transport-speed = { $speed }×
transport-speed-name = Velocità di riproduzione { $speed }×
# machine-drafted, needs native review
motion-badge = Movimento
motion-badge-live = Live
motion-play = Riproduci il clip in movimento
motion-play-live = Riproduci la Live Photo
motion-stop = Torna alla foto statica
toast-motion-failed = Impossibile riprodurre il clip in movimento: { $reason }

## Notifiche

toast-speed = Velocità di riproduzione { $speed }×
toast-loop-forever-on = Ripetizione infinita
toast-loop-forever-off = Ripetizione come indicato nel file
toast-right-to-left-on = Pagine da destra a sinistra
toast-right-to-left-off = Pagine da sinistra a destra
toast-spreads-on = Doppia pagina
toast-spreads-off = Una pagina alla volta

## Archivi

# Titolo della finestra per una pagina di un archivio.
title-archive-page = { $page } — { $archive }
# Indicatore di posizione quando sono mostrate due pagine.
readout-position-spread = { $first }–{ $last } / { $count }
# $format è RAR.
archive-needs-sandbox = Gli archivi { $format } possono essere aperti solo dal processo di decodifica isolato (sandbox), non ancora incluso in questa build.
archive-empty = Questo archivio non contiene immagini.
archive-failed = Impossibile aprire l’archivio: { $error }
archive-encrypted = Questo archivio è cifrato. Riaprilo per inserire la password.
archive-wrong-password = La password di questo archivio è errata.
password-title = Password necessaria
password-archive = L’archivio «{ $archive }» è cifrato.
password-label = Password
password-show = Mostra password
password-wrong = Password errata. Riprova.
password-open = Apri
password-cancel = Annulla

## Impostazioni

settings-viewing-animation-autoplay = Riproduci animazioni automaticamente
settings-viewing-animation-autoplay-help = Avvia le immagini animate (GIF, APNG, WebP…) appena si aprono.
settings-viewing-animation-loop = Ripetizione delle animazioni
settings-viewing-animation-loop-help = Quante volte si ripetono le animazioni.
settings-viewing-animation-loop-file = Come indicato nel file
settings-viewing-animation-loop-forever = All’infinito
settings-viewing-animation-loop-once = Una sola volta
settings-viewing-animation-short-delays = Ritardi dei frame molto brevi
settings-viewing-animation-short-delays-help = Frame previsti per 10 ms o meno: rallentarli a 100 ms come fanno i browser web, o riprodurli come salvati.
settings-viewing-animation-short-delays-browser = Come i browser web (100 ms)
settings-viewing-animation-short-delays-as-stored = Come salvati
settings-browsing-reading-order = Ordine delle pagine negli archivi
settings-browsing-reading-order-help = Gli archivi di fumetti si leggono da destra a sinistra quando il loro ComicInfo.xml lo indica (manga), salvo diversa impostazione qui.
settings-browsing-reading-order-auto = Dall’archivio
settings-browsing-reading-order-left-to-right = Da sinistra a destra
settings-browsing-reading-order-right-to-left = Da destra a sinistra
settings-browsing-page-layout = Pagine negli archivi
settings-browsing-page-layout-help = Mostra una pagina o due affiancate; la copertina è sempre mostrata da sola.
settings-browsing-page-layout-auto = Dall’archivio
settings-browsing-page-layout-single = Una pagina
settings-browsing-page-layout-spreads = Doppia pagina

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Pagina successiva
palette-prev-page = Pagina precedente
palette-first-page = Prima pagina
palette-last-page = Ultima pagina
disabled-one-page = L’immagine ha una sola pagina
page-indicator = Pagina { $index } di { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Livello { $index }
page-label-face = Faccia { $face }
page-label-sphere = Mappa sferica
page-label-slice = Sezione { $index }
page-label-join = { $a } · { $b }
page-label-slide = Vetrino
page-label-slide-label = Etichetta
page-label-slide-macro = Panoramica
page-label-slide-thumbnail = Miniatura
# machine-drafted, needs native review
page-label-thermal = Immagine termica
page-label-visible = Foto visibile
# machine-drafted, needs native review
page-label-left-view = Vista sinistra
# machine-drafted, needs native review
page-label-right-view = Vista destra
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } di { $count }
page-label-t = T { $index } di { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Bande predefinite
page-label-product-bands = Tutte le bande
page-label-product-true-colour = Colori naturali
page-label-product-false-colour = Falsi colori (infrarosso vicino)
page-label-product-swir = Infrarosso a onde corte
page-label-product-agriculture = Agricoltura
page-label-product-scene-class = Classificazione della scena
page-label-product-cloud = Probabilità di nuvole
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Pagina
info-page-position = Posizione
info-page-contents = Contenuto

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO non è usato: { $reason }
ocio-builtin-config = $OCIO indica una configurazione integrata (ocio://); salvala come file e scegli quel file.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } bit

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Immagine unita
page-label-from-layers = Composta dai livelli
page-label-layer-hidden = { $name } (nascosto)
page-label-layer-unnamed = Livello senza nome { $index }
