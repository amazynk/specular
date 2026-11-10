# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Italiano. machine-drafted, needs native review

keys-mode-mesh = Modelli 3D
palette-mesh-orbit-left = Ruota il modello a sinistra
palette-mesh-orbit-right = Ruota il modello a destra
palette-mesh-orbit-up = Ruota il modello verso l’alto
palette-mesh-orbit-down = Ruota il modello verso il basso
palette-mesh-view-front = Vista frontale
palette-mesh-view-top = Vista dall’alto
palette-mesh-view-side = Vista laterale
palette-mesh-view-iso = Vista isometrica
palette-mesh-shading = Ombreggiatura successiva
palette-mesh-projection = Prospettica o ortografica
palette-mesh-ground-grid = Mostra o nascondi la griglia del piano
disabled-not-mesh = Solo per i modelli 3D (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Ombreggiato
mesh-shading-edges = Ombreggiato con spigoli
mesh-shading-wireframe = Wireframe
mesh-shading-xray = Raggi X
mesh-projection-perspective = Prospettica
mesh-projection-ortho = Ortografica
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } triangoli, { $vertices } vertici
mesh-grid-step = Griglia { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Punti più grandi
palette-mesh-points-smaller = Punti più piccoli
palette-mesh-point-colour = Colora i punti in base all’attributo successivo
palette-mesh-eye-dome-lighting = Attiva o disattiva l’illuminazione Eye-Dome (EDL)
disabled-not-points = Solo per le nuvole di punti (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Nuvole di punti
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Colori reali
points-colour-intensity = Intensità
points-colour-elevation = Quota
points-colour-classification = Classificazione
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } punto
   *[other] { $count } punti
}
points-loading = { $loaded } di { $total ->
        [one] { $total } punto
       *[other] { $total } punti
    }
# $size: the points' diameter in pixels.
points-size = Punti { $size } px
points-edl = Illuminazione Eye-Dome (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
   *[other] { $count } splat
}
splats-harmonics = Colore dipendente dalla vista, grado { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Disegna la molecola nello stile successivo
palette-mesh-molecule-colour = Colora la molecola con lo schema successivo
disabled-not-molecule = Solo per strutture molecolari (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Molecole
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Sfere e bastoncini
molecule-style-spacefill = Riempimento spaziale
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Per elemento
molecule-colour-chain = Per catena
molecule-colour-ss = Per struttura secondaria
molecule-colour-bfactor = Per fattore B
molecule-colour-rainbow = Arcobaleno (dall’estremità N alla C)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } atomo
    [many] { $atoms } di atomi
   *[other] { $atoms } atomi
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atomo { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Distanza { $distance } Å
molecule-pick-hint = Fai clic su un atomo per selezionarlo, su un secondo per la distanza
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Streamline più spesse
palette-tracts-thinner = Streamline più sottili
palette-tracts-colour = Colora le streamline con lo schema successivo
cheatsheet-tracts = Trattogrammi
disabled-not-points-or-tracts = Solo per le nuvole di punti e i trattogrammi (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Per direzione
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Per { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } streamline
    [many] { $streamlines } di streamline
   *[other] { $streamlines } streamline
}, { $points ->
    [one] { $points } punto
    [many] { $points } di punti
   *[other] { $points } punti
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, spessore { $width } mm
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Opacità delle streamline successiva
disabled-not-tracts = Solo per i trattogrammi (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Opacità { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Connettività di { $seed }
dconn-pick-hint = Fai clic sulla superficie per mostrare la connettività di un altro vertice
# Toasts after a click on the surface.
dconn-no-seed = Nessuna riga di connettività per questo vertice
dconn-row-failed = Impossibile leggere la riga di connettività di questo vertice
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } riquadro disegnato
   *[other] { $drawn } riquadri disegnati
} su { $read } letti
# machine-drafted, needs native review
mesh-tiles3d-loading = Lettura dei riquadri necessari a questa vista…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Alcuni riquadri sono omessi: troppi da disegnare in una volta
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } riquadro non può essere mostrato (vedi Informazioni)
   *[other] { $count } riquadri non possono essere mostrati (vedi Informazioni)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Impossibile leggere i riquadri di questa vista: { $reason }
