# Specular — 3D models (STL, OBJ, PLY, glTF, 3MF): orbit, standard views, shading, projection, ground grid. עברית. machine-drafted, needs native review

keys-mode-mesh = מודלים בתלת-ממד
palette-mesh-orbit-left = סיבוב המודל שמאלה
palette-mesh-orbit-right = סיבוב המודל ימינה
palette-mesh-orbit-up = סיבוב המודל למעלה
palette-mesh-orbit-down = סיבוב המודל למטה
palette-mesh-view-front = מבט מלפנים
palette-mesh-view-top = מבט מלמעלה
palette-mesh-view-side = מבט מהצד
palette-mesh-view-iso = מבט איזומטרי
palette-mesh-shading = ההצללה הבאה
palette-mesh-projection = הטלה פרספקטיבית או אורתוגרפית
palette-mesh-ground-grid = הצגה או הסתרה של רשת הקרקע
disabled-not-mesh = רק למודלים בתלת-ממד (STL, OBJ, PLY, glTF, 3MF)
# Shadings and projections: in the readout, and as a toast when they change.
mesh-shading-shaded = מוצלל
mesh-shading-edges = מוצלל עם קצוות
mesh-shading-wireframe = מסגרת תיל
mesh-shading-xray = רנטגן
mesh-projection-perspective = פרספקטיבית
mesh-projection-ortho = אורתוגרפית
# Readout lines (bottom corner). $x $y $z: the model's size along each axis; $unit e.g. "mm"
# (empty when the file states none). $length: the ground grid's spacing with its unit.
mesh-size = { $x } × { $y } × { $z } { $unit }
mesh-counts = { $triangles } משולשים, { $vertices } קודקודים
mesh-grid-step = רשת { $length }
# Point clouds (LAS, LAZ, COPC, PCD, XYZ, PTS, PLY, E57) in the 3D view.
palette-mesh-points-bigger = נקודות גדולות יותר
palette-mesh-points-smaller = נקודות קטנות יותר
palette-mesh-point-colour = צביעת הנקודות לפי המאפיין הבא
palette-mesh-eye-dome-lighting = הפעלה או כיבוי של תאורת Eye-Dome (EDL)
disabled-not-points = לענני נקודות בלבד (LAS, LAZ, PCD, XYZ, PTS, PLY, E57)
cheatsheet-points = ענני נקודות
# Colourings: in the readout, and as a toast when they change.
points-colour-rgb = צבע אמיתי
points-colour-intensity = עוצמה
points-colour-elevation = גובה
points-colour-classification = סיווג
# Readout lines. $count, $loaded, $total: point counts.
points-count = { $count ->
    [one] נקודה אחת
    [two] שתי נקודות
   *[other] { $count } נקודות
}
points-loading = { $loaded } מתוך { $total ->
        [one] נקודה אחת
        [two] שתי נקודות
       *[other] { $total } נקודות
    }
# $size: the points' diameter in pixels.
points-size = נקודות { $size } px
points-edl = תאורת Eye-Dome (EDL)
# machine-drafted, needs native review
# Gaussian splats (.splat, 3DGS PLY, .spz) in the 3D view. Readout lines. $count: how many
# splats; $degree: the degree (1 to 3) of the view-dependent colour the file stores.
splats-count = { $count ->
    [one] ספלאט אחד
    [two] { $count } ספלאטים
   *[other] { $count } ספלאטים
}
splats-harmonics = צבע תלוי זווית צפייה, דרגה { $degree }
# machine-drafted, needs native review
# Molecular structures (PDB, mmCIF, CIF, SDF, XYZ) in the 3D view.
palette-mesh-molecule-style = הצגת המולקולה בסגנון הבא
palette-mesh-molecule-colour = צביעת המולקולה לפי הסכמה הבאה
disabled-not-molecule = רק למבנים מולקולריים (PDB, mmCIF, CIF, SDF, XYZ)
cheatsheet-molecules = מולקולות
# Styles: in the readout, and as a toast when they change.
molecule-style-cartoon = סרט
molecule-style-ball-stick = כדור ומקל
molecule-style-spacefill = מילוי מרחב
# Colourings: in the readout, and as a toast when they change.
molecule-colour-element = לפי יסוד
molecule-colour-chain = לפי שרשרת
molecule-colour-ss = לפי מבנה שניוני
molecule-colour-bfactor = לפי פקטור B
molecule-colour-rainbow = קשת (מקצה N לקצה C)
# Readout lines. $atoms, $chains: counts.
molecule-counts = { $atoms ->
    [one] אטום אחד
    [two] שני אטומים
   *[other] { $atoms } אטומים
}
# $style, $colour: the names above.
molecule-style-line = { $style }, { $colour }
# A picked atom: $atom names it, e.g. "ALA A42 CA" (residue, chain and number, atom) or "O3".
molecule-picked = אטום { $atom }
# The distance between the two picked atoms, in ångströms.
molecule-distance = מרחק { $distance } Å
molecule-pick-hint = לחצו על אטום כדי לבחור אותו, ועל אטום שני למדידת המרחק
# machine-drafted, needs native review
# Tractograms (TrackVis .trk, MRtrix .tck) in the 3D view. `]` and `[` make the
# streamlines thicker and thinner, `c` steps the colouring.
palette-tracts-thicker = קווי זרימה עבים יותר
palette-tracts-thinner = קווי זרימה דקים יותר
palette-tracts-colour = צביעת קווי הזרימה לפי הסכמה הבאה
cheatsheet-tracts = טרקטוגרמות
disabled-not-points-or-tracts = לענני נקודות ולטרקטוגרמות בלבד (LAS, LAZ, PCD, XYZ, PTS, PLY, E57, TrackVis, MRtrix)
# Colourings: in the readout, and as a toast when they change.
tracts-colour-direction = לפי כיוון
# $name: the per-point value's name in the file, e.g. "fa".
tracts-colour-scalar = לפי { $name }
# Readout lines. $streamlines, $points: counts.
tracts-counts = { $streamlines ->
    [one] קו זרימה אחד
    [two] שני קווי זרימה
   *[other] { $streamlines } קווי זרימה
}, { $points ->
    [one] נקודה אחת
    [two] שתי נקודות
   *[other] { $points } נקודות
}
# $colour: a colouring above; $width: the streamlines' thickness in millimetres.
tracts-look-line = { $colour }, בעובי { $width } מ״מ
# machine-drafted, needs native review
# Alt+O steps the streamlines' opacity: as a palette entry, a toast and in the readout.
palette-mesh-tract-opacity = האטימות הבאה של קווי הזרימה
disabled-not-tracts = לטרקטוגרמות בלבד (TrackVis, MRtrix)
# $percent: 100 is opaque.
tracts-opacity = אטימות { PERCENT($percent) }

## Dense connectivity (CIFTI .dconn.nii) on its surfaces: each grayordinate (a surface
## vertex or a subcortical voxel) is a "seed" whose row of connectivity values colours them.

# machine-drafted, needs native review
# Readout line. $seed: the structure as the file names it and the vertex or voxel, such
# as "CORTEX_LEFT 1234".
dconn-seed = קישוריות של { $seed }
dconn-pick-hint = לחצו על המשטח כדי להציג קישוריות של קודקוד אחר
# Toasts after a click on the surface.
dconn-no-seed = אין שורת קישוריות לקודקוד זה
dconn-row-failed = לא ניתן לקרוא את שורת הקישוריות של קודקוד זה
# 3D Tiles tilesets in the 3D view. Readout lines. $drawn: tiles drawn; $read: tiles read so far.
# machine-drafted, needs native review
mesh-tiles3d = { $drawn ->
    [one] צויר אריח אחד
    [two] צוירו שני אריחים
   *[other] צוירו { $drawn } אריחים
} מתוך { $read } שנקראו
# machine-drafted, needs native review
mesh-tiles3d-loading = קריאת האריחים הדרושים למבט הזה…
# machine-drafted, needs native review
mesh-tiles3d-left-out = חלק מהאריחים הושמטו: יותר מדי לציור בבת אחת
# $count: tiles whose content could not be shown (the Info panel says why).
# machine-drafted, needs native review
mesh-tiles3d-not-drawn = { $count ->
    [one] לא ניתן להציג אריח אחד (ראו מידע)
    [two] לא ניתן להציג שני אריחים (ראו מידע)
   *[other] לא ניתן להציג { $count } אריחים (ראו מידע)
}
# machine-drafted, needs native review
mesh-tiles3d-failed = לא ניתן לקרוא את האריחים של המבט הזה: { $reason }
