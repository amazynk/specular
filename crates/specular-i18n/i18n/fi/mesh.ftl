# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Suomi. machine-drafted, needs native review

keys-mode-mesh = 3D-mallit
palette-mesh-orbit-left = Käännä mallia vasemmalle
palette-mesh-orbit-right = Käännä mallia oikealle
palette-mesh-orbit-up = Käännä mallia ylöspäin
palette-mesh-orbit-down = Käännä mallia alaspäin
palette-mesh-view-front = Näkymä edestä
palette-mesh-view-top = Näkymä ylhäältä
palette-mesh-view-side = Näkymä sivulta
palette-mesh-view-iso = Isometrinen näkymä
palette-mesh-shading = Seuraava varjostus
palette-mesh-projection = Perspektiivi tai ortografinen
palette-mesh-ground-grid = Näytä tai piilota lattiaruudukko
disabled-not-mesh = Vain 3D-malleille (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Varjostettu
mesh-shading-edges = Varjostettu ja reunat
mesh-shading-wireframe = Rautalanka
mesh-shading-xray = Röntgen
mesh-projection-perspective = Perspektiivi
mesh-projection-ortho = Ortografinen
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } kolmiota, { $vertices } kärkipistettä
mesh-grid-step = Ruudukko { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Suuremmat pisteet
palette-mesh-points-smaller = Pienemmät pisteet
palette-mesh-point-colour = Väritä pisteet seuraavan ominaisuuden mukaan
palette-mesh-eye-dome-lighting = Eye-dome-valaistus (EDL) päälle tai pois
disabled-not-points = Vain pistepilville (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Pistepilvet
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Todelliset värit
points-colour-intensity = Intensiteetti
points-colour-elevation = Korkeus
points-colour-classification = Luokittelu
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } piste
   *[other] { $count } pistettä
}
points-loading = { $loaded } / { $total ->
        [one] { $total } piste
       *[other] { $total } pistettä
    }
# $size: the points' diameter in pixels.
points-size = Pisteet { $size } px
points-edl = Eye-dome-valaistus (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
   *[other] { $count } splattia
}
splats-harmonics = Katselukulmasta riippuva väri, aste { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Piirrä molekyyli seuraavalla tyylillä
palette-mesh-molecule-colour = Väritä molekyyli seuraavalla värityksellä
disabled-not-molecule = Vain molekyylirakenteille (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Molekyylit
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Nauhamalli
molecule-style-ball-stick = Pallotikkumalli
molecule-style-spacefill = Tilaa täyttävä
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Alkuaineen mukaan
molecule-colour-chain = Ketjun mukaan
molecule-colour-ss = Sekundaarirakenteen mukaan
molecule-colour-bfactor = B-tekijän mukaan
molecule-colour-rainbow = Sateenkaari (N-päästä C-päähän)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } atomi
   *[other] { $atoms } atomia
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atomi { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Etäisyys { $distance } Å
molecule-pick-hint = Valitse atomi napsauttamalla, toinen etäisyyttä varten
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Paksummat virtaviivat
palette-tracts-thinner = Ohuemmat virtaviivat
palette-tracts-colour = Väritä virtaviivat seuraavalla värityksellä
cheatsheet-tracts = Traktogrammit
disabled-not-points-or-tracts = Vain pistepilville ja traktogrammeille (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Suunnan mukaan
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Arvon { $name } mukaan
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } virtaviiva
   *[other] { $streamlines } virtaviivaa
}, { $points ->
    [one] { $points } piste
   *[other] { $points } pistettä
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, paksuus { $width } mm
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Seuraava virtaviivojen peittävyys
disabled-not-tracts = Vain traktogrammeille (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Peittävyys { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Konnektiivisuus: { $seed }
dconn-pick-hint = Napsauta pintaa nähdäksesi toisen kärkipisteen konnektiivisuuden
# Toasts after a click on the surface.
dconn-no-seed = Tälle kärkipisteelle ei ole konnektiivisuusriviä
dconn-row-failed = Tämän kärkipisteen konnektiivisuusriviä ei voitu lukea
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } ruutu piirretty
   *[other] { $drawn } ruutua piirretty
} { $read } luetusta
# machine-drafted, needs native review
mesh-tiles3d-loading = Luetaan tämän näkymän tarvitsemia ruutuja…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Osa ruuduista jätetään pois: liikaa piirrettäväksi kerralla
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } ruutua ei voida näyttää (katso Tiedot)
   *[other] { $count } ruutua ei voida näyttää (katso Tiedot)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Tämän näkymän ruutuja ei voitu lukea: { $reason }
