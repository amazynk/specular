# machine-drafted, needs native review
# Specular — la galerie (mode vignettes, DESIGN §10) et Paramètres → Galerie. Français.

## Palette et menus

palette-gallery = Galerie
gallery-palette-view = Affichage de la galerie…
gallery-palette-layout = Disposition
gallery-palette-sort = Trier par
gallery-palette-group = Grouper par
gallery-palette-filter = Filtrer…
gallery-palette-quick-filters = Filtres rapides
gallery-palette-details = Vue détaillée
gallery-palette-info-overlay = Informations sur les vignettes
gallery-palette-stacks = Empiler RAW+JPEG et rafales
gallery-palette-subfolders = Inclure les sous-dossiers
gallery-palette-compare = Comparer la sélection
gallery-palette-rate = Noter…
gallery-palette-settings = Paramètres de la galerie…
gallery-view-menu = Affichage
gallery-more-menu = Plus
gallery-open = Ouvrir
gallery-new-window = Ouvrir dans une nouvelle fenêtre
gallery-reveal = Afficher dans le dossier
gallery-properties = Propriétés
gallery-copy-paths = Copier les chemins
gallery-select-all = Tout sélectionner
gallery-select-none = Ne rien sélectionner
gallery-select-invert = Inverser la sélection
gallery-next-unrated = Suivante non notée
gallery-stack-toggle = Développer ou réduire la pile
gallery-run-on = { $name } ({ $count } fichiers)
gallery-depth-off = Ce dossier uniquement
gallery-depth-all = Tous les sous-dossiers
gallery-depth-levels = { $depth ->
    [one] Un niveau de sous-dossiers
    [many] { $depth } niveaux de sous-dossiers
   *[other] { $depth } niveaux de sous-dossiers
}

## État et vues vides

gallery-items = { $count ->
    [one] { $count } élément
    [many] { $count } éléments
   *[other] { $count } éléments
}
gallery-selected = { $count } sur { $total } sélectionnés
gallery-shown-of = { $count } sur { $total } affichés
gallery-listing = Lecture du dossier… { $count }
gallery-empty = Ce dossier ne contient aucune image.
gallery-empty-filter = Aucune image ne correspond au filtre.
gallery-list-error = Impossible de lire ce dossier : { $reason }
gallery-no-folder = Aucun dossier ouvert
gallery-kind-folder = Dossier
gallery-drag-count = { $count ->
    [one] { $count } fichier
    [many] { $count } fichiers
   *[other] { $count } fichiers
}
gallery-badge-pages = { $count } pages

## Barre de filtre

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Nom, ou rating>=3 camera:X-T5 …
gallery-filter-clear = Effacer le filtre
gallery-filter-rating = { $stars ->
    [one] ★ 1 ou plus
    [many] ★ { $stars } ou plus
   *[other] ★ { $stars } ou plus
}
gallery-filter-label = Étiquette : { $label }
gallery-filter-picks = Retenues
gallery-filter-hide-rejects = Masquer les rejetées
gallery-filter-unrated = Non notées
gallery-filter-raw = RAW
gallery-filter-animated = Animées
gallery-filter-hdr = HDR
gallery-filter-landscape = Paysage
gallery-filter-portrait = Portrait

## Query errors and facets

gallery-query-unknown-field = Champ inconnu « { $field } »
gallery-query-unknown-field-suggest = Champ inconnu « { $field } » : vouliez-vous dire « { $suggestion } » ?
gallery-query-missing-value = « { $field } » attend une valeur
gallery-query-bad-number = « { $value } » n’est pas un nombre ({ $field })
gallery-query-bad-date = « { $value } » n’est pas une date ({ $field }) : 2024, 2024-05 ou 2024-05-12
gallery-query-bad-word = « { $value } » ne convient pas à { $field } : { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = « { $value } » n’est pas un nom de balise Exif
gallery-query-no-compare = { $field } ne se compare pas avec < ou > ici
gallery-query-unterminated = Il manque un guillemet fermant
gallery-query-unavailable = « { $field } » nécessite la Bibliothèque (indisponible ici)
gallery-facets = Facettes
gallery-facets-counting = Comptage…
gallery-facets-empty = Rien à compter
gallery-facets-search = Rechercher dans les facettes
gallery-facets-no-match = Aucune valeur ne correspond
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Écran d’accueil
gallery-facet-format = Format
gallery-facet-camera = Appareil
gallery-facet-lens = Objectif
gallery-facet-rating = Note
gallery-facet-year = Année
gallery-facet-orientation = Orientation
gallery-facet-label = Étiquette
gallery-facet-properties = Propriétés
gallery-facet-square = Carré
gallery-facet-alpha = Transparence
gallery-facet-gps = Lieu
gallery-facet-multipage = Plusieurs pages
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } fichier
   *[other] { $count } fichiers
}

## Groupes

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Ce dossier
gallery-group-unknown = Inconnu
gallery-group-unrated = Non notées

## Tri des photos

gallery-stars = { $stars ->
    [one] 1 étoile
    [many] { $stars } étoiles
   *[other] { $stars } étoiles
}
gallery-rate-none = Aucune note
gallery-pick = Retenir
gallery-reject = Rejeter
gallery-unflag = Aucun marqueur
gallery-label-red = Rouge
gallery-label-yellow = Jaune
gallery-label-green = Vert
gallery-label-blue = Bleu
gallery-label-none = Aucune étiquette
gallery-compare-hint = Sélectionnez de 2 à 4 images pour les comparer.

## Fichiers

gallery-trash-count = { $count ->
    [one] Placer { $count } fichier dans la corbeille
    [many] Placer { $count } fichiers dans la corbeille
   *[other] Placer { $count } fichiers dans la corbeille
}
gallery-moved-to = { $count ->
    [one] { $count } fichier déplacé vers { $folder }
    [many] { $count } fichiers déplacés vers { $folder }
   *[other] { $count } fichiers déplacés vers { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } fichier copié vers { $folder }
    [many] { $count } fichiers copiés vers { $folder }
   *[other] { $count } fichiers copiés vers { $folder }
}
gallery-move-failed = Impossible de déplacer { $file } : { $reason }
gallery-drop-move = Déplacer vers { $folder }
gallery-drop-copy = Copier vers { $folder }
gallery-move-here = Déplacer la sélection ici
gallery-copy-here = Copier la sélection ici
gallery-not-an-image = { $file } n’est pas une image que { -app-name } peut ouvrir.
gallery-cache-cleared = Cache des vignettes vidé
# machine-drafted, needs native review
gallery-thumb-failed = Pas de vignette : { $reason }
gallery-cache-clear-failed = Impossible de vider le cache des vignettes : { $reason }

## Propriétés

gallery-prop-name = Nom
gallery-prop-folder = Dossier
gallery-prop-size = Taille du fichier
gallery-prop-dimensions = Dimensions
gallery-prop-format = Format
gallery-prop-date = Date
gallery-prop-camera = Appareil photo
gallery-prop-lens = Objectif
gallery-prop-rating = Note
gallery-prop-label = Étiquette
gallery-prop-items = Éléments

## Lecteurs d’écran

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } élément
    [many] { $count } éléments
   *[other] { $count } éléments
}
gallery-a11y-dims = { $width } sur { $height }
gallery-a11y-folder = dossier
gallery-a11y-up = Remonté à { $folder }
gallery-a11y-archive = archive
gallery-a11y-stack = pile de { $count }
gallery-a11y-filter-bar = Filtre
gallery-a11y-filter-field = Filtrer par nom ou par champ
gallery-a11y-chip = Critère de filtre { $term }
gallery-a11y-chip-remove = Appuyez pour retirer
gallery-a11y-results = { $count ->
    [0] Aucun élément ne correspond
    [one] { $count } élément correspond
    [many] { $count } éléments correspondent
   *[other] { $count } éléments correspondent
}

## Paramètres → Galerie

settings-section-gallery = Galerie
settings-gallery-layout = Disposition
settings-gallery-layout-help = Les carrés recadrent les vignettes sur une grille ; les lignes justifiées conservent la forme de chaque image ; sans recadrage, les images entières tiennent dans des cellules égales.
settings-gallery-layout-squares = Carrés
settings-gallery-layout-justified = Lignes justifiées
settings-gallery-layout-uncropped = Sans recadrage
settings-gallery-layout-details = Détails
settings-gallery-thumbnail-size = Taille des vignettes
settings-gallery-thumbnail-size-help = Également Ctrl+molette, pincement, ou Ctrl+plus et moins dans la galerie.
settings-gallery-captions = Afficher les noms de fichiers
settings-gallery-captions-help = Une légende sous chaque vignette.
settings-gallery-badges = Afficher les badges
settings-gallery-badges-help = Petites marques pour RAW, animé, HDR, multipage, note et étiquette.
settings-gallery-sort-order = Trier par
settings-gallery-sort-order-help = L’ordre de la galerie. Les dossiers viennent toujours en premier.
settings-gallery-sort-order-name = Nom
settings-gallery-sort-order-date-taken = Date de prise de vue
settings-gallery-sort-order-modified = Date de modification
settings-gallery-sort-order-size = Taille du fichier
settings-gallery-sort-order-type = Type de fichier
settings-gallery-sort-order-dimensions = Dimensions
settings-gallery-sort-descending = Ordre inverse
settings-gallery-sort-descending-help = Trier du dernier au premier.
settings-gallery-group-by = Grouper par
settings-gallery-group-by-help = En-têtes de section qui restent en haut pendant le défilement.
settings-gallery-group-by-none = Aucun
settings-gallery-group-by-day = Jour
settings-gallery-group-by-month = Mois
settings-gallery-group-by-year = Année
settings-gallery-group-by-event = Événement
settings-gallery-group-by-folder = Dossier
settings-gallery-group-by-camera = Appareil photo
settings-gallery-group-by-format = Format
settings-gallery-group-by-rating = Note
settings-gallery-group-by-target = Cible
settings-gallery-group-by-filter = Filtre
settings-gallery-per-folder-view = Mémoriser l’affichage de chaque dossier
settings-gallery-per-folder-view-help = La disposition, la taille, le tri et le regroupement sont conservés par dossier ; sinon, un seul affichage est utilisé partout.
settings-gallery-open-folders-in-gallery = Ouvrir les dossiers dans la galerie
settings-gallery-open-folders-in-gallery-help = À l’ouverture d’un dossier, commencer par ses vignettes plutôt que par la première image.
settings-gallery-escape-opens-gallery = Échap ouvre la galerie
settings-gallery-escape-opens-gallery-help = Quand il n’y a rien d’autre à fermer, Échap dans la visionneuse passe à la galerie au lieu de ne rien faire.
settings-gallery-wrap-around = Les flèches bouclent
settings-gallery-wrap-around-help = En fin de liste, les flèches reviennent au début, et du début à la fin. Une touche maintenue enfoncée s’arrête toujours à la fin.
settings-gallery-show-hidden = Afficher les fichiers cachés
settings-gallery-show-hidden-help = Inclure les fichiers et dossiers dont le nom commence par un point.
settings-gallery-show-other-files = Afficher les autres fichiers
settings-gallery-show-other-files-help = Inclure les fichiers que { -app-name } ne sait pas ouvrir, affichés avec une icône générique.
settings-gallery-show-archives = Afficher les archives
settings-gallery-show-archives-help = Inclure les bandes dessinées et autres archives (zip, cbz, rar, 7z…) sous forme de vignettes. Ouvrir une archive directement fonctionne toujours.
settings-gallery-subfolder-depth = Niveaux de sous-dossiers
settings-gallery-subfolder-depth-help = Afficher aussi les images des sous-dossiers, jusqu’à ce nombre de niveaux (0 : ce dossier uniquement).
settings-gallery-stacks = Empiler les fichiers liés
settings-gallery-stacks-help = Afficher les paires RAW+JPEG et les rafales sous forme d’une seule vignette. Appuyez sur S pour développer une pile.
settings-gallery-info-overlay = Informations sur les vignettes
settings-gallery-info-overlay-help = Dimensions et exposition par-dessus chaque vignette.
settings-gallery-prefer-embedded = Préférer les aperçus intégrés
settings-gallery-prefer-embedded-help = Utiliser l’aperçu stocké dans le fichier s’il est assez grand. Beaucoup plus rapide ; un peu moins exact.
settings-gallery-sharper-raw-thumbnails = Vignettes RAW plus nettes
settings-gallery-sharper-raw-thumbnails-help = Les photos RAW retouchées sont développées à 1024 px pour leurs vignettes. Plus nettes, un peu plus lentes. Désactivé : 512 px.
settings-gallery-rating-sidecars = Écrire les notes dans des fichiers annexes XMP
settings-gallery-rating-sidecars-help = Les notes, étiquettes et marqueurs sont aussi enregistrés dans un fichier .xmp à côté de l’image, que lisent d’autres applications photo.
settings-gallery-cache-size = Taille du cache des vignettes
settings-gallery-cache-size-help = Les vignettes les plus anciennes sont supprimées lorsque le cache est plein.
settings-gallery-cache-location = Emplacement du cache des vignettes
settings-gallery-cache-location-help = Un dossier sur un disque rapide est préférable.
settings-gallery-clear-cache = Vider le cache des vignettes
settings-gallery-clear-cache-help = Supprime les vignettes enregistrées. Elles sont recréées au fil de la navigation.

## Compare
# machine-drafted, needs native review

compare-side = Côte à côte
compare-overlay = Superposition
compare-difference = Différence
compare-flicker = Clignotement
compare-blend = Fondu
compare-kind-absolute = Erreur absolue
compare-kind-squared = Erreur quadratique
compare-kind-relative = Erreur relative
compare-scale-down = Moins sensible
compare-scale-up = Plus sensible
compare-scale = Pleine échelle { $value }
compare-space = Lumière linéaire, telle qu’affichée
compare-space-help = Les erreurs et les mesures sont calculées en lumière linéaire, telle que l’image est affichée, après le mappage des tons, par canal, alpha compris.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = échantillonné à { $width } × { $height }
compare-aligned = Aligné en haut à gauche, { $width } × { $height } comparés
compare-vs = { $b } par rapport à { $a }
compare-overlay-label = { $b } sur { $a } à { PERCENT($percent) }
compare-pending = Mesure…
# machine-drafted, needs native review
compare-diff-failed = Impossible d’afficher la différence : { $reason }
compare-no-overlap = Rien à comparer dans la vue
compare-zoom-fit = Ajuster
compare-zoom = { PERCENT($percent) }
compare-showing = Affiche { $name }
compare-a11y-group = Comparer
compare-a11y-blend = Fondu entre les images
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Comparer côte à côte
gallery-palette-compare-overlay = Comparer en superposition
gallery-palette-compare-difference = Comparer la différence : { $kind }
gallery-palette-compare-flicker = Faire clignoter les images comparées
gallery-palette-compare-actual = Comparer à { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Espace ou Ctrl+Entrée en sélectionne plusieurs

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Profil colorimétrique
gallery-facet-date = Date de prise de vue
gallery-facet-picked = Retenues
gallery-facet-rejected = Rejetées
gallery-facet-flagged = Retenues ou rejetées
gallery-facet-rated = Notées
gallery-facet-labelled = Avec étiquette

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Profondeur de bits
gallery-facet-pages = Pages
gallery-facet-width = Largeur
gallery-facet-height = Hauteur
gallery-facet-megapixels = Mégapixels
gallery-picker-bits = { $bits } bits
gallery-picker-pages = { $pages ->
    [one] { $pages } page
   *[other] { $pages } pages
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } Mpx
gallery-picker-group-any = N’importe lequel d’un groupe
gallery-picker-group-keep = Suggestion : garder
gallery-picker-group-discard = Suggestion : écarter
gallery-picker-ops = Comparer
gallery-picker-op-eq = Égal à
gallery-picker-op-ge = Au moins
gallery-picker-op-le = Au plus
gallery-picker-ge = au moins { $value }
gallery-picker-gt = plus de { $value }
gallery-picker-le = au plus { $value }
gallery-picker-lt = moins de { $value }

# Map view.
gallery-facet-place = Lieu
gallery-palette-map = Carte
map-title = Carte
map-summary = { $count ->
    [one] { $count } photo avec un lieu
   *[other] { $count } photos avec un lieu
}
map-photos = { $count ->
    [one] { $count } photo
   *[other] { $count } photos
}
map-zoom-in = Zoom avant
map-zoom-out = Zoom arrière
map-show-all = Tout afficher
map-done = Terminé
map-credit = Données : Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Carte : { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Tuiles de carte en ligne
map-basemap-consent-server = Les tuiles de carte viennent de { $host }.
map-basemap-consent-sees = Ce serveur voit votre adresse internet et les zones que vous consultez.
map-basemap-consent-private = Vos photos, leurs noms et leurs lieux ne sont pas envoyés.
map-basemap-consent-cache = Les tuiles sont gardées sur cet ordinateur, jusqu’à { $size }.
map-basemap-allow = Utiliser la carte en ligne
map-basemap-decline = Garder la carte hors ligne
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Fond de carte non affiché : { $reason }
map-basemap-bad-address = l’adresse du serveur de tuiles doit contenir {"{"}z{"}"}, {"{"}x{"}"} et {"{"}y{"}"}.
map-basemap-unknown-tiles = les tuiles du fichier de carte sont d’un type inconnu.
map-loading = Lecture des lieux des photos…
map-empty = Aucune de ces photos n’a de lieu
map-hidden = Les lieux des photos sont masqués (Paramètres › Métadonnées et confidentialité)
map-a11y-group = Carte des lieux des photos
map-a11y-status = { $count ->
    [one] { $count } lieu affiché, niveau de zoom { $zoom }
   *[other] { $count } lieux affichés, niveau de zoom { $zoom }
}
