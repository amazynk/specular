# Specular — Export slideshow as video (DESIGN.md §6.5). French. Machine-drafted, needs native review.

palette-export-slideshow = Exporter le diaporama en vidéo…

video-export-title = Exporter le diaporama en vidéo
video-export-resolution = Résolution
# $width and $height in pixels.
video-export-resolution-window = Fenêtre ({ $width } × { $height })
video-export-fps = Fréquence d’images
# $fps frames per second, a number.
video-export-fps-value = { $fps } i/s
video-export-quality = Qualité
video-export-quality-high = Haute
video-export-quality-standard = Standard
video-export-quality-compact = Compacte
video-export-images = { $count ->
    [one] { $count } image, avec l’ordre, la transition et les légendes du diaporama
   *[other] { $count } images, avec l’ordre, la transition et les légendes du diaporama
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Environ { $length }, environ { $size }
video-export-export = Exporter…
video-export-replace = Remplacer
# $file a file name.
video-export-replace-question = { $file } existe déjà. Le remplacer ?
# $percent a formatted percentage ("42%").
video-export-progress = Exportation… { $percent }
video-export-cancelling = Annulation…
# $file a file name, $size a file size ("38 MB").
video-export-saved = { $file } enregistré ({ $size })
video-export-skipped = { $count ->
    [one] { $count } image n’a pas pu être lue et a été omise.
   *[other] { $count } images n’ont pas pu être lues et ont été omises.
}
video-export-failed = Impossible d’exporter la vidéo : { $reason }
video-export-cancelled = Exportation annulée
video-export-no-images = Aucune des images n’a pu être lue.
video-export-show-in-folder = Afficher dans le dossier

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Exporter le diaporama en vidéo
video-export-compatibility = Compatibilité
video-export-compatibility-current = Lecteurs actuels
video-export-compatibility-all = Tous les lecteurs
