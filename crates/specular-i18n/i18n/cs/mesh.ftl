# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Čeština. machine-drafted, needs native review

keys-mode-mesh = 3D modely
palette-mesh-orbit-left = Otočit model doleva
palette-mesh-orbit-right = Otočit model doprava
palette-mesh-orbit-up = Otočit model nahoru
palette-mesh-orbit-down = Otočit model dolů
palette-mesh-view-front = Pohled zepředu
palette-mesh-view-top = Pohled shora
palette-mesh-view-side = Pohled z boku
palette-mesh-view-iso = Izometrický pohled
palette-mesh-shading = Další stínování
palette-mesh-projection = Perspektivní nebo ortografická projekce
palette-mesh-ground-grid = Zobrazit nebo skrýt podlahovou mřížku
disabled-not-mesh = Pouze pro 3D modely (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Stínované
mesh-shading-edges = Stínované s hranami
mesh-shading-wireframe = Drátový model
mesh-shading-xray = Rentgen
mesh-projection-perspective = Perspektivní
mesh-projection-ortho = Ortografická
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = Trojúhelníky: { $triangles }, vrcholy: { $vertices }
mesh-grid-step = Mřížka { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Větší body
palette-mesh-points-smaller = Menší body
palette-mesh-point-colour = Obarvit body podle dalšího atributu
palette-mesh-eye-dome-lighting = Zapnout nebo vypnout osvětlení Eye-Dome (EDL)
disabled-not-points = Pouze pro mračna bodů (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Mračna bodů
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Skutečné barvy
points-colour-intensity = Intenzita
points-colour-elevation = Výška
points-colour-classification = Klasifikace
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } bod
    [few] { $count } body
    [many] { $count } bodu
   *[other] { $count } bodů
}
points-loading = { $loaded } z { $total ->
        [one] { $total } bodu
        [few] { $total } bodů
        [many] { $total } bodu
       *[other] { $total } bodů
    }
# $size: the points' diameter in pixels.
points-size = Body { $size } px
points-edl = Osvětlení Eye-Dome (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
    [few] { $count } splaty
    [many] { $count } splatu
   *[other] { $count } splatů
}
splats-harmonics = Barva závislá na pohledu, stupeň { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Vykreslit molekulu dalším stylem
palette-mesh-molecule-colour = Obarvit molekulu dalším schématem
disabled-not-molecule = Pouze pro molekulové struktury (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Molekuly
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Stuhový
molecule-style-ball-stick = Kuličky a tyčinky
molecule-style-spacefill = Kalotový
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Podle prvku
molecule-colour-chain = Podle řetězce
molecule-colour-ss = Podle sekundární struktury
molecule-colour-bfactor = Podle B-faktoru
molecule-colour-rainbow = Duha (od N- k C-konci)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } atom
    [few] { $atoms } atomy
    [many] { $atoms } atomu
   *[other] { $atoms } atomů
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atom { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Vzdálenost { $distance } Å
molecule-pick-hint = Klikněte na atom pro výběr, na druhý pro vzdálenost
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Silnější proudnice
palette-tracts-thinner = Tenčí proudnice
palette-tracts-colour = Obarvit proudnice dalším schématem
cheatsheet-tracts = Traktogramy
disabled-not-points-or-tracts = Pouze pro mračna bodů a traktogramy (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Podle směru
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Podle { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } proudnice
    [few] { $streamlines } proudnice
    [many] { $streamlines } proudnice
   *[other] { $streamlines } proudnic
}, { $points ->
    [one] { $points } bod
    [few] { $points } body
    [many] { $points } bodu
   *[other] { $points } bodů
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, tloušťka { $width } mm
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Další neprůhlednost proudnic
disabled-not-tracts = Pouze pro traktogramy (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Neprůhlednost { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Konektivita: { $seed }
dconn-pick-hint = Klepnutím na povrch zobrazíte konektivitu jiného vrcholu
# Toasts after a click on the surface.
dconn-no-seed = Pro tento vrchol není řádek konektivity
dconn-row-failed = Řádek konektivity tohoto vrcholu nelze přečíst
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } dlaždice vykreslena
    [few] { $drawn } dlaždice vykresleny
    [many] { $drawn } dlaždice vykresleno
   *[other] { $drawn } dlaždic vykresleno
} z { $read } načtených
# machine-drafted, needs native review
mesh-tiles3d-loading = Načítání dlaždic, které tento pohled potřebuje…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Některé dlaždice jsou vynechány: je jich příliš na vykreslení najednou
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } dlaždici nelze zobrazit (viz Informace)
    [few] { $count } dlaždice nelze zobrazit (viz Informace)
    [many] { $count } dlaždice nelze zobrazit (viz Informace)
   *[other] { $count } dlaždic nelze zobrazit (viz Informace)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Dlaždice pro tento pohled nelze načíst: { $reason }
