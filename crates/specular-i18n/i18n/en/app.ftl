# Specular — viewer chrome, palette, errors, About. English (source catalogue, British spelling).
#
# Conventions (see crates/specular-i18n/src/lib.rs for the full list)
# - Message ids are kebab-case: <area>-<item>[-<detail>]. Never reuse an id for a new meaning;
#   add a new id instead.
# - Numbers are passed as numbers ($count, $index, $value…) and formatted by ICU4X for the
#   current region, so write { $count }, not a pre-formatted string. Plural selectors use the
#   same variable.
# - $file, $path, $shortcut, $reason and similar are passed as strings and are wrapped in
#   Unicode isolates (FSI…PDI) automatically; do not add quotes around file names unless the
#   language's typography requires them.
# - Keyboard shortcuts arrive already localised for the platform ($shortcut = "Ctrl+O" / "⌘O").

-app-name = Specular

## Toolbar tooltips and accessibility labels

toolbar-open = Open image…
toolbar-prev = Previous image
toolbar-next = Next image
toolbar-back-gallery = Back to the gallery
toolbar-zoom-out = Zoom out
toolbar-zoom-in = Zoom in
toolbar-zoom-level = Zoom level and presets
toolbar-zoom-fit = Fit to window
toolbar-zoom-100 = Actual size ({ PERCENT(100) })
toolbar-rotate-left = Rotate anticlockwise
toolbar-rotate-right = Rotate clockwise
toolbar-fullscreen-enter = Enter full screen
toolbar-fullscreen-exit = Exit full screen
toolbar-info = Image information
toolbar-settings = Settings
toolbar-delete = Move to trash
toolbar-copy = Copy image
toolbar-palette = Command palette
toolbar-go-to = Go to image…
toolbar-go-to-range = 1–{ $count }
# The toolbar's right-click menu.
toolbar-customise = Customise toolbar…
toolbar-more = More

## Changes to the toolbar (its list in Settings, or dragging on the bar)

# Spoken by screen readers after a change; $item is a toolbar button's name.
toolbar-announce-moved = { $item }: position { $index } of { $count }
toolbar-announce-added = { $item } added: position { $index } of { $count }
toolbar-announce-removed = { $item } removed from the toolbar
toolbar-announce-reset = Toolbar reset to default
# A tooltip label followed by its keyboard shortcut.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Tooltip of a control that cannot be used right now; $reason is one of the disabled-* messages.
tooltip-unavailable = { $label } (unavailable: { $reason })
# Why a control or command is unavailable (tooltips, command palette, screen readers).
disabled-no-image = No image open
disabled-still-loading = The image is still loading
disabled-decode-failed = This image could not be decoded
disabled-no-other-images = No other images in this folder

## Readouts

# Position in the folder, e.g. "12 / 348".
readout-position = { $index } / { $count }
# Spoken by screen readers for the position readout.
readout-position-a11y = Image { $index } of { $count }
# $zoom is an already formatted percentage, e.g. "150%".
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Fit ({ $zoom })
zoom-preset-fit = Fit to window
zoom-preset-fill = Fill window
zoom-preset-actual = Actual size
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Full screen
# $zoom is an already formatted percentage.
zoom-preset-percent = { $zoom }
# Image dimensions in pixels, e.g. "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Number of images in the current folder.
status-image-count =
    { $count ->
        [one] { $count } image
       *[other] { $count } images
    }

## Command palette

palette-placeholder = Type a command or setting…
palette-no-results = No matching commands
# Changing a setting from the palette, e.g. "Set: Wheel behaviour → Next image".
palette-set-setting = Set: { $setting } → { $value }
palette-next = Next image
palette-prev = Previous image
palette-first = First image
palette-last = Last image
palette-go-to = Go to image…
palette-zoom-in = Zoom in
palette-zoom-out = Zoom out
palette-zoom-fit = Zoom to fit
palette-zoom-fill = Zoom to fill
palette-zoom-fit-width = Fit width
palette-zoom-fit-height = Fit height
palette-zoom-100 = Zoom to { PERCENT(100) }
palette-zoom-200 = Zoom to { PERCENT(200) }
palette-zoom-300 = Zoom to { PERCENT(300) }
palette-zoom-400 = Zoom to { PERCENT(400) }
palette-zoom-rectangle = Zoom to selection
palette-rotate-left = Rotate anticlockwise
palette-rotate-right = Rotate clockwise
palette-flip-horizontal = Flip horizontally
palette-flip-vertical = Flip vertically
palette-toggle-fullscreen = Toggle full screen
palette-navigator = Show or hide navigator
palette-loupe = Loupe
palette-pixel-grid = Show or hide pixel grid
palette-info-panel = Show or hide information panel
palette-copy-image = Copy image
palette-read-codes = Read codes
palette-find-text = Find text
palette-face-boxes = Show faces
palette-copy-path = Copy file path
palette-move-to-trash = Move to trash
palette-undo-trash = Undo move to trash
palette-open = Open image…
palette-open-address = Open address…
palette-settings = Settings
palette-customise-toolbar = Customise toolbar…
palette-cheat-sheet = Keyboard shortcuts
palette-hud = Show or hide frame-time HUD
palette-language = Language
palette-theme = Theme
palette-interface-larger = Larger interface
palette-interface-smaller = Smaller interface
palette-interface-reset = Interface at { PERCENT(100) }
palette-quit = Quit { -app-name }

## Cheat sheet

cheatsheet-title = Keyboard shortcuts
cheatsheet-navigation = Navigation
cheatsheet-zoom-pan = Zoom and pan
cheatsheet-view = View
cheatsheet-file = File
cheatsheet-window = Window
cheatsheet-help = Help
cheatsheet-hold = Hold { $key }
cheatsheet-close-hint = Press { $key } or Esc to close
# The cheat sheet's tabs, one per place with keys of its own (the current one first shown).
cheatsheet-mode-gallery = Gallery
cheatsheet-mode-compare = Compare
# Gallery headings and lines.
cheatsheet-gallery-browse = Browse
cheatsheet-gallery-select = Select
cheatsheet-gallery-mark = Rate and flag
cheatsheet-gallery-move = Move between images
cheatsheet-gallery-page = Page up or down
cheatsheet-gallery-ends = First or last image
cheatsheet-gallery-up = Up to the enclosing folder
cheatsheet-gallery-history-back = Back to the folder before
cheatsheet-gallery-history-forward = Forward to the next folder
cheatsheet-gallery-back = Close the view on top, then go back
cheatsheet-gallery-viewer = Back to the viewer
cheatsheet-gallery-album-add = Add to the last album
cheatsheet-gallery-select-one = Select or deselect
cheatsheet-gallery-quick-look = Quick look
cheatsheet-gallery-filter = Filter
cheatsheet-gallery-bigger = Larger thumbnails
cheatsheet-gallery-smaller = Smaller thumbnails
cheatsheet-gallery-default-size = Standard thumbnail size
cheatsheet-gallery-refresh = Refresh the folder
cheatsheet-gallery-menu = Context menu
# Compare headings and lines.
cheatsheet-compare-views = Compare
cheatsheet-compare-overlay = Overlay or side by side
cheatsheet-compare-difference = Difference (again: next kind)
cheatsheet-compare-step-down = Less blend or difference gain
cheatsheet-compare-step-up = More blend or difference gain
cheatsheet-compare-pick = Image the keys act on
cheatsheet-compare-close = Close compare
cheatsheet-channels = Channels
# Medical image headings.
cheatsheet-medical-window = Window and level
cheatsheet-medical-measure = Measure and annotate
cheatsheet-medical-frames = Frames and series
cheatsheet-medical-3d = 3D and reformatting
cheatsheet-medical-analysis = Segmentation, fusion and PET
# Studies, DICOM servers and the exports, once the user has given them keys.
cheatsheet-medical-studies = Studies and sharing
# The last group of a tab: important actions that have no key, reached through the command
# palette ({ $key } is the key that opens it, e.g. Ctrl+K).
cheatsheet-palette-only-title = In the command palette ({ $key })
cheatsheet-palette-only-title-nokey = In the command palette
# The tag shown where the keys would be, on such a line (short).
cheatsheet-palette-only-tag = Palette
# What a screen reader says for such a line.
cheatsheet-palette-only-line = { $action }: no key; in the command palette ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: no key; in the command palette
# Esc on the cheat sheet (the palette's long description of it does not fit).
cheatsheet-back = Close or go back

## Errors

# An office document (Word, Excel, PowerPoint, iWork, OpenDocument) that stores no preview
# image: shown instead of the picture; the Info panel still lists its properties.
viewer-no-preview = No preview inside this file.
error-cannot-open = Can’t open { $file }
error-cannot-open-reason = Can’t open { $file }: { $reason }
error-unsupported-format = { $file } is in a format { -app-name } can’t read yet.
error-file-truncated = { $file } is incomplete. Showing the part that could be read.
error-file-too-large = { $file } is too large to open ({ $megapixels } megapixels; the limit is { $limit } megapixels).
error-not-found = { $file } no longer exists.
error-decoder-stopped = the decoder stopped without producing an image
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = it needs a brain surface (a .surf.gii file with { $vertices } vertices) in the same folder
error-permission-denied = You don’t have permission to read { $file }.
error-details = Details
crash-last-time = { -app-name } closed unexpectedly last time; a report was saved at { $path }
error-trash-failed = Couldn’t move { $file } to the trash: { $reason }
# Reasons, shown after "Can’t open FILE:" and other "Couldn’t …:" messages, so they start
# lower case. They say what happened and, where it helps, what to do.
error-reason-not-found = it isn’t there any more. It may have been moved, renamed or deleted.
error-reason-permission = you don’t have permission to read it. Check its owner and permissions.
error-reason-disk-full = the disk is full. Free some space and try again.
error-reason-read-only = the disk is read-only.
error-reason-truncated = the file is incomplete. It may still be copying or downloading.
error-reason-is-folder = it is a folder, not a file.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = the server refused access. The data may be private; for an S3 bucket, set up AWS credentials first.
error-reason-remote-missing = there is no Zarr data at that address.
# $status is an HTTP status code ("500").
error-reason-remote-status = the server answered with error { $status }.
error-reason-offline = the server can’t be reached. Check the address and the connection.
error-reason-network-off = the network is off in Settings › Privacy.
error-reason-timed-out = the drive or server didn’t answer in time. Check the connection and try again.
error-reason-memory = there isn’t enough memory. Close other images or applications and try again.
error-reason-unsupported = it is in a format { -app-name } can’t read yet.
# $detail is the decoder's own words, technical and in English ("12-bit lossless JPEG").
error-reason-unsupported-detail = it uses a feature { -app-name } can’t read yet ({ $detail }).
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = it is none of the .{ $ext } files { -app-name } reads ({ $formats }).
error-reason-damaged = the file is damaged or isn’t what its name says ({ $detail }).
error-reason-too-large = it is too large to open.
# Shown after "Can’t open FILE:" when decoding needs more memory than is free right now.
# $needed and $free are amounts of memory ("24 GB", "18 GB"), formatted for the locale.
# "free RAM" is the memory not in use by programs, as system monitors call it.
error-reason-needs-memory = it needs { $needed } of free RAM, but only { $free } is free right now. Close some other apps and try again, or open it on a computer with more memory.
# The same as a sentence of its own: in a gallery tile's tooltip and in the info panel.
# $needed and $free are amounts of memory ("24 GB", "18 GB"), formatted for the locale.
error-needs-memory = This image needs { $needed } of free RAM, but only { $free } is free right now. Close some other apps and try again, or open it on a computer with more memory.
# Short form under a gallery tile that could not be decoded for lack of memory; keep it
# short (it fits under a thumbnail). $needed is an amount of memory ("24 GB").
error-needs-memory-short = Needs { $needed } of free RAM
error-reason-cancelled = opening it was stopped.
error-reason-trash-access = it isn’t there any more, or you don’t have permission to change it.
error-reason-restore-collision = a file with the same name is already in its folder.
error-reason-trash-root = a whole drive can’t be moved to the trash.
error-reason-not-in-trash = it is no longer in the trash.
error-reason-program-missing = the program isn’t installed, or has moved. Choose it again or reinstall it.
error-reason-program-not-allowed = the program isn’t allowed to run. Check that it is executable.
error-reason-permission-write = you don’t have permission to change it. Check its owner and permissions.
error-reason-file-has-errors = it has errors. Fix them first, so nothing in it is lost.
error-reason-not-dicom = it isn’t a DICOM file.
error-reason-no-image = it holds no image.
error-reason-needs-layout = it is a headerless volume. Its size, sample type and byte order are needed to open it.
error-reason-not-archive = it isn’t an archive { -app-name } can read.
error-reason-needs-sandbox = this kind of archive is opened only by { -app-name }’s protected reader, which isn’t available.
error-reason-encrypted = it is encrypted and needs its password.
error-reason-wrong-password = the password is wrong.
error-reason-checksum = the file is damaged: its contents don’t match their checksum.
error-reason-missing-entry = a file inside it is missing.
error-reason-busy = another program is using it. Close it there and try again.
error-reason-catalogue-damaged = the catalogue file is damaged.
error-reason-cannot-open = its file can’t be opened. Check that its folder exists and that you can write to it.
error-reason-newer-version = it was written by a newer version of { -app-name }. Update { -app-name } to change it.
error-reason-internal = something went wrong inside { -app-name }. The log has the details.
# Why a series doesn't make a volume (the multiplanar view).
error-reason-volume-too-few = { $count ->
    [one] one slice is too few to build a volume.
   *[other] { $count } slices are too few to build a volume.
}
error-reason-volume-no-geometry = a slice doesn’t say where it lies or which way it faces.
error-reason-volume-not-parallel = the slices aren’t parallel.
error-reason-volume-shared-position = several images share each of { $count ->
        [one] { $count } position
       *[other] { $count } positions
    } (phases, echoes or a time series).
error-reason-volume-missing = { $count ->
        [one] about { $count } slice is missing.
       *[other] about { $count } slices are missing.
    }
error-reason-volume-mismatch = the slices differ in size or pixel spacing.
error-reason-volume-not-grey = the slices are in colour, not grey values.
error-reason-volume-skewed = the volume’s axes aren’t at right angles.
# $need and $budget are in MiB.
error-reason-volume-too-large = it needs { $need } MiB and only { $budget } MiB is allowed.
# Why a { -app-name } link was refused.
error-reason-url-not-specular = it isn’t a { -app-name } link.
error-reason-url-too-long = it is too long.
error-reason-url-damaged = it is damaged. It may have been cut short or changed when it was copied.
error-reason-url-unknown = it asks for something this version of { -app-name } doesn’t know. Updating { -app-name } may help.
error-reason-url-not-absolute = its file paths must be full paths.
error-reason-url-no-paths = it names no file.
error-reason-url-too-many = it names too many files.

## Toasts

# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Couldn’t finish: { $reason }
toast-moved-to-trash = Moved to trash
toast-moved-to-trash-named = Moved { $file } to trash
toast-moved-to-trash-count =
    { $count ->
        [one] Moved { $count } image to trash
       *[other] Moved { $count } images to trash
    }
toast-undo = Undo
toast-restored = Restored { $file }
toast-copied-image = Image copied
toast-copied-path = Path copied
toast-copied-report = Report copied
toast-copied-value = Value copied
toast-loop-start = Back to the first image
toast-loop-end = Jumped to the last image
# The interface size after Ctrl+Alt+= / Ctrl+Alt+- / Ctrl+Alt+0; $percent is a whole number.
toast-interface-size = Interface size { PERCENT($percent) }
toast-language-changed = Language changed to { $language }

## Empty states

empty-drop = Drop an image here or press { $shortcut }
empty-open-button = Open image…
empty-drop-release = Release to open
empty-folder = This folder has no images { -app-name } can open.
# Under "Open image…" in the empty window: where to find everything else.
empty-hint-keys = Press { $cheatsheet } for keyboard shortcuts, or { $palette } to find any command.
empty-hint-cheatsheet = Press { $cheatsheet } for keyboard shortcuts.
empty-hint-palette = Press { $palette } to find any command.
# Under the spinner when a file takes more than a moment to open.
viewer-opening = Opening { $file }…
fullscreen-exit-hint = Press Esc to exit full screen
# Shown on entering full screen; Esc first resets the zoom when zoomed in.
fullscreen-back-hint = Press Esc to reset the zoom or exit full screen
# Cheat sheet and command palette: what Esc does, one step per press.
action-back = Close panel (in a dialog, back a step first), reset zoom, exit full screen; in the gallery, up a folder

## Language picker

language-picker-title = Language
language-picker-search = Search languages
# $percent is an already formatted percentage, e.g. "87%".
language-completeness = { $percent } translated
language-system-default = System default ({ $language })
language-pseudo-accented = Pseudo-locale: accented and longer
language-pseudo-rtl = Pseudo-locale: mirrored right-to-left

## About box

about-title = About { -app-name }
about-version = Version { $version }
about-tagline = A fast, faithful image viewer.
about-copyright = © { $year } The { -app-name } authors
about-license = Licensed under the MIT License or the Apache License 2.0.
about-geonames = Place names from GeoNames (geonames.org), licensed under Creative Commons Attribution 4.0.
about-natural-earth = Map data from Natural Earth (naturalearthdata.com), public domain.
about-unicode = Locale data from the Unicode CLDR via ICU4X. © Unicode, Inc.
about-lensfun = Lens profiles from the Lensfun database (lensfun.github.io), licensed under Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Third-party licences
about-third-party-file = { -app-name } includes third-party software and data under their own licences; see THIRD-PARTY.md, installed with { -app-name }.
about-copy-info = Copy system information
about-close = Close

## Viewer chrome (Stage 1 app)

app-close = Close
# Badge shown on an image whose file ended early.
app-truncated-badge = Incomplete file
app-info-summary = Summary
app-info-all-fields = All fields
app-info-copy-value = Copy value
app-info-show-on-map = Show on map
app-info-copy-field = Copy the value of { $field }
app-info-loading = Reading metadata…
# All fields, for a file that stores no metadata beyond its pixels.
app-info-no-fields = This file has no metadata fields.
# Gain-map row of the info summary; $headroom is a number like 4.9.
app-info-gain-map = HDR gain map
app-info-gain-map-headroom = Up to { $headroom }× SDR brightness
app-copy-failed = Couldn’t copy: { $reason }
app-restore-failed = Couldn’t restore { $file }: { $reason }
app-open-dialog-title = Open image
app-open-dialog-filter = All supported files
app-open-dialog-all = All files
# The Open dialog's file types, after "All supported files".
app-open-group-images = Images
app-open-group-raw = Camera raw
app-open-group-video = Video
app-open-group-sound = Sound
app-open-group-documents = Documents
app-open-group-drawings = Drawings and CAD
app-open-group-models = 3D models
app-open-group-maps = Maps and tracks
app-open-group-science = Science and medical
app-open-group-fonts = Fonts
app-open-group-other = Other


## Renderer information (About, Settings › Advanced). Technical names (formats, limits,
## features) stay in English.

renderer-title = Renderer information
renderer-section-adapter = Graphics adapter
renderer-section-limits = Limits and features
renderer-section-display = Display
renderer-section-memory = Memory
renderer-section-decoders = Optional decoders
renderer-unknown = Unknown
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = Vendor and device ID
# Wayland, X11, Windows or macOS.
renderer-display-server = Display server
renderer-features = Features used
renderer-surface-format = Window format
renderer-surface-formats = Formats offered
renderer-present-mode = Present mode
renderer-present-modes = Modes offered
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = On ({ $headroom }× headroom)
renderer-hdr-off = Off (the display can show HDR)
renderer-hdr-unavailable = Not available on this display
renderer-decode-budget = Decoding may use
renderer-decode-budget-detail = { PERCENT(80) } of the free memory, shared by all images being opened.
renderer-display-cache = Display cache
## Performance HUD (F12)

app-hud-scale = Scale
app-hud-level = Level
app-hud-level-thumbnail = Embedded thumbnail
app-hud-level-full = Full resolution
# $denominator is 2, 4 or 8: the image decoded at 1/2, 1/4 or 1/8 size.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Open
app-hud-decode = Decode
app-hud-upload = Upload
app-hud-frame = Frame
app-hud-threads = Threads
app-hud-threads-value = { $threads } ({ $queued } queued)
app-hud-tiles = Tiles
app-hud-tiles-value = { $tiles } of { $capacity }
app-hud-ram = Memory
app-hud-vram = GPU memory
app-hud-gpu = GPU

# Trash confirmation
app-confirm-trash-title = Move { $file } to the trash?
app-confirm-trash-sidecars = Its sidecars go to the trash too: { $files }
app-cancel = Cancel
# A warning shown before a very large image is decoded. $need and $available are memory
# sizes ("3.2 GiB").
toast-low-memory = This image may need about { $need } of memory, but only { $available } is available. It may load slowly or fail.

# The hardware report (Settings → Advanced). Values are filled in by the app.
hw-report-title = Hardware report
hw-report-copy = Copy report
hw-report-version = Version
hw-report-cpu = CPU
hw-report-cores = Cores
hw-report-cores-value = { $logical } logical, { $physical } physical
hw-report-performance-cores = Performance cores
hw-report-cpu-features = CPU fast paths
hw-report-none = None
hw-report-memory = Memory
hw-report-memory-value = { $total } total, { $available } available
hw-report-graphics-api = Graphics API
hw-report-driver = Driver
hw-report-max-texture = Largest texture
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Decode threads
hw-report-ram-cache = Image memory cache
hw-report-vram-cache = GPU memory cache
hw-report-processing = Processing
hw-report-path-jpeg-decode = JPEG decoding
hw-report-path-ycbcr-to-rgb = Colour conversion
hw-report-path-png-unfilter = PNG decoding
hw-report-path-heic-decode = HEIC decoding
hw-report-path-inference = Image analysis
hw-report-path-video-encode = Video export
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, in parallel
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = hardware decoder
hw-report-engine-npu = NPU
hw-report-engine-hw-encoder = hardware encoder
hw-report-build-debug = debug build
hw-report-build-release = release build
# Section headings, in the order of `specular --hw-report`.
hw-report-section-system = System
hw-report-section-gpu = GPU
hw-report-section-decoding = Decoding
# Row labels.
hw-report-operating-system = Operating system
hw-report-adapter = Adapter
hw-report-gpu-type = Type
hw-report-gpu-init = Start-up time
hw-report-max-layers = Texture layers
hw-report-compression = Texture compression
hw-report-unified-memory = Memory shared with the CPU
hw-report-display = Display
hw-report-hw-decode = Hardware decoding
hw-report-core-cluster = Core group
hw-report-power = Power
hw-report-calibration = Speed measurements
# Under "Hardware decoding": codecs other than JPEG, HEVC and AV1.
hw-report-codec-others = Others
hw-report-codec-software = Software only
# Values.
# $count of the $total logical cores are performance cores.
hw-report-performance-cores-value = { $count } of { $total }
# A group of alike processor cores; $name is what the system calls it ("Performance").
hw-report-cluster-value = { $name }: { $physical } physical, { $logical } logical
hw-report-thermal-nominal = Normal temperature
hw-report-thermal-fair = Warm
hw-report-thermal-serious = Hot, slowing down
hw-report-thermal-critical = Overheating
# $percent is the battery charge ("80 %").
hw-report-power-battery = On battery ({ $percent })
hw-report-power-battery-unknown = On battery
hw-report-power-mains = Plugged in
hw-report-power-mains-percent = Plugged in ({ $percent })
hw-report-power-saving = Power saving on
hw-report-gpu-discrete = Dedicated
hw-report-gpu-integrated = Integrated
hw-report-gpu-virtual = Virtual
hw-report-gpu-software = Software, on the CPU
hw-report-gpu-other = Other
hw-report-gpu-unavailable = Not available
# $ms is a number of milliseconds ("3.1").
hw-report-ms = { $ms } ms
hw-report-yes = Yes
hw-report-no = No
hw-report-unified-platform-differs = The system reports otherwise
# $fps is the highest refresh rate; $now and $max how much brighter than white HDR can go.
hw-report-display-value = { $fps } Hz, HDR headroom { $now } now, up to { $max }
# $api is the system's video interface ("VA-API"), $device the graphics hardware.
hw-report-hw-decode-value = { $api } on { $device }
hw-report-hw-decode-checking = Still checking…
hw-report-codec-up-to = Up to { $width } × { $height }
hw-report-codec-none = Not available
hw-report-codec-h264-encode = H.264 encoding
# How a screen reader reads a row of capability chips. $list is "BC, ETC2".
hw-report-supported = supported: { $list }
hw-report-unsupported = not supported: { $list }
hw-report-calibration-done = Measured on this computer
hw-report-calibration-pending = Not measured yet
hw-report-paths-power-saving = Saving power: paths that use less energy are preferred
# The path table: how long each way of doing a task takes here, and which one is used.
hw-report-paths-caption = Paths for a { $mpx }-megapixel image
# Task names in that table for tasks timed at a size of their own. $size is hw-report-pixels.
hw-report-task-thumbnail = { $task } (per { $size } thumbnail)
hw-report-task-frame = { $task } (per { $size } frame)
hw-report-col-task = Task
hw-report-col-path = Path
hw-report-col-time = Time
hw-report-col-cpu = CPU time (all threads)
hw-report-col-runs = Runs
hw-report-col-use = Used for
# In the Runs column: not measured here yet, the times are a built-in estimate.
hw-report-runs-untested = Not tested
# What a path is chosen for; $reason is one of the hw-report-reason-* below.
hw-report-use-viewing = Viewing ({ $reason })
hw-report-use-background = Background ({ $reason })
hw-report-reason-only = the only option
hw-report-reason-faster = fastest
hw-report-reason-cheaper = least CPU time
hw-report-reason-lower-power = least energy
hw-report-reason-explore = being tried
hw-report-reason-forced = set in Settings
hw-report-reason-prefer-gpu = Prefer GPU
hw-report-reason-cpu-only = CPU only
hw-report-reason-calibrate = being measured
hw-report-reason-scaled = reduced size
hw-report-trust-unverified = Not checked yet
hw-report-trust-blocked = Turned off after an error
hw-report-trust-absent = Not on this computer

## Accessibility (screen readers)
# The image as a screen reader announces it: file name and size in pixels.
a11y-image = { $name }, { $width } × { $height } pixels
# The region where short notifications (toasts) appear.
a11y-notifications = Notifications
# The bar of playback controls over a video or animation (screen readers).
a11y-transport-playback = Playback
# The bar of page controls over a multi-page file (screen readers).
a11y-transport-pages = Pages
# The list of Settings sections in its sidebar (screen readers).
a11y-settings-sections = Sections

## Read codes (QR codes in the picture)
codes-placeholder = Codes in this picture
codes-looking = Looking for codes…
codes-none = No codes found in this picture.
codes-failed = Couldn't read the picture: { $reason }
codes-copy = Copy “{ $text }”
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Open link on { $host }…
codes-network-off = No network is on, so links don't open.
codes-open-title = Open a link to { $host }?
codes-open-detail = It opens in your web browser. Open only links you trust.
codes-open = Open in browser
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Text in this picture
ocr-looking = Reading the text…
ocr-none = No text found in this picture.
ocr-failed = Couldn't read the text: { $reason }
ocr-needs-pack = Find text needs the Local AI pack’s Text models (Settings › Local AI).
ocr-loading = Loading the text models…
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Copy “{ $text }”
ocr-copy-all = Copy all text
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = { $percent } sure
