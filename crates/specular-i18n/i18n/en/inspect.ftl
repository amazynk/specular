# Inspection tools: clipping warnings, focus peaking, histogram, pixel inspector.

## Command palette
palette-clipping = Show or hide clipping warnings
palette-focus-peaking = Show or hide focus peaking
palette-histogram = Show or hide histogram
palette-pixel-inspector = Show or hide pixel inspector
palette-crosshair = Keyboard crosshair
palette-announce-cursor = Read the value under the cursor

## Histogram panel
histogram-title = Histogram
# Share of visible pixels at or past the clipping levels; values are percentages.
histogram-clipped = Clipped: highlights { $high }, shadows { $low }
histogram-empty = No image visible
# Screen reader summary. $mean is the mean brightness (0–255), $high/$low are percentages.
histogram-a11y = Histogram of the visible image: mean brightness { $mean }, clipped highlights { $high }, clipped shadows { $low }

## Pixel inspector panel
inspector-title = Pixel inspector
inspector-position = Position
inspector-source-rgb = Source RGB
inspector-source-rgba = Source RGBA
inspector-source-ycc = Source Y′CbCr
inspector-source-gray = Source grey
inspector-source-gray16 = Source grey (16-bit)
inspector-source-gray-alpha16 = Source grey + alpha (16-bit)
inspector-source-rgb16 = Source RGB (16-bit)
inspector-source-rgba16 = Source RGBA (16-bit)
inspector-source-pending = Source
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Linear
# Shown when the values come from a reduced preview, not full-resolution pixels.
inspector-approximate = From a reduced preview; zoom in for exact values
inspector-outside = Point at the image to inspect a pixel
# Screen reader summary: pixel position and its sRGB value.
inspector-a11y = Pixel at { $x }, { $y }: sRGB { $value }

## Settings
settings-section-inspection = Inspection
settings-inspection-clip-highlight = Highlight clipping level
settings-inspection-clip-highlight-help = Pixels with any channel at or above this 8-bit value are marked as clipped highlights.
settings-inspection-clip-shadow = Shadow clipping level
settings-inspection-clip-shadow-help = Pixels with any channel at or below this 8-bit value are marked as clipped shadows.
settings-inspection-highlight-color = Highlight warning colour
settings-inspection-highlight-color-help = Colour that marks clipped highlights.
settings-inspection-shadow-color = Shadow warning colour
settings-inspection-shadow-color-help = Colour that marks clipped shadows.
settings-inspection-peaking-sensitivity = Focus peaking sensitivity
settings-inspection-peaking-sensitivity-help = Higher values mark softer edges as in focus.
settings-inspection-peaking-color = Focus peaking colour
settings-inspection-peaking-color-help = Colour that marks sharp edges.
settings-inspection-histogram-channels = Histogram channels
settings-inspection-histogram-channels-help = Which curves the histogram shows.
settings-inspection-histogram-channels-colour = Red, green and blue
settings-inspection-histogram-channels-luminance = Luminance
settings-inspection-histogram-channels-both = Colour and luminance
settings-inspection-histogram-log = Logarithmic histogram
settings-inspection-histogram-log-help = Scale the histogram logarithmically so small counts stay visible.

## Channel isolation
palette-channel-red = Show the red channel
palette-channel-green = Show the green channel
palette-channel-blue = Show the blue channel
# Pressing the key again steps through the alpha views, then back to the image.
palette-channel-alpha = Show the alpha channel (again: over a checkerboard, then over a colour)
palette-channel-luma = Show luminance
# Badge at the top of the window, also announced to screen readers when it changes.
channel-normal = All channels
channel-red = Red channel
channel-green = Green channel
channel-blue = Blue channel
channel-alpha = Alpha channel
channel-alpha-checker = Transparency over checkerboard
channel-alpha-matte = Transparency over colour
channel-luma = Luminance
# Why the alpha view is unavailable.
disabled-no-alpha = This image has no transparency
settings-inspection-alpha-matte-color = Alpha matte colour
settings-inspection-alpha-matte-color-help = Solid colour behind the image in the “transparency over colour” channel view.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Show or hide false colour
palette-next-colour-map = Next colour map
palette-channel-colours = Next channel colours (red–green, magenta–green, magenta–yellow–cyan)
palette-hilo = Show or hide HiLo (pixels at the bit depth’s maximum and minimum)
palette-colour-vision = Next colour-vision simulation
palette-colour-vision-off = Stop simulating colour vision
palette-colour-vision-protanopia = Simulate protanopia (no red cones)
palette-colour-vision-deuteranopia = Simulate deuteranopia (no green cones)
palette-colour-vision-tritanopia = Simulate tritanopia (no blue cones)
palette-colour-vision-achromatopsia = Simulate achromatopsia (no colour)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (not colour-vision-safe)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = raw values
# Screen reader summary of the colour bar.
colour-bar-a11y = Colour bar { $map }: { $low } to { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = False colour: { $map }
badge-hilo = HiLo: marking pixels at the maximum and minimum
badge-colour-vision-protanopia = Simulating protanopia
badge-colour-vision-deuteranopia = Simulating deuteranopia
badge-colour-vision-tritanopia = Simulating tritanopia
badge-colour-vision-achromatopsia = Simulating achromatopsia
channel-colours-red-green = Channels: red, green, blue
channel-colours-magenta-green = Channels: magenta, green
channel-colours-magenta-yellow-cyan = Channels: magenta, yellow, cyan
settings-inspection-false-colour-map = False colour map
settings-inspection-false-colour-map-help = Colour map for false colour and its colour bar. All but jet and phase stay readable with any colour vision.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (diverging)
settings-inspection-false-colour-map-vik = Vik (diverging)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (diverging)
settings-inspection-false-colour-map-curl = Curl (diverging)
settings-inspection-false-colour-map-phase = Phase (cyclic)
settings-inspection-false-colour-map-jet = Jet (not colour-vision-safe)
settings-inspection-colour-bar = Colour bar
settings-inspection-colour-bar-help = Show a colour bar with values in the image’s own units while false colour is on.
settings-inspection-channel-colours = Channel colours
settings-inspection-channel-colours-help = How the colour channels are shown. Magenta–green and magenta–yellow–cyan stay distinct with red–green colour blindness.
settings-inspection-channel-colours-red-green = Red, green, blue
settings-inspection-channel-colours-magenta-green = Magenta, green
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, yellow, cyan
settings-inspection-colour-vision = Colour-vision simulation
settings-inspection-colour-vision-help = Show the whole window as seen with a colour-vision deficiency (Machado et al. 2009). Exports are never affected.
settings-inspection-colour-vision-off = Off
settings-inspection-colour-vision-protanopia = Protanopia
settings-inspection-colour-vision-deuteranopia = Deuteranopia
settings-inspection-colour-vision-tritanopia = Tritanopia
settings-inspection-colour-vision-achromatopsia = Achromatopsia
settings-inspection-auto-range = Auto-range high-bit greyscale
settings-inspection-auto-range-help = Stretch 16-bit greyscale images (TIFF, PNG, FITS) from their darkest to their brightest value. DICOM images keep their own window.

## Keyboard crosshair and the value under the cursor

crosshair-on = Crosshair on. Arrow keys move it, Shift for bigger steps. Enter clicks. Esc ends.
crosshair-off = Crosshair off
announce-cursor-nothing = Nothing under the cursor
