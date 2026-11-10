# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Svenska. machine-drafted, needs native review

keys-mode-mesh = 3D-modeller
palette-mesh-orbit-left = Vrid modellen åt vänster
palette-mesh-orbit-right = Vrid modellen åt höger
palette-mesh-orbit-up = Vrid modellen uppåt
palette-mesh-orbit-down = Vrid modellen nedåt
palette-mesh-view-front = Vy framifrån
palette-mesh-view-top = Vy ovanifrån
palette-mesh-view-side = Vy från sidan
palette-mesh-view-iso = Isometrisk vy
palette-mesh-shading = Nästa skuggning
palette-mesh-projection = Perspektiv eller ortografisk
palette-mesh-ground-grid = Visa eller dölj golvrutnätet
disabled-not-mesh = Endast för 3D-modeller (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Skuggad
mesh-shading-edges = Skuggad med kanter
mesh-shading-wireframe = Trådmodell
mesh-shading-xray = Röntgen
mesh-projection-perspective = Perspektiv
mesh-projection-ortho = Ortografisk
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } trianglar, { $vertices } hörn
mesh-grid-step = Rutnät { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Större punkter
palette-mesh-points-smaller = Mindre punkter
palette-mesh-point-colour = Färglägg punkterna efter nästa attribut
palette-mesh-eye-dome-lighting = Slå på eller av eye-dome-belysning (EDL)
disabled-not-points = Endast för punktmoln (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Punktmoln
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Äkta färger
points-colour-intensity = Intensitet
points-colour-elevation = Höjd
points-colour-classification = Klassificering
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } punkt
   *[other] { $count } punkter
}
points-loading = { $loaded } av { $total ->
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
splats-harmonics = Vyberoende färg, grad { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Rita molekylen i nästa stil
palette-mesh-molecule-colour = Färga molekylen efter nästa schema
disabled-not-molecule = Endast för molekylstrukturer (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Molekyler
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Kula och pinne
molecule-style-spacefill = Rymdfyllande
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Efter grundämne
molecule-colour-chain = Efter kedja
molecule-colour-ss = Efter sekundärstruktur
molecule-colour-bfactor = Efter B-faktor
molecule-colour-rainbow = Regnbåge (N- till C-terminal)
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
molecule-distance = Avstånd { $distance } Å
molecule-pick-hint = Klicka på en atom för att välja den, på en andra för avståndet
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Tjockare streamlines
palette-tracts-thinner = Tunnare streamlines
palette-tracts-colour = Färga streamlines efter nästa schema
cheatsheet-tracts = Traktogram
disabled-not-points-or-tracts = Endast för punktmoln och traktogram (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Efter riktning
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
tracts-look-line = { $colour }, { $width } mm tjock
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Nästa opacitet för streamlines
disabled-not-tracts = Endast för traktogram (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Opacitet { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Konnektivitet för { $seed }
dconn-pick-hint = Klicka på ytan för att visa konnektiviteten för en annan vertex
# Toasts after a click on the surface.
dconn-no-seed = Ingen konnektivitetsrad för denna vertex
dconn-row-failed = Kunde inte läsa konnektivitetsraden för denna vertex
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } ruta ritad
   *[other] { $drawn } rutor ritade
} av { $read } lästa
# machine-drafted, needs native review
mesh-tiles3d-loading = Läser rutorna som den här vyn behöver…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Vissa rutor utelämnas: för mycket att rita på en gång
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } ruta kan inte visas (se Information)
   *[other] { $count } rutor kan inte visas (se Information)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Rutorna för den här vyn kunde inte läsas: { $reason }
