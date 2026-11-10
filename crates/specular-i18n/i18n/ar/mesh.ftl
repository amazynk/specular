# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. العربية. machine-drafted, needs native review

keys-mode-mesh = النماذج ثلاثية الأبعاد
palette-mesh-orbit-left = تدوير النموذج لليسار
palette-mesh-orbit-right = تدوير النموذج لليمين
palette-mesh-orbit-up = تدوير النموذج للأعلى
palette-mesh-orbit-down = تدوير النموذج للأسفل
palette-mesh-view-front = منظر أمامي
palette-mesh-view-top = منظر علوي
palette-mesh-view-side = منظر جانبي
palette-mesh-view-iso = منظر متساوي القياس
palette-mesh-shading = التظليل التالي
palette-mesh-projection = إسقاط منظوري أو متعامد
palette-mesh-ground-grid = إظهار شبكة الأرضية أو إخفاؤها
disabled-not-mesh = للنماذج ثلاثية الأبعاد فقط (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = مظلل
mesh-shading-edges = مظلل مع الحواف
mesh-shading-wireframe = إطار سلكي
mesh-shading-xray = أشعة سينية
mesh-projection-perspective = منظوري
mesh-projection-ortho = متعامد
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = المثلثات: { $triangles }، الرؤوس: { $vertices }
mesh-grid-step = الشبكة { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = نقاط أكبر
palette-mesh-points-smaller = نقاط أصغر
palette-mesh-point-colour = تلوين النقاط حسب السمة التالية
palette-mesh-eye-dome-lighting = تشغيل إضاءة قبة العين (EDL) أو إيقافها
disabled-not-points = للسحب النقطية فقط (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = السحب النقطية
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = الألوان الحقيقية
points-colour-intensity = الشدة
points-colour-elevation = الارتفاع
points-colour-classification = التصنيف
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [zero] لا نقاط
    [one] نقطة واحدة
    [two] نقطتان
    [few] { $count } نقاط
    [many] { $count } نقطة
   *[other] { $count } نقطة
}
points-loading = { $loaded } من { $total ->
        [zero] { $total } نقطة
        [one] نقطة واحدة
        [two] نقطتين
        [few] { $total } نقاط
        [many] { $total } نقطة
       *[other] { $total } نقطة
    }
# $size: the points' diameter in pixels.
points-size = النقاط { $size } px
points-edl = إضاءة قبة العين (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [zero] { $count } لطخة
    [one] لطخة واحدة
    [two] لطختان
    [few] { $count } لطخات
    [many] { $count } لطخة
   *[other] { $count } لطخة
}
splats-harmonics = لون يعتمد على زاوية الرؤية، الدرجة { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = ارسم الجزيء بالنمط التالي
palette-mesh-molecule-colour = لوّن الجزيء بالمخطط التالي
disabled-not-molecule = للبنى الجزيئية فقط (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = الجزيئات
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = شريطي
molecule-style-ball-stick = كرة وعصا
molecule-style-spacefill = ملء الفراغ
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = حسب العنصر
molecule-colour-chain = حسب السلسلة
molecule-colour-ss = حسب البنية الثانوية
molecule-colour-bfactor = حسب عامل B
molecule-colour-rainbow = قوس قزح (من الطرف N إلى الطرف C)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [zero] لا ذرات
    [one] ذرة واحدة
    [two] ذرتان
    [few] { $atoms } ذرات
    [many] { $atoms } ذرة
   *[other] { $atoms } ذرة
}
# $style, $colour: the names above.
molecule-style-line = { $style }، { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = الذرة { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = المسافة { $distance } Å
molecule-pick-hint = انقر ذرة لاختيارها، وذرة ثانية لقياس المسافة
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = خطوط انسياب أسمك
palette-tracts-thinner = خطوط انسياب أرفع
palette-tracts-colour = لوّن خطوط الانسياب بالمخطط التالي
cheatsheet-tracts = مخططات المسالك العصبية
disabled-not-points-or-tracts = للسحب النقطية ومخططات المسالك العصبية فقط (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = حسب الاتجاه
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = حسب { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [zero] لا خطوط انسياب
    [one] خط انسياب واحد
    [two] خطا انسياب
    [few] { $streamlines } خطوط انسياب
    [many] { $streamlines } خط انسياب
   *[other] { $streamlines } خط انسياب
}، { $points ->
    [zero] لا نقاط
    [one] نقطة واحدة
    [two] نقطتان
    [few] { $points } نقاط
    [many] { $points } نقطة
   *[other] { $points } نقطة
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }، بسماكة { $width } مم
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = عتامة خطوط الانسياب التالية
disabled-not-tracts = لمخططات المسالك العصبية فقط (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = العتامة { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = اتصالية { $seed }
dconn-pick-hint = انقر على السطح لعرض اتصالية رأس آخر
# Toasts after a click on the surface.
dconn-no-seed = لا يوجد صف اتصالية لهذا الرأس
dconn-row-failed = تعذّرت قراءة صف اتصالية هذا الرأس
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [zero] رُسمت { $drawn } بلاطة
    [one] رُسمت بلاطة واحدة
    [two] رُسمت بلاطتان
    [few] رُسمت { $drawn } بلاطات
    [many] رُسمت { $drawn } بلاطة
   *[other] رُسمت { $drawn } بلاطة
} من أصل { $read } مقروءة
# machine-drafted, needs native review
mesh-tiles3d-loading = جارٍ قراءة البلاطات التي يحتاجها هذا المنظر…
# machine-drafted, needs native review
mesh-tiles3d-left-out = بعض البلاطات مستبعدة: أكثر مما يمكن رسمه دفعة واحدة
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [zero] يتعذّر عرض { $count } بلاطة (انظر المعلومات)
    [one] يتعذّر عرض بلاطة واحدة (انظر المعلومات)
    [two] يتعذّر عرض بلاطتين (انظر المعلومات)
    [few] يتعذّر عرض { $count } بلاطات (انظر المعلومات)
    [many] يتعذّر عرض { $count } بلاطة (انظر المعلومات)
   *[other] يتعذّر عرض { $count } بلاطة (انظر المعلومات)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = تعذّرت قراءة بلاطات هذا المنظر: { $reason }
