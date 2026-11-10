# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Português (Portugal). machine-drafted, needs native review

keys-mode-mesh = Modelos 3D
palette-mesh-orbit-left = Rodar o modelo para a esquerda
palette-mesh-orbit-right = Rodar o modelo para a direita
palette-mesh-orbit-up = Rodar o modelo para cima
palette-mesh-orbit-down = Rodar o modelo para baixo
palette-mesh-view-front = Vista frontal
palette-mesh-view-top = Vista de cima
palette-mesh-view-side = Vista lateral
palette-mesh-view-iso = Vista isométrica
palette-mesh-shading = Sombreamento seguinte
palette-mesh-projection = Perspetiva ou ortográfica
palette-mesh-ground-grid = Mostrar ou ocultar a grelha do chão
disabled-not-mesh = Apenas para modelos 3D (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Sombreado
mesh-shading-edges = Sombreado com arestas
mesh-shading-wireframe = Malha de arame
mesh-shading-xray = Raio X
mesh-projection-perspective = Perspetiva
mesh-projection-ortho = Ortográfica
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } triângulos, { $vertices } vértices
mesh-grid-step = Grelha { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Pontos maiores
palette-mesh-points-smaller = Pontos mais pequenos
palette-mesh-point-colour = Colorir os pontos pelo atributo seguinte
palette-mesh-eye-dome-lighting = Ativar ou desativar a iluminação Eye-Dome (EDL)
disabled-not-points = Apenas para nuvens de pontos (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Nuvens de pontos
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Cor real
points-colour-intensity = Intensidade
points-colour-elevation = Altitude
points-colour-classification = Classificação
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } ponto
   *[other] { $count } pontos
}
points-loading = { $loaded } de { $total ->
        [one] { $total } ponto
       *[other] { $total } pontos
    }
# $size: the points' diameter in pixels.
points-size = Pontos { $size } px
points-edl = Iluminação Eye-Dome (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } splat
   *[other] { $count } splats
}
splats-harmonics = Cor dependente da vista, grau { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Desenhar a molécula no estilo seguinte
palette-mesh-molecule-colour = Colorir a molécula pelo esquema seguinte
disabled-not-molecule = Apenas para estruturas moleculares (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Moléculas
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Bolas e varetas
molecule-style-spacefill = Preenchimento espacial
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Por elemento
molecule-colour-chain = Por cadeia
molecule-colour-ss = Por estrutura secundária
molecule-colour-bfactor = Por fator B
molecule-colour-rainbow = Arco-íris (do terminal N ao C)
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
molecule-distance = Distância { $distance } Å
molecule-pick-hint = Clique num átomo para o selecionar e noutro para a distância
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Linhas de corrente mais grossas
palette-tracts-thinner = Linhas de corrente mais finas
palette-tracts-colour = Colorir as linhas de corrente pelo esquema seguinte
cheatsheet-tracts = Tractogramas
disabled-not-points-or-tracts = Apenas para nuvens de pontos e tractogramas (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Por direção
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = Por { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } linha de corrente
    [many] { $streamlines } de linhas de corrente
   *[other] { $streamlines } linhas de corrente
}, { $points ->
    [one] { $points } ponto
    [many] { $points } de pontos
   *[other] { $points } pontos
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, { $width } mm de espessura
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Opacidade das linhas de corrente seguinte
disabled-not-tracts = Apenas para tractogramas (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Opacidade { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Conectividade de { $seed }
dconn-pick-hint = Clique na superfície para mostrar a conectividade de outro vértice
# Toasts after a click on the surface.
dconn-no-seed = Nenhuma linha de conectividade para este vértice
dconn-row-failed = Não foi possível ler a linha de conectividade deste vértice
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } mosaico desenhado
   *[other] { $drawn } mosaicos desenhados
} de { $read } lidos
# machine-drafted, needs native review
mesh-tiles3d-loading = A ler os mosaicos de que esta vista precisa…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Alguns mosaicos ficaram de fora: são demasiados para desenhar de uma vez
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } mosaico não pode ser mostrado (ver Informações)
   *[other] { $count } mosaicos não podem ser mostrados (ver Informações)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Não foi possível ler os mosaicos desta vista: { $reason }
