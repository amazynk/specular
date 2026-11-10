# Intégration poussée au bureau : Réglages → Intégration, définir comme fond d’écran.

settings-section-integration = Intégration
settings-integration-default-viewer = Visionneuse d’images par défaut
settings-integration-default-viewer-help = Ouvrir les images avec { -app-name } par un double-clic. Le retrait rétablit la visionneuse précédente.
# machine-drafted, needs native review
settings-integration-extra-default-types = Autres types de fichiers
settings-integration-extra-default-types-help = Autres types de fichiers à ouvrir avec { -app-name } par un double-clic, choisis sous Visionneuse d’images par défaut.
settings-integration-thumbnailer = Vignettes du gestionnaire de fichiers
settings-integration-thumbnailer-help = Permettre aux gestionnaires de fichiers d’afficher l’aperçu des formats qu’ils ne savent pas lire, comme JPEG XL, HEIC et les fichiers RAW d’appareils photo.
settings-integration-single-instance = Instance unique via D-Bus
settings-integration-single-instance-help = Les fichiers ouverts depuis le gestionnaire de fichiers, un lanceur ou le terminal vont dans la fenêtre { -app-name } déjà ouverte. S’applique aux fenêtres ouvertes ensuite.
settings-integration-file-manager-actions = Actions du gestionnaire de fichiers
settings-integration-file-manager-actions-help = Ajouter « Ouvrir », « Comparer » et « Définir comme fond d’écran » aux menus contextuels de Nautilus, Dolphin, Thunar et Nemo.
settings-integration-search-provider = Recherche du bureau
# machine-drafted, needs native review
settings-integration-search-provider-help = Trouver les images de votre bibliothèque dans la recherche de GNOME et de KDE. Les mots fonctionnent comme dans la barre de recherche de la bibliothèque.
# machine-drafted, needs native review
settings-integration-url-scheme = Ouvrir les liens specular://
settings-integration-url-scheme-help = Les liens comme specular:///home/me/photo.jpg dans un navigateur ou un autre programme s’ouvrent dans { -app-name }. Seuls les fichiers de cet ordinateur sont ouverts, et les fichiers inhabituels seulement après confirmation.

integration-state-on = Activé
integration-state-off = Désactivé
integration-state-partial = Partiellement configuré. Appliquez à nouveau pour réparer.
integration-state-unavailable = Non disponible sur ce bureau
integration-state-sandboxed = Non disponible dans un bac à sable (Flatpak)
integration-state-no-bus = Non disponible : aucune session D-Bus
integration-state-no-file-manager = Non disponible : aucun gestionnaire de fichiers pris en charge trouvé
integration-state-no-search = Non disponible : ni GNOME Shell ni KRunner trouvé
integration-state-working = En cours…
integration-state-details = { $state } · { $details }
integration-detail-types = types d’images : { $count } sur { $total }
integration-detail-thumbnail-types = types d’images : { $count }
integration-apply = Appliquer
integration-remove = Retirer
integration-done-on = « { $option } » est activé
integration-done-off = « { $option } » est désactivé
integration-failed = Impossible de modifier « { $option } » : { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Désactiver la recherche du bureau ?
integration-search-remove-detail = { -app-name } est retiré de la recherche de { $desktops }. Votre bibliothèque et vos images restent.
search-provider-open-folder = Ouvrir le dossier

integration-label-open = Ouvrir dans { -app-name }
integration-label-compare = Comparer dans { -app-name }
integration-label-wallpaper = Définir comme fond d’écran
integration-label-generic-name = Visionneuse d’images
integration-label-comment = Ouvrir et parcourir instantanément de grandes photos

palette-set-wallpaper = Définir comme fond d’écran
wallpaper-set = Fond d’écran défini ({ $method })
wallpaper-failed = Impossible de définir le fond d’écran : { $reason }

# machine-drafted, needs native review
url-invalid = Impossible d’ouvrir ce lien { -app-name } : { $reason }
url-not-found = Les fichiers indiqués par ce lien n’existent pas
url-remote-refused = { -app-name } n’ouvre que les fichiers de cet ordinateur, pas { $source }
url-confirm-files = { $more ->
    [0] « { $file } »
   *[other] « { $file } » et { $more } autres
}
url-confirm-title = { $reason ->
    [dots] Ouvrir { url-confirm-files } ? Le lien y accède par des dossiers « .. ».
    [network] Ouvrir { url-confirm-files } ? Il se trouve sur un partage réseau ou un périphérique.
   *[hidden] Ouvrir { url-confirm-files } ? Son nom contient des caractères invisibles ou de direction du texte.
}
url-confirm-open = Ouvrir

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nouvelle fenêtre
integration-label-library = Bibliothèque
integration-label-slideshow = Diaporama des Images
settings-integration-recent-files = Fichiers récents du bureau
settings-integration-recent-files-help = Les images ouvertes apparaissent dans « Récents » du gestionnaire de fichiers et dans les listes de fichiers récents du bureau. La suppression retire de ces listes les entrées de { -app-name }.
integration-detail-recent = fichiers : { $count }
# machine-drafted, needs native review
integration-extra-title = Autres types de fichiers
# machine-drafted, needs native review
integration-extra-intro = En plus des images, { -app-name } peut ouvrir ces fichiers par un double-clic. Rien n’est coché tant que vous ne le cochez pas. Les programmes qui modifient ces fichiers restent dans « Ouvrir avec », et décocher un type le rend au programme qu’il avait avant. Blackmagic RAW, DWG, STEP, IGES et Zeiss CZI nécessitent aussi un logiciel gratuit de leur éditeur : voir Paramètres › Décodeurs facultatifs.
integration-extra-chosen = { $count ->
    [0] Aucun choisi
    [one] { $count } type choisi
    [many] { $count } de types choisis
   *[other] { $count } types choisis
}
integration-extra-count = { $on } sur { $total }
integration-extra-none-yet = Aucun pour l’instant
integration-extra-keywords = types de fichiers formats associations double-clic ouvrir avec éditeur science médical CAO SIG 3D molécules vidéo SVG PDF
integration-extra-group-editors = Fichiers d’éditeurs
# machine-drafted, needs native review
integration-extra-group-editors-help = Documents de logiciels de peinture et de dessin : Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch et Figma. Le double-clic affiche l’image ; vous la modifiez toujours dans le logiciel qui l’a créée.
integration-extra-group-science = Science
integration-extra-group-science-help = Fichiers d’instruments et de logiciels de recherche : caméras thermiques, microscopes, mesures de surface, HDF5, NetCDF, données sismiques et météo.
integration-extra-group-medical = Médical
integration-extra-group-medical-help = Imagerie médicale et cérébrale : DICOM, NIfTI, MINC, NRRD, tractographie et surfaces cérébrales.
integration-extra-group-cad-gis = CAO et SIG
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Tracés de traceur, illustrations techniques, dessins de puces et cartes : HP-GL, CGM, GDSII, shapefiles, GeoPackage, KML, GPX, GeoJSON et tuiles d’altitude. DWG s’ouvre avec ODA File Converter, gratuit, et STEP et IGES avec OpenCASCADE.
integration-extra-group-models = 3D et molécules
integration-extra-group-models-help = Modèles 3D, maquettes de bâtiments, nuages de points et molécules : STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF et plus.
integration-extra-group-videos = Vidéos
# machine-drafted, needs native review
integration-extra-group-videos-help = Vidéos : MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF et MXF. Blackmagic RAW s’ouvre avec le Blackmagic RAW SDK, gratuit.
integration-extra-group-other-images = SVG et autres images
# machine-drafted, needs native review
integration-extra-group-other-images-help = Images et documents généralement laissés à d’autres logiciels : dessins SVG, PDF, XPS, DjVu, EPS, WMF et EMF, Visio, OpenDocument, WordPerfect Graphics, livres EPUB et polices, et animations comme MNG, FLIC et curseurs animés.
integration-extra-group-text = Texte, tableaux et sous-titres
integration-extra-group-text-help = Texte brut et journaux, NFO, Markdown, tableaux CSV et TSV, RTF, et fichiers de sous-titres : SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML et SCC.
