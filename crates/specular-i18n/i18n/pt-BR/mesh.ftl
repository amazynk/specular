# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Português (Brasil). machine-drafted, needs native review

keys-mode-mesh = Modelos 3D
palette-mesh-orbit-left = Girar o modelo para a esquerda
palette-mesh-orbit-right = Girar o modelo para a direita
palette-mesh-orbit-up = Girar o modelo para cima
palette-mesh-orbit-down = Girar o modelo para baixo
palette-mesh-view-front = Vista frontal
palette-mesh-view-top = Vista superior
palette-mesh-view-side = Vista lateral
palette-mesh-view-iso = Vista isométrica
palette-mesh-shading = Próximo sombreamento
palette-mesh-projection = Perspectiva ou ortográfica
palette-mesh-ground-grid = Mostrar ou ocultar a grade do chão
disabled-not-mesh = Somente para modelos 3D (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Sombreado
mesh-shading-edges = Sombreado com arestas
mesh-shading-wireframe = Aramado
mesh-shading-xray = Raio X
mesh-projection-perspective = Perspectiva
mesh-projection-ortho = Ortográfica
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } triângulos, { $vertices } vértices
mesh-grid-step = Grade { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Pontos maiores
palette-mesh-points-smaller = Pontos menores
palette-mesh-point-colour = Colorir os pontos pelo próximo atributo
palette-mesh-eye-dome-lighting = Ativar ou desativar a iluminação Eye-Dome (EDL)
disabled-not-points = Somente para nuvens de pontos (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Nuvens de pontos
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Cor real
points-colour-intensity = Intensidade
points-colour-elevation = Elevação
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
palette-mesh-molecule-style = Desenhar a molécula no próximo estilo
palette-mesh-molecule-colour = Colorir a molécula pelo próximo esquema
disabled-not-molecule = Somente para estruturas moleculares (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Moléculas
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Cartoon
molecule-style-ball-stick = Bolas e bastões
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
molecule-pick-hint = Clique em um átomo para selecioná-lo e em outro para a distância
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Linhas de corrente mais grossas
palette-tracts-thinner = Linhas de corrente mais finas
palette-tracts-colour = Colorir as linhas de corrente pelo próximo esquema
cheatsheet-tracts = Tractogramas
disabled-not-points-or-tracts = Somente para nuvens de pontos e tractogramas (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
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
palette-mesh-tract-opacity = Próxima opacidade das linhas de corrente
disabled-not-tracts = Somente para tractogramas (TrackVis, MRtrix)
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
    [one] { $drawn } bloco desenhado
   *[other] { $drawn } blocos desenhados
} de { $read } lidos
# machine-drafted, needs native review
mesh-tiles3d-loading = Lendo os blocos de que esta vista precisa…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Alguns blocos ficaram de fora: são muitos para desenhar de uma vez
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } bloco não pode ser exibido (veja Informações)
   *[other] { $count } blocos não podem ser exibidos (veja Informações)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Não foi possível ler os blocos desta vista: { $reason }
