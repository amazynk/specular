# Specular — Medical images (DICOM): window / level, presets, the value readout. English.

cheatsheet-medical = Medical images
palette-window-level = Window/level tool (left drag)
palette-window-default = Window: file default
palette-window-lung = Window: lung (W 1500, L −600)
palette-window-mediastinum = Window: mediastinum (W 350, L 50)
palette-window-bone = Window: bone (W 1800, L 400)
palette-window-brain = Window: brain (W 80, L 40)
palette-window-stroke = Window: stroke (W 40, L 40)
palette-window-soft-tissue = Window: soft tissue (W 400, L 50)
palette-window-liver = Window: liver (W 150, L 30)
palette-window-subdural = Window: subdural (W 200, L 75)
palette-window-full-range = Window: full range
palette-invert-grey = Invert grey
palette-next-slice = Next frame
palette-prev-slice = Previous frame
disabled-not-medical = Only for greyscale medical images
disabled-one-frame = The image has one frame
settings-section-medical = Medical images
settings-medical-keep-window = Keep window/level for the next series
settings-medical-keep-window-help = Keep the window you set when moving to another series, instead of starting from its default. Frames of one file and slices of one series always share it.
settings-medical-orientation-labels = Show orientation labels
settings-medical-orientation-labels-help = Letters at the image's edges for the patient's directions (R/L right/left, A/P anterior/posterior, S/I head/feet), taken from the file and following rotation and flips. L toggles them for the session.
settings-medical-wheel-steps-frames = Mouse wheel steps through frames
settings-medical-wheel-steps-frames-help = In a file or series with several frames the wheel moves between frames instead of zooming or changing image. Ctrl+wheel still zooms.
settings-medical-display-function = Grey display
settings-medical-display-function-help = How the greys of medical images are shown. DICOM GSDF spreads them in equal steps of visibility between the display's black and white luminance (DICOM PS3.14), as on a calibrated diagnostic display. The colour profile of the display still applies.
settings-medical-display-function-srgb = Standard (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Display black luminance
settings-medical-display-black-help = The luminance of the display's black, ambient light included, from a measurement or the display's data sheet. Used by DICOM GSDF.
settings-medical-display-white = Display white luminance
settings-medical-display-white-help = The luminance of the display's white, from a measurement or the display's data sheet. Used by DICOM GSDF.
# Readout in the image's corner: centre and width of the window, in the image's units.
medical-window = C { $center }  W { $width }
medical-inverted = inverted
medical-frame = Frame { $frame } of { $count }
medical-tool = Window/level tool: drag
medical-tool-on = Window/level tool on: drag to adjust
medical-tool-off = Window/level tool off
medical-preset-not-ct = CT presets need values in Hounsfield units
medical-file-window-none = No window { $index } in this file
medical-file-window-unnamed = File window { $index } of { $count }
medical-file-window = File window { $index } of { $count }: { $name }
measure-templates-need-medical = Geometry templates come with the Medical profile
medical-off = No medical tools for this image: { $reason }
medical-readout = Value and window
palette-cine = Play frames (cine)
palette-measure-length = Measure length
palette-measure-angle = Measure angle
palette-measure-ellipse = Measure elliptical region
palette-measure-rectangle = Measure rectangular region
palette-clear-measurements = Clear measurements
palette-measure-templates = Geometry templates…
palette-overlays = Show or hide overlays
# Readout line while cine plays; $rate is frames per second.
medical-cine = Playing at { $rate } frames/s
medical-overlays-shown = Overlays shown
medical-overlays-hidden = Overlays hidden
# Hints when a measuring tool is picked (also a readout line while it is on).
measure-hint-length = Length: drag, or click both ends
measure-hint-angle = Angle: click three points, the vertex second
measure-hint-ellipse = Ellipse: drag across the region
measure-hint-rectangle = Rectangle: drag across the region
measure-off = Measuring tool off
measure-cleared = Measurements cleared
measure-length-mm = { $value } mm
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Area { $value } mm²
measure-area-px = Area { $value } px²
measure-angle = { $value }°
# Statistics of the values in a region; $unit is the values' unit (HU) or empty.
measure-mean = Mean { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Max { $max }  ({ $count } px)
# Caveats under a size: how far it can be trusted.
measure-caveat-detector = At the detector: not corrected for magnification
measure-caveat-magnification = Estimated magnification correction
measure-caveat-uncalibrated = No pixel spacing: in pixels
measure-caveat-outside-region = Outside a calibrated ultrasound region: in pixels
palette-orientation-labels = Show or hide orientation labels
palette-presentation-state = Next presentation state
disabled-no-presentation-state = No presentation state refers to this image
medical-labels-shown = Orientation labels shown
medical-labels-hidden = Orientation labels hidden
# A presentation state (GSPS) applied to the image: its label, and which of how many.
medical-presentation = Presentation state: { $label } ({ $index } of { $count })
medical-presentation-none = Presentation state off
# Readout line while a presentation state is applied.
medical-presentation-readout = Presentation: { $label }
medical-preset-presentation = The presentation state's lookup table sets the grey levels
# Screen reader name of the orientation labels at the image edges: the patient direction letters (R/L, A/P, S/I, combined when oblique) of each edge.
medical-orientation-a11y = Orientation: left { $left }, right { $right }, top { $top }, bottom { $bottom }
