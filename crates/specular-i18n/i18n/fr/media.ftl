# Lecture des animations et mode lecture (archives, bandes dessinées).

disabled-not-animated = Cette image n’est pas animée
disabled-in-archive = Les pages d’une archive ne peuvent pas être placées dans la corbeille

## Palette de commandes et aide-mémoire

palette-play-pause = Lire ou mettre en pause l’animation
palette-next-frame = Image suivante de l’animation
palette-prev-frame = Image précédente de l’animation
palette-faster = Lire plus vite
palette-slower = Lire plus lentement
palette-normal-speed = Vitesse de lecture normale
palette-loop-forever = Répéter l’animation indéfiniment
palette-right-to-left = Pages de droite à gauche
# machine-drafted, needs native review
palette-contents = Table des matières
palette-spreads = Doubles pages

## Commandes d’animation

transport-region = Commandes d’animation
transport-play = Lire
transport-pause = Pause
transport-prev-frame = Image précédente
transport-next-frame = Image suivante
transport-slower = Plus lent
transport-faster = Plus rapide
transport-frame = Image { $index } / { $count }
transport-frame-unknown = Image { $index }
transport-speed = { $speed }×
transport-speed-name = Vitesse de lecture { $speed }×
# machine-drafted, needs native review
motion-badge = Animée
motion-badge-live = Live
motion-play = Lire le clip animé
motion-play-live = Lire la Live Photo
motion-stop = Revenir à la photo fixe
toast-motion-failed = Impossible de lire le clip animé : { $reason }

## Notifications

toast-speed = Vitesse de lecture { $speed }×
toast-loop-forever-on = Répétition infinie
toast-loop-forever-off = Répétition selon le fichier
toast-right-to-left-on = Pages lues de droite à gauche
toast-right-to-left-off = Pages lues de gauche à droite
toast-spreads-on = Doubles pages
toast-spreads-off = Une page à la fois

## Archives

title-archive-page = { $page } — { $archive }
readout-position-spread = { $first }–{ $last } / { $count }
archive-needs-sandbox = Les archives { $format } ne peuvent être ouvertes que par le décodeur isolé, absent de cette version.
archive-empty = Cette archive ne contient aucune image.
archive-failed = Impossible d’ouvrir l’archive : { $error }
archive-encrypted = Cette archive est chiffrée. Ouvrez-la à nouveau pour saisir son mot de passe.
archive-wrong-password = Le mot de passe de cette archive est incorrect.
password-title = Mot de passe requis
password-archive = L’archive « { $archive } » est chiffrée.
password-label = Mot de passe
password-show = Afficher le mot de passe
password-wrong = Mot de passe incorrect. Réessayez.
password-open = Ouvrir
password-cancel = Annuler

## Réglages

settings-viewing-animation-autoplay = Lire les animations automatiquement
settings-viewing-animation-autoplay-help = Lancer les images animées (GIF, APNG, WebP…) dès leur ouverture.
settings-viewing-animation-loop = Répétition des animations
settings-viewing-animation-loop-help = Combien de fois les animations se répètent.
settings-viewing-animation-loop-file = Selon le fichier
settings-viewing-animation-loop-forever = Indéfiniment
settings-viewing-animation-loop-once = Une seule fois
settings-viewing-animation-short-delays = Délais d’image très courts
settings-viewing-animation-short-delays-help = Images prévues pour 10 ms ou moins : les ralentir à 100 ms comme les navigateurs web, ou les lire telles qu’enregistrées.
settings-viewing-animation-short-delays-browser = Comme les navigateurs (100 ms)
settings-viewing-animation-short-delays-as-stored = Tels qu’enregistrés
settings-browsing-reading-order = Ordre des pages dans les archives
settings-browsing-reading-order-help = Les archives de bandes dessinées se lisent de droite à gauche quand leur ComicInfo.xml l’indique (manga), sauf réglage contraire ici.
settings-browsing-reading-order-auto = Selon l’archive
settings-browsing-reading-order-left-to-right = De gauche à droite
settings-browsing-reading-order-right-to-left = De droite à gauche
settings-browsing-page-layout = Pages dans les archives
settings-browsing-page-layout-help = Afficher une page ou deux côte à côte ; la couverture est toujours seule.
settings-browsing-page-layout-auto = Selon l’archive
settings-browsing-page-layout-single = Une page
settings-browsing-page-layout-spreads = Doubles pages

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Page suivante
palette-prev-page = Page précédente
palette-first-page = Première page
palette-last-page = Dernière page
disabled-one-page = L’image n’a qu’une page
page-indicator = Page { $index } sur { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Couche { $index }
page-label-face = Face { $face }
page-label-sphere = Carte sphérique
page-label-slice = Coupe { $index }
page-label-join = { $a } · { $b }
page-label-slide = Lame
page-label-slide-label = Étiquette
page-label-slide-macro = Vue d’ensemble
page-label-slide-thumbnail = Vignette
# machine-drafted, needs native review
page-label-thermal = Image thermique
page-label-visible = Photo visible
# machine-drafted, needs native review
page-label-left-view = Vue gauche
# machine-drafted, needs native review
page-label-right-view = Vue droite
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } sur { $count }
page-label-t = T { $index } sur { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Bandes par défaut
page-label-product-bands = Toutes les bandes
page-label-product-true-colour = Couleurs naturelles
page-label-product-false-colour = Fausses couleurs (proche infrarouge)
page-label-product-swir = Infrarouge à ondes courtes
page-label-product-agriculture = Agriculture
page-label-product-scene-class = Classification de scène
page-label-product-cloud = Probabilité de nuages
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Page
info-page-position = Position
info-page-contents = Contenu

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO n’est pas utilisé : { $reason }
ocio-builtin-config = $OCIO désigne une configuration intégrée (ocio://) ; enregistrez-la dans un fichier et choisissez ce fichier.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } bits

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Image fusionnée
page-label-from-layers = Composée à partir des calques
page-label-layer-hidden = { $name } (masqué)
page-label-layer-unnamed = Calque sans nom { $index }
