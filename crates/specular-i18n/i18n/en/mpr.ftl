# Multiplanar reformatting (MPR): any volume (a series, a multi-frame file, a 3D volume,
# a microscopy z-stack), shown in three orthogonal views with linked crosshairs.

palette-mpr = Multiplanar reformat (MPR)
# The toolbar's MPR button (shown while the image is a volume) and its Settings name.
toolbar-mpr = Three planes (MPR)
toolbar-mpr-close = Close three planes (MPR)
settings-viewing-toolbar-items-mpr = Three planes (MPR)
palette-mpr-slab = Next MPR slab (thin, MIP, MinIP, average)
palette-mpr-preset = Next 3D preset (CT bone, soft tissue, lung, angio; MR)
palette-mpr-render = Next 3D rendering (shaded, cinematic, MIP, MinIP)
palette-mpr-clip = Next 3D clipping (none, the 2D views’ box, a corner cut away)
palette-mpr-export-stl = Export the 3D surface as STL…
palette-mpr-tract-slab = Next streamline slab in MPR
palette-mpr-maximise = Maximise the MPR view, or show all views
palette-mpr-export-planes = Export the MPR planes as images…
disabled-no-volume = This is not a volume: open a series, a multi-frame file, a 3D volume or a z-stack
disabled-mpr-closed = Open the multiplanar views first

# The three views and the 3D view in the fourth slot.
mpr-axial = Axial
mpr-coronal = Coronal
mpr-sagittal = Sagittal
mpr-oblique = Oblique
# The views of a volume without patient orientation (a microscopy stack, an industrial CT):
# the volume's axes each view shows.
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D view

# Slab modes, as buttons and as toasts.
mpr-thin = Thin
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Average
mpr-slab = Slab: { $mode }, { $thickness } mm
# The same on a density map or cube grid, whose lengths are Ångström.
mpr-slab-angstrom = Slab: { $mode }, { $thickness } Å
# The same on a microscopy stack; $unit: µm, nm.
mpr-slab-unit = Slab: { $mode }, { $thickness } { $unit }
# The same when the file gives no voxel size: the thickness in voxels.
mpr-slab-voxels = Slab: { $mode }, { $thickness } voxels
mpr-thicker = Thicker slab
mpr-thinner = Thinner slab
mpr-reset = Reset views
mpr-close = Close MPR
mpr-more = More…
# A toggle: the active view fills the area (the controls stay), or all views again.
mpr-maximise = Maximise view
mpr-export-planes = Export planes
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Streamlines from { $files }: { $thickness } mm slab
mpr-tracts-off = Streamlines from { $files }: hidden
mpr-tracts-none = No TrackVis or MRtrix streamlines beside this volume

# Views and readouts.
mpr-position = { $view } { $position } mm
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
# No voxel size known: the position in voxels.
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } voxels
# What a stack's other axes show in MPR: "Channel: DAPI", "T: 3 of 12".
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } of { $count }
mpr-axis-channel = Channel
mpr-axis-unnamed = Axis { $axis }
mpr-one-channel = MPR shows one channel at a time
# $count: channels drawn together in their colours.
mpr-composite = { $count } channels in colour, each at its display range
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = One channel shown: { $reason }
mpr-isotropic = No voxel size in the file: voxels shown as cubes
mpr-isotropic-z = No slice spacing in the file: slices shown one pixel apart
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Loading { $loaded } of { $count ->
        [one] { $count } slice
       *[other] { $count } slices
    }
mpr-loaded = { $count ->
        [one] { $count } slice
       *[other] { $count } slices
    }, { $width } × { $height }
mpr-downsampled = Shown at 1/{ $factor } resolution (memory or graphics limits)
mpr-converted = Values shown as 32-bit floats
mpr-opening = Opening the volume…
mpr-gpu-memory = { $size } on the graphics card

# Failures, as toasts. $reason is the volume's own explanation (gaps, uneven spacing, …).
mpr-refused = Cannot build a volume: { $reason }
mpr-failed = The volume did not load: { $reason }
mpr-no-gpu = The graphics card cannot hold this volume: { $reason }
mpr-planes-saved = { $count ->
    [one] Saved { $count } plane to { $folder }
   *[other] Saved { $count } planes to { $folder }
}
mpr-planes-failed = The planes could not be saved: { $reason }

# Accessibility.
mpr-view-a11y = { $view } view, { $position } mm
mpr-crosshair-a11y = Crosshair at { $x }, { $y }, { $z } mm
mpr-crosshair-a11y-angstrom = Crosshair at { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Crosshair at { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Crosshair at voxel { $x }, { $y }, { $z }
mpr-controls = MPR controls

# The 3D view: its transfer function presets, renderings and clipping, as buttons and
# toasts; the surface exported as STL at the transfer function's threshold.
vr-preset = { $preset ->
    [bone] CT bone
    [soft] CT soft tissue
    [lung] CT lung
    [angio] CT angio
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Shaded
}
vr-clip = { $clip ->
    [box] Clip: views’ box
    [cut] Clip: corner cut
   *[off] No clipping
}
vr-smooth = Smooth surface
vr-export = Export STL
vr-threshold = Surface at { $value }
vr-stl-exporting = Building the surface…
vr-stl-saved = { $count ->
    [0] Nothing at this threshold: no surface saved
   *[other] Saved { $count } triangles to { $name }
}
vr-stl-failed = The surface could not be saved: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, surface at { $threshold }. Shift+arrows orbit, plus and minus zoom.

## Raw volume: how a headerless .raw / .vol file is laid out

raw-volume-title = Open raw volume
raw-volume-file = { $file } has no header. Say how its samples are laid out.
raw-volume-guess-label = Suggested layout
raw-volume-guess = { $index } of { $count }
raw-volume-custom = Custom
raw-volume-width = Width (voxels)
raw-volume-height = Height (voxels)
raw-volume-slices = Slices
raw-volume-type = Sample type
raw-volume-type-unsigned = Unsigned { $bits }-bit
raw-volume-type-signed = Signed { $bits }-bit
raw-volume-type-float = Float { $bits }-bit
raw-volume-order = Byte order
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Header offset
raw-volume-bytes = { $count ->
    [one] 1 byte
   *[other] { $count } bytes
}
raw-volume-pixel = Pixel size
raw-volume-spacing = Slice spacing
raw-volume-mm = { $value } mm
raw-volume-exact = Fills the file exactly ({ $size })
raw-volume-fits = Uses { $size } of { $file }; { $left } left over
raw-volume-too-big = Needs { $size }, but the file is { $file }
raw-volume-cancel = Cancel
raw-volume-open = Open
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Connectivity of { $seed }; move the crosshair to another seed

# Editing a length measured in MPR: its ends can be dragged.
mpr-measure-hint-edit = Drag an end to move it; onto the other end to remove it
mpr-measure-removed = Measurement removed
mpr-measure-lengths-only = MPR measures lengths only

## Curved MPR: a reformat along a curve clicked on a plane.

palette-mpr-curve = Curved MPR along a clicked curve
palette-mpr-curve-layout = Curved MPR: straightened or stretched
palette-mpr-curve-save = Save the curve…
palette-mpr-curve-load = Load a curve…
palette-mpr-export-segments = Export each segment as STL…
palette-mpr-dental = Dental layout (panoramic and cross-sections)
palette-mpr-nerve = Trace the nerve canal
cpr-kind = { $kind ->
    [stretched] Stretched
   *[straightened] Straightened
}
cpr-button = Curved
cpr-controls = Curved MPR controls
cpr-at = At { $position }
cpr-section = Cross-section
cpr-hint-start = Click points along the structure in the { $view } view
cpr-hint-more = Click one more point
cpr-add-points = Add points
cpr-undo-point = Undo point
cpr-mark = Mark here
cpr-mark-set = Mark { $n } set.
cpr-mark-removed = Mark removed.
cpr-mark-here = Mark { $n }
cpr-diameter = Diameter { $value }
cpr-diameters = Diameter { $min } to { $max }
cpr-mark-span = Mark { $from } to { $to }: { $length } along the centreline
cpr-mark-total = All marks: { $length }
cpr-clear = Clear curve
cpr-save = Save curve
cpr-load = Load curve
cpr-done = Done
cpr-a11y = { $name } along { $length }, cursor at { $position }. The wheel moves the cursor.
cpr-none = No curve yet: click points first
cpr-cleared = Curve cleared
cpr-saved = Curve saved: { $name }
cpr-save-failed = The curve could not be saved: { $reason }
cpr-loaded = Curve loaded
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 centreline loaded
   *[other] { $count } centrelines loaded
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] 1 saved centreline found
   *[other] { $count } saved centrelines found
}
# A centreline without a name, by its place in the list.
cpr-centreline = Centreline { $number }
cpr-new = New centreline
cpr-rename = Rename
cpr-name = Centreline name
cpr-delete = Delete
# $name: the centreline's name.
cpr-delete-title = Delete the centreline “{ $name }”?
cpr-delete-detail = Its points go. Files saved on disk stay.
cpr-deleted = Deleted “{ $name }”
cpr-load-failed = The curve could not be loaded: { $reason }
cpr-load-not-json = the file is not JSON
cpr-load-not-curves = the file is not a Specular curve
cpr-load-empty = the file holds no curve of two points or more
cpr-panoramic = Panoramic
cpr-panoramic-slab = Panoramic, { $thickness } mm slab
cpr-panoramic-thin = Panoramic, thin

## The dental layout (Medical).

dental-button = Dental
dental-not-for-diagnosis = Not for diagnosis or treatment planning
dental-nerve = Nerve canal
dental-nerve-hint = Click points along the nerve canal on the panoramic or a cross-section
dental-nerve-done = Nerve canal tracing off
dental-spacing = Every { $spacing } mm
dental-section = Section { $number }
dental-off = Open the dental layout first
dental-suggesting = Finding the dental arch…
dental-suggested = Arch suggested: drag its points to adjust
dental-no-arch = No arch found: click points along it in the axial view
dental-vendor = { $vendor } dental CBCT

## Each segment as its own STL.

seg-stl-export-button = Export segments
seg-stl-detail = Segments: { $count ->
        [one] { $count } triangle
       *[other] { $count } triangles
    }
seg-stl-detail-full = Segments: full detail
seg-stl-none = No segments to export
seg-stl-not-labels = this volume is not a label map
seg-stl-exporting = Building the segments’ surfaces…
seg-stl-saved = { $count ->
    [one] Saved 1 segment to { $folder }
   *[other] Saved { $count } segments to { $folder }
}
seg-stl-open = { $count } not closed.
seg-stl-empty = The segments are empty: nothing saved
seg-stl-failed = The segments could not be saved: { $reason }
