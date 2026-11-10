# Animation playback and reading mode (archives, comic books).

disabled-not-animated = This image is not animated
disabled-in-archive = Pages inside an archive cannot be moved to the trash

## Command palette and cheat sheet

palette-play-pause = Play or pause animation
palette-next-frame = Next frame
palette-prev-frame = Previous frame
palette-faster = Play faster
palette-slower = Play slower
palette-normal-speed = Normal playback speed
palette-loop-forever = Loop animation forever
palette-right-to-left = Right-to-left page order
palette-contents = Table of contents
palette-spreads = Two-page spreads

## Animation controls (bar above the toolbar)

# Accessibility name of the group of playback controls.
transport-region = Animation controls
transport-play = Play
transport-pause = Pause
transport-prev-frame = Previous frame
transport-next-frame = Next frame
transport-slower = Slower
transport-faster = Faster
# $index is the frame shown, $count the number of frames.
transport-frame = Frame { $index } / { $count }
# The number of frames is not known until the animation has played once.
transport-frame-unknown = Frame { $index }
# $speed is a formatted number such as 1.5.
transport-speed = { $speed }×
transport-speed-name = Playback speed { $speed }×

## Motion Photos and Live Photos (a still with a short video clip)

# The badge on the photo and on its gallery thumbnail.
motion-badge = Motion
motion-badge-live = Live
# The badge is a button: what pressing it does.
motion-play = Play the motion clip
motion-play-live = Play the Live Photo
motion-stop = Back to the still photo
# Shown for H.264 clips only (HEVC clips play without hardware, decoded in software).
# $reason is a technical message, in English.
toast-motion-failed = Can’t play the motion clip: { $reason }

## Toasts

toast-speed = Playback speed { $speed }×
toast-loop-forever-on = Looping forever
toast-loop-forever-off = Looping as the file says
toast-right-to-left-on = Pages read right to left
toast-right-to-left-off = Pages read left to right
toast-spreads-on = Two-page spreads
toast-spreads-off = One page at a time

## Archives

# Window title for a page of an archive.
title-archive-page = { $page } — { $archive }
# Position readout while two pages are shown.
readout-position-spread = { $first }–{ $last } / { $count }
# $format is RAR.
archive-needs-sandbox = { $format } archives can only be opened by the sandboxed decoder host, which this build does not include yet.
archive-empty = This archive contains no images.
archive-failed = The archive could not be opened: { $error }
archive-encrypted = This archive is encrypted. Open it again to enter its password.
archive-wrong-password = The password for this archive is wrong.
password-title = Password needed
password-archive = “{ $archive }” is encrypted.
password-label = Password
password-show = Show password
password-wrong = Wrong password. Try again.
password-open = Open
password-cancel = Cancel

## Settings

settings-viewing-animation-autoplay = Play animations automatically
settings-viewing-animation-autoplay-help = Start animated images (GIF, APNG, WebP…) as soon as they open.
settings-viewing-animation-loop = Animation looping
settings-viewing-animation-loop-help = How often animations repeat.
settings-viewing-animation-loop-file = As the file says
settings-viewing-animation-loop-forever = Forever
settings-viewing-animation-loop-once = Play once
settings-viewing-animation-short-delays = Very short frame delays
settings-viewing-animation-short-delays-help = Frames meant to last 10 ms or less: slow them to 100 ms like web browsers do, or play them as stored.
settings-viewing-animation-short-delays-browser = Like web browsers (100 ms)
settings-viewing-animation-short-delays-as-stored = As stored
settings-browsing-reading-order = Page order in archives
settings-browsing-reading-order-help = Comic book archives read right to left when their ComicInfo.xml says so (manga), unless set here.
settings-browsing-reading-order-auto = From the archive
settings-browsing-reading-order-left-to-right = Left to right
settings-browsing-reading-order-right-to-left = Right to left
settings-browsing-page-layout = Pages in archives
settings-browsing-page-layout-help = Show one page or two side by side; the cover is always shown alone.
settings-browsing-page-layout-auto = From the archive
settings-browsing-page-layout-single = One page
settings-browsing-page-layout-spreads = Two-page spreads

## Pages of multi-image files (ICNS sizes, ICO/CUR images, texture layers, faces and slices, TIFF pages, HEIF/AVIF collections)
palette-next-page = Next page
palette-prev-page = Previous page
palette-first-page = First page
palette-last-page = Last page
disabled-one-page = The image has only one page
# Page stepper above the toolbar, the info panel and the window title.
page-indicator = Page { $index } of { $count }
# Spoken on a page change: $page is page-indicator, $label what the page is (page-label-*).
page-indicator-labelled = { $page }, { $label }
# An icon size in points, e.g. "128×128".
page-label-icon = { $width }×{ $height }
# An icon size with its pixel density, e.g. "128×128 @2x".
page-label-icon-density = { $width }×{ $height } @{ $density }x
# An image's pixel size in a multi-image file (HEIF/AVIF collection), e.g. "1440×960".
page-label-size = { $width }×{ $height }
# An icon or cursor image's pixel size and bits per pixel (ICO/CUR), e.g. "32×32 · 32-bit".
page-label-size-bits = { $width }×{ $height } · { $bits }-bit
# Array layer of a texture, counted from 0 as texture tools do.
page-label-layer = Layer { $index }
# Cube face of a texture; $face is +X, −X, +Y, −Y, +Z or −Z.
page-label-face = Face { $face }
# A texture's sphere-map face.
page-label-sphere = Sphere map
# Depth slice of a volume texture, counted from 0.
page-label-slice = Slice { $index }
# Page 0 of a layered Photoshop document: the merged (flattened) image.
page-label-merged = Merged image
page-label-from-layers = Built from layers
# A Photoshop layer that is hidden in the file; $name is the layer's name.
page-label-layer-hidden = { $name } (hidden)
# A Photoshop layer without a name; $index is its page (1 = the top-most layer).
page-label-layer-unnamed = Unnamed layer { $index }
# Joins the parts of a page label, e.g. "Layer 1 · Face +Z".
page-label-join = { $a } · { $b }
# The pages of a whole-slide image (pathology scanner): the slide itself, the photo of its
# paper label, the overview photo of the whole glass slide, the scanner's small preview.
page-label-slide = Slide
page-label-slide-label = Label
page-label-slide-macro = Overview
page-label-slide-thumbnail = Thumbnail
# A thermal camera image (page 1) and the visible-light photo taken with it (page 2).
page-label-thermal = Thermal image
page-label-visible = Visible photo
page-label-left-view = Left view
page-label-right-view = Right view
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } of { $count }
page-label-t = T { $index } of { $count }
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Default bands
page-label-product-bands = All bands
page-label-product-true-colour = True colour
page-label-product-false-colour = False colour (near infrared)
page-label-product-swir = Short-wave infrared
page-label-product-agriculture = Agriculture
page-label-product-scene-class = Scene classification
page-label-product-cloud = Cloud probability
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
# Window title for a page of a multi-image file; $page is page-indicator.
title-file-page = { $file } — { $page }
# Info panel group of a multi-image file.
info-page-group = Page
info-page-position = Position
info-page-contents = Contents

# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO isn’t used: { $reason }
ocio-builtin-config = $OCIO names a built-in config (ocio://); save it as a file and choose that file.
