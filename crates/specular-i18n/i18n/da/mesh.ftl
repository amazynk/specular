# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Dansk. machine-drafted, needs native review

keys-mode-mesh = 3D-modeller
palette-mesh-orbit-left = Drej modellen til venstre
palette-mesh-orbit-right = Drej modellen til højre
palette-mesh-orbit-up = Drej modellen opad
palette-mesh-orbit-down = Drej modellen nedad
palette-mesh-view-front = Visning forfra
palette-mesh-view-top = Visning ovenfra
palette-mesh-view-side = Visning fra siden
palette-mesh-view-iso = Isometrisk visning
palette-mesh-shading = Næste skyggelægning
palette-mesh-projection = Perspektivisk eller ortografisk
palette-mesh-ground-grid = Vis eller skjul gulvgitteret
disabled-not-mesh = Kun for 3D-modeller (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Skygget
mesh-shading-edges = Skygget med kanter
mesh-shading-wireframe = Trådmodel
mesh-shading-xray = Røntgen
mesh-projection-perspective = Perspektivisk
mesh-projection-ortho = Ortografisk
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } trekanter, { $vertices } hjørnepunkter
mesh-grid-step = Gitter { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Større punkter
palette-mesh-points-smaller = Mindre punkter
palette-mesh-point-colour = Farv punkterne efter næste attribut
palette-mesh-eye-dome-lighting = Slå eye-dome-belysning (EDL) til eller fra
disabled-not-points = Kun til punktskyer (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Punktskyer
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Ægte farver
points-colour-intensity = Intensitet
points-colour-elevation = Højde
points-colour-classification = Klassifikation
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } punkt
   *[other] { $count } punkter
}
points-loading = { $loaded } af { $total ->
        [one] { $total } punkt
       *[other] { $total } punkter
    }
# $size: the points' diameter in pixels.
points-size = Punkter { $size } px
points-edl = Eye-dome-belysning (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
   *[other] { $count } splats
}
splats-harmonics = Visningsafhængig farve, grad { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Tegn molekylet i næste stil
palette-mesh-molecule-colour = Farv molekylet efter næste skema
disabled-not-molecule = Kun for molekylstrukturer (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Molekyler
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Kugle og pind
molecule-style-spacefill = Rumfyldende
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Efter grundstof
molecule-colour-chain = Efter kæde
molecule-colour-ss = Efter sekundær struktur
molecule-colour-bfactor = Efter B-faktor
molecule-colour-rainbow = Regnbue (N- til C-terminal)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } atom
   *[other] { $atoms } atomer
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atom { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Afstand { $distance } Å
molecule-pick-hint = Klik på et atom for at vælge det, på et andet for afstanden
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Tykkere streamlines
palette-tracts-thinner = Tyndere streamlines
palette-tracts-colour = Farv streamlines efter næste skema
cheatsheet-tracts = Traktogrammer
disabled-not-points-or-tracts = Kun til punktskyer og traktogrammer (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Efter retning
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Efter { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } streamline
   *[other] { $streamlines } streamlines
}, { $points ->
    [one] { $points } punkt
   *[other] { $points } punkter
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, { $width } mm tyk
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Næste opacitet for streamlines
disabled-not-tracts = Kun til traktogrammer (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Opacitet { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Konnektivitet for { $seed }
dconn-pick-hint = Klik på overfladen for at vise konnektiviteten for et andet vertex
# Toasts after a click on the surface.
dconn-no-seed = Ingen konnektivitetsrække for dette vertex
dconn-row-failed = Kunne ikke læse konnektivitetsrækken for dette vertex
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } flise tegnet
   *[other] { $drawn } fliser tegnet
} af { $read } læst
# machine-drafted, needs native review
mesh-tiles3d-loading = Læser de fliser, denne visning har brug for…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Nogle fliser er udeladt: for meget at tegne på én gang
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } flise kan ikke vises (se Oplysninger)
   *[other] { $count } fliser kan ikke vises (se Oplysninger)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Fliserne til denne visning kunne ikke læses: { $reason }
