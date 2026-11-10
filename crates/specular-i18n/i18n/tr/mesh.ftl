# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. Türkçe. machine-drafted, needs native review

keys-mode-mesh = 3D modeller
palette-mesh-orbit-left = Modeli sola döndür
palette-mesh-orbit-right = Modeli sağa döndür
palette-mesh-orbit-up = Modeli yukarı döndür
palette-mesh-orbit-down = Modeli aşağı döndür
palette-mesh-view-front = Önden görünüm
palette-mesh-view-top = Üstten görünüm
palette-mesh-view-side = Yandan görünüm
palette-mesh-view-iso = İzometrik görünüm
palette-mesh-shading = Sonraki gölgelendirme
palette-mesh-projection = Perspektif veya ortografik
palette-mesh-ground-grid = Zemin ızgarasını göster veya gizle
disabled-not-mesh = Yalnızca 3D modeller için (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = Gölgeli
mesh-shading-edges = Kenarlı gölgeli
mesh-shading-wireframe = Tel kafes
mesh-shading-xray = Röntgen
mesh-projection-perspective = Perspektif
mesh-projection-ortho = Ortografik
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } üçgen, { $vertices } köşe
mesh-grid-step = Izgara { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = Daha büyük noktalar
palette-mesh-points-smaller = Daha küçük noktalar
palette-mesh-point-colour = Noktaları sonraki özniteliğe göre renklendir
palette-mesh-eye-dome-lighting = Göz kubbesi aydınlatmasını (EDL) aç veya kapat
disabled-not-points = Yalnızca nokta bulutları için (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = Nokta bulutları
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = Gerçek renk
points-colour-intensity = Yoğunluk
points-colour-elevation = Yükseklik
points-colour-classification = Sınıflandırma
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
   *[other] { $count } nokta
}
points-loading = { $loaded } / { $total } nokta
# $size: the points' diameter in pixels.
points-size = Noktalar { $size } px
points-edl = Göz kubbesi aydınlatması (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
   *[other] { $count } splat
}
splats-harmonics = Görüş açısına bağlı renk, derece { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = Molekülü sonraki stilde çiz
palette-mesh-molecule-colour = Molekülü sonraki şemaya göre renklendir
disabled-not-molecule = Yalnızca molekül yapıları için (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = Moleküller
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = Şerit
molecule-style-ball-stick = Top ve çubuk
molecule-style-spacefill = Uzay dolgulu
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = Elemente göre
molecule-colour-chain = Zincire göre
molecule-colour-ss = İkincil yapıya göre
molecule-colour-bfactor = B faktörüne göre
molecule-colour-rainbow = Gökkuşağı (N ucundan C ucuna)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
   *[other] { $atoms } atom
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = Atom { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = Uzaklık { $distance } Å
molecule-pick-hint = Seçmek için bir atoma, uzaklık için ikinci bir atoma tıklayın
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = Daha kalın akım çizgileri
palette-tracts-thinner = Daha ince akım çizgileri
palette-tracts-colour = Akım çizgilerini sonraki şemaya göre renklendir
cheatsheet-tracts = Traktogramlar
disabled-not-points-or-tracts = Yalnızca nokta bulutları ve traktogramlar için (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = Yöne göre
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = { $name } değerine göre
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
   *[other] { $streamlines } akım çizgisi
}, { $points ->
   *[other] { $points } nokta
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, { $width } mm kalınlık
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = Sonraki akım çizgisi opaklığı
disabled-not-tracts = Yalnızca traktogramlar için (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = Opaklık { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = { $seed } bağlantısallığı
dconn-pick-hint = Başka bir köşenin bağlantısallığını göstermek için yüzeye tıklayın
# Toasts after a click on the surface.
dconn-no-seed = Bu köşe için bağlantısallık satırı yok
dconn-row-failed = Bu köşenin bağlantısallık satırı okunamadı
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
   *[other] Okunan { $read } döşemeden { $drawn } tanesi çizildi
}
# machine-drafted, needs native review
mesh-tiles3d-loading = Bu görünümün gerektirdiği döşemeler okunuyor…
# machine-drafted, needs native review
mesh-tiles3d-left-out = Bazı döşemeler dışarıda bırakıldı: bir kerede çizilemeyecek kadar çok
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
   *[other] { $count } döşeme gösterilemiyor (Bilgi’ye bakın)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = Bu görünümün döşemeleri okunamadı: { $reason }
