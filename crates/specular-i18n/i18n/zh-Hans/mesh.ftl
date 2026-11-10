# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. 简体中文. machine-drafted, needs native review

keys-mode-mesh = 3D 模型
palette-mesh-orbit-left = 向左旋转模型
palette-mesh-orbit-right = 向右旋转模型
palette-mesh-orbit-up = 向上旋转模型
palette-mesh-orbit-down = 向下旋转模型
palette-mesh-view-front = 前视图
palette-mesh-view-top = 俯视图
palette-mesh-view-side = 侧视图
palette-mesh-view-iso = 等轴测视图
palette-mesh-shading = 下一着色方式
palette-mesh-projection = 透视或正交
palette-mesh-ground-grid = 显示或隐藏地面网格
disabled-not-mesh = 仅适用于 3D 模型 (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = 明暗着色
mesh-shading-edges = 明暗着色加边线
mesh-shading-wireframe = 线框
mesh-shading-xray = X 光
mesh-projection-perspective = 透视
mesh-projection-ortho = 正交
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } 个三角形，{ $vertices } 个顶点
mesh-grid-step = 网格 { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = 增大点
palette-mesh-points-smaller = 减小点
palette-mesh-point-colour = 按下一个属性为点着色
palette-mesh-eye-dome-lighting = 开启或关闭眼穹光照 (EDL)
disabled-not-points = 仅适用于点云 (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = 点云
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = 真彩色
points-colour-intensity = 强度
points-colour-elevation = 高程
points-colour-classification = 分类
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
   *[other] { $count } 个点
}
points-loading = { $loaded } / { $total } 个点
# $size: the points' diameter in pixels.
points-size = 点大小 { $size } px
points-edl = 眼穹光照 (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
   *[other] { $count } 个泼溅点
}
splats-harmonics = 视角相关颜色，阶数 { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = 用下一种样式绘制分子
palette-mesh-molecule-colour = 按下一种方案为分子着色
disabled-not-molecule = 仅适用于分子结构 (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = 分子
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = 卡通
molecule-style-ball-stick = 球棍
molecule-style-spacefill = 空间填充
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = 按元素
molecule-colour-chain = 按链
molecule-colour-ss = 按二级结构
molecule-colour-bfactor = 按 B 因子
molecule-colour-rainbow = 彩虹（N 端到 C 端）
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
   *[other] { $atoms } 个原子
}
# $style, $colour: the names above.
molecule-style-line = { $style }、{ $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = 原子 { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = 距离 { $distance } Å
molecule-pick-hint = 点击一个原子将其选中，再点击第二个以测量距离
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = 加粗流线
palette-tracts-thinner = 变细流线
palette-tracts-colour = 按下一种方案为流线着色
cheatsheet-tracts = 纤维束图
disabled-not-points-or-tracts = 仅适用于点云和纤维束图 (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = 按方向
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = 按 { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
   *[other] { $streamlines } 条流线
}、{ $points ->
   *[other] { $points } 个点
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }，粗 { $width } mm
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = 下一流线不透明度
disabled-not-tracts = 仅适用于纤维束图 (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = 不透明度 { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = { $seed } 的连接
dconn-pick-hint = 点击表面以显示另一个顶点的连接
# Toasts after a click on the surface.
dconn-no-seed = 此顶点没有连接行
dconn-row-failed = 无法读取此顶点的连接行
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
   *[other] 已绘制 { $drawn } 个图块，共读取 { $read } 个
}
# machine-drafted, needs native review
mesh-tiles3d-loading = 正在读取此视图所需的图块…
# machine-drafted, needs native review
mesh-tiles3d-left-out = 部分图块已省略：一次要绘制的内容太多
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
   *[other] { $count } 个图块无法显示（见“信息”）
}
# machine-drafted, needs native review
mesh-tiles3d-failed = 无法读取此视图的图块：{ $reason }
