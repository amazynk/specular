# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Deutsch. machine-drafted, needs native review

keys-mode-mesh = 3D-Modelle
palette-mesh-orbit-left = Modell nach links drehen
palette-mesh-orbit-right = Modell nach rechts drehen
palette-mesh-orbit-up = Modell nach oben drehen
palette-mesh-orbit-down = Modell nach unten drehen
palette-mesh-view-front = Vorderansicht
palette-mesh-view-top = Draufsicht
palette-mesh-view-side = Seitenansicht
palette-mesh-view-iso = Isometrische Ansicht
palette-mesh-shading = Nächste Schattierung
palette-mesh-projection = Perspektivisch oder orthografisch
palette-mesh-ground-grid = Bodenraster ein- oder ausblenden
disabled-not-mesh = Nur für 3D-Modelle (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Schattiert
mesh-shading-edges = Schattiert mit Kanten
mesh-shading-wireframe = Drahtgitter
mesh-shading-xray = Röntgen
mesh-projection-perspective = Perspektivisch
mesh-projection-ortho = Orthografisch
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } Dreiecke, { $vertices } Eckpunkte
mesh-grid-step = Raster { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Größere Punkte
palette-mesh-points-smaller = Kleinere Punkte
palette-mesh-point-colour = Punkte nach dem nächsten Attribut einfärben
palette-mesh-eye-dome-lighting = Eye-Dome-Lighting ein- oder ausschalten
disabled-not-points = Nur für Punktwolken (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Punktwolken
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Echtfarben
points-colour-intensity = Intensität
points-colour-elevation = Höhe
points-colour-classification = Klassifizierung
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } Punkt
   *[other] { $count } Punkte
}
points-loading = { $loaded } von { $total ->
        [one] { $total } Punkt
       *[other] { $total } Punkten
    }
# $size: the points' diameter in pixels.
points-size = Punkte { $size } px
points-edl = Eye-Dome-Lighting (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } Splat
   *[other] { $count } Splats
}
splats-harmonics = Blickwinkelabhängige Farbe, Grad { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Molekül im nächsten Stil darstellen
palette-mesh-molecule-colour = Molekül nach dem nächsten Schema einfärben
disabled-not-molecule = Nur für Molekülstrukturen (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Moleküle
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Kugel-Stab
molecule-style-spacefill = Kalottenmodell
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Nach Element
molecule-colour-chain = Nach Kette
molecule-colour-ss = Nach Sekundärstruktur
molecule-colour-bfactor = Nach B-Faktor
molecule-colour-rainbow = Regenbogen (N- bis C-Terminus)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } Atom
   *[other] { $atoms } Atome
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atom { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Abstand { $distance } Å
molecule-pick-hint = Atom anklicken, um es auszuwählen, ein zweites für den Abstand
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Dickere Streamlines
palette-tracts-thinner = Dünnere Streamlines
palette-tracts-colour = Streamlines nach dem nächsten Schema einfärben
cheatsheet-tracts = Traktogramme
disabled-not-points-or-tracts = Nur für Punktwolken und Traktogramme (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Nach Richtung
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Nach { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } Streamline
   *[other] { $streamlines } Streamlines
}, { $points ->
    [one] { $points } Punkt
   *[other] { $points } Punkte
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, { $width } mm dick
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Nächste Streamline-Deckkraft
disabled-not-tracts = Nur für Traktogramme (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Deckkraft { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Konnektivität von { $seed }
dconn-pick-hint = Oberfläche anklicken, um die Konnektivität eines anderen Vertex zu zeigen
# Toasts after a click on the surface.
dconn-no-seed = Keine Konnektivitätszeile für diesen Vertex
dconn-row-failed = Konnektivitätszeile dieses Vertex konnte nicht gelesen werden
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } Kachel gezeichnet
   *[other] { $drawn } Kacheln gezeichnet
}, { $read } gelesen
# machine-drafted, needs native review
mesh-tiles3d-loading = Die für diese Ansicht nötigen Kacheln werden gelesen …
# machine-drafted, needs native review
mesh-tiles3d-left-out = Einige Kacheln werden weggelassen: zu viel, um alles auf einmal zu zeichnen
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } Kachel kann nicht angezeigt werden (siehe Informationen)
   *[other] { $count } Kacheln können nicht angezeigt werden (siehe Informationen)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Die Kacheln für diese Ansicht konnten nicht gelesen werden: { $reason }
