# Specular — Settings window. English (source catalogue, British spelling).
#
# Id scheme, generated from the settings schema in crates/specular-settings:
#   settings-section-<section>              section title
#   settings-<section>-<key>                setting label
#   settings-<section>-<key>-help           help text under the label
#   settings-<section>-<key>-<value>        one option of a choice setting
# Keys and values are the settings.toml names with "_" written as "-".

## Window chrome

settings-title = Settings
settings-search-placeholder = Search settings
settings-search-no-results = No settings match { $query }
settings-reset = Reset
settings-reset-section = Reset section
settings-modified = Modified
settings-default-value = Default: { $value }
settings-restart-required = Takes effect after a restart
settings-restart-now = Restart now
settings-undo = Undo
settings-redo = Redo
# Asked before Settings › Advanced › Reset all settings… does anything.
settings-reset-all-confirm = Reset every setting to its default?
settings-reset-all-confirm-detail = This affects every section of Settings. It can be undone afterwards.
settings-reset-all-confirm-ok = Reset all
# Said to screen readers once every setting has been reset.
settings-reset-all-done = All settings were reset to their defaults.
settings-value-on = On
settings-value-off = Off
settings-value-auto = Automatic
settings-value-system = Follow the system
settings-value-system-language = System default
settings-value-follow-language = Follow the interface language
settings-value-default-location = Default location
settings-choose = Choose…
settings-clear = Clear
settings-file-reloaded = settings.toml was changed outside { -app-name } and has been reloaded.
settings-warning-unknown-key = Unknown setting { $key } in settings.toml. It is kept in the file but ignored.
settings-warning-invalid-value = Invalid value for { $key }: { $reason } The default is used instead.
settings-warning-newer-version = settings.toml was written by a newer version of { -app-name }. Settings this version doesn’t know are kept.
settings-warning-syntax = settings.toml can’t be read (line { $line }): { $reason } The defaults are used until it is fixed.

## Validation errors (also printed by `specular --set`)

settings-error-unknown-key = There is no setting called { $key }.
settings-error-unknown-key-suggestion = There is no setting called { $key }. Did you mean { $suggestion }?
settings-error-not-bool = Expected on or off (true or false).
settings-error-not-integer = Expected a whole number.
settings-error-not-number = Expected a number.
settings-error-out-of-range = Must be between { $min } and { $max }.
settings-error-out-of-range-auto = Must be 0 (automatic) or between { $min } and { $max }.
settings-error-unknown-option = Expected one of: { $options }.
settings-error-bad-colour = Expected a colour such as #1E1E1E or #1E1E1E80.
settings-error-bad-language = Expected a language tag such as de, pt-BR or zh-Hans, or “system”.
settings-error-bad-path = Expected a file or folder path.
settings-error-not-settable = This entry is a command and has no value.
settings-error-syntax = Expected key=value, for example viewing.wheel_behaviour=navigate.
settings-error-bad-text = Can’t hold tabs, line breaks or other control characters.
# Why the program that reads the setting can't use the value (said by that program).
settings-error-invalid = { $reason }
settings-error-tile-url = Needs an http or https address with {"{"}z{"}"}, {"{"}x{"}"} and {"{"}y{"}"}.

## Units

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } image
       *[other] { $value } images
    }
unit-threads =
    { $value ->
        [one] { $value } thread
       *[other] { $value } threads
    }
unit-bytes =
    { $value ->
        [one] { $value } byte
       *[other] { $value } bytes
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
# Unit symbols written after a number, as the language writes them (Hebrew "ק״מ").
# Translate only where the language has its own everyday symbols; µm, nm and Å stay.
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Sections

settings-section-general = General
settings-section-viewing = Viewing
settings-section-browsing = Browsing
settings-section-colour = Colour
settings-section-metadata = Metadata and privacy
settings-section-privacy = Privacy
settings-section-performance = Performance
settings-section-accessibility = Accessibility
settings-section-keys = Keys
settings-section-advanced = Advanced

## General

settings-general-language = Interface language
settings-general-language-help = Changes take effect immediately. “System default” follows your operating system’s language.
settings-general-region-formats = Region formats
settings-general-region-formats-help = How dates, times, numbers and sizes are written. By default they follow the interface language.
settings-general-theme = Theme
settings-general-theme-help = Light or dark appearance for windows and controls. Images are never changed.
settings-general-theme-system = Follow the system
settings-general-theme-light = Light
settings-general-theme-dark = Dark
settings-general-on-start = When { -app-name } opens without a file
settings-general-on-start-help = What to show when { -app-name } is opened on its own. A file opened from your file manager or the command line always shows in the viewer.
# The window with Open, Browse, Your pictures, recent folders and "Things to try".
settings-general-on-start-start-screen = The start screen
# The gallery of the folder viewed last (the start screen when there is none).
settings-general-on-start-gallery = Gallery of the last folder
settings-general-on-start-library = Library
# "Things to try": the row of small cards on the start screen (gallery, compare, cull…).
settings-general-start-tips = Show “Things to try” on the start screen
settings-general-start-tips-help = A row of cards with useful features and their keys. “Show welcome” in the command palette brings it back.
# One-time hints, such as which key shows a folder as a gallery.
settings-general-tips = Show tips
settings-general-tips-help = A short hint, once, the first time a feature could help, such as the key that shows a folder as a gallery. Never during a slideshow.
# Hidden setting (not shown in the window): which one-time hints were already given.
settings-general-tips-shown = Tips already shown
settings-general-tips-shown-help = The one-time hints already given, so they are not shown again.
settings-general-your-name = Your name
settings-general-your-name-help = Shown on stamps and markup you add. Saved only in the markup files you make.
settings-general-single-instance = Open files in the existing window
settings-general-single-instance-help = When { -app-name } is already running, opening another image shows it in that window instead of starting a new one.
settings-general-file-associations = File associations…
settings-general-file-associations-help = Make { -app-name } the default viewer for the image formats it supports.

## Viewing

settings-viewing-initial-zoom = Initial zoom
settings-viewing-initial-zoom-help = How large a newly opened image is shown.
settings-viewing-initial-zoom-fit = Fit to window
settings-viewing-initial-zoom-fit-if-larger = Fit large images, show small ones at { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Actual size ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Fill the window
settings-viewing-wheel-behaviour = Mouse wheel
settings-viewing-wheel-behaviour-help = What scrolling the wheel does. Hold Ctrl for the other action.
settings-viewing-wheel-behaviour-help-macos = What scrolling the wheel does. Hold ⌘ for the other action.
settings-viewing-wheel-behaviour-zoom = Zoom
settings-viewing-wheel-behaviour-navigate = Next and previous image
settings-viewing-keep-zoom = Keep zoom between images
settings-viewing-keep-zoom-help = Zoomed in, the next and previous image open at the same zoom, showing the same part of the frame: handy for checking focus across a burst.
settings-viewing-zoom-animation = Animate zooming
settings-viewing-zoom-animation-help = Zoom smoothly instead of jumping. Turned off automatically when “Reduce motion” is on.
settings-viewing-zoom-animation-speed = Zoom animation speed
settings-viewing-zoom-animation-speed-help = How quickly animated zooms settle.
settings-viewing-zoom-animation-speed-relaxed = Relaxed
settings-viewing-zoom-animation-speed-normal = Normal
settings-viewing-zoom-animation-speed-fast = Fast
settings-viewing-background = Background
settings-viewing-background-help = The colour behind the image in a window.
settings-viewing-background-theme = Match the theme
settings-viewing-background-black = Black
settings-viewing-background-dark-grey = Dark grey
settings-viewing-background-light-grey = Light grey
settings-viewing-background-white = White
settings-viewing-background-custom = Custom colour
settings-viewing-background-color = Custom background colour
settings-viewing-background-color-help = Used when Background is set to “Custom colour”.
settings-viewing-fullscreen-background = Full-screen background
settings-viewing-fullscreen-background-help = The colour behind the image in full screen.
settings-viewing-transparency = Transparent areas
settings-viewing-transparency-help = How transparent parts of an image are drawn.
settings-viewing-transparency-checkerboard = Checkerboard
settings-viewing-transparency-background = Background colour
settings-viewing-transparency-black = Black
settings-viewing-transparency-white = White
settings-viewing-pixel-grid-threshold = Show pixel grid from
settings-viewing-pixel-grid-threshold-help = Draw a grid between pixels when zoomed in at least this far.
settings-viewing-smoothing-high-zoom = Enlarged pixels
settings-viewing-smoothing-high-zoom-help = How pixels look when the image is shown larger than { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Smooth, then sharp from { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Always smooth
settings-viewing-smoothing-high-zoom-pixelated = Always sharp squares
settings-viewing-double-click = Double-click on the image
settings-viewing-double-click-help = Double-clicking the empty background always toggles full screen.
settings-viewing-double-click-toggle-zoom = Switch between fit and { PERCENT(100) }
settings-viewing-double-click-fullscreen = Toggle full screen
settings-viewing-double-click-nothing = Do nothing
settings-viewing-fullscreen-on-open = Open images in full screen
settings-viewing-fullscreen-on-open-help = Start in full screen when an image is opened from outside { -app-name }.
settings-viewing-fullscreen-mode = Full-screen style
settings-viewing-fullscreen-mode-help = Native full screen uses its own Space; instant full screen covers the current one without an animation.
settings-viewing-fullscreen-mode-native = Native (own Space)
settings-viewing-fullscreen-mode-borderless = Instant
settings-viewing-toolbar-autohide = Hide the toolbar automatically
settings-viewing-toolbar-autohide-help = The toolbar and edge arrows fade out when the pointer is still and come back when it moves.
settings-viewing-toolbar-autohide-delay = Hide the toolbar after
settings-viewing-toolbar-autohide-delay-help = How long the pointer must be still before the toolbar fades.
settings-viewing-toolbar-items = Toolbar buttons
settings-viewing-toolbar-items-help = Which buttons the toolbar shows, in order.
settings-viewing-toolbar-drag = Rearrange the toolbar by dragging
settings-viewing-toolbar-drag-help = Drag a button along the toolbar to move it. A short click still presses it; Esc puts a dragged button back.
settings-viewing-toolbar-items-open = Open
settings-viewing-toolbar-items-prev = Previous image
settings-viewing-toolbar-items-next = Next image
settings-viewing-toolbar-items-zoom-out = Zoom out
settings-viewing-toolbar-items-zoom-level = Zoom level
settings-viewing-toolbar-items-zoom-in = Zoom in
settings-viewing-toolbar-items-fit = Fit to window
settings-viewing-toolbar-items-rotate-left = Rotate anticlockwise
settings-viewing-toolbar-items-rotate-right = Rotate clockwise
settings-viewing-toolbar-items-info = Information
settings-viewing-toolbar-items-copy = Copy image
settings-viewing-toolbar-items-delete = Move to trash
settings-viewing-toolbar-items-palette = Command palette
settings-viewing-toolbar-items-settings = Settings
settings-viewing-toolbar-items-fullscreen = Full screen
settings-viewing-toolbar-items-spacer = Flexible space
settings-viewing-toolbar-items-separator = Separator
# The toolbar list editor (a preview, the buttons in order, then the hidden ones).
settings-toolbar-preview = Toolbar preview
settings-toolbar-hidden = Hidden buttons
settings-toolbar-add-spacer = Add flexible space
settings-toolbar-add-separator = Add separator
settings-toolbar-reset = Reset to default
# Removes a spacer or separator from the toolbar ($item is its name).
settings-toolbar-remove = Remove { $item }
settings-toolbar-hint = Drag a row to move it, or select it and press Alt+↑ or Alt+↓. Tab moves between the search field and the list.
settings-toolbar-move-up = Move up
settings-toolbar-move-down = Move down
settings-toolbar-move-start = Move to start
settings-toolbar-move-end = Move to end
# The menu of a row of the toolbar list: hide or show a button, remove a spacer or separator.
settings-toolbar-menu-hide = Hide
settings-toolbar-menu-show = Show
settings-toolbar-menu-remove = Remove
# The button that opens a row's menu ($item is the button's name).
settings-toolbar-row-menu = More actions for { $item }
# Where a button is on the toolbar, for screen readers.
settings-toolbar-position = Position { $index } of { $count }
settings-toolbar-hidden-state = Hidden

## Browsing

settings-browsing-sort-order = Sort images by
settings-browsing-sort-order-help = The order used when moving through a folder. Name order puts “img2” before “img10” and follows your language’s alphabet.
settings-browsing-sort-order-natural = Name
settings-browsing-sort-order-date-taken = Date taken
settings-browsing-sort-order-modified = Date modified
settings-browsing-sort-order-size = File size
settings-browsing-sort-order-type = File type
settings-browsing-sort-descending = Reverse order
settings-browsing-sort-descending-help = Sort from last to first.
settings-browsing-loop-folder = Loop at the end of the folder
settings-browsing-loop-folder-help = After the last image, continue with the first, and the other way round.
settings-browsing-include-subfolders = Include subfolders
settings-browsing-include-subfolders-help = Move through images in subfolders as well.
settings-browsing-hold-acceleration = Speed-up when holding next or previous
settings-browsing-hold-acceleration-help = How fast flipping speeds up while a key or button is held down.
settings-browsing-hold-acceleration-off = No speed-up
settings-browsing-hold-acceleration-gentle = Gentle
settings-browsing-hold-acceleration-normal = Normal
settings-browsing-hold-acceleration-fast = Fast
settings-browsing-confirm-trash = Ask before moving to trash
settings-browsing-confirm-trash-help = Moving to trash can always be undone, so { -app-name } doesn’t ask by default.
settings-browsing-arrows-follow-reading-direction = Arrow keys follow reading direction
settings-browsing-arrows-follow-reading-direction-help = In right-to-left languages, make the left arrow key go to the next image.

## Colour

settings-colour-management = Colour management
settings-colour-management-help = Show colours as they were meant to look, using the image’s colour profile and your display’s profile.
settings-colour-display-profile = Display profile
settings-colour-display-profile-help = The colour profile of your display. Automatic uses the one your operating system reports.
settings-colour-rendering-intent = Rendering intent
settings-colour-rendering-intent-help = How colours that the display can’t show are brought into its range.
settings-colour-rendering-intent-perceptual = Perceptual
settings-colour-rendering-intent-relative-colorimetric = Relative colorimetric
settings-colour-rendering-intent-saturation = Saturation
settings-colour-rendering-intent-absolute-colorimetric = Absolute colorimetric
settings-colour-black-point-compensation = Black point compensation
settings-colour-black-point-compensation-help = Keep shadow detail when the display’s black is lighter than the image’s.
settings-colour-untagged-images = Images without a colour profile
settings-colour-untagged-images-help = How to show images that don’t say which colours they use.
settings-colour-untagged-images-srgb = Treat as sRGB
settings-colour-untagged-images-display = Show unchanged
settings-colour-dithering = Dithering
settings-colour-dithering-help = Add fine blue noise when reducing colour depth, which prevents banding in skies and gradients.
settings-colour-hdr-output = HDR output
settings-colour-hdr-output-help = Show HDR images with their full brightness on displays that support it. Automatic uses HDR when the display reports extra brightness; otherwise bright areas are gently compressed to fit.
settings-colour-hdr-output-auto = Automatic
settings-colour-hdr-output-on = On
settings-colour-hdr-output-off = Off
settings-colour-ocio-config = OCIO config
settings-colour-ocio-config-help = An OpenColorIO config (.ocio) for EXR, Radiance HDR and PFM images. Empty uses the one $OCIO names.
settings-colour-ocio-display = OCIO display
settings-colour-ocio-display-help = The display of the OCIO config your monitor is.
settings-colour-ocio-display-default = The config’s first
settings-colour-ocio-view = OCIO view
settings-colour-ocio-view-help = How scene-linear images are shown on that display, such as a tone-mapped view.
settings-colour-ocio-view-default = The display’s first
settings-colour-ocio-input = OCIO input colour space
settings-colour-ocio-input-help = The colour space of the images’ values.
settings-colour-ocio-input-default = The config’s scene-linear role
# Develop’s Soft proof card remembers these (not shown in the Settings window).
settings-colour-proof-profile = Soft proof profile
settings-colour-proof-profile-help = The printer profile Develop’s soft proof used last.
settings-colour-proof-intent = Soft proof intent
settings-colour-proof-intent-help = How colours the printer cannot print are brought in.
settings-colour-proof-intent-perceptual = Perceptual
settings-colour-proof-intent-relative = Relative colorimetric
settings-colour-proof-bpc = Soft proof black point compensation
settings-colour-proof-bpc-help = Map black to the printer’s black.
settings-colour-proof-paper = Soft proof paper white
settings-colour-proof-paper-help = Show the paper’s own colour.
settings-colour-proof-ink = Soft proof ink black
settings-colour-proof-ink-help = Show how dark the ink’s black is.

## Metadata and privacy

settings-metadata-show-info-on-open = Show the information panel on open
settings-metadata-show-info-on-open-help = Open the panel with camera, date and place details whenever an image is opened.
settings-metadata-info-detail = Information panel shows
settings-metadata-info-detail-help = The summary explains the most useful details in plain language; all fields lists everything stored in the file.
settings-metadata-info-detail-summary = Summary
settings-metadata-info-detail-all = All fields
settings-metadata-reverse-geocoding = Show place names for GPS positions
settings-metadata-reverse-geocoding-help = Look up the nearest town from built-in data. It works offline: positions never leave your computer.
settings-metadata-hide-gps = Hide GPS positions on screen
settings-metadata-hide-gps-help = Don’t show coordinates or places, for example while sharing your screen. Files are not changed.
settings-metadata-develop-sidecar = Save Develop edits next to the photo
settings-metadata-develop-sidecar-help = Writes IMG_1234.CR2.specular.xmp beside the file, so edits travel with it. Off: edits stay in the library only.
settings-metadata-develop-import-lightroom = Import Lightroom settings in Develop
settings-metadata-develop-import-lightroom-help = A photo with Lightroom settings and no Develop edit opens with them, approximately. Off: Develop offers a button.
settings-metadata-units = Units
settings-metadata-units-help = Units for altitude, distances and speeds.
settings-metadata-units-auto = Follow region formats
settings-metadata-units-metric = Metric
settings-metadata-units-imperial = Imperial
settings-metadata-basemap = Base map
settings-metadata-basemap-help = The map under photo locations. Online maps download tiles for the area you view.
settings-metadata-basemap-off = Built-in map only (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Other tile server (online)
settings-metadata-basemap-file = My own map file
settings-metadata-basemap-url = Tile server address
settings-metadata-basemap-url-help = An address with {"{"}z{"}"}, {"{"}x{"}"} and {"{"}y{"}"}, for example https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Map file
settings-metadata-basemap-path-help = An MBTiles or PMTiles file. It works offline.
settings-metadata-basemap-cache = Map tile cache
settings-metadata-basemap-cache-help = Disk space for downloaded tiles. The oldest go first.
settings-metadata-basemap-allowed = Allowed tile servers
settings-metadata-basemap-allowed-help = Map servers you agreed to download tiles from.

## Performance

settings-performance-ram-cache = Memory cache
settings-performance-ram-cache-help = Memory used to keep decoded images ready. Automatic chooses based on how much memory your computer has.
settings-performance-vram-cache = GPU memory cache
settings-performance-vram-cache-help = GPU memory used for images and tiles. Automatic chooses based on your GPU.
settings-performance-prefetch-depth = Images to prepare ahead
settings-performance-prefetch-depth-help = How many images in the direction you are moving are decoded in advance.
settings-performance-decoder-threads = Decoder threads
settings-performance-decoder-threads-help = CPU threads used to decode images. Automatic uses all performance cores and adapts to the file.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Which GPU to use when the computer has more than one.
settings-performance-gpu-adapter-high-performance = Highest performance
settings-performance-gpu-adapter-low-power = Lowest power use
settings-performance-processing = Where images are processed
settings-performance-processing-help = Some work can run on the CPU, the GPU or a hardware decoder. Automatic measures this computer and picks the fastest way for the image you are looking at and the most economical way for work in the background. What it chose on this computer, and why, is shown below.
settings-performance-processing-automatic = Automatic
settings-performance-processing-prefer-gpu = Prefer the GPU
settings-performance-processing-cpu-only = CPU only
settings-performance-drag-preview = Preview while dragging
settings-performance-drag-preview-help = How Develop shows the photo while you drag a slider. Automatic uses a smaller picture only when this computer can’t keep up.
settings-performance-drag-preview-automatic = Automatic
settings-performance-drag-preview-sharp = Always sharp
settings-performance-drag-preview-fast = Always fast

## Settings → Performance → How { -app-name } runs on this computer (the path selector,
## DESIGN.md §12.3.1). $engine and $other are the hw-report-engine-* names (lower case, used
## mid-sentence); paths-engine-* are the standalone names.
settings-paths-title = How { -app-name } runs on this computer
settings-paths-intro = { -app-name } times each way of doing these jobs on this computer and uses the best one. Times are for a { $mpx }-megapixel photo.
settings-paths-intro-measuring = Not finished measuring yet: { -app-name } measures while the computer is idle.
settings-paths-intro-battery = On battery: the choices favour lower energy use.
settings-paths-keywords = GPU CPU NPU graphics card hardware acceleration decoder benchmark speed
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, all cores
paths-engine-gpu = GPU
paths-engine-hw-decoder = Hardware decoder
paths-engine-npu = NPU
paths-engine-hw-encoder = Hardware encoder
settings-paths-your-setting = { $engine } (your setting)
settings-paths-measured-faster = Measured on this computer: { $ratio } times as fast as the { $other }.
settings-paths-measured-similar = Measured on this computer: about as fast as the { $other }.
settings-paths-measured-power = On battery this uses the least energy (measured on this computer).
settings-paths-not-measured = Not measured yet: the usual choice for this kind of computer.
settings-paths-set-by = Chosen by your setting “{ $setting }: { $option }”.
settings-paths-blocked-check = The { $engine } was turned off on { $date }: its result did not match the CPU’s.
settings-paths-blocked-error = The { $engine } was turned off on { $date } after an error.
settings-paths-blocked-lost = The { $engine } was turned off on { $date } after it stopped responding.
settings-paths-blocked-undated = The { $engine } was turned off after an error.
settings-paths-checking = The { $engine } is checked against the CPU before it is used for the image you are looking at.
# Tasks that do not work on the image being viewed (image analysis on a thumbnail, video
# encoding).
settings-paths-checking-before-use = The { $engine } is checked against the CPU before it is used.
# First in a row's details, for tasks not timed on the intro's photo. $size is hw-report-pixels.
settings-paths-unit-thumbnail = Times are for one { $size } thumbnail
settings-paths-unit-frame = Times are per { $size } video frame
settings-paths-exploring = Trying the { $engine } in the background now.
settings-paths-unavailable = This computer has no { $engine } for this.
settings-paths-background-cheaper = In the background: the { $engine }, measured to use { $percent } less CPU time.
settings-paths-background-power = In the background: the { $engine }, to save energy.
settings-paths-background = In the background: the { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms CPU time, { $runs ->
    [one] { $runs } measurement
   *[other] { $runs } measurements
} ({ $confidence })
settings-paths-detail-guess = { $engine }: about { $time } ms (estimate)
settings-paths-confidence-low = low confidence
settings-paths-confidence-medium = medium confidence
settings-paths-confidence-high = high confidence
settings-paths-machine = Measured for { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Try again
settings-paths-measure-again = Measure again
settings-paths-measure-again-question = Forget what was measured on this computer, including the turned-off paths, and measure again while the computer is idle?
settings-paths-measure-again-confirm = Forget measurements

settings-performance-display-cache = Keep a display cache on disk
settings-performance-display-cache-help = Store screen-sized copies of images so folders you have seen open instantly.
settings-performance-display-cache-size = Display cache size
settings-performance-display-cache-size-help = Oldest entries are removed when the cache is full.
settings-performance-display-cache-location = Display cache location
settings-performance-display-cache-location-help = A folder on a fast drive works best.
settings-performance-fast-simd = Use SIMD instructions
settings-performance-fast-simd-help = Use the CPU’s vector instructions for decoding. Turn off only to diagnose a problem.
settings-performance-fast-dct-scaling = Decode at screen size
settings-performance-fast-dct-scaling-help = Decode large JPEGs directly at the size they are shown. Turn off only to diagnose a problem.
settings-performance-fast-parallel-decode = Decode on several cores
settings-performance-fast-parallel-decode-help = Split large images across CPU cores. Turn off only to diagnose a problem.
settings-performance-fast-restart-index = Index large JPEGs for zooming
settings-performance-fast-restart-index-help = Remember where each part of a JPEG starts, so zoomed-in regions decode quickly. Turn off only to diagnose a problem.
settings-performance-fast-yuv-upload = Convert colours on the GPU
settings-performance-fast-yuv-upload-help = Convert JPEG colours on the GPU or the CPU. Automatic picks whichever is faster on this computer. Turn off only to diagnose a problem.
settings-performance-fast-yuv-upload-auto = Automatic
settings-performance-fast-yuv-upload-on = On
settings-performance-fast-yuv-upload-off = Off
settings-performance-fast-async-io = Asynchronous file reading
settings-performance-fast-async-io-help = Read ahead with the operating system’s fastest file interface. Turn off only to diagnose a problem.
settings-performance-fast-mmap = Memory-mapped files
settings-performance-fast-mmap-help = Read files without copying them. Turn off only to diagnose a problem.
settings-performance-hardware-decode = Hardware decoding
settings-performance-hardware-decode-help = Let the GPU's built-in decoder open JPEG photos. Automatic uses it where it helps, usually for images prepared in the background. It is first checked against the normal decoder, and anything it can't handle opens normally.
settings-performance-hardware-decode-auto = Automatic
settings-performance-hardware-decode-on = On
settings-performance-hardware-decode-off = Off
settings-performance-frame-time-hud = Frame-time HUD
settings-performance-frame-time-hud-help = Show decode and frame times and which fast paths are in use.
settings-performance-stall-warning = Report stalled loads after
settings-performance-stall-warning-help = When an image is still loading after this long, write its load state and the pending jobs to the log (standard error, or SPECULAR_LOG_FILE).
settings-performance-insights = Performance insights
settings-performance-insights-help = When images keep opening slowly for the same reason, show “Why is this slow?” in the corner with what would help. What it learns stays on this computer.
settings-performance-insights-storage = Insights about slow storage
settings-performance-insights-storage-help = Show insights when reading files (from a network share, a slow drive or archives) keeps holding up opening images.
settings-performance-insights-decoder = Insights about decoding
settings-performance-insights-decoder-help = Show insights when images keep being decoded on the CPU although a hardware decoder could help.
settings-performance-insights-restore = Show hidden insights again
settings-performance-insights-restore-help = Brings back the insights you hid with × on “Why is this slow?”, both for opening images and for thumbnails. Unavailable while none are hidden.

## Accessibility

settings-accessibility-reduce-motion = Reduce motion
settings-accessibility-reduce-motion-help = Replace zoom, slide and fade animations with instant changes.
settings-accessibility-reduce-motion-system = Follow the system
settings-accessibility-reduce-motion-on = On
settings-accessibility-reduce-motion-off = Off
settings-accessibility-increase-contrast = Increase contrast
settings-accessibility-increase-contrast-help = Stronger borders and text, and less translucency.
settings-accessibility-increase-contrast-system = Follow the system
settings-accessibility-increase-contrast-on = On
settings-accessibility-increase-contrast-off = Off
settings-accessibility-text-scale = Text size
settings-accessibility-text-scale-help = Size of text relative to the standard size.
settings-accessibility-ui-scale = Interface scale
settings-accessibility-ui-scale-help = Size of the whole interface, on top of the display scaling set in your operating system.
settings-accessibility-focus-ring-always = Always show the focus ring
settings-accessibility-focus-ring-always-help = Show which control has keyboard focus even when using the mouse.
settings-accessibility-screen-reader-verbosity = Screen reader detail
settings-accessibility-screen-reader-verbosity-help = How much a screen reader announces when you move between images, and how much of the readout under the cursor it reads.
settings-accessibility-screen-reader-verbosity-concise = Brief
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Detailed
settings-accessibility-announce-cursor = Read the value under the cursor
settings-accessibility-announce-cursor-help = A screen reader reads the pixel value, HU, temperature or position under the cursor or keyboard crosshair once it stops moving.

## Privacy

settings-privacy-no-network = No network
settings-privacy-no-network-help = Off: only these connect, each once you turn it on or open something with it: update checks, Local AI model downloads, online maps, images from a web address or cloud storage, and servers you add (DICOMweb, OMERO, maps). On: nothing connects.

## Advanced

settings-advanced-portable-mode = Portable mode
settings-advanced-portable-mode-help = Keep settings and caches in the folder next to the program instead of your user profile.
settings-advanced-log-level = Log detail
settings-advanced-log-level-help = How much { -app-name } writes to its log file.
settings-advanced-log-level-error = Errors only
settings-advanced-log-level-warn = Warnings
settings-advanced-log-level-info = Information
settings-advanced-log-level-debug = Debugging
settings-advanced-log-level-trace = Everything
settings-advanced-developer-mode = Developer mode
settings-advanced-developer-mode-help = Show tools for developing and testing { -app-name }, such as the pseudo-languages in the language list.
settings-advanced-audit-log = Local audit log
settings-advanced-audit-log-help = Record opened medical images, exports, anonymisation, prints and sends to a PACS in audit.jsonl in the data folder. It stays on this computer.
settings-advanced-hw-report = Hardware report…
settings-advanced-hw-report-help = Show which CPU and GPU features { -app-name } detected and which fast paths it uses.
settings-advanced-renderer-info = Renderer information…
settings-advanced-renderer-info-help = Show the graphics adapter, driver, display formats and memory limits { -app-name } uses, ready to copy into a bug report.
settings-advanced-edit-settings-file = Edit settings.toml
settings-advanced-edit-settings-file-help = Edit the settings file in { -app-name }’s own editor. It checks the file as you type and saves it only when it has no errors.
settings-advanced-import = Import settings…
settings-advanced-import-help = Load settings from a file exported from { -app-name }.
settings-advanced-export = Export settings…
settings-advanced-export-help = Save all settings to one file, for backup or another computer.
settings-advanced-reset-all = Reset all settings…
settings-advanced-reset-all-help = Return every setting to its default. This can be undone.

## settings.toml editor

config-editor-title = settings.toml
config-editor-save = Save
config-editor-revert = Revert
config-editor-close = Close
config-editor-modified = Modified
config-editor-text = Settings file
config-editor-position = Line { $line }, column { $column }
config-editor-saved = settings.toml saved and applied.
config-editor-cant-save =
    { $count ->
        [one] Can’t save: 1 problem. Fix it first.
       *[other] Can’t save: { $count } problems. Fix them first.
    }
config-editor-save-failed = Can’t save settings.toml: { $reason }
config-editor-conflict = settings.toml changed elsewhere.
config-editor-reload = Reload
config-editor-keep-mine = Keep mine
config-editor-discard-question = Discard your unsaved changes to settings.toml?
config-editor-discard = Discard
config-editor-keep-editing = Keep editing
config-editor-find-placeholder = Find in file
config-editor-find-count = { $current } of { $total }
config-editor-find-none = No matches
config-editor-find-previous = Previous match
config-editor-find-next = Next match
config-editor-find-close = Close find
config-editor-problems = Problems
config-editor-problems-count =
    { $count ->
        [0] No problems
        [one] 1 problem
       *[other] { $count } problems
    }
config-editor-no-problems = No problems. The file can be saved.
config-editor-problem-at = Line { $line }: { $message }
config-editor-severity-error = Error
config-editor-severity-warning = Warning
config-editor-severity-info = Note
config-editor-problem-syntax = Not valid TOML: { $reason }
config-editor-problem-duplicate = { $key } is set twice in the same table. Remove one of them.
config-editor-problem-unknown-section = There is no settings section called [{ $key }].
config-editor-problem-unknown-section-suggestion = There is no settings section called [{ $key }]. Did you mean [{ $suggestion }]?
config-editor-problem-unknown-key = There is no setting called { $key }.
config-editor-problem-unknown-key-suggestion = There is no setting called { $key }. Did you mean { $suggestion }?
config-editor-problem-invalid = Invalid value for { $key }: { $reason }
config-editor-problem-migrated = { $key } is written the way an older version of { -app-name } wrote it. It is converted when read.
config-editor-problem-newer-version = This file was written by a newer version of { -app-name } (format { $found }; this version reads { $supported }). Settings it doesn’t know are kept.
config-editor-problem-command = Command { $index } in [[commands]]: { $reason }
config-editor-problem-hook = Hook { $index } in [[hooks]]: { $reason }

## keymap.toml in the same editor (Settings → Keys → Edit keymap.toml)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Key bindings file
config-editor-keymap-saved = keymap.toml saved and applied.
config-editor-keymap-save-failed = Can’t save keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml changed elsewhere.
config-editor-keymap-discard-question = Discard your unsaved changes to keymap.toml?
# After the path of a file that does not exist yet: saving creates it.
config-editor-new-file = new file, created when saved
# Saving keymap.toml without a settings folder (e.g. a portable or headless run).
config-editor-no-folder = there is no settings folder to save it in.
settings-metadata-develop-calibration = Calibration frames folder
settings-metadata-develop-calibration-help = Flat and dark frames for Develop. Their file or folder names contain "flat" or "dark".

## Colour swatches under a colour setting's field
settings-colour-white = White
settings-colour-light-grey = Light grey
settings-colour-grey = Grey
settings-colour-dark-grey = Dark grey
settings-colour-black = Black
settings-colour-yellow = Yellow
settings-colour-orange = Orange
settings-colour-red = Red
settings-colour-magenta = Magenta
settings-colour-blue = Blue
settings-colour-cyan = Cyan
settings-colour-green = Green
