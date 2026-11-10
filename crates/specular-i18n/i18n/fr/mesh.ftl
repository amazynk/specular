# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Français. machine-drafted, needs native review

keys-mode-mesh = Modèles 3D
palette-mesh-orbit-left = Tourner le modèle vers la gauche
palette-mesh-orbit-right = Tourner le modèle vers la droite
palette-mesh-orbit-up = Tourner le modèle vers le haut
palette-mesh-orbit-down = Tourner le modèle vers le bas
palette-mesh-view-front = Vue de face
palette-mesh-view-top = Vue de dessus
palette-mesh-view-side = Vue de côté
palette-mesh-view-iso = Vue isométrique
palette-mesh-shading = Ombrage suivant
palette-mesh-projection = Perspective ou orthographique
palette-mesh-ground-grid = Afficher ou masquer la grille au sol
disabled-not-mesh = Uniquement pour les modèles 3D (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Ombré
mesh-shading-edges = Ombré avec arêtes
mesh-shading-wireframe = Filaire
mesh-shading-xray = Rayons X
mesh-projection-perspective = Perspective
mesh-projection-ortho = Orthographique
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } triangles, { $vertices } sommets
mesh-grid-step = Grille { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Points plus gros
palette-mesh-points-smaller = Points plus petits
palette-mesh-point-colour = Colorer les points selon l’attribut suivant
palette-mesh-eye-dome-lighting = Activer ou désactiver l’éclairage Eye-Dome (EDL)
disabled-not-points = Uniquement pour les nuages de points (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Nuages de points
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Couleurs réelles
points-colour-intensity = Intensité
points-colour-elevation = Altitude
points-colour-classification = Classification
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } point
   *[other] { $count } points
}
points-loading = { $loaded } sur { $total ->
        [one] { $total } point
       *[other] { $total } points
    }
# $size: the points' diameter in pixels.
points-size = Points { $size } px
points-edl = Éclairage Eye-Dome (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
    [many] { $count } de splats
   *[other] { $count } splats
}
splats-harmonics = Couleur dépendante de la vue, degré { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Représenter la molécule dans le style suivant
palette-mesh-molecule-colour = Colorer la molécule selon le schéma suivant
disabled-not-molecule = Uniquement pour les structures moléculaires (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Molécules
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Boules et bâtonnets
molecule-style-spacefill = Remplissage spatial
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Par élément
molecule-colour-chain = Par chaîne
molecule-colour-ss = Par structure secondaire
molecule-colour-bfactor = Par facteur B
molecule-colour-rainbow = Arc-en-ciel (de l’extrémité N à C)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } atome
    [many] { $atoms } d’atomes
   *[other] { $atoms } atomes
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atome { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Distance { $distance } Å
molecule-pick-hint = Cliquez sur un atome pour le sélectionner, sur un second pour la distance
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Lignes de courant plus épaisses
palette-tracts-thinner = Lignes de courant plus fines
palette-tracts-colour = Colorer les lignes de courant selon le schéma suivant
cheatsheet-tracts = Tractogrammes
disabled-not-points-or-tracts = Uniquement pour les nuages de points et les tractogrammes (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Par direction
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Par { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } ligne de courant
    [many] { $streamlines } de lignes de courant
   *[other] { $streamlines } lignes de courant
}, { $points ->
    [one] { $points } point
    [many] { $points } de points
   *[other] { $points } points
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, { $width } mm d’épaisseur
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Opacité des lignes de courant suivante
disabled-not-tracts = Uniquement pour les tractogrammes (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Opacité { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Connectivité de { $seed }
dconn-pick-hint = Cliquez sur la surface pour afficher la connectivité d’un autre sommet
# Toasts after a click on the surface.
dconn-no-seed = Aucune ligne de connectivité pour ce sommet
dconn-row-failed = Impossible de lire la ligne de connectivité de ce sommet
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } tuile dessinée
   *[other] { $drawn } tuiles dessinées
} sur { $read } lues
# machine-drafted, needs native review
mesh-tiles3d-loading = Lecture des tuiles nécessaires à cette vue…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Certaines tuiles sont omises : trop à dessiner d’un coup
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } tuile ne peut pas être affichée (voir Informations)
   *[other] { $count } tuiles ne peuvent pas être affichées (voir Informations)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Impossible de lire les tuiles de cette vue : { $reason }
