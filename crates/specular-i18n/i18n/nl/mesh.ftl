# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Nederlands. machine-drafted, needs native review

keys-mode-mesh = 3D-modellen
palette-mesh-orbit-left = Model naar links draaien
palette-mesh-orbit-right = Model naar rechts draaien
palette-mesh-orbit-up = Model omhoog draaien
palette-mesh-orbit-down = Model omlaag draaien
palette-mesh-view-front = Vooraanzicht
palette-mesh-view-top = Bovenaanzicht
palette-mesh-view-side = Zijaanzicht
palette-mesh-view-iso = Isometrisch aanzicht
palette-mesh-shading = Volgende arcering
palette-mesh-projection = Perspectief of orthografisch
palette-mesh-ground-grid = Grondraster tonen of verbergen
disabled-not-mesh = Alleen voor 3D-modellen (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Gearceerd
mesh-shading-edges = Gearceerd met randen
mesh-shading-wireframe = Draadmodel
mesh-shading-xray = Röntgen
mesh-projection-perspective = Perspectief
mesh-projection-ortho = Orthografisch
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } driehoeken, { $vertices } hoekpunten
mesh-grid-step = Raster { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Grotere punten
palette-mesh-points-smaller = Kleinere punten
palette-mesh-point-colour = Punten kleuren op het volgende attribuut
palette-mesh-eye-dome-lighting = Eye-dome lighting (EDL) aan of uit
disabled-not-points = Alleen voor puntenwolken (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Puntenwolken
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Ware kleuren
points-colour-intensity = Intensiteit
points-colour-elevation = Hoogte
points-colour-classification = Classificatie
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } punt
   *[other] { $count } punten
}
points-loading = { $loaded } van { $total ->
        [one] { $total } punt
       *[other] { $total } punten
    }
# $size: the points' diameter in pixels.
points-size = Punten { $size } px
points-edl = Eye-dome lighting (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
   *[other] { $count } splats
}
splats-harmonics = Kijkrichtingsafhankelijke kleur, graad { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Molecuul in de volgende stijl tekenen
palette-mesh-molecule-colour = Molecuul volgens het volgende schema kleuren
disabled-not-molecule = Alleen voor molecuulstructuren (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Moleculen
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Bal-en-stok
molecule-style-spacefill = Ruimtevullend
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Per element
molecule-colour-chain = Per keten
molecule-colour-ss = Per secundaire structuur
molecule-colour-bfactor = Per B-factor
molecule-colour-rainbow = Regenboog (N- tot C-terminus)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } atoom
   *[other] { $atoms } atomen
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atoom { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Afstand { $distance } Å
molecule-pick-hint = Klik op een atoom om het te kiezen, op een tweede voor de afstand
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Dikkere streamlines
palette-tracts-thinner = Dunnere streamlines
palette-tracts-colour = Streamlines volgens het volgende schema kleuren
cheatsheet-tracts = Tractogrammen
disabled-not-points-or-tracts = Alleen voor puntenwolken en tractogrammen (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Op richting
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Op { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } streamline
   *[other] { $streamlines } streamlines
}, { $points ->
    [one] { $points } punt
   *[other] { $points } punten
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, { $width } mm dik
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Volgende dekking van streamlines
disabled-not-tracts = Alleen voor tractogrammen (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Dekking { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Connectiviteit van { $seed }
dconn-pick-hint = Klik op het oppervlak om de connectiviteit van een andere vertex te tonen
# Toasts after a click on the surface.
dconn-no-seed = Geen connectiviteitsrij voor deze vertex
dconn-row-failed = Kon de connectiviteitsrij van deze vertex niet lezen
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } tegel getekend
   *[other] { $drawn } tegels getekend
} van { $read } gelezen
# machine-drafted, needs native review
mesh-tiles3d-loading = De tegels die deze weergave nodig heeft worden gelezen…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Sommige tegels zijn weggelaten: te veel om in één keer te tekenen
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } tegel kan niet worden getoond (zie Informatie)
   *[other] { $count } tegels kunnen niet worden getoond (zie Informatie)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = De tegels voor deze weergave konden niet worden gelezen: { $reason }
