# machine-drafted, needs native review
# Specular — fenêtre Paramètres. Français.

## Habillage de la fenêtre

settings-title = Paramètres
settings-search-placeholder = Rechercher dans les paramètres
settings-search-no-results = Aucun paramètre ne correspond à { $query }
settings-reset = Réinitialiser
settings-reset-section = Réinitialiser la section
settings-modified = Modifié
settings-default-value = Par défaut : { $value }
settings-restart-required = Prend effet après un redémarrage
settings-restart-now = Redémarrer maintenant
settings-undo = Annuler
settings-redo = Rétablir
settings-value-on = Activé
settings-value-off = Désactivé
settings-value-auto = Automatique
settings-value-system = Selon le système
settings-value-system-language = Langue du système
settings-value-follow-language = Selon la langue de l’interface
settings-value-default-location = Emplacement par défaut
settings-choose = Choisir…
settings-clear = Effacer
settings-file-reloaded = settings.toml a été modifié en dehors de { -app-name } et a été rechargé.
settings-warning-unknown-key = Paramètre inconnu { $key } dans settings.toml. Il est conservé dans le fichier, mais ignoré.
settings-warning-invalid-value = Valeur non valide pour { $key } : { $reason } La valeur par défaut est utilisée à la place.
settings-warning-newer-version = settings.toml a été écrit par une version plus récente de { -app-name }. Les paramètres inconnus de cette version sont conservés.
settings-warning-syntax = Impossible de lire settings.toml (ligne { $line }) : { $reason } Les valeurs par défaut sont utilisées jusqu’à ce que le problème soit corrigé.

## Erreurs de validation (également affichées par `specular --set`)

settings-error-unknown-key = Il n’existe aucun paramètre nommé { $key }.
settings-error-unknown-key-suggestion = Il n’existe aucun paramètre nommé { $key }. Vouliez-vous dire { $suggestion } ?
settings-error-not-bool = Valeur attendue : on ou off (true ou false).
settings-error-not-integer = Un nombre entier est attendu.
settings-error-not-number = Un nombre est attendu.
settings-error-out-of-range = La valeur doit être comprise entre { $min } et { $max }.
settings-error-out-of-range-auto = La valeur doit être 0 (automatique) ou comprise entre { $min } et { $max }.
settings-error-unknown-option = Valeur attendue parmi : { $options }.
settings-error-bad-colour = Une couleur est attendue, par exemple #1E1E1E ou #1E1E1E80.
settings-error-bad-language = Une étiquette de langue est attendue, par exemple de, pt-BR ou zh-Hans, ou “system”.
settings-error-bad-path = Un chemin de fichier ou de dossier est attendu.
settings-error-not-settable = Cette entrée est une commande et n’a pas de valeur.
settings-error-syntax = Format attendu : clé=valeur, par exemple viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Ne peut pas contenir de tabulations, de sauts de ligne ni d’autres caractères de contrôle.
settings-error-invalid = { $reason }
settings-error-tile-url = Il faut une adresse http ou https avec {"{"}z{"}"}, {"{"}x{"}"} et {"{"}y{"}"}.

## Unités

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } Mio
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } image
        [many] { $value } images
       *[other] { $value } images
    }
unit-threads =
    { $value ->
        [one] { $value } thread
        [many] { $value } threads
       *[other] { $value } threads
    }
unit-bytes =
    { $value ->
        [one] { $value } octet
        [many] { $value } octets
       *[other] { $value } octets
    }
unit-kilobytes = { $value } ko
unit-megabytes = { $value } Mo
unit-gigabytes = { $value } Go
unit-terabytes = { $value } To
unit-kibibytes = { $value } Kio
unit-gibibytes = { $value } Gio
unit-tebibytes = { $value } Tio
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Sections

settings-section-general = Général
settings-section-viewing = Affichage
settings-section-browsing = Navigation
settings-section-colour = Couleur
settings-section-metadata = Métadonnées et confidentialité
# machine-drafted, needs native review
settings-section-privacy = Confidentialité
settings-section-performance = Performances
settings-section-accessibility = Accessibilité
settings-section-keys = Touches
settings-section-advanced = Avancé

## Général

settings-general-language = Langue de l’interface
settings-general-language-help = Les modifications prennent effet immédiatement. « Langue du système » suit la langue de votre système d’exploitation.
settings-general-region-formats = Formats régionaux
settings-general-region-formats-help = Façon d’écrire les dates, les heures, les nombres et les tailles. Par défaut, ils suivent la langue de l’interface.
settings-general-theme = Thème
settings-general-theme-help = Apparence claire ou sombre des fenêtres et des commandes. Les images ne sont jamais modifiées.
settings-general-theme-system = Selon le système
settings-general-theme-light = Clair
settings-general-theme-dark = Sombre
# machine-drafted, needs native review
settings-general-on-start = Quand { -app-name } s’ouvre sans fichier
settings-general-on-start-help = Ce qui s’affiche lorsque { -app-name } est ouvert seul. Un fichier ouvert depuis le gestionnaire de fichiers ou la ligne de commande s’affiche toujours dans la visionneuse.
settings-general-on-start-start-screen = L’écran d’accueil
settings-general-on-start-gallery = Galerie du dernier dossier
settings-general-on-start-library = Bibliothèque
settings-general-start-tips = Afficher « À essayer » sur l’écran d’accueil
settings-general-start-tips-help = Une rangée de cartes avec des fonctions utiles et leurs touches. « Afficher l’accueil » dans la palette de commandes la fait revenir.
settings-general-tips = Afficher les conseils
settings-general-tips-help = Un court conseil, une seule fois, la première fois qu’une fonction peut aider, comme la touche qui affiche un dossier en galerie. Jamais pendant un diaporama.
settings-general-tips-shown = Conseils déjà affichés
settings-general-tips-shown-help = Les conseils uniques déjà donnés, pour ne pas les répéter.
# machine-drafted, needs native review
settings-general-your-name = Votre nom
settings-general-your-name-help = Affiché sur les tampons et annotations que vous ajoutez. Enregistré uniquement dans les fichiers d’annotations que vous créez.
settings-general-single-instance = Ouvrir les fichiers dans la fenêtre existante
settings-general-single-instance-help = Lorsque { -app-name } est déjà en cours d’exécution, une autre image ouverte s’affiche dans cette fenêtre au lieu de lancer une nouvelle instance.
settings-general-file-associations = Associations de fichiers…
settings-general-file-associations-help = Faire de { -app-name } la visionneuse par défaut pour les formats d’image qu’il prend en charge.

## Affichage

settings-viewing-initial-zoom = Zoom initial
settings-viewing-initial-zoom-help = Taille d’affichage d’une image qui vient d’être ouverte.
settings-viewing-initial-zoom-fit = Ajuster à la fenêtre
settings-viewing-initial-zoom-fit-if-larger = Ajuster les grandes images, afficher les petites à { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Taille réelle ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Remplir la fenêtre
settings-viewing-wheel-behaviour = Molette de la souris
settings-viewing-wheel-behaviour-help = Action de la molette. Maintenez Ctrl pour l’autre action.
settings-viewing-wheel-behaviour-help-macos = Action de la molette. Maintenez ⌘ pour l’autre action.
settings-viewing-wheel-behaviour-zoom = Zoom
settings-viewing-wheel-behaviour-navigate = Image suivante et précédente
settings-viewing-keep-zoom = Conserver le zoom entre les images
settings-viewing-keep-zoom-help = Zoomée, l’image suivante ou précédente s’ouvre au même zoom et montre la même partie du cadre : pratique pour vérifier la netteté d’une rafale.
settings-viewing-zoom-animation = Animer le zoom
settings-viewing-zoom-animation-help = Zoomer de façon fluide plutôt que par sauts. Désactivé automatiquement lorsque « Réduire les animations » est activé.
settings-viewing-zoom-animation-speed = Vitesse de l’animation du zoom
settings-viewing-zoom-animation-speed-help = Rapidité avec laquelle les zooms animés se stabilisent.
settings-viewing-zoom-animation-speed-relaxed = Détendue
settings-viewing-zoom-animation-speed-normal = Normale
settings-viewing-zoom-animation-speed-fast = Rapide
settings-viewing-background = Arrière-plan
settings-viewing-background-help = Couleur derrière l’image dans une fenêtre.
settings-viewing-background-theme = Selon le thème
settings-viewing-background-black = Noir
settings-viewing-background-dark-grey = Gris foncé
settings-viewing-background-light-grey = Gris clair
settings-viewing-background-white = Blanc
settings-viewing-background-custom = Couleur personnalisée
settings-viewing-background-color = Couleur d’arrière-plan personnalisée
settings-viewing-background-color-help = Utilisée lorsque Arrière-plan est réglé sur « Couleur personnalisée ».
settings-viewing-fullscreen-background = Arrière-plan en plein écran
settings-viewing-fullscreen-background-help = Couleur derrière l’image en plein écran.
settings-viewing-transparency = Zones transparentes
settings-viewing-transparency-help = Façon d’afficher les parties transparentes d’une image.
settings-viewing-transparency-checkerboard = Damier
settings-viewing-transparency-background = Couleur d’arrière-plan
settings-viewing-transparency-black = Noir
settings-viewing-transparency-white = Blanc
settings-viewing-pixel-grid-threshold = Afficher la grille de pixels à partir de
settings-viewing-pixel-grid-threshold-help = Dessiner une grille entre les pixels lorsque le zoom atteint au moins ce niveau.
settings-viewing-smoothing-high-zoom = Pixels agrandis
settings-viewing-smoothing-high-zoom-help = Aspect des pixels lorsque l’image est affichée à plus de { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Lissés, puis nets à partir de { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Toujours lissés
settings-viewing-smoothing-high-zoom-pixelated = Toujours en carrés nets
settings-viewing-double-click = Double-clic sur l’image
settings-viewing-double-click-help = Un double-clic sur l’arrière-plan vide active ou désactive toujours le plein écran.
settings-viewing-double-click-toggle-zoom = Basculer entre ajusté et { PERCENT(100) }
settings-viewing-double-click-fullscreen = Activer ou désactiver le plein écran
settings-viewing-double-click-nothing = Ne rien faire
settings-viewing-fullscreen-on-open = Ouvrir les images en plein écran
settings-viewing-fullscreen-on-open-help = Démarrer en plein écran lorsqu’une image est ouverte depuis l’extérieur de { -app-name }.
settings-viewing-fullscreen-mode = Style du plein écran
settings-viewing-fullscreen-mode-help = Le plein écran natif utilise son propre Space ; le plein écran instantané recouvre le Space actuel sans animation.
settings-viewing-fullscreen-mode-native = Natif (Space dédié)
settings-viewing-fullscreen-mode-borderless = Instantané
settings-viewing-toolbar-autohide = Masquer automatiquement la barre d’outils
settings-viewing-toolbar-autohide-help = La barre d’outils et les flèches latérales s’estompent lorsque le pointeur est immobile et réapparaissent lorsqu’il bouge.
settings-viewing-toolbar-autohide-delay = Masquer la barre d’outils après
settings-viewing-toolbar-autohide-delay-help = Durée pendant laquelle le pointeur doit rester immobile avant que la barre d’outils s’estompe.
settings-viewing-toolbar-items = Boutons de la barre d’outils
settings-viewing-toolbar-items-help = Boutons affichés dans la barre d’outils, dans l’ordre.
settings-viewing-toolbar-drag = Réorganiser la barre d’outils par glisser-déposer
settings-viewing-toolbar-drag-help = Faites glisser un bouton le long de la barre d’outils pour le déplacer. Un clic bref l’active toujours, et Échap remet en place un bouton déplacé.
settings-viewing-toolbar-items-open = Ouvrir
settings-viewing-toolbar-items-prev = Image précédente
settings-viewing-toolbar-items-next = Image suivante
settings-viewing-toolbar-items-zoom-out = Zoom arrière
settings-viewing-toolbar-items-zoom-level = Niveau de zoom
settings-viewing-toolbar-items-zoom-in = Zoom avant
settings-viewing-toolbar-items-fit = Ajuster à la fenêtre
settings-viewing-toolbar-items-rotate-left = Faire pivoter à gauche
settings-viewing-toolbar-items-rotate-right = Faire pivoter à droite
settings-viewing-toolbar-items-info = Informations
settings-viewing-toolbar-items-copy = Copier l’image
settings-viewing-toolbar-items-delete = Placer dans la corbeille
settings-viewing-toolbar-items-palette = Palette de commandes
settings-viewing-toolbar-items-settings = Paramètres
settings-viewing-toolbar-items-fullscreen = Plein écran
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Espace flexible
settings-viewing-toolbar-items-separator = Séparateur
settings-toolbar-preview = Aperçu de la barre d’outils
settings-toolbar-hidden = Boutons masqués
settings-toolbar-add-spacer = Ajouter un espace flexible
settings-toolbar-add-separator = Ajouter un séparateur
settings-toolbar-reset = Rétablir les valeurs par défaut
settings-toolbar-remove = Retirer { $item }
settings-toolbar-hint = Faites glisser une ligne pour la déplacer, ou sélectionnez-la et appuyez sur Alt+↑ ou Alt+↓. Tab passe du champ de recherche à la liste.
settings-toolbar-move-up = Monter
settings-toolbar-move-down = Descendre
settings-toolbar-move-start = Placer au début
settings-toolbar-move-end = Placer à la fin
settings-toolbar-menu-hide = Masquer
settings-toolbar-menu-show = Afficher
settings-toolbar-menu-remove = Retirer
settings-toolbar-row-menu = Plus d’actions pour { $item }
settings-toolbar-position = Position { $index } sur { $count }
settings-toolbar-hidden-state = Masqué

## Navigation

settings-browsing-sort-order = Trier les images par
settings-browsing-sort-order-help = Ordre utilisé pour parcourir un dossier. Le tri par nom place « img2 » avant « img10 » et suit l’alphabet de votre langue.
settings-browsing-sort-order-natural = Nom
settings-browsing-sort-order-date-taken = Date de prise de vue
settings-browsing-sort-order-modified = Date de modification
settings-browsing-sort-order-size = Taille du fichier
settings-browsing-sort-order-type = Type de fichier
settings-browsing-sort-descending = Ordre inverse
settings-browsing-sort-descending-help = Trier du dernier au premier.
settings-browsing-loop-folder = Boucler à la fin du dossier
settings-browsing-loop-folder-help = Après la dernière image, reprendre à la première, et inversement.
settings-browsing-include-subfolders = Inclure les sous-dossiers
settings-browsing-include-subfolders-help = Parcourir aussi les images des sous-dossiers.
settings-browsing-hold-acceleration = Accélération en maintenant suivant ou précédent
settings-browsing-hold-acceleration-help = Rapidité avec laquelle le défilement accélère lorsqu’une touche ou un bouton est maintenu enfoncé.
settings-browsing-hold-acceleration-off = Aucune accélération
settings-browsing-hold-acceleration-gentle = Douce
settings-browsing-hold-acceleration-normal = Normale
settings-browsing-hold-acceleration-fast = Rapide
settings-browsing-confirm-trash = Demander avant de placer dans la corbeille
settings-browsing-confirm-trash-help = Le placement dans la corbeille peut toujours être annulé ; c’est pourquoi { -app-name } ne demande pas de confirmation par défaut.
settings-browsing-arrows-follow-reading-direction = Les touches fléchées suivent le sens de lecture
settings-browsing-arrows-follow-reading-direction-help = Dans les langues s’écrivant de droite à gauche, la flèche gauche passe à l’image suivante.

## Couleur

settings-colour-management = Gestion des couleurs
settings-colour-management-help = Afficher les couleurs telles qu’elles ont été prévues, à l’aide du profil colorimétrique de l’image et de celui de votre écran.
settings-colour-display-profile = Profil de l’écran
settings-colour-display-profile-help = Profil colorimétrique de votre écran. « Automatique » utilise celui indiqué par votre système d’exploitation.
settings-colour-rendering-intent = Mode de rendu
settings-colour-rendering-intent-help = Façon dont les couleurs que l’écran ne peut pas afficher sont ramenées dans sa gamme.
settings-colour-rendering-intent-perceptual = Perceptif
settings-colour-rendering-intent-relative-colorimetric = Colorimétrie relative
settings-colour-rendering-intent-saturation = Saturation
settings-colour-rendering-intent-absolute-colorimetric = Colorimétrie absolue
settings-colour-black-point-compensation = Compensation du point noir
settings-colour-black-point-compensation-help = Préserver les détails dans les ombres lorsque le noir de l’écran est plus clair que celui de l’image.
settings-colour-untagged-images = Images sans profil colorimétrique
settings-colour-untagged-images-help = Façon d’afficher les images qui n’indiquent pas quelles couleurs elles utilisent.
settings-colour-untagged-images-srgb = Traiter comme sRGB
settings-colour-untagged-images-display = Afficher sans modification
settings-colour-dithering = Tramage
settings-colour-dithering-help = Ajouter un léger bruit bleu lors de la réduction de la profondeur de couleur, ce qui évite les effets de bandes dans les ciels et les dégradés.
settings-colour-hdr-output = Sortie HDR
settings-colour-hdr-output-help = Afficher les images HDR avec toute leur luminosité sur les écrans compatibles. Automatique utilise le HDR lorsque l’écran signale une luminosité supplémentaire ; sinon, les zones claires sont doucement compressées.
settings-colour-hdr-output-auto = Automatique
settings-colour-hdr-output-on = Activée
settings-colour-hdr-output-off = Désactivée
# machine-drafted, needs native review
settings-colour-ocio-config = Configuration OCIO
settings-colour-ocio-config-help = Configuration OpenColorIO (.ocio) pour les images EXR, Radiance HDR et PFM. Si ce champ est vide, celle désignée par $OCIO est utilisée.
settings-colour-ocio-display = Affichage OCIO
settings-colour-ocio-display-help = Affichage de la configuration OCIO qui correspond à votre écran.
settings-colour-ocio-display-default = Le premier de la configuration
settings-colour-ocio-view = Vue OCIO
settings-colour-ocio-view-help = Façon dont les images en scène linéaire (scene-linear) sont montrées sur cet affichage, par exemple avec une vue à mappage de tons.
settings-colour-ocio-view-default = La première de l’affichage
settings-colour-ocio-input = Espace colorimétrique d’entrée OCIO
settings-colour-ocio-input-help = Espace colorimétrique des valeurs des images.
settings-colour-ocio-input-default = Rôle scene-linear de la configuration
# machine-drafted, needs native review
settings-colour-proof-profile = Profil d’épreuvage écran
settings-colour-proof-profile-help = Le profil d’imprimante utilisé en dernier par l’épreuvage de Développement.
settings-colour-proof-intent = Mode de rendu de l’épreuvage
settings-colour-proof-intent-help = Comment ramener les couleurs que l’imprimante ne peut pas imprimer.
settings-colour-proof-intent-perceptual = Perceptif
settings-colour-proof-intent-relative = Colorimétrie relative
settings-colour-proof-bpc = Compensation du point noir à l’épreuvage
settings-colour-proof-bpc-help = Ramène le noir au noir de l’imprimante.
settings-colour-proof-paper = Blanc du papier à l’épreuvage
settings-colour-proof-paper-help = Montre la couleur propre du papier.
settings-colour-proof-ink = Noir de l’encre à l’épreuvage
settings-colour-proof-ink-help = Montre à quel point le noir de l’encre est sombre.

## Métadonnées et confidentialité

settings-metadata-show-info-on-open = Afficher le panneau d’informations à l’ouverture
settings-metadata-show-info-on-open-help = Ouvrir le panneau indiquant l’appareil photo, la date et le lieu à chaque ouverture d’une image.
settings-metadata-info-detail = Contenu du panneau d’informations
settings-metadata-info-detail-help = Le résumé explique simplement les détails les plus utiles ; « Tous les champs » liste tout ce qui est enregistré dans le fichier.
settings-metadata-info-detail-summary = Résumé
settings-metadata-info-detail-all = Tous les champs
settings-metadata-reverse-geocoding = Afficher les noms de lieux des positions GPS
settings-metadata-reverse-geocoding-help = Rechercher la localité la plus proche à partir de données intégrées. Fonctionne hors ligne : les positions ne quittent jamais votre ordinateur.
settings-metadata-hide-gps = Masquer les positions GPS à l’écran
settings-metadata-hide-gps-help = Ne pas afficher les coordonnées ni les lieux, par exemple pendant un partage d’écran. Les fichiers ne sont pas modifiés.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Enregistrer les réglages de Développement à côté de la photo
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Écrit IMG_1234.CR2.specular.xmp à côté du fichier, pour que les réglages le suivent. Désactivé : les réglages restent dans la bibliothèque seulement.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importer les réglages Lightroom dans Développement
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Une photo avec des réglages Lightroom et sans retouche s’ouvre avec eux, approximativement. Désactivé : un bouton le propose.
settings-metadata-units = Unités
settings-metadata-units-help = Unités d’altitude, de distance et de vitesse.
settings-metadata-units-auto = Selon les formats régionaux
settings-metadata-units-metric = Métriques
settings-metadata-units-imperial = Impériales
# machine-drafted, needs native review
settings-metadata-basemap = Fond de carte
settings-metadata-basemap-help = La carte sous les lieux des photos. Les cartes en ligne téléchargent les tuiles de la zone affichée.
settings-metadata-basemap-off = Carte intégrée uniquement (hors ligne)
settings-metadata-basemap-openstreetmap = OpenStreetMap (en ligne)
settings-metadata-basemap-custom = Autre serveur de tuiles (en ligne)
settings-metadata-basemap-file = Mon propre fichier de carte
settings-metadata-basemap-url = Adresse du serveur de tuiles
settings-metadata-basemap-url-help = Une adresse avec {"{"}z{"}"}, {"{"}x{"}"} et {"{"}y{"}"}, par exemple https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Fichier de carte
settings-metadata-basemap-path-help = Un fichier MBTiles ou PMTiles. Il fonctionne hors ligne.
settings-metadata-basemap-cache = Cache des tuiles de carte
settings-metadata-basemap-cache-help = Espace disque pour les tuiles téléchargées. Les plus anciennes partent en premier.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Serveurs de tuiles autorisés
settings-metadata-basemap-allowed-help = Serveurs de carte dont vous avez accepté de télécharger les tuiles.

## Performances

settings-performance-ram-cache = Cache en mémoire
settings-performance-ram-cache-help = Mémoire utilisée pour garder les images décodées prêtes. « Automatique » choisit en fonction de la mémoire de votre ordinateur.
settings-performance-vram-cache = Cache en mémoire GPU
settings-performance-vram-cache-help = Mémoire GPU utilisée pour les images et les tuiles. « Automatique » choisit en fonction de votre GPU.
settings-performance-prefetch-depth = Images à préparer à l’avance
settings-performance-prefetch-depth-help = Nombre d’images décodées à l’avance dans le sens de votre parcours.
settings-performance-decoder-threads = Threads de décodage
settings-performance-decoder-threads-help = Threads du CPU utilisés pour décoder les images. « Automatique » utilise tous les cœurs performance et s’adapte au fichier.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = GPU à utiliser lorsque l’ordinateur en possède plusieurs.
settings-performance-gpu-adapter-high-performance = Performances maximales
settings-performance-gpu-adapter-low-power = Consommation minimale
settings-performance-processing = Où les images sont traitées
settings-performance-processing-help = Certaines tâches peuvent s’exécuter sur le CPU, le GPU ou un décodeur matériel. Automatique mesure cet ordinateur et choisit la voie la plus rapide pour l’image affichée et la plus économe pour le travail en arrière-plan. Ce qui a été choisi sur cet ordinateur, et pourquoi, est indiqué plus bas.
settings-performance-processing-automatic = Automatique
settings-performance-processing-prefer-gpu = Préférer le GPU
settings-performance-processing-cpu-only = CPU uniquement
settings-performance-drag-preview = Aperçu pendant le glissement
settings-performance-drag-preview-help = Comment Développement affiche la photo pendant que vous faites glisser un curseur. Automatique utilise une image plus petite seulement quand l’ordinateur ne suit pas.
settings-performance-drag-preview-automatic = Automatique
settings-performance-drag-preview-sharp = Toujours net
settings-performance-drag-preview-fast = Toujours rapide

## Settings → Performance → How { -app-name } runs on this computer (DESIGN.md §12.3.1)
# machine-drafted, needs native review
settings-paths-title = Comment { -app-name } fonctionne sur cet ordinateur
settings-paths-intro = { -app-name } chronomètre chaque façon d’effectuer ces tâches sur cet ordinateur et utilise la meilleure. Les durées valent pour une photo de { $mpx } mégapixels.
settings-paths-intro-measuring = Mesures pas encore terminées : { -app-name } mesure quand l’ordinateur est inactif.
settings-paths-intro-battery = Sur batterie : les choix privilégient une consommation d’énergie plus faible.
settings-paths-keywords = GPU CPU NPU carte graphique matériel accélération décodeur benchmark vitesse
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, tous les cœurs
paths-engine-gpu = GPU
paths-engine-hw-decoder = Décodeur matériel
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (votre réglage)
settings-paths-measured-faster = Mesuré sur cet ordinateur : { $ratio } fois plus rapide que le { $other }.
settings-paths-measured-similar = Mesuré sur cet ordinateur : à peu près aussi rapide que le { $other }.
settings-paths-measured-power = Sur batterie, c’est ce qui consomme le moins d’énergie (mesuré sur cet ordinateur).
settings-paths-not-measured = Pas encore mesuré : le choix habituel pour ce type d’ordinateur.
settings-paths-set-by = Choisi par votre réglage « { $setting } : { $option } ».
settings-paths-blocked-check = Le { $engine } a été désactivé le { $date } : son résultat ne correspondait pas à celui du CPU.
settings-paths-blocked-error = Le { $engine } a été désactivé le { $date } après une erreur.
settings-paths-blocked-lost = Le { $engine } a été désactivé le { $date } après avoir cessé de répondre.
settings-paths-blocked-undated = Le { $engine } a été désactivé après une erreur.
settings-paths-checking = Le { $engine } est comparé au CPU avant d’être utilisé pour l’image affichée.
settings-paths-exploring = Essai du { $engine } en arrière-plan en cours.
settings-paths-unavailable = Cet ordinateur n’a pas de { $engine } pour cela.
settings-paths-background-cheaper = En arrière-plan : le { $engine }, mesuré à { $percent } de temps CPU en moins.
settings-paths-background-power = En arrière-plan : le { $engine }, pour économiser l’énergie.
settings-paths-background = En arrière-plan : le { $engine }.
settings-paths-detail-measured = { $engine } : { $time } ms, { $cpu } ms de temps CPU, { $runs ->
    [one] { $runs } mesure
   *[other] { $runs } mesures
} ({ $confidence })
settings-paths-detail-guess = { $engine } : environ { $time } ms (estimation)
settings-paths-confidence-low = confiance faible
settings-paths-confidence-medium = confiance moyenne
settings-paths-confidence-high = confiance élevée
settings-paths-machine = Mesuré pour { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Réessayer
settings-paths-measure-again = Mesurer à nouveau
settings-paths-measure-again-question = Oublier ce qui a été mesuré sur cet ordinateur, y compris les voies désactivées, et mesurer à nouveau quand l’ordinateur est inactif ?
settings-paths-measure-again-confirm = Oublier les mesures

settings-performance-display-cache = Conserver un cache d’affichage sur le disque
settings-performance-display-cache-help = Enregistrer des copies des images à la taille de l’écran pour que les dossiers déjà consultés s’ouvrent instantanément.
settings-performance-display-cache-size = Taille du cache d’affichage
settings-performance-display-cache-size-help = Les entrées les plus anciennes sont supprimées lorsque le cache est plein.
settings-performance-display-cache-location = Emplacement du cache d’affichage
settings-performance-display-cache-location-help = Un dossier sur un disque rapide est préférable.
settings-performance-fast-simd = Utiliser les instructions SIMD
settings-performance-fast-simd-help = Utiliser les instructions vectorielles du CPU pour le décodage. Ne désactivez cette option que pour diagnostiquer un problème.
settings-performance-fast-dct-scaling = Décoder à la taille de l’écran
settings-performance-fast-dct-scaling-help = Décoder les grandes images JPEG directement à leur taille d’affichage. Ne désactivez cette option que pour diagnostiquer un problème.
settings-performance-fast-parallel-decode = Décoder sur plusieurs cœurs
settings-performance-fast-parallel-decode-help = Répartir les grandes images entre les cœurs du CPU. Ne désactivez cette option que pour diagnostiquer un problème.
settings-performance-fast-restart-index = Indexer les grandes images JPEG pour le zoom
settings-performance-fast-restart-index-help = Mémoriser l’emplacement de chaque partie d’une image JPEG afin que les zones agrandies soient décodées rapidement. Ne désactivez cette option que pour diagnostiquer un problème.
settings-performance-fast-yuv-upload = Convertir les couleurs sur le GPU
settings-performance-fast-yuv-upload-help = Convertir les couleurs JPEG sur le GPU ou sur le CPU. Automatique choisit le plus rapide sur cet ordinateur. Ne désactivez cette option que pour diagnostiquer un problème.
settings-performance-fast-yuv-upload-auto = Automatique
settings-performance-fast-yuv-upload-on = Activé
settings-performance-fast-yuv-upload-off = Désactivé
settings-performance-fast-async-io = Lecture asynchrone des fichiers
settings-performance-fast-async-io-help = Lire à l’avance avec l’interface de fichiers la plus rapide du système d’exploitation. Ne désactivez cette option que pour diagnostiquer un problème.
settings-performance-fast-mmap = Fichiers mappés en mémoire
settings-performance-fast-mmap-help = Lire les fichiers sans les copier. Ne désactivez cette option que pour diagnostiquer un problème.
settings-performance-hardware-decode = Décodage matériel
settings-performance-hardware-decode-help = Ouvrir les photos JPEG avec le décodeur intégré du GPU. Automatique l’utilise là où il aide, en général pour les images préparées en arrière-plan. Il est d’abord comparé au décodeur normal, et ce qu’il ne prend pas en charge s’ouvre normalement.
settings-performance-hardware-decode-auto = Automatique
settings-performance-hardware-decode-on = Activé
settings-performance-hardware-decode-off = Désactivé
settings-performance-frame-time-hud = HUD des temps d’image
settings-performance-frame-time-hud-help = Afficher les temps de décodage et d’image, ainsi que les chemins rapides utilisés.

## Accessibilité

settings-accessibility-reduce-motion = Réduire les animations
settings-accessibility-reduce-motion-help = Remplacer les animations de zoom, de glissement et de fondu par des changements instantanés.
settings-accessibility-reduce-motion-system = Selon le système
settings-accessibility-reduce-motion-on = Activé
settings-accessibility-reduce-motion-off = Désactivé
settings-accessibility-increase-contrast = Augmenter le contraste
settings-accessibility-increase-contrast-help = Bordures et texte plus marqués, et moins de transparence.
settings-accessibility-increase-contrast-system = Selon le système
settings-accessibility-increase-contrast-on = Activé
settings-accessibility-increase-contrast-off = Désactivé
settings-accessibility-text-scale = Taille du texte
settings-accessibility-text-scale-help = Taille du texte par rapport à la taille standard.
settings-accessibility-ui-scale = Échelle de l’interface
settings-accessibility-ui-scale-help = Taille de l’ensemble de l’interface, en plus de la mise à l’échelle de l’affichage définie dans votre système d’exploitation.
settings-accessibility-focus-ring-always = Toujours afficher l’anneau de focus
settings-accessibility-focus-ring-always-help = Indiquer quelle commande a le focus clavier, même lors de l’utilisation de la souris.
settings-accessibility-screen-reader-verbosity = Niveau de détail du lecteur d’écran
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Quantité d’informations annoncées par un lecteur d’écran lorsque vous passez d’une image à l’autre, et quantité de l’affichage sous le curseur qu’il lit.
settings-accessibility-screen-reader-verbosity-concise = Concis
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Détaillé
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Lire la valeur sous le curseur
settings-accessibility-announce-cursor-help = Un lecteur d’écran lit la valeur du pixel, les HU, la température ou la position sous le curseur ou le réticule clavier une fois qu’il cesse de bouger.

## Confidentialité

# machine-drafted, needs native review
settings-privacy-no-network = Pas de réseau
# machine-drafted, needs native review
settings-privacy-no-network-help = Désactivé : seuls ceux-ci se connectent, chacun une fois activé ou utilisé pour ouvrir quelque chose : recherche de mises à jour, téléchargement des modèles d’IA locale, cartes en ligne, images depuis une adresse web ou un stockage cloud, et serveurs que vous ajoutez (DICOMweb, OMERO, cartes). Activé : rien ne se connecte.

## Avancé

settings-advanced-portable-mode = Mode portable
settings-advanced-portable-mode-help = Conserver les paramètres et les caches dans le dossier du programme plutôt que dans votre profil utilisateur.
settings-advanced-log-level = Niveau de détail du journal
settings-advanced-log-level-help = Quantité d’informations que { -app-name } écrit dans son fichier journal.
settings-advanced-log-level-error = Erreurs uniquement
settings-advanced-log-level-warn = Avertissements
settings-advanced-log-level-info = Informations
settings-advanced-log-level-debug = Débogage
settings-advanced-log-level-trace = Tout
settings-advanced-developer-mode = Mode développeur
settings-advanced-developer-mode-help = Affiche des outils pour développer et tester { -app-name }, comme les pseudo-langues dans la liste des langues.
# machine-drafted, needs native review
settings-advanced-audit-log = Journal d’audit local
# machine-drafted, needs native review
settings-advanced-audit-log-help = Enregistre les images médicales ouvertes, les exports, les anonymisations, les impressions et les envois vers un PACS dans audit.jsonl, dans le dossier de données. Il reste sur cet ordinateur.
settings-advanced-hw-report = Rapport matériel…
settings-advanced-hw-report-help = Afficher les fonctionnalités du CPU et du GPU détectées par { -app-name } et les chemins rapides utilisés.
# machine-drafted, needs native review
settings-advanced-renderer-info = Informations sur le moteur de rendu…
settings-advanced-renderer-info-help = Afficher la carte graphique, le pilote, les formats d’affichage et les limites de mémoire utilisés par { -app-name }, prêts à copier dans un rapport de bug.
settings-advanced-edit-settings-file = Modifier settings.toml
settings-advanced-edit-settings-file-help = Modifier le fichier de réglages dans l’éditeur intégré de { -app-name }. Il vérifie le fichier pendant la saisie et ne l’enregistre que s’il ne contient aucune erreur.
settings-advanced-import = Importer des paramètres…
settings-advanced-import-help = Charger les paramètres depuis un fichier exporté par { -app-name }.
settings-advanced-export = Exporter les paramètres…
settings-advanced-export-help = Enregistrer tous les paramètres dans un seul fichier, pour une sauvegarde ou un autre ordinateur.
settings-advanced-reset-all = Réinitialiser tous les paramètres…
settings-advanced-reset-all-help = Rétablir la valeur par défaut de chaque paramètre. Cette action peut être annulée.

# Stage 1 merge (machine-drafted, needs native review)
settings-performance-stall-warning = Signaler les chargements bloqués après
settings-performance-stall-warning-help = Si une image est encore en cours de chargement après ce délai, son état de chargement et les tâches en attente sont écrits dans le journal (sortie d’erreur standard ou SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Conseils de performances
settings-performance-insights-help = Quand des images continuent de s’ouvrir lentement pour la même raison, afficher « Pourquoi est-ce lent ? » dans le coin avec ce qui aiderait. Ce qui est appris reste sur cet ordinateur.
settings-performance-insights-storage = Conseils sur le stockage lent
settings-performance-insights-storage-help = Afficher des conseils quand la lecture des fichiers (depuis un partage réseau, un disque lent ou des archives) continue de retarder l’ouverture des images.
settings-performance-insights-decoder = Conseils sur le décodage
settings-performance-insights-decoder-help = Afficher des conseils quand des images continuent d’être décodées par le CPU alors qu’un décodeur matériel pourrait aider.
# machine-drafted, needs native review
settings-performance-insights-restore = Réafficher les conseils masqués
settings-performance-insights-restore-help = Fait réapparaître les conseils masqués avec × sur « Pourquoi est-ce lent ? », pour l’ouverture des images comme pour les vignettes. Indisponible tant qu’aucun n’est masqué.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Enregistrer
config-editor-revert = Rétablir
config-editor-close = Fermer
config-editor-modified = Modifié
config-editor-text = Fichier de réglages
config-editor-position = Ligne { $line }, colonne { $column }
config-editor-saved = settings.toml enregistré et appliqué.
config-editor-cant-save =
    { $count ->
        [one] Enregistrement impossible : 1 problème. Corrigez-le d’abord.
       *[other] Enregistrement impossible : { $count } problèmes. Corrigez-les d’abord.
    }
config-editor-save-failed = Impossible d’enregistrer settings.toml : { $reason }
config-editor-conflict = settings.toml a été modifié ailleurs.
config-editor-reload = Recharger
config-editor-keep-mine = Garder ma version
config-editor-discard-question = Abandonner les modifications non enregistrées de settings.toml ?
config-editor-discard = Abandonner
config-editor-keep-editing = Continuer la modification
config-editor-find-placeholder = Rechercher dans le fichier
config-editor-find-count = { $current } sur { $total }
config-editor-find-none = Aucun résultat
config-editor-find-previous = Résultat précédent
config-editor-find-next = Résultat suivant
config-editor-find-close = Fermer la recherche
config-editor-problems = Problèmes
config-editor-problems-count =
    { $count ->
        [0] Aucun problème
        [one] 1 problème
       *[other] { $count } problèmes
    }
config-editor-no-problems = Aucun problème. Le fichier peut être enregistré.
config-editor-problem-at = Ligne { $line } : { $message }
config-editor-severity-error = Erreur
config-editor-severity-warning = Avertissement
config-editor-severity-info = Remarque
config-editor-problem-syntax = TOML non valide : { $reason }
config-editor-problem-duplicate = { $key } est défini deux fois dans la même table. Supprimez l’une des deux.
config-editor-problem-unknown-section = Il n’existe pas de section de réglages [{ $key }].
config-editor-problem-unknown-section-suggestion = Il n’existe pas de section de réglages [{ $key }]. Vouliez-vous dire [{ $suggestion }] ?
config-editor-problem-unknown-key = Il n’existe pas de réglage { $key }.
config-editor-problem-unknown-key-suggestion = Il n’existe pas de réglage { $key }. Vouliez-vous dire { $suggestion } ?
config-editor-problem-invalid = Valeur non valide pour { $key } : { $reason }
config-editor-problem-migrated = { $key } est écrit comme le faisait une ancienne version de { -app-name }. Il est converti à la lecture.
config-editor-problem-newer-version = Ce fichier a été écrit par une version plus récente de { -app-name } (format { $found } ; cette version lit { $supported }). Les réglages inconnus sont conservés.
config-editor-problem-command = Commande { $index } dans [[commands]] : { $reason }
config-editor-problem-hook = Crochet { $index } dans [[hooks]] : { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Fichier des raccourcis clavier
config-editor-keymap-saved = keymap.toml enregistré et appliqué.
config-editor-keymap-save-failed = Impossible d’enregistrer keymap.toml : { $reason }
config-editor-keymap-conflict = keymap.toml a été modifié ailleurs.
config-editor-keymap-discard-question = Abandonner vos modifications non enregistrées de keymap.toml ?
config-editor-new-file = nouveau fichier, créé à l’enregistrement
config-editor-no-folder = il n’y a pas de dossier de réglages où l’enregistrer.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Rétablir tous les réglages à leur valeur par défaut ?
settings-reset-all-confirm-detail = Cela concerne toutes les sections des Paramètres. Vous pourrez l’annuler ensuite.
settings-reset-all-confirm-ok = Tout rétablir
settings-reset-all-done = Tous les réglages ont été rétablis à leur valeur par défaut.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Encodeur matériel

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = Le { $engine } est comparé au CPU avant d’être utilisé.
settings-paths-unit-thumbnail = Durées pour une vignette de { $size }
settings-paths-unit-frame = Durées par image vidéo de { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Dossier des images de calibrage
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flats et darks pour Développer. Leurs noms de fichier ou de dossier contiennent « flat » ou « dark ».

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Blanc
settings-colour-light-grey = Gris clair
settings-colour-grey = Gris
settings-colour-dark-grey = Gris foncé
settings-colour-black = Noir
settings-colour-yellow = Jaune
settings-colour-orange = Orange
settings-colour-red = Rouge
settings-colour-magenta = Magenta
settings-colour-blue = Bleu
settings-colour-cyan = Cyan
settings-colour-green = Vert
