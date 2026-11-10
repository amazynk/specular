# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. English.

keys-mode-mesh = 3D models
palette-mesh-orbit-left = Turn the model left
palette-mesh-orbit-right = Turn the model right
palette-mesh-orbit-up = Turn the model up
palette-mesh-orbit-down = Turn the model down
palette-mesh-view-front = Front view
palette-mesh-view-top = Top view
palette-mesh-view-side = Side view
palette-mesh-view-iso = Isometric view
palette-mesh-shading = Next shading
palette-mesh-projection = Perspective or orthographic
palette-mesh-ground-grid = Show or hide the ground grid
disabled-not-mesh = Only for 3D models (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Shaded
mesh-shading-edges = Shaded with edges
mesh-shading-wireframe = Wireframe
mesh-shading-xray = X-ray
mesh-projection-perspective = Perspective
mesh-projection-ortho = Orthographic
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } triangles, { $vertices } vertices
mesh-grid-step = Grid { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Larger points
palette-mesh-points-smaller = Smaller points
palette-mesh-point-colour = Colour the points by the next attribute
palette-mesh-eye-dome-lighting = Eye-dome lighting on or off
disabled-not-points = Only for point clouds (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Point clouds
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = True colour
points-colour-intensity = Intensity
points-colour-elevation = Elevation
points-colour-classification = Classification
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } point
   *[other] { $count } points
}
points-loading = { $loaded } of { $total ->
        [one] { $total } point
       *[other] { $total } points
    }
# $size: the points' diameter in pixels.
points-size = Points { $size } px
points-edl = Eye-dome lighting
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
   *[other] { $count } splats
}
splats-harmonics = View-dependent colour, degree { $degree }
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Draw the molecule in the next style
palette-mesh-molecule-colour = Colour the molecule by the next scheme
disabled-not-molecule = Only for molecular structures (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Molecules
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Ball and stick
molecule-style-spacefill = Space-filling
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = By element
molecule-colour-chain = By chain
molecule-colour-ss = By secondary structure
molecule-colour-bfactor = By B-factor
molecule-colour-rainbow = Rainbow (N to C terminus)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } atom
   *[other] { $atoms } atoms
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atom { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Distance { $distance } Å
molecule-pick-hint = Click an atom to pick it, a second for the distance
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Thicker streamlines
palette-tracts-thinner = Thinner streamlines
palette-tracts-colour = Colour the streamlines by the next scheme
cheatsheet-tracts = Tractograms
disabled-not-points-or-tracts = Only for point clouds and tractograms (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = By direction
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = By { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } streamline
   *[other] { $streamlines } streamlines
}, { $points ->
    [one] { $points } point
   *[other] { $points } points
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, { $width } mm thick
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Next streamline opacity
disabled-not-tracts = Only for tractograms (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Opacity { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Connectivity of { $seed }
dconn-pick-hint = Click the surface to show another vertex's connectivity
# Toasts after a click on the surface.
dconn-no-seed = No connectivity row for this vertex
dconn-row-failed = Could not read this vertex's connectivity row
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
mesh-tiles3d = { $drawn ->
    [one] { $drawn } tile drawn
   *[other] { $drawn } tiles drawn
} of { $read } read
mesh-tiles3d-loading = Reading the tiles this view needs…
mesh-tiles3d-left-out = Some tiles are left out: too much to draw at once
# $count: tiles whose content could not be shown (the Info panel says why).
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } tile can't be shown (see Info)
   *[other] { $count } tiles can't be shown (see Info)
}
mesh-tiles3d-failed = The tiles for this view could not be read: { $reason }
