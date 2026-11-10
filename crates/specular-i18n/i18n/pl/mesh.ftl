# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Polski. machine-drafted, needs native review

keys-mode-mesh = Modele 3D
palette-mesh-orbit-left = Obróć model w lewo
palette-mesh-orbit-right = Obróć model w prawo
palette-mesh-orbit-up = Obróć model w górę
palette-mesh-orbit-down = Obróć model w dół
palette-mesh-view-front = Widok z przodu
palette-mesh-view-top = Widok z góry
palette-mesh-view-side = Widok z boku
palette-mesh-view-iso = Widok izometryczny
palette-mesh-shading = Następne cieniowanie
palette-mesh-projection = Rzut perspektywiczny lub ortogonalny
palette-mesh-ground-grid = Pokaż lub ukryj siatkę podłoża
disabled-not-mesh = Tylko dla modeli 3D (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Cieniowany
mesh-shading-edges = Cieniowany z krawędziami
mesh-shading-wireframe = Model szkieletowy
mesh-shading-xray = Rentgen
mesh-projection-perspective = Perspektywa
mesh-projection-ortho = Rzut ortogonalny
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = Trójkąty: { $triangles }, wierzchołki: { $vertices }
mesh-grid-step = Siatka { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Większe punkty
palette-mesh-points-smaller = Mniejsze punkty
palette-mesh-point-colour = Koloruj punkty według następnego atrybutu
palette-mesh-eye-dome-lighting = Włącz lub wyłącz oświetlenie Eye-Dome (EDL)
disabled-not-points = Tylko dla chmur punktów (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Chmury punktów
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Kolory rzeczywiste
points-colour-intensity = Intensywność
points-colour-elevation = Wysokość
points-colour-classification = Klasyfikacja
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } punkt
    [few] { $count } punkty
    [many] { $count } punktów
   *[other] { $count } punktu
}
points-loading = { $loaded } z { $total ->
        [one] { $total } punktu
        [few] { $total } punktów
        [many] { $total } punktów
       *[other] { $total } punktu
    }
# $size: the points' diameter in pixels.
points-size = Punkty { $size } px
points-edl = Oświetlenie Eye-Dome (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
    [few] { $count } splaty
    [many] { $count } splatów
   *[other] { $count } splata
}
splats-harmonics = Kolor zależny od kierunku patrzenia, stopień { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Rysuj cząsteczkę w następnym stylu
palette-mesh-molecule-colour = Koloruj cząsteczkę według następnego schematu
disabled-not-molecule = Tylko dla struktur cząsteczkowych (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Cząsteczki
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Wstęgowy
molecule-style-ball-stick = Kulkowo-pręcikowy
molecule-style-spacefill = Czaszowy
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Według pierwiastka
molecule-colour-chain = Według łańcucha
molecule-colour-ss = Według struktury drugorzędowej
molecule-colour-bfactor = Według czynnika B
molecule-colour-rainbow = Tęcza (od końca N do C)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } atom
    [few] { $atoms } atomy
    [many] { $atoms } atomów
   *[other] { $atoms } atomu
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atom { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Odległość { $distance } Å
molecule-pick-hint = Kliknij atom, aby go wybrać, a drugi, aby zmierzyć odległość
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Grubsze linie prądu
palette-tracts-thinner = Cieńsze linie prądu
palette-tracts-colour = Koloruj linie prądu według następnego schematu
cheatsheet-tracts = Traktogramy
disabled-not-points-or-tracts = Tylko dla chmur punktów i traktogramów (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Według kierunku
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Według { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } linia prądu
    [few] { $streamlines } linie prądu
    [many] { $streamlines } linii prądu
   *[other] { $streamlines } linii prądu
}, { $points ->
    [one] { $points } punkt
    [few] { $points } punkty
    [many] { $points } punktów
   *[other] { $points } punktu
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, grubość { $width } mm
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Następna nieprzezroczystość linii prądu
disabled-not-tracts = Tylko dla traktogramów (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Nieprzezroczystość { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Łączność: { $seed }
dconn-pick-hint = Kliknij powierzchnię, aby pokazać łączność innego wierzchołka
# Toasts after a click on the surface.
dconn-no-seed = Brak wiersza łączności dla tego wierzchołka
dconn-row-failed = Nie udało się odczytać wiersza łączności tego wierzchołka
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } kafelek narysowany
    [few] { $drawn } kafelki narysowane
    [many] { $drawn } kafelków narysowanych
   *[other] { $drawn } kafelka narysowanego
} z { $read } odczytanych
# machine-drafted, needs native review
mesh-tiles3d-loading = Odczytywanie kafelków potrzebnych dla tego widoku…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Część kafelków pominięto: za dużo do narysowania naraz
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] Nie można pokazać { $count } kafelka (zobacz Informacje)
    [few] Nie można pokazać { $count } kafelków (zobacz Informacje)
    [many] Nie można pokazać { $count } kafelków (zobacz Informacje)
   *[other] Nie można pokazać { $count } kafelka (zobacz Informacje)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Nie udało się odczytać kafelków dla tego widoku: { $reason }
