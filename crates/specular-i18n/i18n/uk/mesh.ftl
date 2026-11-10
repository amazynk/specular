# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Українська. machine-drafted, needs native review

keys-mode-mesh = 3D-моделі
palette-mesh-orbit-left = Повернути модель ліворуч
palette-mesh-orbit-right = Повернути модель праворуч
palette-mesh-orbit-up = Повернути модель вгору
palette-mesh-orbit-down = Повернути модель вниз
palette-mesh-view-front = Вигляд спереду
palette-mesh-view-top = Вигляд згори
palette-mesh-view-side = Вигляд збоку
palette-mesh-view-iso = Ізометричний вигляд
palette-mesh-shading = Наступне затінення
palette-mesh-projection = Перспективна або ортографічна проєкція
palette-mesh-ground-grid = Показати або сховати сітку підлоги
disabled-not-mesh = Лише для 3D-моделей (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Затінений
mesh-shading-edges = Затінений з ребрами
mesh-shading-wireframe = Каркас
mesh-shading-xray = Рентген
mesh-projection-perspective = Перспективна
mesh-projection-ortho = Ортографічна
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = Трикутники: { $triangles }, вершини: { $vertices }
mesh-grid-step = Сітка { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Більші точки
palette-mesh-points-smaller = Менші точки
palette-mesh-point-colour = Розфарбувати точки за наступним атрибутом
palette-mesh-eye-dome-lighting = Увімкнути або вимкнути освітлення Eye-Dome (EDL)
disabled-not-points = Лише для хмар точок (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Хмари точок
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Справжні кольори
points-colour-intensity = Інтенсивність
points-colour-elevation = Висота
points-colour-classification = Класифікація
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] { $count } точка
    [few] { $count } точки
    [many] { $count } точок
   *[other] { $count } точки
}
points-loading = { $loaded } з { $total ->
        [one] { $total } точки
        [few] { $total } точок
        [many] { $total } точок
       *[other] { $total } точки
    }
# $size: the points' diameter in pixels.
points-size = Точки { $size } px
points-edl = Освітлення Eye-Dome (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] { $count } пляма
    [few] { $count } плями
    [many] { $count } плям
   *[other] { $count } плями
}
splats-harmonics = Колір залежно від напрямку погляду, степінь { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Показати молекулу в наступному стилі
palette-mesh-molecule-colour = Розфарбувати молекулу за наступною схемою
disabled-not-molecule = Лише для молекулярних структур (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Молекули
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Стрічки
molecule-style-ball-stick = Кульки й стрижні
molecule-style-spacefill = Заповнення простору
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = За елементом
molecule-colour-chain = За ланцюгом
molecule-colour-ss = За вторинною структурою
molecule-colour-bfactor = За B-фактором
molecule-colour-rainbow = Веселка (від N- до C-кінця)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] { $atoms } атом
    [few] { $atoms } атоми
    [many] { $atoms } атомів
   *[other] { $atoms } атома
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Атом { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Відстань { $distance } Å
molecule-pick-hint = Клацніть атом, щоб вибрати його, а другий — для відстані
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Товщі лінії струму
palette-tracts-thinner = Тонші лінії струму
palette-tracts-colour = Розфарбувати лінії струму за наступною схемою
cheatsheet-tracts = Трактограми
disabled-not-points-or-tracts = Лише для хмар точок і трактограм (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = За напрямком
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = За { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] { $streamlines } лінія струму
    [few] { $streamlines } лінії струму
    [many] { $streamlines } ліній струму
   *[other] { $streamlines } лінії струму
}, { $points ->
    [one] { $points } точка
    [few] { $points } точки
    [many] { $points } точок
   *[other] { $points } точки
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, товщина { $width } мм
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Наступна непрозорість ліній струму
disabled-not-tracts = Лише для трактограм (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Непрозорість { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = Зв’язність: { $seed }
dconn-pick-hint = Клацніть поверхню, щоб показати зв’язність іншої вершини
# Toasts after a click on the surface.
dconn-no-seed = Немає рядка зв’язності для цієї вершини
dconn-row-failed = Не вдалося прочитати рядок зв’язності цієї вершини
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] { $drawn } плитку намальовано
    [few] { $drawn } плитки намальовано
    [many] { $drawn } плиток намальовано
   *[other] { $drawn } плитки намальовано
} з { $read } прочитаних
# machine-drafted, needs native review
mesh-tiles3d-loading = Читання плиток, потрібних для цього вигляду…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Деякі плитки пропущено: забагато, щоб намалювати все одразу
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] { $count } плитку не вдається показати (див. Відомості)
    [few] { $count } плитки не вдається показати (див. Відомості)
    [many] { $count } плиток не вдається показати (див. Відомості)
   *[other] { $count } плитки не вдається показати (див. Відомості)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Не вдалося прочитати плитки для цього вигляду: { $reason }
