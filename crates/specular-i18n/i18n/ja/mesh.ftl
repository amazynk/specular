# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. 日本語. machine-drafted, needs native review

keys-mode-mesh = 3D モデル
palette-mesh-orbit-left = モデルを左に回転
palette-mesh-orbit-right = モデルを右に回転
palette-mesh-orbit-up = モデルを上に回転
palette-mesh-orbit-down = モデルを下に回転
palette-mesh-view-front = 正面図
palette-mesh-view-top = 上面図
palette-mesh-view-side = 側面図
palette-mesh-view-iso = 等角図
palette-mesh-shading = 次のシェーディング
palette-mesh-projection = 透視投影または平行投影
palette-mesh-ground-grid = 地面グリッドの表示/非表示
disabled-not-mesh = 3D モデル (STL, OBJ, PLY, glTF, 3MF) のみ
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = シェーディング
mesh-shading-edges = エッジ付きシェーディング
mesh-shading-wireframe = ワイヤーフレーム
mesh-shading-xray = X 線
mesh-projection-perspective = 透視投影
mesh-projection-ortho = 平行投影
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = 三角形 { $triangles }、頂点 { $vertices }
mesh-grid-step = グリッド { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = 点を大きく
palette-mesh-points-smaller = 点を小さく
palette-mesh-point-colour = 次の属性で点を色分け
palette-mesh-eye-dome-lighting = アイドームライティング (EDL) のオン/オフ
disabled-not-points = 点群 (LAS, LAZ, PCD, XYZ, PTS, PLY, E57) のみ
cheatsheet-points = 点群
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = トゥルーカラー
points-colour-intensity = 強度
points-colour-elevation = 標高
points-colour-classification = 分類
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
   *[other] { $count } 点
}
points-loading = { $total } 点中 { $loaded } 点
# $size: the points' diameter in pixels.
points-size = 点サイズ { $size } px
points-edl = アイドームライティング (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
   *[other] スプラット { $count } 個
}
splats-harmonics = 視点依存の色、次数 { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = 分子を次のスタイルで描画
palette-mesh-molecule-colour = 分子を次の配色で着色
disabled-not-molecule = 分子構造 (PDB, mmCIF, CIF, SDF, XYZ) のみ
cheatsheet-molecules = 分子
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = カートゥーン
molecule-style-ball-stick = ボール＆スティック
molecule-style-spacefill = 空間充填
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = 元素別
molecule-colour-chain = 鎖別
molecule-colour-ss = 二次構造別
molecule-colour-bfactor = B 因子別
molecule-colour-rainbow = レインボー (N 末端から C 末端)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
   *[other] { $atoms } 原子
}
# $style, $colour: the names above.
molecule-style-line = { $style }、{ $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = 原子 { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = 距離 { $distance } Å
molecule-pick-hint = 原子をクリックして選択、2 つ目で距離を表示
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = ストリームラインを太く
palette-tracts-thinner = ストリームラインを細く
palette-tracts-colour = ストリームラインを次の配色で着色
cheatsheet-tracts = トラクトグラム
disabled-not-points-or-tracts = 点群とトラクトグラム (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix) のみ
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = 方向別
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = { $name } 別
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
   *[other] ストリームライン { $streamlines } 本
}、{ $points ->
   *[other] { $points } 点
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }、太さ { $width } mm
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = 次のストリームライン不透明度
disabled-not-tracts = トラクトグラム (TrackVis, MRtrix) のみ
# $percent: 100 is opaque.
tracts-opacity = 不透明度 { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = { $seed } の結合性
dconn-pick-hint = サーフェスをクリックすると別の頂点の結合性を表示します
# Toasts after a click on the surface.
dconn-no-seed = この頂点の結合性の行がありません
dconn-row-failed = この頂点の結合性の行を読み込めませんでした
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
   *[other] 読み込んだ { $read } 個のうち { $drawn } 個のタイルを描画
}
# machine-drafted, needs native review
mesh-tiles3d-loading = このビューに必要なタイルを読み込んでいます…
# machine-drafted, needs native review
mesh-tiles3d-left-out = 一部のタイルを省略しています：一度に描画するには多すぎます
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
   *[other] { $count } 個のタイルを表示できません（情報を参照）
}
# machine-drafted, needs native review
mesh-tiles3d-failed = このビューのタイルを読み込めませんでした：{ $reason }
