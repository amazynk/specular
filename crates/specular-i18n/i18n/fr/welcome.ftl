# machine-drafted, needs native review
# Specular — L’écran d’accueil (quand Specular s’ouvre sans fichier et quand la dernière image
# est fermée), ses cartes « À essayer » et les conseils affichés une seule fois. Français.

## Écran d’accueil

welcome-title = { -app-name }
welcome-tagline = Une visionneuse rapide et fidèle pour les photos et les images techniques, scientifiques et médicales.
welcome-open = Ouvrir un fichier…
welcome-open-about = Une image, un document ou un fichier de données.
welcome-browse = Parcourir un dossier…
welcome-browse-about = Choisir un dossier et l’afficher en galerie.
welcome-pictures = Vos images
welcome-pictures-about = Toutes les photos de votre bibliothèque, par date et par lieu.
welcome-pictures-setup-title = Configurer votre bibliothèque…
welcome-pictures-setup = À partir de votre dossier { $folder }.
welcome-pictures-ask = Ajouter votre dossier { $folder } à la bibliothèque ?
welcome-pictures-add = Ajouter le dossier
welcome-drop = Vous pouvez aussi déposer des fichiers ou des dossiers n’importe où dans cette fenêtre.
welcome-recent = Dossiers récents
welcome-recent-name = { $name }, dans { $parent }
welcome-tips = À essayer
welcome-tips-hide = Masquer ces conseils
welcome-tips-hidden = Conseils masqués. « Afficher l’accueil » dans la palette de commandes les fait revenir.

## À essayer : un nom et une ligne chacun. La touche affichée est celle de l’utilisateur.

welcome-tip-gallery = Galerie
welcome-tip-gallery-about = Voir tout un dossier d’un coup d’œil.
welcome-tip-compare = Comparer
welcome-tip-compare-about = Sélectionnez deux à quatre images dans la galerie pour les voir côte à côte.
welcome-tip-cull = Mode tri
welcome-tip-cull-about = Retenir, rejeter et noter une séance, une touche pour chaque.
welcome-tip-slideshow = Diaporama
welcome-tip-slideshow-about = Afficher un dossier en plein écran, une image après l’autre.
welcome-tip-palette = Palette de commandes
welcome-tip-palette-about = Trouver toute commande ou tout paramètre par son nom.
welcome-tip-studies = Études
welcome-tip-studies-about = Les patients, études et séries d’un disque ou d’un dossier DICOM.
welcome-tip-dicom-servers = Serveurs DICOMweb
welcome-tip-dicom-servers-about = Parcourir et ouvrir des études d’un PACS.
welcome-tip-dicom-tags = Balises DICOM
welcome-tip-dicom-tags-about = Toutes les balises d’une image, avec recherche.
welcome-tip-window-level = Fenêtrage
welcome-tip-window-level-about = Faites glisser pour régler le contraste d’une image DICOM.
welcome-tip-omero-servers = Serveurs OMERO
welcome-tip-omero-servers-about = Parcourir des projets de microscopie sur OMERO et IDR.
welcome-tip-stretch = Étirement
welcome-tip-stretch-about = Faire ressortir les détails ténus des données astronomiques et scientifiques.
welcome-tip-drawing-layers = Calques du dessin
welcome-tip-drawing-layers-about = Afficher et masquer les calques d’un dessin technique.
welcome-tip-needs-image = Ouvrez d’abord une image, puis appuyez sur { $key }.
welcome-tip-needs-gallery = Dans la galerie, sélectionnez deux à quatre images, puis appuyez sur { $key }.
welcome-tip-key = Raccourci : { $key }

## Conseils uniques : chacun une fois, discrètement, au plus un par démarrage. Paramètres ›
## Général › Afficher les conseils les désactive tous.

hint-gallery = Appuyez sur { $key } pour afficher ce dossier en galerie.
hint-gallery-first = Sélectionnez deux à quatre images et appuyez sur { $compare } pour les comparer, ou sur { $cull } pour trier le dossier.
hint-raw = Un dossier de photos RAW : appuyez sur { $key } pour le trier (retenir, rejeter, noter).
hint-pages =
    { $count ->
        [one] Ce fichier a { $count } page. { $next } et { $prev } les tournent.
        [many] Ce fichier a { $count } pages. { $next } et { $prev } les tournent.
       *[other] Ce fichier a { $count } pages. { $next } et { $prev } les tournent.
    }
hint-turn-off = Désactiver les conseils
hint-turned-off = Conseils désactivés. Paramètres › Général › Afficher les conseils les réactive.

## Palette de commandes

palette-show-welcome = Afficher l’accueil
palette-show-welcome-help = L’écran d’accueil, avec Ouvrir, Parcourir, vos images et des choses à essayer.
palette-close = Fermer
palette-close-help = Fermer l’image ou le dossier et revenir à l’écran d’accueil. Sur l’écran d’accueil, quitter.

## Premier lancement (sous la question du profil)

profile-on-start = Quand { -app-name } s’ouvre sans fichier :
profile-skip = Passer
