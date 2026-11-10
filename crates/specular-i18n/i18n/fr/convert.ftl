# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Convertir…

## Settings → Convert
settings-section-convert = Conversion
settings-convert-format = Format
settings-convert-format-help = Le format écrit par « Convertir… ». PNG et TIFF sont sans perte ; WebP et JPEG XL peuvent être l’un ou l’autre.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = Qualité JPEG
settings-convert-jpeg-quality-help = Une valeur plus élevée garde plus de détails dans des fichiers plus gros. À 90, la différence avec l’original est difficile à voir.
settings-convert-jpeg-full-chroma = Couleur en pleine résolution en JPEG
settings-convert-jpeg-full-chroma-help = Garde la couleur en pleine résolution (4:4:4) au lieu de la moitié (4:2:0) : contours de couleur plus nets, fichiers plus gros.
settings-convert-png-effort = Compression PNG
settings-convert-png-effort-help = Une valeur plus élevée donne des fichiers plus petits, plus lentement. 1 est la plus rapide.
settings-convert-jxl-effort = Effort JPEG XL
settings-convert-jxl-effort-help = Une valeur plus élevée donne des fichiers plus petits, plus lentement.
settings-convert-tiff-compression = Compression TIFF
settings-convert-tiff-compression-help = Deflate est sans perte et lu par tous les lecteurs TIFF.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Aucune
settings-convert-depth = Profondeur de bits
settings-convert-depth-help = Automatique écrit 16 bits par échantillon quand l’image en a plus de 8 et que le format les accepte (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automatique
settings-convert-depth-8 = 8 bits
settings-convert-depth-16 = 16 bits
settings-convert-resize = Redimensionner
settings-convert-resize-help = Les images sont réduites en lumière linéaire avec un filtre de haute qualité.
settings-convert-resize-none = Taille d’origine
settings-convert-resize-fit = Tenir dans
settings-convert-resize-long-edge = Grand côté
settings-convert-resize-percent = Pourcentage
settings-convert-width = Largeur
settings-convert-width-help = La largeur maximale d’une image avec Tenir dans.
settings-convert-height = Hauteur
settings-convert-height-help = La hauteur maximale d’une image avec Tenir dans.
settings-convert-long-edge = Grand côté
settings-convert-long-edge-help = La longueur du plus grand côté de l’image avec Grand côté.
settings-convert-percent = Pourcentage
settings-convert-percent-help = La taille par rapport à l’original avec Pourcentage.
settings-convert-upscale = Agrandir les petites images
settings-convert-upscale-help = Désactivé, les images déjà plus petites que la taille demandée gardent leur taille.
settings-convert-colour = Couleur
settings-convert-colour-help = Conserver enregistre le profil colorimétrique de l’image avec elle. Convertir en sRGB convient au web et aux appareils sans gestion des couleurs.
settings-convert-colour-keep = Conserver le profil
settings-convert-colour-srgb = Convertir en sRGB
settings-convert-hdr = Images HDR
settings-convert-hdr-help = PNG et JPEG XL conservent le HDR tel quel ; les autres formats sont toujours ramenés en SDR par mappage tonal.
settings-convert-hdr-keep = Conserver si possible
settings-convert-hdr-tone-map = Mappage tonal en SDR
settings-convert-metadata = Métadonnées
settings-convert-metadata-help = Exif et XMP, comme l’appareil photo, la date de prise de vue et la position. Le profil colorimétrique est toujours conservé.
settings-convert-metadata-all = Tout conserver
settings-convert-metadata-no-location = Tout sauf la position
settings-convert-metadata-strip = Supprimer
settings-convert-folder = Dossier de destination
settings-convert-folder-help = Où vont les fichiers convertis. S’il n’est pas défini, ils vont dans un sous-dossier à côté de chaque image.
settings-convert-folder-unset = Sous-dossier à côté des images
settings-convert-subfolder = Nom du sous-dossier
settings-convert-subfolder-help = Le sous-dossier utilisé quand aucun dossier de destination n’est défini. Vide : le dossier des images lui-même.
settings-convert-pattern = Noms de fichier
settings-convert-pattern-help = {"{"}name{"}"} est le nom d’origine, {"{"}n:03{"}"} le numéro dans le lot, {"{"}date:%Y-%m-%d{"}"} la date de prise de vue, {"{"}w{"}"} et {"{"}h{"}"} la nouvelle taille, {"{"}ext{"}"} l’extension du format.
settings-convert-conflicts = Quand un nom est déjà pris
settings-convert-conflicts-help = Les fichiers existants ne sont jamais remplacés.
settings-convert-conflicts-suffix = Ajouter un numéro
settings-convert-conflicts-skip = Ignorer l’image

## The dialog
convert-title = Convertir
convert-images = Images
convert-images-current = Cette image
convert-images-selection = Sélection ({ $count })
convert-images-folder = Dossier ({ $count })
convert-images-picked = Choix ({ $count })
convert-quality = Qualité
convert-chroma = Détail des couleurs
convert-chroma-half = 4:2:0 (plus petit)
convert-chroma-full = 4:4:4 (plus net)
convert-effort = Effort
convert-on = Activé
convert-off = Désactivé
convert-pixels = { $value } px
convert-effort-fastest = { $value } (le plus rapide)
convert-folder-subfolder = Sous-dossier « { $name } »
convert-folder-same = Même dossier que l’image
convert-choose-folder = Choisir le dossier…
convert-name = Nom
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Une {"{"} n’est pas fermée.
convert-name-error-stray = Une {"}"} n’a pas de {"{"}. Écrivez {"}}"} pour une accolade.
convert-name-error-unknown = { $token } n’est pas un élément de nom. Utilisez name, n, date, w, h ou ext.
convert-name-error-format = { $token } a un format incorrect.
convert-name-error-empty = Saisissez un modèle de nom.
convert-start = Convertir
convert-stop = Arrêter
convert-hide = Masquer
convert-close = Fermer
convert-progress = Conversion de { $done } sur { $total }
convert-progress-eta = Conversion de { $done } sur { $total } · il reste environ { $time }
convert-eta-seconds = { $count ->
    [one] { $count } seconde
   *[other] { $count } secondes
}
convert-eta-minutes = { $count ->
    [one] { $count } minute
   *[other] { $count } minutes
}
convert-done = { $count ->
    [one] { $count } image convertie
   *[other] { $count } images converties
}
convert-done-skipped = { $count } ignorées : le nom était déjà pris
convert-done-failed = { $count } en échec
convert-stopped = Arrêté après { $count }
convert-failure = { $file } : { $reason }
convert-failure-open = impossible à ouvrir ({ $error })
convert-failure-decode = illisible ({ $error })
convert-failure-encode = impossible à écrire dans ce format ({ $error })
convert-failure-write = impossible à enregistrer ({ $error })
convert-failure-same = le nouveau fichier remplacerait l’original
convert-report-more = et { $count } de plus
convert-running = Une conversion est déjà en cours.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = WebP sans perte
settings-convert-webp-lossless-help = Conserve chaque pixel à l’identique. Désactivé, les fichiers WebP sont bien plus petits à la qualité WebP.
settings-convert-webp-quality = Qualité WebP
settings-convert-webp-quality-help = Une valeur plus élevée garde plus de détails dans des fichiers plus gros. Utilisé quand WebP n’est pas sans perte.
settings-convert-webp-effort = Effort WebP
settings-convert-webp-effort-help = Une valeur plus élevée donne des fichiers plus petits, plus lentement. Utilisé quand WebP n’est pas sans perte.
settings-convert-jxl-lossless = JPEG XL sans perte
settings-convert-jxl-lossless-help = Conserve chaque pixel à l’identique. Désactivé, les fichiers JPEG XL sont bien plus petits à la qualité JPEG XL.
settings-convert-jxl-quality = Qualité JPEG XL
settings-convert-jxl-quality-help = Une valeur plus élevée garde plus de détails dans des fichiers plus gros. À 90, la différence avec l’original est difficile à voir. Utilisé quand JPEG XL n’est pas sans perte.
settings-convert-avif-quality = Qualité AVIF
settings-convert-avif-quality-help = Une valeur plus élevée garde plus de détails dans des fichiers plus gros.
settings-convert-avif-effort = Effort AVIF
settings-convert-avif-effort-help = Une valeur plus élevée donne des fichiers plus petits, plus lentement. 1 est la plus rapide.
convert-lossless = Sans perte
