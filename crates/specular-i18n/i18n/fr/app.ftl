# machine-drafted, needs native review
# Specular — interface de la visionneuse, palette de commandes, erreurs, À propos. Français.

-app-name = Specular

## Infobulles et libellés d’accessibilité de la barre d’outils

toolbar-open = Ouvrir une image…
toolbar-prev = Image précédente
toolbar-next = Image suivante
toolbar-back-gallery = Retour à la galerie
toolbar-zoom-out = Zoom arrière
toolbar-zoom-in = Zoom avant
toolbar-zoom-level = Niveau de zoom et préréglages
toolbar-zoom-fit = Ajuster à la fenêtre
toolbar-zoom-100 = Taille réelle ({ PERCENT(100) })
toolbar-rotate-left = Faire pivoter à gauche
toolbar-rotate-right = Faire pivoter à droite
toolbar-fullscreen-enter = Passer en plein écran
toolbar-fullscreen-exit = Quitter le plein écran
toolbar-info = Informations sur l’image
toolbar-settings = Paramètres
toolbar-delete = Placer dans la corbeille
toolbar-copy = Copier l’image
toolbar-palette = Palette de commandes
toolbar-go-to = Aller à l’image…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Personnaliser la barre d’outils…
toolbar-more = Plus
toolbar-announce-moved = { $item } : position { $index } sur { $count }
toolbar-announce-added = { $item } ajouté : position { $index } sur { $count }
toolbar-announce-removed = { $item } retiré de la barre d’outils
toolbar-announce-reset = Barre d’outils réinitialisée
tooltip-with-shortcut = { $label } ({ $shortcut })

## Indicateurs

readout-position = { $index } / { $count }
readout-position-a11y = Image { $index } sur { $count }
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Ajusté ({ $zoom })
zoom-preset-fit = Ajuster à la fenêtre
zoom-preset-fill = Remplir la fenêtre
zoom-preset-actual = Taille réelle
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Plein écran
zoom-preset-percent = { $zoom }
readout-dimensions = { $width } × { $height }
status-image-count =
    { $count ->
        [one] { $count } image
        [many] { $count } images
       *[other] { $count } images
    }

## Palette de commandes

palette-placeholder = Saisissez une commande ou un paramètre…
palette-no-results = Aucune commande correspondante
palette-set-setting = Définir : { $setting } → { $value }
palette-next = Image suivante
palette-prev = Image précédente
palette-first = Première image
palette-last = Dernière image
palette-go-to = Aller à l’image…
palette-zoom-in = Zoom avant
palette-zoom-out = Zoom arrière
palette-zoom-fit = Ajuster à la fenêtre
palette-zoom-fill = Remplir la fenêtre
# machine-drafted, needs native review
palette-zoom-fit-width = Ajuster à la largeur
palette-zoom-fit-height = Ajuster à la hauteur
palette-zoom-100 = Zoom à { PERCENT(100) }
palette-zoom-200 = Zoom à { PERCENT(200) }
palette-zoom-300 = Zoom à { PERCENT(300) }
palette-zoom-400 = Zoom à { PERCENT(400) }
palette-zoom-rectangle = Zoomer sur la sélection
palette-rotate-left = Faire pivoter à gauche
palette-rotate-right = Faire pivoter à droite
palette-flip-horizontal = Retourner horizontalement
palette-flip-vertical = Retourner verticalement
palette-toggle-fullscreen = Activer ou désactiver le plein écran
palette-navigator = Afficher ou masquer le navigateur
palette-loupe = Loupe
palette-pixel-grid = Afficher ou masquer la grille de pixels
palette-info-panel = Afficher ou masquer le panneau d’informations
palette-copy-image = Copier l’image
# machine-drafted, needs native review
palette-read-codes = Lire les codes
# machine-drafted, needs native review
palette-find-text = Trouver du texte
# machine-drafted, needs native review
palette-face-boxes = Afficher les visages
palette-copy-path = Copier le chemin du fichier
palette-move-to-trash = Placer dans la corbeille
palette-undo-trash = Annuler le placement dans la corbeille
palette-open = Ouvrir une image…
# machine-drafted, needs native review
palette-open-address = Ouvrir une adresse…
palette-settings = Paramètres
# machine-drafted, needs native review
palette-customise-toolbar = Personnaliser la barre d’outils…
palette-cheat-sheet = Raccourcis clavier
palette-hud = Afficher ou masquer le HUD des temps d’image
palette-language = Langue
palette-theme = Thème
# machine-drafted, needs native review
palette-interface-larger = Interface plus grande
palette-interface-smaller = Interface plus petite
palette-interface-reset = Interface à { PERCENT(100) }
palette-quit = Quitter { -app-name }

## Aide-mémoire

cheatsheet-title = Raccourcis clavier
cheatsheet-navigation = Navigation
cheatsheet-zoom-pan = Zoom et déplacement
cheatsheet-view = Présentation
cheatsheet-file = Fichier
cheatsheet-window = Fenêtre
cheatsheet-help = Aide
cheatsheet-hold = Maintenir { $key }
cheatsheet-close-hint = Appuyez sur { $key } ou Échap pour fermer

## Erreurs

# machine-drafted, needs native review
viewer-no-preview = Aucun aperçu dans ce fichier.
error-cannot-open = Impossible d’ouvrir { $file }
error-cannot-open-reason = Impossible d’ouvrir { $file } : { $reason }
error-unsupported-format = { $file } est dans un format que { -app-name } ne sait pas encore lire.
error-file-truncated = { $file } est incomplet. Seule la partie lisible est affichée.
error-file-too-large = { $file } est trop volumineux pour être ouvert ({ $megapixels } mégapixels ; la limite est de { $limit } mégapixels).
error-not-found = { $file } n’existe plus.
error-permission-denied = Vous n’avez pas l’autorisation de lire { $file }.
error-details = Détails
crash-last-time = { -app-name } s’est fermé de manière inattendue la dernière fois ; un rapport a été enregistré dans { $path }
error-trash-failed = Impossible de placer { $file } dans la corbeille : { $reason }
# machine-drafted, needs native review
# Raisons, affichées après « Impossible d’ouvrir FICHIER : » et d’autres messages « Impossible de … : »,
# elles commencent donc par une minuscule. Elles disent ce qui s’est passé et, si utile, quoi faire.
error-reason-not-found = il n’existe plus. Il a peut-être été déplacé, renommé ou supprimé.
error-reason-permission = vous n’avez pas l’autorisation de le lire. Vérifiez son propriétaire et ses autorisations.
error-reason-disk-full = le disque est plein. Libérez de l’espace et réessayez.
error-reason-read-only = le disque est en lecture seule.
error-reason-truncated = le fichier est incomplet. Il est peut-être encore en cours de copie ou de téléchargement.
error-reason-is-folder = c’est un dossier, pas un fichier.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = le serveur a refusé l’accès. Les données sont peut-être privées ; pour un compartiment S3, configurez d’abord vos identifiants AWS.
error-reason-remote-missing = il n’y a pas de données Zarr à cette adresse.
# $status is an HTTP status code ("500").
error-reason-remote-status = le serveur a répondu par l’erreur { $status }.
error-reason-offline = le serveur est injoignable. Vérifiez l’adresse et la connexion.
# machine-drafted, needs native review
error-reason-network-off = le réseau est désactivé dans Paramètres › Confidentialité.
error-reason-timed-out = le lecteur ou le serveur n’a pas répondu à temps. Vérifiez la connexion et réessayez.
error-reason-memory = la mémoire est insuffisante. Fermez d’autres images ou applications et réessayez.
error-reason-unsupported = il est dans un format que { -app-name } ne sait pas encore lire.
# $detail est le texte du décodeur, technique et en anglais (« 12-bit lossless JPEG »).
error-reason-unsupported-detail = il utilise une fonctionnalité que { -app-name } ne sait pas encore lire ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = il ne correspond à aucun des fichiers .{ $ext } que { -app-name } sait lire ({ $formats }).
error-reason-damaged = le fichier est endommagé ou n’est pas ce qu’indique son nom ({ $detail }).
error-reason-too-large = il est trop volumineux pour être ouvert.
# machine-drafted, needs native review
error-reason-needs-memory = il nécessite { $needed } de RAM libre, mais seuls { $free } sont libres pour le moment. Fermez d’autres applications et réessayez, ou ouvrez-le sur un ordinateur doté de plus de mémoire.
error-needs-memory = Cette image nécessite { $needed } de RAM libre, mais seuls { $free } sont libres pour le moment. Fermez d’autres applications et réessayez, ou ouvrez-la sur un ordinateur doté de plus de mémoire.
error-needs-memory-short = Nécessite { $needed } de RAM libre
error-reason-cancelled = son ouverture a été interrompue.
error-reason-trash-access = il n’existe plus, ou vous n’avez pas l’autorisation de le modifier.
error-reason-restore-collision = un fichier du même nom se trouve déjà dans son dossier.
error-reason-trash-root = un lecteur entier ne peut pas être placé dans la corbeille.
error-reason-not-in-trash = il n’est plus dans la corbeille.
error-reason-program-missing = le programme n’est pas installé ou a été déplacé. Choisissez-le à nouveau ou réinstallez-le.
error-reason-program-not-allowed = le programme n’est pas autorisé à s’exécuter. Vérifiez qu’il est exécutable.
# machine-drafted, needs native review
error-reason-permission-write = vous n’avez pas l’autorisation de le modifier. Vérifiez son propriétaire et ses autorisations.
error-reason-file-has-errors = il contient des erreurs. Corrigez-les d’abord, pour que rien de son contenu ne soit perdu.
error-reason-not-dicom = ce n’est pas un fichier DICOM.
error-reason-no-image = il ne contient aucune image.
error-reason-needs-layout = c’est un volume sans en-tête. Sa taille, son type d’échantillon et son ordre des octets sont nécessaires pour l’ouvrir.
error-reason-not-archive = ce n’est pas une archive que { -app-name } sait lire.
error-reason-needs-sandbox = ce type d’archive ne s’ouvre qu’avec le lecteur protégé de { -app-name }, qui n’est pas disponible.
error-reason-encrypted = il est chiffré et nécessite son mot de passe.
error-reason-wrong-password = le mot de passe est incorrect.
error-reason-checksum = le fichier est endommagé : son contenu ne correspond pas à sa somme de contrôle.
error-reason-missing-entry = un fichier qu’il contient est manquant.
error-reason-busy = un autre programme l’utilise. Fermez-le dans ce programme et réessayez.
error-reason-catalogue-damaged = le fichier du catalogue est endommagé.
error-reason-cannot-open = son fichier ne peut pas être ouvert. Vérifiez que son dossier existe et que vous pouvez y écrire.
error-reason-newer-version = il a été écrit par une version plus récente de { -app-name }. Mettez à jour { -app-name } pour le modifier.
error-reason-internal = une erreur interne s’est produite dans { -app-name }. Le journal contient les détails.
error-reason-volume-too-few = { $count ->
    [one] { $count } coupe ne suffit pas pour construire un volume.
   *[other] { $count } coupes ne suffisent pas pour construire un volume.
}
error-reason-volume-no-geometry = une coupe n’indique pas où elle se situe ni dans quelle direction elle est orientée.
error-reason-volume-not-parallel = les coupes ne sont pas parallèles.
error-reason-volume-shared-position = plusieurs images partagent { $count ->
        [one] { $count } position
       *[other] chacune des { $count } positions
    } (phases, échos ou série temporelle).
error-reason-volume-missing = { $count ->
        [one] environ { $count } coupe est manquante.
       *[other] environ { $count } coupes sont manquantes.
    }
error-reason-volume-mismatch = les coupes diffèrent par leur taille ou leur espacement des pixels.
error-reason-volume-not-grey = les coupes sont en couleur, pas en niveaux de gris.
error-reason-volume-skewed = les axes du volume ne sont pas à angle droit.
error-reason-volume-too-large = il nécessite { $need } Mio alors que seuls { $budget } Mio sont autorisés.
error-reason-url-not-specular = ce n’est pas un lien { -app-name }.
error-reason-url-too-long = il est trop long.
error-reason-url-damaged = il est endommagé. Il a peut-être été tronqué ou modifié lors de sa copie.
error-reason-url-unknown = il demande quelque chose que cette version de { -app-name } ne connaît pas. Mettre à jour { -app-name } peut aider.
error-reason-url-not-absolute = ses chemins de fichier doivent être des chemins complets.
error-reason-url-no-paths = il ne désigne aucun fichier.
error-reason-url-too-many = il désigne trop de fichiers.

## Notifications

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Impossible de terminer : { $reason }
toast-moved-to-trash = Placé dans la corbeille
toast-moved-to-trash-named = { $file } a été placé dans la corbeille
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } image placée dans la corbeille
        [many] { $count } images placées dans la corbeille
       *[other] { $count } images placées dans la corbeille
    }
toast-undo = Annuler
toast-restored = { $file } a été restauré
toast-copied-image = Image copiée
toast-copied-path = Chemin copié
toast-copied-report = Rapport copié
toast-copied-value = Valeur copiée
toast-loop-start = Retour à la première image
toast-loop-end = Passage à la dernière image
# machine-drafted, needs native review
toast-interface-size = Taille de l’interface { PERCENT($percent) }
toast-language-changed = Langue changée en { $language }

## États vides

empty-drop = Déposez une image ici ou appuyez sur { $shortcut }
empty-open-button = Ouvrir une image…
empty-drop-release = Relâchez pour ouvrir
empty-folder = Ce dossier ne contient aucune image que { -app-name } peut ouvrir.
# machine-drafted, needs native review
# Sous « Ouvrir une image… » dans la fenêtre vide : où trouver tout le reste.
empty-hint-keys = Appuyez sur { $cheatsheet } pour les raccourcis clavier, ou sur { $palette } pour trouver n’importe quelle commande.
empty-hint-cheatsheet = Appuyez sur { $cheatsheet } pour les raccourcis clavier.
empty-hint-palette = Appuyez sur { $palette } pour trouver n’importe quelle commande.
# Sous l’indicateur d’activité quand un fichier met un moment à s’ouvrir.
viewer-opening = Ouverture de { $file }…
fullscreen-exit-hint = Appuyez sur Échap pour quitter le plein écran

## Choix de la langue

language-picker-title = Langue
# machine-drafted, needs native review
language-picker-search = Rechercher une langue
language-completeness = Traduit à { $percent }
language-system-default = Langue du système ({ $language })
language-pseudo-accented = Pseudo-langue : accentuée et plus longue
language-pseudo-rtl = Pseudo-langue : en miroir, de droite à gauche

## Fenêtre À propos

about-title = À propos de { -app-name }
about-version = Version { $version }
about-tagline = Une visionneuse d’images rapide et fidèle.
about-copyright = © { $year } Les auteurs de { -app-name }
about-license = Distribué sous la licence MIT License / Apache License 2.0.
about-geonames = Noms de lieux issus de GeoNames (geonames.org), sous licence Creative Commons Attribution 4.0.
about-unicode = Données régionales issues du CLDR Unicode via ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Profils d’objectifs issus de la base Lensfun (lensfun.github.io), sous licence Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Licences tierces
about-copy-info = Copier les informations système
about-close = Fermer

## Interface du visionneur (étape 1)
# machine-drafted, needs native review
app-close = Fermer
app-truncated-badge = Fichier incomplet
app-info-summary = Résumé
app-info-all-fields = Tous les champs
app-info-copy-value = Copier la valeur
app-info-copy-field = Copier la valeur de { $field }
app-info-loading = Lecture des métadonnées…
# machine-drafted, needs native review
# Tous les champs, pour un fichier sans métadonnées au-delà de ses pixels.
app-info-no-fields = Ce fichier ne contient aucun champ de métadonnées.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = Carte de gain HDR
app-info-gain-map-headroom = Jusqu’à { $headroom }× la luminosité SDR
app-copy-failed = Impossible de copier : { $reason }
app-restore-failed = Impossible de restaurer { $file } : { $reason }
app-open-dialog-title = Ouvrir une image
app-open-dialog-filter = Fichiers pris en charge
app-open-dialog-all = Tous les fichiers
app-open-group-images = Images
app-open-group-raw = Fichiers RAW
app-open-group-video = Vidéo
app-open-group-sound = Son
app-open-group-documents = Documents
app-open-group-drawings = Dessins et CAO
app-open-group-models = Modèles 3D
app-open-group-maps = Cartes et traces
app-open-group-science = Science et médecine
app-open-group-fonts = Polices
app-open-group-other = Autres

## Informations sur le moteur de rendu (À propos, Paramètres › Avancé)

# machine-drafted, needs native review
renderer-title = Informations sur le moteur de rendu
renderer-section-adapter = Carte graphique
renderer-section-limits = Limites et fonctionnalités
renderer-section-display = Affichage
renderer-section-memory = Mémoire
renderer-section-decoders = Décodeurs optionnels
renderer-unknown = Inconnu
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ID du fabricant et du périphérique
# Wayland, X11, Windows or macOS.
renderer-display-server = Serveur d’affichage
renderer-features = Fonctionnalités utilisées
renderer-surface-format = Format de la fenêtre
renderer-surface-formats = Formats proposés
renderer-present-mode = Mode de présentation
renderer-present-modes = Modes proposés
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Activé (marge de { $headroom }×)
renderer-hdr-off = Désactivé (l’écran peut afficher le HDR)
renderer-hdr-unavailable = Non disponible sur cet écran
renderer-decode-budget = Le décodage peut utiliser
renderer-decode-budget-detail = { PERCENT(80) } de la mémoire libre, partagés entre toutes les images en cours d’ouverture.
renderer-display-cache = Cache d’affichage

## HUD de performances (F12)
# machine-drafted, needs native review
app-hud-scale = Échelle
app-hud-level = Niveau
app-hud-level-thumbnail = Miniature intégrée
app-hud-level-full = Pleine résolution
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Ouverture
app-hud-decode = Décodage
app-hud-upload = Envoi au GPU
app-hud-frame = Image
app-hud-threads = Threads
app-hud-threads-value = { $threads } ({ $queued } en attente)
app-hud-tiles = Tuiles
app-hud-tiles-value = { $tiles } sur { $capacity }
app-hud-ram = Mémoire
app-hud-vram = Mémoire GPU
app-hud-gpu = GPU

# Trash confirmation (machine-drafted, needs native review)
app-confirm-trash-title = Mettre { $file } à la corbeille ?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Ses fichiers annexes vont aussi à la corbeille : { $files }
app-cancel = Annuler

# Stage 1 merge (machine-drafted, needs native review)
disabled-no-image = Aucune image ouverte
disabled-still-loading = L’image est encore en cours de chargement
disabled-decode-failed = Impossible de décoder cette image
disabled-no-other-images = Aucune autre image dans ce dossier
tooltip-unavailable = { $label } (indisponible : { $reason })
error-decoder-stopped = le décodeur s’est arrêté sans produire d’image
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = il faut une surface cérébrale (un fichier .surf.gii de { $vertices } sommets) dans le même dossier
fullscreen-back-hint = Appuyez sur Échap pour réinitialiser le zoom ou quitter le plein écran
action-back = Fermer le panneau (dans une boîte de dialogue, revenir d’abord d’une étape), réinitialiser le zoom, quitter le plein écran ; dans la galerie, remonter d’un dossier
about-third-party-file = { -app-name } inclut des logiciels et des données de tiers soumis à leurs propres licences ; voir THIRD-PARTY.md, installé avec { -app-name }.
toast-low-memory = Cette image peut nécessiter environ { $need } de mémoire, mais seulement { $available } sont disponibles. Elle risque de se charger lentement ou d’échouer.

hw-report-title = Rapport matériel
hw-report-copy = Copier le rapport
hw-report-version = Version
hw-report-cpu = CPU
hw-report-cores = Cœurs
hw-report-cores-value = { $logical } logiques, { $physical } physiques
hw-report-performance-cores = Cœurs performance
hw-report-cpu-features = Accélérations CPU
hw-report-none = Aucune
hw-report-memory = Mémoire
hw-report-memory-value = { $total } au total, { $available } disponibles
hw-report-graphics-api = API graphique
hw-report-driver = Pilote
hw-report-max-texture = Texture maximale
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Threads de décodage
hw-report-ram-cache = Cache d’images en mémoire
hw-report-vram-cache = Cache de mémoire GPU
hw-report-processing = Traitement
hw-report-path-jpeg-decode = Décodage JPEG
hw-report-path-ycbcr-to-rgb = Conversion des couleurs
hw-report-path-png-unfilter = Décodage PNG
hw-report-path-heic-decode = Décodage HEIC
hw-report-path-inference = Analyse d’image
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, en parallèle
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = décodeur matériel
hw-report-engine-npu = NPU
# machine-drafted, needs native review
hw-report-build-debug = version de débogage
hw-report-build-release = version de publication

## Accessibility (screen readers)
# The image as a screen reader announces it: file name and size in pixels.
a11y-image = { $name }, { $width } × { $height } pixels
# The region where short notifications (toasts) appear.
a11y-notifications = Notifications
# machine-drafted, needs native review
a11y-transport-playback = Lecture
a11y-transport-pages = Pages
a11y-settings-sections = Sections

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Codes dans cette image
codes-looking = Recherche de codes…
codes-none = Aucun code trouvé dans cette image.
codes-failed = Impossible de lire l’image : { $reason }
codes-copy = Copier « { $text } »
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Ouvrir le lien vers { $host }…
codes-network-off = Aucun réseau n’est activé, les liens ne s’ouvrent donc pas.
codes-open-title = Ouvrir un lien vers { $host } ?
codes-open-detail = Il s’ouvre dans votre navigateur web. N’ouvrez que les liens de confiance.
codes-open = Ouvrir dans le navigateur
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Texte dans cette image
# machine-drafted, needs native review
ocr-looking = Lecture du texte…
# machine-drafted, needs native review
ocr-none = Aucun texte trouvé dans cette image.
# machine-drafted, needs native review
ocr-failed = Impossible de lire le texte : { $reason }
# machine-drafted, needs native review
ocr-needs-pack = « Trouver du texte » nécessite les modèles de texte du pack d’IA locale (Paramètres › IA locale).
# machine-drafted, needs native review
ocr-loading = Chargement des modèles de texte…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Copier « { $text } »
# machine-drafted, needs native review
ocr-copy-all = Copier tout le texte
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Fiabilité : { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Système
hw-report-section-gpu = GPU
hw-report-section-decoding = Décodage
hw-report-operating-system = Système d’exploitation
hw-report-adapter = Adaptateur
hw-report-gpu-type = Type
hw-report-gpu-init = Temps de démarrage
hw-report-max-layers = Couches de texture
hw-report-compression = Compression de texture
hw-report-unified-memory = Mémoire partagée avec le CPU
hw-report-display = Écran
hw-report-hw-decode = Décodage matériel
hw-report-core-cluster = Groupe de cœurs
hw-report-power = Alimentation
hw-report-calibration = Mesures de vitesse
hw-report-codec-others = Autres
hw-report-performance-cores-value = { $count } sur { $total }
hw-report-cluster-value = { $name } : { $physical } physiques, { $logical } logiques
hw-report-thermal-nominal = Température normale
hw-report-thermal-fair = Chaud
hw-report-thermal-serious = Très chaud, ralenti
hw-report-thermal-critical = Surchauffe
hw-report-power-battery = Sur batterie ({ $percent })
hw-report-power-battery-unknown = Sur batterie
hw-report-power-mains = Sur secteur
hw-report-power-mains-percent = Sur secteur ({ $percent })
hw-report-power-saving = Économie d’énergie activée
hw-report-gpu-discrete = Dédié
hw-report-gpu-integrated = Intégré
hw-report-gpu-virtual = Virtuel
hw-report-gpu-software = Logiciel, sur le CPU
hw-report-gpu-other = Autre
hw-report-gpu-unavailable = Non disponible
hw-report-ms = { $ms } ms
hw-report-yes = Oui
hw-report-no = Non
hw-report-unified-platform-differs = Le système indique le contraire
hw-report-display-value = { $fps } Hz, marge HDR { $now } actuellement, jusqu’à { $max }
hw-report-hw-decode-value = { $api } sur { $device }
hw-report-hw-decode-checking = Vérification en cours…
hw-report-codec-up-to = Jusqu’à { $width } × { $height }
hw-report-codec-none = Non disponible
hw-report-codec-h264-encode = Encodage H.264
hw-report-supported = pris en charge : { $list }
hw-report-unsupported = non pris en charge : { $list }
hw-report-calibration-done = Mesuré sur cet ordinateur
hw-report-calibration-pending = Pas encore mesuré
hw-report-paths-power-saving = Économie d’énergie : les voies qui consomment moins sont privilégiées
hw-report-paths-caption = Voies pour une image de { $mpx } mégapixels
hw-report-col-task = Tâche
hw-report-col-path = Voie
hw-report-col-time = Durée
hw-report-col-cpu = Temps CPU (tous les threads)
hw-report-col-runs = Mesures
hw-report-col-use = Utilisée pour
hw-report-runs-untested = Non testé
hw-report-use-viewing = Affichage ({ $reason })
hw-report-use-background = Arrière-plan ({ $reason })
hw-report-reason-only = seule option
hw-report-reason-faster = la plus rapide
hw-report-reason-cheaper = moins de temps CPU
hw-report-reason-lower-power = moins d’énergie
hw-report-reason-explore = en essai
hw-report-reason-forced = défini dans les Paramètres
hw-report-reason-prefer-gpu = Préférer le GPU
hw-report-reason-cpu-only = CPU uniquement
hw-report-reason-calibrate = en cours de mesure
hw-report-reason-scaled = taille réduite
hw-report-trust-unverified = Pas encore vérifié
hw-report-trust-blocked = Désactivé après une erreur
hw-report-trust-absent = Absent de cet ordinateur

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galerie
cheatsheet-mode-compare = Comparer
cheatsheet-gallery-browse = Parcourir
cheatsheet-gallery-select = Sélectionner
cheatsheet-gallery-mark = Noter et marquer
cheatsheet-gallery-move = Passer d’une image à l’autre
cheatsheet-gallery-page = Page précédente ou suivante
cheatsheet-gallery-ends = Première ou dernière image
cheatsheet-gallery-up = Remonter au dossier parent
cheatsheet-gallery-history-back = Revenir au dossier précédent
cheatsheet-gallery-history-forward = Avancer au dossier suivant
cheatsheet-gallery-back = Fermer la vue du dessus, puis revenir
cheatsheet-gallery-viewer = Retour à la visionneuse
cheatsheet-gallery-album-add = Ajouter au dernier album
cheatsheet-gallery-select-one = Sélectionner ou désélectionner
cheatsheet-gallery-quick-look = Aperçu rapide
cheatsheet-gallery-filter = Filtrer
cheatsheet-gallery-bigger = Vignettes plus grandes
cheatsheet-gallery-smaller = Vignettes plus petites
cheatsheet-gallery-default-size = Taille de vignette standard
cheatsheet-gallery-refresh = Actualiser le dossier
cheatsheet-gallery-menu = Menu contextuel
cheatsheet-compare-views = Comparer
cheatsheet-compare-overlay = Superposition ou côte à côte
cheatsheet-compare-difference = Différence (à nouveau : type suivant)
cheatsheet-compare-step-down = Moins de fondu ou de gain de différence
cheatsheet-compare-step-up = Plus de fondu ou de gain de différence
cheatsheet-compare-pick = Image visée par les touches
cheatsheet-compare-close = Fermer la comparaison
cheatsheet-channels = Canaux
cheatsheet-medical-window = Fenêtrage
cheatsheet-medical-measure = Mesurer et annoter
cheatsheet-medical-frames = Trames et séries
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D et reconstruction
cheatsheet-medical-analysis = Segmentation, fusion et TEP
# machine-drafted, needs native review
cheatsheet-medical-studies = Études et partage
cheatsheet-palette-only-title = Dans la palette de commandes ({ $key })
cheatsheet-palette-only-title-nokey = Dans la palette de commandes
cheatsheet-palette-only-tag = Palette
cheatsheet-palette-only-line = { $action } : pas de touche ; dans la palette de commandes ({ $key })
cheatsheet-palette-only-line-nokey = { $action } : pas de touche ; dans la palette de commandes
# Échap dans l’aide-mémoire (la longue description de la palette ne tient pas).
cheatsheet-back = Fermer ou revenir en arrière

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Export vidéo
hw-report-engine-hw-encoder = encodeur matériel

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (par vignette de { $size })
hw-report-task-frame = { $task } (par image de { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Logiciel uniquement

about-natural-earth = Données cartographiques de Natural Earth (naturalearthdata.com), domaine public.
app-info-show-on-map = Afficher sur la carte
