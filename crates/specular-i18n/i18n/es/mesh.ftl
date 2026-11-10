# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Español. machine-drafted, needs native review

keys-mode-mesh = Modelos 3D
palette-mesh-orbit-left = Girar el modelo a la izquierda
palette-mesh-orbit-right = Girar el modelo a la derecha
palette-mesh-orbit-up = Girar el modelo hacia arriba
palette-mesh-orbit-down = Girar el modelo hacia abajo
palette-mesh-view-front = Vista frontal
palette-mesh-view-top = Vista superior
palette-mesh-view-side = Vista lateral
palette-mesh-view-iso = Vista isométrica
palette-mesh-shading = Siguiente sombreado
palette-mesh-projection = Perspectiva u ortográfica
palette-mesh-ground-grid = Mostrar u ocultar la cuadrícula del suelo
disabled-not-mesh = Solo para modelos 3D (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Sombreado
mesh-shading-edges = Sombreado con aristas
mesh-shading-wireframe = Alámbrico
mesh-shading-xray = Rayos X
mesh-projection-perspective = Perspectiva
mesh-projection-ortho = Ortográfica
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } triángulos, { $vertices } vértices
mesh-grid-step = Cuadrícula { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Puntos más grandes
palette-mesh-points-smaller = Puntos más pequeños
palette-mesh-point-colour = Colorear los puntos según el siguiente atributo
palette-mesh-eye-dome-lighting = Activar o desactivar la iluminación Eye-Dome (EDL)
disabled-not-points = Solo para nubes de puntos (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Nubes de puntos
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Color real
points-colour-intensity = Intensidad
points-colour-elevation = Elevación
points-colour-classification = Clasificación
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } punto
   *[other] { $count } puntos
}
points-loading = { $loaded } de { $total ->
        [one] { $total } punto
       *[other] { $total } puntos
    }
# $size: the points' diameter in pixels.
points-size = Puntos { $size } px
points-edl = Iluminación Eye-Dome (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
    [many] { $count } de splats
   *[other] { $count } splats
}
splats-harmonics = Color dependiente de la vista, grado { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Dibujar la molécula en el estilo siguiente
palette-mesh-molecule-colour = Colorear la molécula con el esquema siguiente
disabled-not-molecule = Solo para estructuras moleculares (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Moléculas
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Bolas y varillas
molecule-style-spacefill = Relleno de espacio
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Por elemento
molecule-colour-chain = Por cadena
molecule-colour-ss = Por estructura secundaria
molecule-colour-bfactor = Por factor B
molecule-colour-rainbow = Arcoíris (del extremo N al C)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } átomo
    [many] { $atoms } de átomos
   *[other] { $atoms } átomos
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Átomo { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Distancia { $distance } Å
molecule-pick-hint = Haga clic en un átomo para seleccionarlo y en otro para la distancia
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Líneas de corriente más gruesas
palette-tracts-thinner = Líneas de corriente más finas
palette-tracts-colour = Colorear las líneas de corriente con el esquema siguiente
cheatsheet-tracts = Tractogramas
disabled-not-points-or-tracts = Solo para nubes de puntos y tractogramas (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Por dirección
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Por { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } línea de corriente
    [many] { $streamlines } de líneas de corriente
   *[other] { $streamlines } líneas de corriente
}, { $points ->
    [one] { $points } punto
    [many] { $points } de puntos
   *[other] { $points } puntos
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, { $width } mm de grosor
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Siguiente opacidad de las líneas de corriente
disabled-not-tracts = Solo para tractogramas (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Opacidad { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Conectividad de { $seed }
dconn-pick-hint = Haga clic en la superficie para ver la conectividad de otro vértice
# Toasts after a click on the surface.
dconn-no-seed = No hay fila de conectividad para este vértice
dconn-row-failed = No se pudo leer la fila de conectividad de este vértice
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } mosaico dibujado
   *[other] { $drawn } mosaicos dibujados
} de { $read } leídos
# machine-drafted, needs native review
mesh-tiles3d-loading = Leyendo los mosaicos que necesita esta vista…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Se omiten algunos mosaicos: demasiado para dibujar de una vez
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } mosaico no se puede mostrar (ver Información)
   *[other] { $count } mosaicos no se pueden mostrar (ver Información)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = No se pudieron leer los mosaicos de esta vista: { $reason }
