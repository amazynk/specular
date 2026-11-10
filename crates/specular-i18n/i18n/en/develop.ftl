# Specular — Develop (non-destructive photo editing). English.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
toast-develop-edit-migrated = { $count ->
    [one] One module of this edit was updated to a newer version. It may look slightly different.
   *[other] { $count } modules of this edit were updated to a newer version. It may look slightly different.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
toast-develop-edit-newer = { $count ->
    [one] One module of this edit needs a newer Specular. It is kept but not shown.
   *[other] { $count } modules of this edit need a newer Specular. They are kept but not shown.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25%".
develop-curve-point = Point { $index } of { $count }: in { $input }, out { $output }
# The curve editor's keys, read by screen readers.
develop-curve-keys = Arrows move the point. Page Up and Page Down pick another. Insert adds a point, Delete removes it.
# A colour wheel with no colour.
develop-wheel-neutral = Neutral
# A colour wheel's colour. $hue: degrees; $strength: "20%".
develop-wheel-value = Hue { $hue }°, strength { $strength }
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Arrows move the colour. Delete resets it.

## Develop's panel.

develop-title = Develop
develop-undo = Undo
develop-redo = Redo
develop-before-after = Before and after
develop-close = Leave Develop
develop-history = History
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } on
develop-card-menu = { $module } options
develop-card-changed = Changed
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
develop-filmstrip = Filmstrip

## Develop's module cards and controls.

develop-module-exposure = Exposure
develop-module-light = Light
develop-look-profile = Profile
develop-look-standard = Standard
develop-look-film = Film
develop-look-natural = Natural
develop-module-grading = Colour grading
develop-add-tool = Add a tool
develop-module-white-balance = White balance
develop-module-tone = Tone
develop-module-contrast = Contrast and range
develop-module-colour = Colour
develop-module-curve = Curve
develop-module-crop = Crop and straighten
# A module this version has no card for.
develop-module-other = Other module
develop-exposure = Exposure
develop-black-level = Black level
develop-temperature = Temperature
develop-tint = Tint
develop-highlights = Highlights
develop-shadows = Shadows
develop-contrast = Contrast
develop-whites = Whites
develop-blacks = Blacks
# The view transform: how scene brightness becomes screen brightness.
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
develop-hdr-limit = HDR limit
develop-hdr-preview = Show
develop-hdr-preview-hdr = HDR
develop-hdr-preview-sdr = SDR
develop-vibrance = Vibrance
develop-saturation = Saturation
develop-colour-wheels = Colour wheels
develop-curve = Tone curve
develop-module-local-contrast = Local contrast
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Auto exposure
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Smoothing
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Texture
develop-clarity = Clarity
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Black point
develop-levels-mid = Midpoint
develop-levels-white = White point
# The Detail card: noise reduction and sharpening.
develop-module-detail = Detail
develop-module-denoise = Noise reduction
develop-module-sharpen = Sharpening
develop-noise-luminance = Luminance noise
# How much fine texture noise reduction keeps.
develop-noise-detail = Noise detail
develop-noise-colour = Colour noise
develop-sharpen-amount = Sharpen
# Size of the details sharpened, in pixels.
develop-sharpen-radius = Sharpen radius
# Higher lets edges overshoot a little for crisper texture.
develop-sharpen-detail = Sharpen detail
# Higher sharpens only edges, leaving smooth areas alone.
develop-sharpen-masking = Masking
# Undoes the lens and sensor blur.
develop-capture-amount = Capture sharpening
# Blur undone, in pixels. 0 takes it from the lens.
develop-capture-radius = Capture radius
# How a raw's colours are filled in between its sensor sites.
develop-demosaic = Raw detail
# Follows edges (the default).
develop-demosaic-directional = Standard
# Directional on detail, smooth in flat areas.
develop-demosaic-dual = Smooth flat areas
develop-demosaic-fast = Fast
# Below this contrast, Dual uses the smooth method.
develop-dual-threshold = Flat-area contrast
develop-straighten = Straighten
# History rows of the crop tool.
develop-crop = Crop
develop-crop-turn = Turn
develop-crop-flip = Flip
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Lens corrections
develop-module-optics = Optics
develop-lens-distortion = Distortion
develop-lens-vignette = Vignetting
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Red fringe
develop-lens-tca-blue = Blue fringe
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Lens profile
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automatic
develop-lensfun-off = None
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
develop-lensfun-none-chosen = No lens profile.
# $lens: the lens name the file gives.
develop-lensfun-no-match = No Lensfun profile for { $lens }.
develop-lensfun-no-profile = No Lensfun profile for this lens.
develop-lensfun-no-name = The file does not name the lens. Pick it above.
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } lenses match. Pick one above.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

develop-crop-open = Crop. Drag the edges, or Ctrl+drag along a line that should be level. Enter keeps, Esc goes back.
develop-crop-kept = Crop kept
develop-crop-cancelled = Crop undone
develop-crop-reset = Crop and straighten reset
# $angle: "-1.5".
develop-crop-straightened = Straightened to { $angle }°
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Aspect { $aspect }
develop-crop-aspect-free = free
develop-crop-aspect-original = original
develop-crop-turned-right = Turned right
develop-crop-turned-left = Turned left
develop-crop-flipped-horizontal = Flipped left to right
develop-crop-flipped-vertical = Flipped top to bottom
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Aspect { $aspect } · Guide: { $guide }
develop-crop-keys = A aspect · X swap · O guide · [ ] turn · H V flip · Enter keep · Esc back
develop-crop-guide-thirds = thirds
develop-crop-guide-golden = golden ratio
develop-crop-guide-diagonal = diagonals
develop-crop-guide-grid = grid

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

develop-module-perspective = Perspective
# Upright: how the camera's tilt is found.
develop-persp-upright = Upright
develop-persp-mode-off = Off
develop-persp-mode-auto = Auto
develop-persp-mode-level = Level
develop-persp-mode-vertical = Vertical
develop-persp-mode-full = Full
develop-persp-mode-guided = Guided
develop-persp-vertical = Vertical
develop-persp-horizontal = Horizontal
develop-persp-rotate = Rotate
develop-persp-aspect = Aspect
develop-persp-scale = Scale
develop-persp-offset-x = Sideways
develop-persp-offset-y = Up or down
# Zoom so no empty edges show.
develop-persp-auto-scale = Avoid empty edges
develop-persp-guides = Guides
develop-persp-focal = Focal length
develop-persp-draw-guides = Draw guides
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Draw guides ({ $n } of { $max })
develop-persp-reset = Reset perspective
# $mode: one of the mode names above.
develop-persp-mode-set = Upright: { $mode }
develop-persp-analysing = Finding lines…
# $n: the guides drawn.
develop-persp-reset-confirm = Reset perspective? Your { $n } guides are removed. Undo brings them back.
develop-persp-reset-ok = Reset
develop-persp-reset-done = Perspective reset
# $n: the guide's number.
develop-persp-guide-remove-confirm = Remove guide { $n }? Undo brings it back.
develop-persp-guide-remove-ok = Remove
develop-persp-guide-removed = Guide removed
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Vertical { $vertical }, horizontal { $horizontal }
# $error: what went wrong.
develop-persp-failed = Upright failed: { $error }
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: tilt { $vertical }°, turn { $horizontal }°, rotation { $rotate }°
# $mode: a mode name.
develop-perspective-few-lines = { $mode } needs at least two straight lines it can tell apart. Try another mode, or draw guides.
# $n: 4.
develop-persp-guides-full = { $n } guides at most. Drag an end to move one.
develop-persp-guides-on = Guides. Drag along a line that should be plumb or level.
develop-persp-guides-off = Crop
develop-persp-guide-moved = Guide moved
# $n: how many guides now.
develop-persp-guide-added = Guide { $n } added
develop-persp-guides-keys = Drag to add · Drag an end to move · Delete removes · Shift+R crop
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Guides: { $n } of { $max }
# The three colour wheels.
develop-wheel-shadows = Shadows
develop-wheel-midtones = Midtones
develop-wheel-highlights = Highlights
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Shadows luminance
develop-luminance-midtones = Midtones luminance
develop-luminance-highlights = Highlights luminance
# The fourth colour wheel: the whole picture.
develop-wheel-global = Global
develop-luminance-global = Global luminance
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Colour intensity
develop-brilliance = Colour brightness

## Colour mixer: hue, saturation and brightness per colour.

develop-module-colour-mixer = Colour mixer
develop-colour-red = Red
develop-colour-orange = Orange
develop-colour-yellow = Yellow
develop-colour-green = Green
develop-colour-aqua = Aqua
develop-colour-blue = Blue
develop-colour-purple = Purple
develop-colour-magenta = Magenta
# $colour: one of the colour names above.
develop-mixer-hue = { $colour } hue
develop-mixer-saturation = { $colour } saturation
develop-mixer-brightness = { $colour } brightness

## Point colour: pick a colour in the photo, then shift it and its neighbours.

develop-module-point-colour = Point colour
develop-point-pick = Pick a colour
# On the pick button once a colour is picked.
develop-point-pick-again = Pick another colour
# Said and shown while picking.
develop-point-picking = Click the colour to change. Esc cancels.
develop-point-picked = Colour picked
develop-point-cancelled = Picking cancelled
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Range
develop-point-hue-range = Hue range
develop-point-saturation-range = Saturation range
develop-point-lightness-range = Lightness range
develop-point-hue = Hue shift
develop-point-saturation = Saturation shift
develop-point-lightness = Lightness shift

## Black and white.

develop-module-black-white = Black and white
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = { $colour } brightness

## LUT: a colour look-up table file.

develop-module-lut = LUT
develop-lut-choose = Choose a LUT file…
# The file chosen. $name: its file name.
develop-lut-file = File: { $name }
# The file can't be used. $name: its file name.
develop-lut-unreadable = Can't read { $name }
# The file dialog.
develop-lut-dialog = Choose a LUT file
develop-lut-kind = LUT files
# The encoding the LUT was made for.
develop-lut-space = Made for
develop-lut-space-linear = Linear (Rec.2020)
develop-lut-space-acescg = ACES linear (ACEScg)
develop-lut-space-acescct = ACES log (ACEScct)
develop-lut-space-srgb = Photo (sRGB)
develop-lut-space-rec709 = Video (Rec.709)
develop-lut-interpolation = Method
develop-lut-tetrahedral = Accurate
develop-lut-trilinear = Simple
develop-lut-strength = Strength

## Develop's history and status.

develop-history-original = Original
# A module switched on or off. $module: its name.
develop-history-on = { $module } on
develop-history-off = { $module } off
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Back to the original
develop-at-step = Back to: { $step }
develop-undone = Undone: { $step }
develop-redone = Redone: { $step }
develop-entered = Develop. Edits never change the file.
develop-left = Left Develop
develop-split-on = Before and after side by side
develop-split-off = Edit only
develop-showing-before = Showing the original
develop-showing-after = Showing the edit
# Labels over the image.
develop-before = Before
develop-after = After
develop-working = Developing…
develop-preview-failed = Can't show the edit for this file

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

keys-mode-develop = Develop
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Speed edit: hold, then scroll
palette-develop = Develop
palette-develop-help = Edit this photo without changing the file. Again to leave.
palette-develop-before-after = Before and after
palette-develop-before-after-help = Show the original, or the edit again.
palette-develop-split = Split before and after
palette-develop-split-help = Show the original and the edit side by side.
palette-develop-panel = Go to the Develop panel
palette-develop-panel-help = Move the keyboard to the panel. Esc comes back.
palette-develop-speed-exposure = Speed edit exposure
palette-develop-speed-exposure-help = Hold, then scroll or press arrows to change exposure.
palette-develop-speed-contrast = Speed edit contrast
palette-develop-speed-contrast-help = Hold, then scroll or press arrows to change contrast.
palette-develop-speed-highlights = Speed edit highlights
palette-develop-speed-highlights-help = Hold, then scroll or press arrows to change highlights.
palette-develop-speed-shadows = Speed edit shadows
palette-develop-speed-shadows-help = Hold, then scroll or press arrows to change shadows.
palette-develop-speed-whites = Speed edit whites
palette-develop-speed-whites-help = Hold, then scroll or press arrows to change whites.
palette-develop-speed-blacks = Speed edit blacks
palette-develop-speed-blacks-help = Hold, then scroll or press arrows to change blacks.
palette-develop-speed-temperature = Speed edit temperature
palette-develop-speed-temperature-help = Hold, then scroll or press arrows to change the white balance temperature.
palette-develop-speed-vibrance = Speed edit vibrance
palette-develop-speed-vibrance-help = Hold, then scroll or press arrows to change vibrance.
palette-develop-crop = Crop and straighten
palette-develop-crop-help = Crop, straighten, turn or flip the photo. Enter keeps, Esc goes back.
disabled-not-photo = Only for still photos
disabled-develop-off = Develop is off in the Medical and Science profiles
# The Corrections card: clipped highlights, hot pixels, colour fringes, calibration frames.
develop-module-corrections = Corrections
# The raw stage (history rows).
develop-module-raw = Raw
develop-off = Off
develop-on = On
# How clipped highlights are treated.
develop-highlight-method = Clipped highlights
# Clipped colours rebuilt from the unclipped ones around them.
develop-highlight-opposed = Rebuild colour
# Also rebuilds the brightness of large blown areas.
develop-highlight-rebuild = Rebuild large areas
# Clipped areas become neutral white.
develop-highlight-clip = Clip to white
# Values above this share of the sensor maximum count as clipped.
develop-highlight-threshold = Clipping level
develop-module-hot-pixels = Hot pixels
# How far a pixel must stand out from its neighbours to be fixed.
develop-hot-threshold = Hot pixel threshold
develop-module-ca = Chromatic aberration
develop-ca-mode = Fringes
develop-ca-auto = Automatic
develop-ca-manual = Manual
# Radial scale of the red channel, percent of the radius.
develop-ca-red = Red shift
develop-ca-blue = Blue shift
develop-module-defringe = Defringe
develop-defringe = Defringe
develop-defringe-purple = Purple hue
develop-defringe-green = Green hue
develop-defringe-range = Hue range
# Only edges at least this many stops from dark to bright are treated.
develop-defringe-edge = Edge contrast
develop-module-flat-field = Calibration frames
# Flat-field and dark-frame calibration.
develop-calibration = Calibration
# Frames picked from the calibration folder in Settings.
develop-calibration-auto = From folder
develop-calibration-chosen = Chosen files
develop-flat-choose = Choose a flat frame…
develop-dark-choose = Choose a dark frame…
develop-flat-file = Flat: { $name }
develop-dark-file = Dark: { $name }
develop-flat-use = Flat frame
# Corrects vignetting and dust spots.
develop-flat-use-full = Vignetting and dust
develop-flat-use-vignetting = Vignetting only
develop-flat-dialog = Choose a flat frame
develop-dark-dialog = Choose a dark frame
develop-raw-kind = Raw files
# History rows: the calibration frame chosen.
develop-flat-frame = Flat frame
develop-dark-frame = Dark frame

## Masks
develop-mask-add = Add mask
develop-mask-kind-ellipse = Ellipse
develop-mask-kind-rectangle = Rectangle
develop-mask-kind-polygon = Polygon
develop-mask-kind-linear = Linear gradient
develop-mask-kind-radial = Radial gradient
develop-mask-kind-brush = Brush
develop-mask-kind-luminance = Luminance range
develop-mask-kind-hue = Hue range
develop-mask-kind-chroma = Saturation range
develop-mask-kind-colour = Colour range
develop-mask-kind-other = Other mask
develop-mask-row = { $kind } { $n }
develop-mask-mode = Blend
develop-blend-normal = Normal
develop-blend-multiply = Multiply
develop-blend-screen = Screen
develop-blend-overlay = Overlay
develop-blend-soft-light = Soft light
develop-blend-lightness = Lightness only
develop-blend-colour = Colour only
develop-mask-amount = Mask amount
develop-mask-invert = Invert mask
develop-mask-off = Off
develop-mask-on = On
develop-mask-refine = Refine edges
develop-mask-show = Show mask
develop-mask-show-red = Red
develop-mask-show-grey = Grey
develop-mask-remove = Remove mask
develop-mask-combine-add = Add
develop-mask-combine-subtract = Subtract
develop-mask-combine-intersect = Intersect
develop-mask-row-opacity = Opacity
develop-mask-row-invert = Invert
develop-mask-feather = Feather
develop-mask-roundness = Roundness
develop-mask-brush-size = Brush size
develop-mask-brush-feather = Brush feather
develop-mask-brush-flow = Flow
develop-mask-brush-mode = Brush
develop-mask-brush-paint = Paint
develop-mask-brush-erase = Erase
develop-mask-range-from = From
develop-mask-range-full-from = Full from
develop-mask-range-full-to = Full to
develop-mask-range-to = To
develop-mask-range-test = Test
develop-mask-range-input = Input
develop-mask-range-output = Output
develop-mask-colour-pick = Pick a colour
develop-mask-colour-pick-more = Pick another colour
develop-mask-colour-range = Range
develop-mask-colour-falloff = Falloff
develop-mask-edit = Edit on the picture
develop-mask-row-remove = Remove { $name }
develop-mask-remove-confirm = Remove the mask of { $module }? Its shapes go. The photo on disk stays.
develop-mask-row-remove-confirm = Remove { $name } from { $module }? The other masks and the photo on disk stay.
develop-mask-remove-ok = Remove
develop-mask-removed = Mask removed.
develop-mask-added = { $name } added.
develop-mask-full = This mask has as many parts as it can take.
develop-mask-editing = Editing { $name }. Drag the handles, or press Tab and the arrows.
develop-mask-painting = Painting { $name }. Drag on the picture.
develop-mask-picking = Click the picture to pick a colour.
develop-mask-picked = Colour picked.
develop-mask-done = Done editing the mask.
develop-mask-handles = Mask handles
develop-handle-centre = Centre
develop-handle-width = Width
develop-handle-height = Height
develop-handle-rotate = Rotate
develop-handle-feather = Feather
develop-handle-inner = Inner edge
develop-handle-start = Start point
develop-handle-end = End point
develop-handle-point = Point { $n }
develop-instance = Layer
develop-instance-n = { $n }
develop-instance-chosen = Layer { $n }.
develop-instance-new = New layer
develop-instance-duplicate = Duplicate layer
develop-instance-delete = Delete layer
develop-instance-earlier = Move earlier
develop-instance-later = Move later
develop-instance-delete-confirm = Delete layer { $n } of { $module }? Its settings and masks go. The photo on disk stays.
develop-instance-delete-ok = Delete
develop-instance-deleted = Layer deleted.
develop-history-mask = Mask: { $module }
develop-history-mask-shapes = Mask shapes: { $module }
develop-history-instance-new = New { $module }
develop-history-instance-delete = Delete { $module }
develop-history-instance-move = Reorder { $module }
# Other tools' edits and sidecar conflicts.
develop-others-title = Other tools
develop-darktable-edited = Edited in darktable:
develop-darktable-open = Open in darktable
develop-darktable-opening = Opening in darktable
develop-darktable-failed = darktable did not start: { $reason }
develop-lightroom-import = Import Lightroom edit
develop-lightroom-imported = { $count ->
    [one] Imported 1 Lightroom setting. The result is approximate.
   *[other] Imported { $count } Lightroom settings. The result is approximate.
}
develop-lightroom-nothing = Nothing to import: these Lightroom settings change nothing here.
develop-history-lightroom = Imported from Lightroom (approximate)
develop-conflict-title = The edit of { $file } changed outside Specular
develop-conflict-file = Sidecar file: saved { $time }
develop-conflict-library = Library: saved { $time }
develop-conflict-writer = The sidecar was written by { $writer }.
develop-conflict-detail = Keep one. The other is replaced.
develop-conflict-use-file = Use the file's
develop-conflict-keep-library = Keep the library's
develop-conflict-used-file = Using the edit from the sidecar file
develop-conflict-kept-library = Keeping the library's edit
toast-develop-edit-newer-process = This edit was made with a newer Specular. It may look different here.
info-darktable-edited = Edited in darktable
info-darktable-no-modules = No modules on
info-darktable-group = File
toast-develop-edit-partial = This edit was made with a newer Specular. Parts this version can't show are left out. Changes here are not saved.
toast-develop-sidecar-unreadable = The edit file beside this photo can't be read. It is kept as it is. Changes here are not saved.
