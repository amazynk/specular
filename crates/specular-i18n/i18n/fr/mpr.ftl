# Multiplanar reformatting (MPR).

palette-mpr = Reconstruction multiplanaire (MPR)
palette-mpr-slab = Épaisseur MPR suivante (fine, MIP, MinIP, moyenne)
disabled-no-volume = Ce n’est pas un volume : ouvrez une série, un fichier multi-images, un volume 3D ou une pile z
disabled-mpr-closed = Ouvrez d’abord les vues multiplanaires
mpr-axial = Axial
mpr-coronal = Coronal
mpr-sagittal = Sagittal
mpr-oblique = Oblique
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = Vue 3D
mpr-thin = Fine
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Moyenne
mpr-slab = Épaisseur : { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Épaisseur : { $mode }, { $thickness } Å
mpr-slab-unit = Épaisseur : { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Épaisseur : { $mode }, { $thickness } voxels
mpr-thicker = Épaisseur plus grande
mpr-thinner = Épaisseur plus fine
mpr-reset = Réinitialiser les vues
mpr-close = Fermer la MPR
mpr-maximise = Agrandir la vue
mpr-export-planes = Exporter les plans
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Lignes de courant de { $files } : épaisseur { $thickness } mm
mpr-tracts-off = Lignes de courant de { $files } : masquées
mpr-tracts-none = Aucune ligne de courant TrackVis ou MRtrix à côté de ce volume
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } voxels
mpr-axis-label = { $axis } : { $label }
mpr-axis-position = { $axis } : { $position } sur { $count }
mpr-axis-channel = Canal
mpr-axis-unnamed = Axe { $axis }
mpr-one-channel = La MPR affiche un canal à la fois
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } canaux en couleur, chacun dans sa plage d’affichage
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Un seul canal affiché : { $reason }
mpr-isotropic = Pas de taille de voxel dans le fichier : voxels affichés en cubes
mpr-isotropic-z = Pas d’espacement des coupes dans le fichier : coupes affichées à un pixel d’écart
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Chargement : { $loaded } sur { $count ->
        [one] { $count } coupe
       *[other] { $count } coupes
    }
mpr-loaded = { $count ->
        [one] { $count } coupe
       *[other] { $count } coupes
    }, { $width } × { $height }
mpr-downsampled = Affiché à 1/{ $factor } de la résolution (limites de mémoire ou graphiques)
mpr-converted = Valeurs affichées en flottants 32 bits
mpr-opening = Ouverture du volume…
mpr-gpu-memory = { $size } sur la carte graphique
mpr-refused = Impossible de construire un volume : { $reason }
mpr-failed = Le volume n’a pas été chargé : { $reason }
mpr-no-gpu = La carte graphique ne peut pas contenir ce volume : { $reason }
mpr-planes-saved = { $count ->
        [one] { $count } plan enregistré
       *[other] { $count } plans enregistrés
    } dans { $folder }
mpr-planes-failed = Les plans n’ont pas pu être enregistrés : { $reason }
mpr-view-a11y = Vue { $view }, { $position } mm
mpr-crosshair-a11y = Réticule à { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Réticule à { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Réticule à { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Réticule au voxel { $x }, { $y }, { $z }
mpr-controls = Commandes MPR

# 3D view palette entries.
palette-mpr-preset = Préréglage 3D suivant (TDM os, tissus mous, poumon, angio ; IRM)
palette-mpr-render = Rendu 3D suivant (ombré, Cinematic, MIP, MinIP)
palette-mpr-clip = Découpe 3D suivante (aucune, boîte des vues 2D, coin retiré)
palette-mpr-export-stl = Exporter la surface 3D en STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Épaisseur des lignes de courant suivante en MPR
palette-mpr-maximise = Agrandir la vue MPR, ou afficher toutes les vues
palette-mpr-export-planes = Exporter les plans MPR en images…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] TDM os
    [soft] TDM tissus mous
    [lung] TDM poumon
    [angio] Angio-TDM
   *[mr] IRM
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Ombré
}
vr-clip = { $clip ->
    [box] Découpe : boîte des vues
    [cut] Découpe : coin retiré
   *[off] Sans découpe
}
vr-smooth = Lisser la surface
vr-export = Exporter en STL
vr-threshold = Surface à { $value }
vr-stl-exporting = Construction de la surface…
vr-stl-saved = { $count ->
    [0] Rien à ce seuil : aucune surface enregistrée
   *[other] { $count } triangles enregistrés dans { $name }
}
vr-stl-failed = La surface n’a pas pu être enregistrée : { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, surface à { $threshold }. Maj+flèches pour orbiter, plus et moins pour zoomer.

## Volume brut : disposition d’un fichier .raw / .vol sans en-tête

# machine-drafted, needs native review
raw-volume-title = Ouvrir un volume brut
raw-volume-file = { $file } n’a pas d’en-tête. Indiquez comment ses échantillons sont disposés.
raw-volume-guess-label = Disposition suggérée
raw-volume-guess = { $index } sur { $count }
raw-volume-custom = Personnalisée
raw-volume-width = Largeur (voxels)
raw-volume-height = Hauteur (voxels)
raw-volume-slices = Coupes
raw-volume-type = Type d’échantillon
raw-volume-type-unsigned = Non signé { $bits } bits
raw-volume-type-signed = Signé { $bits } bits
raw-volume-type-float = Flottant { $bits } bits
raw-volume-order = Ordre des octets
raw-volume-little = Petit-boutiste
raw-volume-big = Gros-boutiste
raw-volume-offset = Décalage de l’en-tête
raw-volume-bytes = { $count ->
    [one] { $count } octet
   *[other] { $count } octets
}
raw-volume-pixel = Taille de pixel
raw-volume-spacing = Espacement des coupes
raw-volume-mm = { $value } mm
raw-volume-exact = Remplit exactement le fichier ({ $size })
raw-volume-fits = Utilise { $size } sur { $file }, { $left } restants
raw-volume-too-big = Nécessite { $size }, mais le fichier fait { $file }
raw-volume-cancel = Annuler
raw-volume-open = Ouvrir
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Connectivité de { $seed } ; déplacez le réticule vers une autre graine
# machine-drafted, needs native review
mpr-measure-hint-edit = Faites glisser une extrémité pour la déplacer ; sur l’autre pour supprimer la mesure
# machine-drafted, needs native review
mpr-measure-removed = Mesure supprimée
mpr-measure-lengths-only = La MPR ne mesure que des longueurs
mpr-more = Plus…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = MPR curviligne le long d’une courbe cliquée
palette-mpr-curve-layout = MPR curviligne : redressée ou étirée
palette-mpr-curve-save = Enregistrer la courbe…
palette-mpr-curve-load = Charger une courbe…
palette-mpr-export-segments = Exporter chaque segment en STL…
palette-mpr-dental = Disposition dentaire (panoramique et coupes transversales)
palette-mpr-nerve = Tracer le canal mandibulaire
cpr-kind = { $kind ->
    [stretched] Étirée
   *[straightened] Redressée
}
cpr-button = Curviligne
cpr-controls = Commandes de la MPR curviligne
cpr-at = À { $position }
cpr-section = Coupe transversale
cpr-hint-start = Cliquez des points le long de la structure dans la vue { $view }
cpr-hint-more = Cliquez encore un point
cpr-add-points = Ajouter des points
cpr-undo-point = Annuler le point
# machine-drafted, needs native review
cpr-mark = Marquer ici
cpr-mark-set = Repère { $n } placé.
cpr-mark-removed = Repère supprimé.
cpr-mark-here = Repère { $n }
cpr-diameter = Diamètre { $value }
cpr-diameters = Diamètre de { $min } à { $max }
cpr-mark-span = Repères { $from } à { $to } : { $length } le long de la ligne centrale
cpr-mark-total = Tous les repères : { $length }
cpr-clear = Effacer la courbe
cpr-save = Enregistrer la courbe
cpr-load = Charger une courbe
cpr-done = Terminé
cpr-a11y = { $name } sur { $length }, curseur à { $position }. La molette déplace le curseur.
cpr-none = Pas encore de courbe : cliquez d’abord des points
cpr-cleared = Courbe effacée
cpr-saved = Courbe enregistrée : { $name }
cpr-save-failed = La courbe n’a pas pu être enregistrée : { $reason }
cpr-loaded = Courbe chargée
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] { $count } ligne centrale chargée
    [many] { $count } de lignes centrales chargées
   *[other] { $count } lignes centrales chargées
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] { $count } ligne centrale enregistrée trouvée
    [many] { $count } de lignes centrales enregistrées trouvées
   *[other] { $count } lignes centrales enregistrées trouvées
}
# A centreline without a name, by its place in the list.
cpr-centreline = Ligne centrale { $number }
cpr-new = Nouvelle ligne centrale
cpr-rename = Renommer
cpr-name = Nom de la ligne centrale
cpr-delete = Supprimer
# $name: the centreline's name.
cpr-delete-title = Supprimer la ligne centrale « { $name } » ?
cpr-delete-detail = Ses points disparaissent. Les fichiers enregistrés sur le disque restent.
cpr-deleted = « { $name } » supprimée
cpr-load-failed = La courbe n’a pas pu être chargée : { $reason }
cpr-load-not-json = le fichier n’est pas du JSON
cpr-load-not-curves = le fichier n’est pas une courbe Specular
cpr-load-empty = le fichier ne contient aucune courbe de deux points ou plus
cpr-panoramic = Panoramique
cpr-panoramic-slab = Panoramique, épaisseur { $thickness } mm
cpr-panoramic-thin = Panoramique, fine

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Dentaire
dental-not-for-diagnosis = Non destiné au diagnostic ni à la planification du traitement
dental-nerve = Canal mandibulaire
dental-nerve-hint = Cliquez des points le long du canal mandibulaire sur la panoramique ou une coupe transversale
dental-nerve-done = Tracé du canal mandibulaire désactivé
dental-spacing = Tous les { $spacing } mm
dental-section = Coupe { $number }
dental-off = Ouvrez d’abord la disposition dentaire
dental-suggesting = Recherche de l’arcade dentaire…
dental-suggested = Arcade proposée : faites glisser ses points pour l’ajuster
dental-no-arch = Aucune arcade trouvée : cliquez des points le long de l’arcade dans la vue axiale
dental-vendor = CBCT dentaire { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Exporter les segments
seg-stl-detail = Segments : { $count ->
        [one] { $count } triangle
       *[other] { $count } triangles
    }
seg-stl-detail-full = Segments : détail complet
seg-stl-none = Aucun segment à exporter
seg-stl-not-labels = ce volume n’est pas une carte d’étiquettes
seg-stl-exporting = Construction des surfaces des segments…
seg-stl-saved = { $count ->
    [one] { $count } segment enregistré dans { $folder }
    [many] { $count } segments enregistrés dans { $folder }
   *[other] { $count } segments enregistrés dans { $folder }
}
seg-stl-open = Non fermés : { $count }.
seg-stl-empty = Les segments sont vides : rien n’a été enregistré
seg-stl-failed = Les segments n’ont pas pu être enregistrés : { $reason }
# machine-drafted, needs native review
toolbar-mpr = Trois plans (MPR)
toolbar-mpr-close = Fermer les trois plans (MPR)
settings-viewing-toolbar-items-mpr = Trois plans (MPR)
