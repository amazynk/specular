# Specular metadata panel — English (source catalogue).
#
# Conventions
# - Numbers arrive already formatted for the locale (ICU4X) as strings, e.g.
#   $num = "2,456,789". Where a plural form is needed, the raw count is passed
#   too as $n (a number) and used only as the selector.
# - Dates and times arrive already formatted by ICU4X ($date, $time).
# - Every interpolated value is wrapped in Unicode isolates by Fluent.
# - Message ids are stable; never reuse an id for a different meaning.

## Summary group titles

group-camera = Camera
group-exposure = Exposure
group-lens = Lens
group-date = Date and time
group-location = Location
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drone
label-gimbal = Gimbal
label-flight = Aircraft
label-relative-altitude = Above take-off
drone-attitude = yaw { $yaw }, pitch { $pitch }, roll { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperatures
drone-thermal-refused = Not shown: DJI’s SDK licence doesn’t allow it
drone-thermal-refused-detail = Reading DJI temperatures needs DJI’s Thermal SDK, whose licence covers only programs you build yourself. The image and its metadata are shown.
group-image = Image
group-file = File
group-rights = Rights and description
group-credentials = Content credentials
group-medical = Medical image
# Industrial non-destructive testing (DICONDE files).
group-inspection = Component and inspection
group-nde-image = Inspection image (DICONDE)
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Inspection image
group-science = Scientific image
group-em = Electron microscopy
group-spm = Scanning probe microscopy
# Whole-slide images (pathology scanners) and OME microscopy pyramids.
group-slide = Slide
group-microscopy = Microscopy
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Diffraction
group-surface = Surface topography
group-ebsd = Electron backscatter diffraction
group-seismic = Seismic data
# ASDF files (Advanced Scientific Data Format: Roman and JWST data products).
group-asdf = ASDF
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA image
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Tractography
# Shapes a pathologist drew on a whole-slide image, from a file saved beside it.
group-annotations = Annotations
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Mass spectrometry imaging
# Radiometric thermal images (FLIR, InfiRay, InfraTec): a temperature per pixel.
group-thermal = Thermal image
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Flow cytometry
# Spectroscopy camera frames (Princeton Instruments SPE, Andor SIF): spectra against wavelength.
group-spectroscopy = Spectroscopy
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Unstructured mesh
group-drawing = Drawing
group-volume = Volume

## Summary item labels

label-camera = Camera
label-software = Software
# The program that made a document, drawing, model or font (shown in the File group).
label-application = Application
label-shutter = Shutter speed
label-aperture = Aperture
label-iso = ISO
label-focal-length = Focal length
label-exposure-bias = Exposure compensation
label-program = Mode
label-metering = Metering
label-white-balance = White balance
label-flash = Flash
label-lens = Lens
label-lens-make = Lens maker
label-lens-range = Lens range
label-focus-mode = Focus mode
label-stabilisation = Image stabilisation
label-taken = Taken
label-digitised = Digitised
label-edited = Last edited
label-place = Place
label-coordinates = Coordinates
label-altitude = Altitude
label-direction = Direction
label-accuracy = Accuracy
label-gps-time = GPS time
label-dimensions = Dimensions
label-encoding = Encoding
# A still with a short video clip (Motion Photo, Live Photo).
label-motion = Motion
motion-kind-motion-photo = Motion Photo
motion-kind-micro-video = Motion Photo (MicroVideo)
motion-kind-samsung = Samsung Motion Photo
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Colour profile
label-orientation = Orientation
label-file-name = Name
label-folder = Folder
label-file-size = Size
label-format = Format
label-modified = Modified
label-title = Title
label-description = Description
label-creator = Creator
label-copyright = Copyright
label-keywords = Keywords
label-rating = Rating
label-colour-label = Label
label-credentials = Content credentials

## "All fields" group titles (tag names themselves stay as in the standards)

fields-ifd0 = IFD0 (main image)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperability
fields-ifd1 = IFD1 (thumbnail)
fields-icc = ICC profile header
# $namespace: from xmp-namespace; $prefix: e.g. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Basic
    [xmpRights] Rights
    [xmpMM] Media management
    [xmpDM] Dynamic media
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Camera accessories
    [photoshop] Photoshop
    [crs] Camera Raw settings
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS licensing
    [GPano] Photo sphere
    [GCamera] Google camera
    [hdrgm] HDR gain map
    [mwg-rs] Regions
    [pdf] PDF
    [stEvt] Resource events
    [stRef] Resource references
   *[other] Other
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Unknown tag { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } byte)
   *[other] ({ $num } bytes)
}
maker-note-bytes = ({ $num ->
        [one] { $num } byte
       *[other] { $num } bytes
    }, maker note, not decoded)
more-values = { $shown }, … ({ $total ->
        [one] { $total } value
       *[other] { $total } values
    })

## Generic glue

# Separates the parts of a list built in code (flash description, place areas).
list-separator = {", "}
# Separates shutter · aperture · ISO · focal length in the one-line summary.
one-line-separator = {" · "}

## Exposure

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm ({ $equiv } mm equiv.)
focal-length-equiv-only = { $equiv } mm equiv.
# $ev is the magnitude; the sign comes from the selector.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manual
    [2] Program (auto)
    [3] Aperture priority
    [4] Shutter priority
    [5] Creative (depth of field)
    [6] Action (fast shutter)
    [7] Portrait
    [8] Landscape
   *[other] Unknown
}
metering-mode = { $value ->
    [1] Average
    [2] Centre-weighted average
    [3] Spot
    [4] Multi-spot
    [5] Matrix (evaluative)
    [6] Partial
   *[other] Other
}
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Manual focus
    [4] Single
    [5] Continuous
    [6] Manual focus
    [16] Pan focus
    [256] One-shot AF (live view)
    [257] AI Servo AF (live view)
    [258] AI Focus AF (live view)
    [512] Movie snap focus
    [519] Movie Servo AF
   *[other] Other
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Off
    [1] On
    [2] Shoot only
    [3] Panning
    [4] Dynamic
    [256] Off
    [257] On
    [258] Shoot only
    [259] Panning
    [260] Dynamic
   *[other] Other
}
# Panasonic maker note FocusMode (AF-S single, AF-C continuous, AF-F flexible autofocus).
panasonic-focus-mode = { $value ->
    [1] Auto
    [2] Manual focus
    [4] Auto (focus button)
    [5] Auto (continuous)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Other
}
# Panasonic maker note ImageStabilization: optical (lens), body (sensor shift), or both
# together (Dual IS, Dual IS 2); "panning" stabilises vertical shake only.
panasonic-stabilisation = { $value ->
    [2] On (optical)
    [3] Off
    [4] On (mode 2)
    [5] On (optical, panning)
    [6] On (body only)
    [7] On (body only, panning)
    [9] Dual IS
    [10] Dual IS (panning)
    [11] Dual IS 2
    [12] Dual IS 2 (panning)
   *[other] Other
}
# A comment stored in the file (JPEG, PNG and GIF comments, a movie's comment tag).
label-comment = Comment
# "All fields" group of a file's own descriptive tags; $kind: "IPTC", "PNG", "QuickTime"…
fields-tags = { $kind } tags
# Fujifilm maker note: the camera's picture settings (film simulation, tone, effects).
group-picture = Picture settings
fields-fujifilm = Fujifilm maker note
label-film-simulation = Film simulation
label-dynamic-range = Dynamic range
label-highlight-tone = Highlights
label-shadow-tone = Shadows
label-colour-setting = Colour
label-sharpness = Sharpness
label-noise-reduction = Noise reduction
label-clarity = Clarity
label-grain-effect = Grain effect
label-colour-chrome = Colour chrome
label-colour-chrome-blue = Colour chrome blue
label-shutter-type = Shutter type
label-drive-mode = Drive mode
# A setting's steps from normal; $n is the magnitude, the sign comes from the selector.
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
# Film simulation names are Fujifilm's product names; keep them as they are.
fuji-film-mode = { $value ->
    [0] Provia (standard)
    [256] Studio portrait
    [272] Studio portrait (more saturation)
    [288] Astia (soft)
    [304] Studio portrait (sharper)
    [512] Velvia (vivid)
    [768] Studio portrait Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Other
}
fuji-monochrome = { $value ->
    [768] Monochrome
    [769] Monochrome, red filter
    [770] Monochrome, yellow filter
    [771] Monochrome, green filter
    [784] Sepia
    [1280] Acros
    [1281] Acros, red filter
    [1282] Acros, yellow filter
    [1283] Acros, green filter
   *[other] Monochrome
}
fuji-white-balance = { $value ->
    [0] Auto
    [1] Auto (white priority)
    [2] Auto (ambience priority)
    [256] Daylight
    [512] Cloudy
    [768] Daylight fluorescent
    [769] Day white fluorescent
    [770] White fluorescent
    [771] Warm white fluorescent
    [772] Living room warm white fluorescent
    [1024] Incandescent
    [1280] Flash
    [1536] Underwater
    [3840] Custom 1
    [3841] Custom 2
    [3842] Custom 3
    [3843] Custom 4
    [3844] Custom 5
    [4080] Colour temperature
   *[other] Other
}
fuji-white-balance-kelvin = { $kelvin } K
# $percent: the dynamic range setting (100, 200 or 400 %).
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Auto (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Auto
    [256] Standard (DR100)
    [512] Wide 1 (DR230)
    [513] Wide 2 (DR400)
   *[other] Other
}
fuji-effect = { $value ->
    [0] Off
    [32] Weak
    [64] Strong
   *[other] Other
}
fuji-grain-size = { $value ->
    [16] small
    [32] large
   *[other] other
}
# Grain effect: its strength ("Weak") and grain size ("small").
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mechanical
    [1] Electronic
    [2] Electronic (long exposures)
    [3] Electronic front curtain
   *[other] Other
}
fuji-drive-mode = { $value ->
    [0] Single frame
    [1] Continuous, low speed
    [2] Continuous, high speed
   *[other] Other
}
# 0–2: the focus switch (manual, single AF, continuous AF); 100–101: older cameras' auto or manual.
fuji-focus-mode = { $value ->
    [0] Manual
    [1] AF-S
    [2] AF-C
    [100] Auto
    [101] Manual
   *[other] Other
}
# Image stabilisation: its kind ($kind) and mode ($mode), "Sensor shift (on, continuous)".
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] None
    [1] Optical
    [2] Sensor shift
    [3] Lens
    [258] Sensor shift and digital
    [512] Digital
   *[other] Other
}
fuji-stabilisation-mode = { $value ->
    [0] Off
    [1] on, continuous
    [2] on, while shooting
   *[other] Other
}
light-source = { $value ->
    [1] Daylight
    [2] Fluorescent
    [3] Tungsten
    [4] Flash
    [9] Fine weather
    [10] Cloudy
    [11] Shade
    [12] Daylight fluorescent
    [13] Day white fluorescent
    [14] Cool white fluorescent
    [15] White fluorescent
    [16] Warm white fluorescent
    [17] Standard light A
    [18] Standard light B
    [19] Standard light C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Studio tungsten
   *[other] Other
}
white-balance-auto = Auto
white-balance-manual = Manual
white-balance-manual-with-light = Manual ({ $light })
# The flash description is built from these parts, joined with list-separator.
flash-none = No flash function
flash-fired = Flash fired
flash-not-fired = Flash did not fire
flash-forced-on = forced on
flash-forced-off = forced off
flash-auto = auto mode
flash-return-not-detected = return light not detected
flash-return-detected = return light detected
flash-red-eye = red-eye reduction

## Lens

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Date and time

# $date: "Tuesday 14 May 2024"; $time: "18:32"
# $weekday: "Tuesday" (ICU); $date: "14 May 2024" (ICU)
date-with-weekday = { $weekday } { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Time zone worked out from the GPS clock

## Location

# $lat and $lon are unsigned; the hemisphere comes from the selector.
coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] W
   *[east] E
}
altitude = { $ref ->
    [above] { $m } m above sea level
    [below] { $m } m below sea level
   *[sea] At sea level
}
compass-point = { $point ->
    [n] north
    [nne] north-northeast
    [ne] northeast
    [ene] east-northeast
    [e] east
    [ese] east-southeast
    [se] southeast
    [sse] south-southeast
    [s] south
    [ssw] south-southwest
    [sw] southwest
    [wsw] west-southwest
    [w] west
    [wnw] west-northwest
    [nw] northwest
   *[nnw] north-northwest
}
# $point: from compass-point; $degrees: "287"
facing = Facing { $point } ({ $degrees }°)
detail-true-north = Relative to true north
detail-magnetic-north = Relative to magnetic north
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# More than 30 km from the matched town.
place-near = near { $place }
place-detail = { $areas } · { $distance } from { $city }
place-detail-distance = { $distance } from { $city }

## Image

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits }-bit
# $format: "8-bit YCbCr 4:2:0"
encoding-progressive = { $format }, progressive
encoding-baseline = { $format }, baseline
# A hierarchical JPEG (T.81 Annex J: the picture refined over several resolutions).
encoding-hierarchical = { $format }, hierarchical
# $format: "8-bit RGB"; $alpha: how the image stores transparency (an alpha channel, one
# premultiplied into the colour, or transparent palette entries / a colour key).
encoding-alpha = { $format } with { $alpha ->
    [premultiplied] premultiplied alpha
    [transparency] transparency
   *[alpha] alpha
}
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Greyscale
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Indexed colour
    [Multichannel] Multichannel
    [Duotone] Duotone
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminance/chroma (YC)
    [Bayer] Bayer raw
    [X-Trans] X-Trans raw
    [CFA] Colour filter array raw
    [LinearRaw] Linear raw
    [RGBE] RGBE (shared exponent)
    [XYZE] XYZE (shared exponent)
    [R] Single channel (R)
    [RG] Two channels (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (from EXIF tag; no embedded profile)
orientation = { $value ->
    [1] Upright (no rotation needed)
    [2] Mirrored horizontally (applied)
    [3] Rotated 180° (applied)
    [4] Mirrored vertically (applied)
    [5] Mirrored horizontally and rotated 90° counter-clockwise (applied)
    [6] Rotated 90° clockwise (applied)
    [7] Mirrored horizontally and rotated 90° clockwise (applied)
    [8] Rotated 90° counter-clockwise (applied)
   *[other] Unknown ({ $value })
}
# Short form for warnings.
orientation-short = { $value ->
    [1] Upright
    [2] Mirrored horizontally
    [3] Rotated 180°
    [4] Mirrored vertically
    [5] Mirrored horizontally and rotated 90° counter-clockwise
    [6] Rotated 90° clockwise
    [7] Mirrored horizontally and rotated 90° clockwise
    [8] Rotated 90° counter-clockwise
   *[other] value { $value }
}

## File

file-size-bytes = { $num ->
    [one] { $num } byte
   *[other] { $num } bytes
}
# Decimal units (1 KB = 1000 bytes).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Rights

rating-rejected = Rejected
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n } of 5)

## Content credentials

c2pa-present = Present (not verified)
c2pa-detail = This file carries a C2PA manifest; its signature has not been checked.

## ICC header values

icc-class = { $class ->
    [scnr] Input device
    [mntr] Display device
    [prtr] Output device
    [link] Device link
    [spac] Colour space
    [abst] Abstract
    [nmcl] Named colour
   *[other] Unknown
}
icc-intent = { $intent ->
    [0] Perceptual
    [1] Relative colorimetric
    [2] Saturation
    [3] Absolute colorimetric
   *[other] Unknown
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Not specified
   *[other] Unknown
}
icc-coded = { $name } ({ $code })

## Medical images (DICOM)

label-modality = Modality
label-body-part = Body part
label-study = Study
label-series = Series
label-slice-thickness = Slice thickness
label-pixel-spacing = Pixel spacing
label-imager-pixel-spacing = Detector pixel spacing
label-frames = Frames
label-transfer-syntax = Transfer syntax
label-document = Document
label-document-type = Document type
# Row spacing × column spacing.
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
# A transfer syntax name, for pixel data that is or was lossy compressed.
medical-lossy = { $name } (lossy compression)
detail-pixel-spacing = Distance between pixel centres in the patient (rows × columns).
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Distance between pixel centres in the object (rows × columns).
detail-imager-pixel-spacing = Measured at the detector, not corrected for magnification: distances in the patient are usually smaller.

## Industrial inspection (DICONDE)

label-nde-component-shape = Component shape
label-nde-component-manufacturer = Component manufacturer
label-nde-material = Material
label-nde-thickness = Material thickness
label-nde-pipe-diameter = Pipe diameter
# Peak tube voltage (kV) or accelerator energy (MeV).
label-nde-source = Source
label-nde-environment = Environment
# Image quality indicator (penetrameter) type, with its material.
label-nde-iqi = Image quality indicator
label-nde-indications = Indications
label-nde-standard = Standard

## Research volumes (NIfTI, Analyze, NRRD, MetaImage)

label-voxel-size = Voxel size
label-data-type = Data type
label-time-points = Time points
label-orientation-source = Orientation from
label-intent = Values
label-compression = Compression
# Voxel spacing along the stored axes, in millimetres.
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze header (not reliable)
volume-orient-space = NRRD space directions
volume-orient-transform = MetaImage transform
volume-orient-spacings = Voxel spacing only
volume-orient-none = Not stated
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurfer default (not reliable)
volume-orient-minc = MINC dimension cosines
volume-orient-object = Object coordinates (no patient orientation)
volume-orient-parrec = Philips angulation and off-centre
volume-orient-ecat = ECAT voxel sizes and offsets (not reliable)
volume-content-projections = Projections (raw scan)
volume-content-reconstruction = Reconstructed volume
volume-coords-rectilinear = Rectilinear (uneven spacing, shown evenly)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Repetition time
label-volume-te = Echo time
label-volume-ti = Inversion time
label-volume-flip = Flip angle
label-volume-fov = Field of view
label-volume-field = Field strength
label-volume-pedir = Phase encoding
label-volume-cmdline = Command line
label-volume-xform = Talairach transform
label-volume-content = Content
label-volume-projections = Projections
label-volume-angles = Rotation range
label-volume-angle-step = Rotation step
label-volume-kv = Tube voltage
label-volume-current = Tube current
label-volume-power = Source power
label-volume-exposure = Exposure
label-volume-sod = Source to object
label-volume-odd = Object to detector
label-volume-sdd = Source to detector
label-volume-magnification = Optical magnification
label-volume-binning = Binning
label-volume-objective = Objective
label-volume-filter = Source filter
label-volume-camera = Detector
label-volume-instrument = Instrument
label-volume-software = Software
label-volume-date = Acquired
label-volume-ring = Ring artefact correction
label-volume-smoothing = Smoothing
label-volume-coords = Grid
# An eye scan (OCT): which eye.
label-volume-eye = Eye
# Philips PAR/REC: the scan technique (e.g. FEEPI) and the PAR format version.
label-volume-technique = Technique
label-volume-version = Format version
# PET (ECAT 7): the radioisotope, the tracer and the units of the values.
label-volume-isotope = Isotope
label-volume-tracer = Tracer
label-volume-units = Units
volume-eye-right = Right (OD)
volume-eye-left = Left (OS)
detail-volume-unreliable = The file does not say where its axes point in the patient: left and right may be swapped, so no orientation labels are shown.
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = The axes are the scanner's, not a patient's, so no orientation labels are shown.

## Scientific images (FITS, XISF)

label-object = Object
label-telescope = Telescope
label-instrument = Instrument
label-filter = Filter
label-observed = Observed
label-exposure-time = Exposure
label-sample = Samples
label-unit = Unit
label-hdu = Extension
label-centre = Centre
label-centre-deg = Centre (degrees)
label-frame = Coordinate frame
# The galactic and ecliptic sky coordinate frames (ICRS, FK5 J2000… stay as named).
science-frame-galactic = Galactic
science-frame-ecliptic = Ecliptic
label-projection = Projection
label-pixel-scale = Pixel scale
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ per pixel
# Distortions applied to the world coordinates: FITS convention names, e.g. "SIP, D2IM, CPDIS".
label-distortion = Distortion
# The file's world coordinate system (FITS WCS).
label-wcs = World coordinates
# The file's world coordinates cannot be followed. $reason is technical and stays
# untranslated, e.g. "projection TNX", "missing table WCSDVARR,1".
science-wcs-unsupported = Not supported: { $reason }
# The sky position comes from the pointing alone (a TAN projection), not the file's full
# distortion model (GWCS), so it is off by up to a few arcseconds.
science-wcs-approximate = Approximate (pointing only, no distortion)
# SER planetary video captures.
label-observer = Observer
# A one-shot colour camera's colour filter array, e.g. "RGGB".
label-bayer = Bayer pattern
label-capture-start = Capture start (UTC)
label-capture-end = Capture end (UTC)
label-frame-rate = Frame rate
science-fps = { $fps } frames/s
# The video file was cut short: "1,200 of 2,000".
science-frames-cut = { $frames } of { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Dataset
label-pixel-size = Pixel size
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } per pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Microscope
label-voltage = Accelerating voltage
label-magnification = Magnification
label-imaging-mode = Mode
label-detector = Detector
label-acquired = Acquired
label-field-of-view = Field of view
science-kilovolts = { $kv } kV
science-magnification = { $value }×
# Whole-slide images.
label-vendor = Vendor
label-scanner = Scanner
# The objective lens magnification the slide was scanned at ("20×").
label-objective = Objective
label-pyramid-levels = Levels
# Thermal images (FLIR, InfiRay, InfraTec): the object parameters the temperatures were
# computed with.
label-emissivity = Emissivity
label-object-distance = Object distance
# The temperature of the surroundings that the object reflects.
label-reflected-temperature = Reflected temperature
label-atmospheric-temperature = Atmospheric temperature
label-humidity = Relative humidity
# The smallest and largest raw detector value stored in the file.
label-raw-range = Raw range
# The coldest and hottest pixel.
label-temperature-range = Temperature range
# A temperature: $unit is "°C", "°F" or "K".
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
# A range from low to high, e.g. "7,312 – 15,021" or "18.5 °C – 42.1 °C".
thermal-range = { $low } – { $high }
# "3: 46,000 × 32,914, 11,500 × 8,228, 2,875 × 2,057"
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Wavelength
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Photon energy
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Detector distance
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Beam centre
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = Detector 2θ
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Phi (φ)
label-omega = Omega (ω)
label-chi = Chi (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscillation
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° about { $axis }
# The count at which a pixel saturates.
label-overload = Overload
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Resolution
xtal-resolution = { $edge } Å at the edge, { $corner } Å in the corners
label-geometry = Geometry
xtal-curved = Curved image plate: no q, d or resolution rings
xtal-incomplete = Incomplete header: no q, d or resolution rings
# All-fields group title: "FITS header".
fields-science-header = { $format } header
# All-fields group title for an ASDF file's YAML tree.
fields-asdf-tree = ASDF tree
# ASDF files: the data model (e.g. "wfi_image"), the file format and ASDF Standard versions,
# and the library that wrote the file (e.g. "asdf 3.5.0").
label-asdf-model = Data model
label-asdf-version = File version
label-asdf-standard = ASDF Standard
label-asdf-library = Written by
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT keys
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT version
label-root-compression = Compression
label-root-keys = Keys
label-root-class = Class
label-root-name = Name
label-root-title = Title
label-root-entries = Entries
label-root-sum-weights = Sum of weights
# A graph's number of points.
label-root-points = Points
# $axis is "x", "y" or "z".
label-root-axis = Axis { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
       *[other] { $bins } bins
    }, { $min } to { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } variable bin
       *[other] { $bins } variable bins
    }, { $min } to { $max }
label-root-mean-rms = Mean and RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Trees
root-tree = { $name } ({ $entries ->
        [one] { $entries } entry
       *[other] { $entries } entries
    })
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Image type
casa-type-image = Restored image
casa-type-residual = Residual
casa-type-model = Model
casa-type-pb = Primary beam
casa-type-mask = Clean mask
casa-type-psf = Point spread function
casa-type-sumwt = Sum of weights
casa-type-weight = Weights
label-casa-beam = Restoring beam
casa-beam = { $major }″ × { $minor }″, PA { $pa }°
label-casa-plane-beams = Per-plane beams
casa-plane-beams = { $count ->
        [one] { $count } beam
       *[other] { $count } beams
    }, major axis { $min }″ to { $max }″
label-casa-rest-frequency = Rest frequency
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spectral frame
label-casa-velocity = Velocity definition
casa-velocity-radio = Radio
casa-velocity-optical = Optical
casa-velocity-relativistic = Relativistic
label-casa-mask = Default mask
label-casa-storage = Storage
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Mesh
label-ugrid-convention = Convention
label-ugrid-nodes = Nodes
label-ugrid-faces = Faces
label-ugrid-edges = Edges
label-ugrid-shapes = Face shapes
label-ugrid-coordinates = Coordinates
label-ugrid-values = Values on
label-ugrid-split = Split for drawing
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } triangle
   *[other] { $count } triangles
}
ugrid-quads = { $count ->
    [one] { $count } quadrilateral
   *[other] { $count } quadrilaterals
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } polygon of { $max } nodes
   *[other] { $count } polygons of up to { $max } nodes
}
ugrid-lonlat = Longitude and latitude
ugrid-projected = Projected x and y
ugrid-projected-unit = Projected x and y ({ $unit })
ugrid-on-faces = Faces (drawn flat)
ugrid-on-nodes = Nodes (interpolated across the faces)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = None: the image shows the nodes per face
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } face at the antimeridian or a pole
   *[other] { $count } faces at the antimeridian or a pole
}
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = File
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Shapes
label-annotation-groups = Groups
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Left out
annotation-polygon = Polygon
annotation-polyline = Line
annotation-rectangle = Rectangle
annotation-ellipse = Ellipse
annotation-point = Point
annotation-points = Points
annotation-ruler = Ruler
annotation-arrow = Arrow
# The list of annotations is cut short: "and 1,200 more".
annotation-more = and { $count } more
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Annotations in { $file }
label-drawing-version = Version
label-drawing-units = Units
label-drawing-extents = Extents
label-drawing-sheet = Sheet
label-drawing-layouts = Layouts
label-drawing-layers = Layers
label-drawing-entities = Entities
label-drawing-codepage = Code page
label-drawing-missing-fonts = Substituted fonts
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (styles { $styles }), drawn with { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (styles { $styles }), drawn with fallback fonts
# Survey data (LandXML).
label-survey-crs = Coordinate system
survey-crs-none = Not stated
label-survey-project = Project name
label-survey-application = Written by
label-survey-surface = Surface
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } point
       *[other] { $points } points
    }, { $triangles ->
        [one] { $triangles } triangle
       *[other] { $triangles } triangles
    }; plan area { $area } { $unit }², surface area { $area3d } { $unit }²; { $volume } { $unit }³ above its lowest point; elevations { $zmin }–{ $zmax } { $unit }; contours every { $interval } { $unit }
label-survey-alignment = Alignment
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, stations { $start } to { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX fonts
label-drawing-warnings = Problems
label-plot-paper = Paper
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } of { $pages }
label-plot-page = Page
label-plot-pens = Pens used
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Printer job
# What a plot draws: strokes, fills and labels.
label-plot-items = Drawn items
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Model
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } off or frozen
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Inches
    [2] Feet
    [3] Miles
    [4] Millimetres
    [5] Centimetres
    [6] Metres
    [7] Kilometres
    [8] Microinches
    [9] Mils
    [10] Yards
    [11] Ångströms
    [12] Nanometres
    [13] Microns
    [14] Decimetres
    [15] Decametres
    [16] Hectometres
    [17] Gigametres
    [18] Astronomical units
    [19] Light years
    [20] Parsecs
    [21] US survey feet
   *[other] Unitless
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format } entities

## IC layouts (GDSII)

label-layout-library = Library
label-layout-units = Units
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } database unit, { $user } user unit
# The top cell shown (each top cell is a page).
label-layout-cell = Cell
label-layout-cells = Cells
# Cells no other cell places.
label-layout-top-cells = Top cells
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } and { $more } more
# Layer/datatype pairs.
label-layout-layers = Layers
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Shapes
label-layout-texts = Labels
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = References
layout-references = { $refs } ({ $placements } { $placements ->
    [one] placement
   *[other] placements
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Shapes when flattened
label-layout-bounds = Bounds
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } to { $x1 }, { $y1 } µm

## Circuit boards and schematics (Gerber, Excellon, KiCad)

group-board = Board
group-schematic = Schematic
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = View
label-pcb-project = Project
label-pcb-revision = Revision
label-pcb-date = Date
# The CAD program that wrote the file.
label-pcb-generator = Made with
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Board size
label-pcb-thickness = Thickness
label-pcb-copper-layers = Copper layers
label-pcb-sheets = Sheets
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Layers
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Footprints
label-pcb-symbols = Symbols
label-pcb-nets = Nets
label-pcb-tracks = Tracks
label-pcb-wires = Wires
label-pcb-vias = Vias
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Holes
pcb-holes = { $holes } in { $sizes } { $sizes ->
    [one] size
   *[other] sizes
}
label-pcb-apertures = Apertures
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Inner copper { $n }
pcb-role-top-copper = Top copper
pcb-role-bottom-copper = Bottom copper
pcb-role-top-solder-mask = Top solder mask
pcb-role-bottom-solder-mask = Bottom solder mask
pcb-role-top-silkscreen = Top silkscreen
pcb-role-bottom-silkscreen = Bottom silkscreen
pcb-role-top-paste = Top paste
pcb-role-bottom-paste = Bottom paste
pcb-role-top-solder-paste = Top solder paste
pcb-role-bottom-solder-paste = Bottom solder paste
pcb-role-top-adhesive = Top adhesive
pcb-role-bottom-adhesive = Bottom adhesive
pcb-role-top-courtyard = Top courtyard
pcb-role-bottom-courtyard = Bottom courtyard
pcb-role-top-fabrication = Top fabrication
pcb-role-bottom-fabrication = Bottom fabrication
pcb-role-board-outline = Board outline
pcb-role-plated-drill = Plated drill
pcb-role-non-plated-drill = Non-plated drill
pcb-role-drill = Drill
pcb-role-drill-holes = Drill holes
pcb-role-mechanical = Mechanical
pcb-role-margin = Margin
pcb-role-drawings = Drawings
pcb-role-comments = Comments
pcb-role-engineering-change-1 = Engineering change 1
pcb-role-engineering-change-2 = Engineering change 2
pcb-role-vias = Vias
pcb-role-other = Other
pcb-role-unknown = Unknown
label-pcb-macros = Aperture macros

## Spectra (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

group-spectrum = Spectrum
label-spectrum-format = Format
label-spectrum-technique = Technique
label-spectrum-title = Title
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Page
spectrum-page-of = { $page } of { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X axis
label-spectrum-y = Y axis
label-spectrum-nucleus = Nucleus
label-spectrum-frequency = Spectrometer frequency
label-spectrum-solvent = Solvent
label-spectrum-pulse-program = Pulse program
label-spectrum-spectra = Spectra
label-spectrum-points = Points
label-spectrum-instrument = Instrument
# Who or what lab measured it.
label-spectrum-origin = Origin
label-spectrum-date = Measured
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Processing
spectrum-technique-ir = Infrared
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescence
spectrum-technique-nmr = NMR
spectrum-technique-ms = Mass spectrum
spectrum-technique-chromatogram = Chromatogram
spectrum-technique-other = Spectrum

## 3D models (STL, OBJ, PLY, glTF, 3MF)

group-mesh = 3D model
label-triangles = Triangles
label-vertices = Vertices
label-materials = Materials
label-textures = Textures
label-up-axis = Up axis
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } missing)

## Molecular structures (PDB, mmCIF, CIF, SDF, XYZ)

group-structure = Structure
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Entry
label-structure-classification = Classification
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Method
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Resolution
structure-resolution = { $value } Å
label-structure-deposited = Deposited
label-structure-formula = Formula
label-structure-chains = Chains
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Residues
label-structure-atoms = Atoms
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } in the asymmetric unit)
label-structure-bonds = Bonds
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Models
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Records
structure-first-of = { $count } (the first shown)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligands
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } and { $more } more
label-structure-cell = Unit cell
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Space group
label-structure-secondary = Secondary structure
structure-ss-file = From the file
structure-ss-computed = Computed from the backbone

## HDF4 and HDF-EOS2

group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Library
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Datasets
label-hdf4-images = Raster images
label-hdf4-vgroups = Groups
label-hdf4-vdatas = Tables
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Version
# The grids, swaths and point structures the file defines.
label-eos-structures = Structures
eos-grids = { $count ->
    [one] { $count } grid ({ $names })
   *[other] { $count } grids ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } swath ({ $names })
   *[other] { $count } swaths ({ $names })
}
eos-points = { $count ->
    [one] { $count } point structure
   *[other] { $count } point structures
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Structure
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projection
label-eos-grid-size = Grid size
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Corners
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Product
# When the data was acquired.
label-eos-time = Time
eos-time = { $begin } to { $end }

## Planetary images (VICAR, PDS3, PDS4: NASA and ESA mission data)

group-planetary = Planetary image
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Target
label-planetary-mission = Mission
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Spacecraft
label-planetary-start = Start time
label-planetary-stop = Stop time
# The product's identifier in the archive.
label-planetary-product = Product ID
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Map projection
label-planetary-resolution = Resolution
planetary-metres-per-pixel = { $metres } m per pixel
planetary-pixels-per-degree = { $pixels } pixels per degree
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Projection centre
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Covers
planetary-coverage = latitude { $south }° to { $north }°, longitude { $west }° to { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Body radius
planetary-km = { $km } km

## OMERO (images opened from an OMERO server or the Image Data Resource)

group-omero = OMERO
label-omero-server = Server
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (as { $user })
omero-server-public = { $server } (public)
label-omero-image = Image
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Project
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Screen
label-omero-owner = Owner
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Acquired
label-omero-dimensions = Dimensions
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } Z plane
   *[other] { $count } Z planes
}
omero-c = { $count ->
    [one] { $count } channel
   *[other] { $count } channels
}
omero-t = { $count ->
    [one] { $count } time point
   *[other] { $count } time points
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Pixel size
label-omero-channels = Channels
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (hidden)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Fetched as
omero-fetched-tiles = { $encoding } tiles of { $width } × { $height }, { $levels ->
    [one] { $levels } level
   *[other] { $levels } levels
}
omero-fetched-whole = Whole planes as { $encoding }

## Reflectance transformation images (PTM, RTI, Relight)

group-rti = Reflectance transformation
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Basis
label-rti-order = Order
# Coefficients per colour channel.
label-rti-terms = Coefficients
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Colour layout
# Coefficient images stored in the file.
label-rti-planes = Planes
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Scale
label-rti-bias = Bias

## Point clouds

group-points = Point cloud
label-points = Points
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } of { $total }
# The LAS point data record format number (0–10).
label-point-format = Point format
# The coordinate reference system.
label-crs = Coordinate system
# What each point carries besides its position.
label-attributes = Attributes
points-channel-rgb = RGB
points-channel-intensity = Intensity
points-channel-classification = Classification
points-channel-returns = Returns
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Classes
# E57 scans, with point counts.
label-scans = Scans
point-class-0 = Never classified
point-class-1 = Unclassified
point-class-2 = Ground
point-class-3 = Low vegetation
point-class-4 = Medium vegetation
point-class-5 = High vegetation
point-class-6 = Building
point-class-7 = Low point (noise)
point-class-8 = Model key point
point-class-9 = Water
point-class-10 = Rail
point-class-11 = Road surface
point-class-12 = Overlap
point-class-13 = Wire guard
point-class-14 = Wire conductor
point-class-15 = Transmission tower
point-class-16 = Wire connector
point-class-17 = Bridge deck
point-class-18 = High noise
point-class-19 = Overhead structure
point-class-20 = Ignored ground
point-class-21 = Snow
point-class-22 = Temporal exclusion
# A class without a standard name, by number.
point-class-other = Class { $class }

## Gaussian splats

group-splats = Gaussian splats
label-splats = Splats
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } of { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = View-dependent colour
splats-harmonics-none = None
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, trained with anti-aliasing

## Warnings

warn-not-diagnostic = Not for diagnostic use. Specular is a general image viewer, not a medical device: its rendering is not calibrated for diagnosis.

warn-gps = This file records where it was taken. Remove the location before sharing if that should stay private.
warn-null-island = The GPS position is exactly 0°, 0°; the camera probably had no fix, so the location is likely wrong.
warn-date-future = The capture date is in the future; the camera clock was probably set wrong.
warn-icc-damaged = The embedded colour profile is damaged and could not be read.
warn-metadata-failed = The metadata could not be read because of an error in Specular. The image itself is fine.
warn-orientation-conflict = EXIF and XMP disagree about orientation (EXIF: { $exif }, XMP: { $xmp }). The EXIF value is used.
warn-exif-damaged = Some EXIF data could not be read ({ $error }); the rest is shown.
warn-xmp-damaged = The XMP metadata is malformed ({ $error }); only part of it could be read.

## About box

geonames-attribution = Place names: GeoNames (www.geonames.org), licensed under Creative Commons Attribution 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Offline lookup by the reverse_geocoder crate by Grant Miner (MIT or Apache-2.0). The data has been converted to a city list; it is provided as is, without warranty.

## TIFF and DNG structure values (All fields)

subfile-type = { $value ->
    [0] Full-resolution image
    [1] Reduced-resolution image
    [2] One page of a multi-page image
    [4] Transparency mask
   *[other] Other
}
predictor = { $value ->
    [1] None
    [2] Horizontal differencing
    [3] Floating-point differencing
   *[other] Other
}
extra-samples = { $value ->
    [0] Unspecified
    [1] Premultiplied alpha
    [2] Straight alpha
   *[other] Other
}
sample-format = { $value ->
    [1] Unsigned integer
    [2] Signed integer
    [3] Floating point
    [4] Undefined
   *[other] Other
}

## Geospatial (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

group-geospatial = Geospatial
label-geo-crs = Coordinate system
label-geo-datum = Datum
label-geo-projection = Projection
label-geo-unit = Units
label-geo-bounds = Bounds
label-geo-lonlat-bounds = Longitude and latitude
label-geo-pixel-size = Pixel size
label-geo-overviews = Overviews
label-geo-nodata = No data value
label-geo-raster-type = Raster type
label-geo-vertical = Vertical
label-geo-citation = Citation
label-geo-features = Features
label-geo-vertices = Vertices
label-geo-layer = Layer
label-geo-layers = Layers
label-geo-tiles = Tiles
label-geo-encoding = Text encoding
label-geo-warnings = Problems
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Companion files
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (coordinates not supported)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = To WGS 84
geo-to-wgs84-same = None needed
geo-to-wgs84-helmert = Datum shift (Helmert)
geo-to-wgs84-by-area = Datum shift (Helmert, chosen by area)
geo-to-wgs84-unknown = Not known: coordinates are on the file's own datum
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Not stated
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } to { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° to { $east }° longitude, { $south }° to { $north }° latitude
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] 1 overview: { $sizes }
   *[other] { $n } overviews: { $sizes }
}
geo-pixel-is-point = Pixels are points
geo-layer = { $features ->
    [one] { $features } feature
   *[other] { $features } features
}, { $fields ->
    [one] { $fields } field
   *[other] { $fields } fields
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoom { $zoom }, { $tiles ->
    [one] { $tiles } tile
   *[other] { $tiles } tiles
}
fields-geo-geokeys = GeoTIFF keys
fields-geo-gdal = GDAL metadata
fields-geo-wkt = Coordinate system (WKT)
fields-geo-projjson = Coordinate system (PROJJSON)
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Tile set
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Credit
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Header
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI version
label-gifti-role = Holds
gifti-role-surface = Surface
gifti-role-labels = Labels per vertex
gifti-role-data = Values per vertex
label-gifti-arrays = Data arrays
label-gifti-encoding = Encoding
label-gifti-vertices = Vertices
label-gifti-triangles = Triangles
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Structure
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Geometric type
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Coordinates
gifti-transform = { $from } (transform to { $to } not needed)
gifti-transform-applied = { $from } → { $to } (applied)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Shown on
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Array
label-gifti-intent = Intent
label-gifti-data-type = Data type
# Entries of the label table (names and colours of regions).
label-gifti-labels = Label table
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI version
label-cifti-intent = Intent
label-cifti-matrix = Matrix
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Maps
cifti-maps-scalars = { $count ->
        [one] { $count } scalar map
       *[other] { $count } scalar maps
    }
cifti-maps-labels = { $count ->
        [one] { $count } label map
       *[other] { $count } label maps
    }
cifti-maps-series = { $count ->
        [one] { $count } time point
       *[other] { $count } time points
    }
cifti-maps-parcels = { $count ->
        [one] { $count } parcel
       *[other] { $count } parcels
    }
cifti-maps-other = { $count }
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Connectivity
cifti-dense = { $seeds ->
        [one] { $seeds } seed
       *[other] { $seeds } seeds
    }, { $size } read per seed
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Series
cifti-series = from { $start }, every { $step } ({ $unit })
label-cifti-models = Brain models
label-cifti-parcels = Parcels
label-cifti-structures = Structures
label-cifti-voxels = Subcortical voxels
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Surface
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } vertex
       *[other] { $vertices } vertices
    }, { $file }
cifti-surface-missing = { $structure }: needs a surface with { $vertices ->
        [one] { $vertices } vertex
       *[other] { $vertices } vertices
    } (none beside the file)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Shown as
cifti-shown-surfaces = Coloured surfaces
cifti-shown-voxels = Volume of the subcortical voxels
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Connectivity of the picked seed
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Maps left out
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Format
label-tract-streamlines = Streamlines
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } shown)
label-tract-points = Points
label-tract-mean-length = Mean length
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Scalars per point
label-tract-properties = Properties per streamline
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Grid
label-tract-voxel-size = Voxel size
label-tract-voxel-order = Voxel order
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Coordinates
tract-space-vox-to-ras = From the voxel-to-RAS matrix
tract-space-voxel-order = From the voxel order (no voxel-to-RAS matrix)
tract-space-scanner = Scanner space
# MRtrix: the tracking step size.
label-tract-step = Step size

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# What the file's own header records, for formats without Exif or XMP.
group-header = File header
label-header-created = Created
# The program or person that wrote the file.
label-header-creator = Creator
label-header-copyright = Copyright
label-header-project = Project
label-header-comment = Comment
# The scanner, camera or other device the image came from.
label-header-device = Device
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Time code
# A film frame's edge code (KeyKode).
label-header-edge-code = Edge code
label-header-frame-rate = Frame rate
# Where the frame sits in its sequence.
label-header-frame = Frame
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Transfer
# The colour primaries the file states.
label-header-colorimetric = Colorimetry
# The X window a screen dump was taken of.
label-header-window = Window
label-header-resolution = Resolution
label-header-compression = Compression
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profile
label-header-level = Level
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Wavelet levels
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Colour transform
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Artboards
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Not drawn
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Drawn approximately
# Text, tables and subtitles: the character set ("UTF-8", "windows-1252", "CP437 (DOS)").
label-header-encoding = Character set
label-header-lines = Lines
# How lines end: "LF", "CRLF", "CR", or the kinds found ("LF + CRLF").
label-header-line-ends = Line ends
label-header-words = Words
label-header-pages = Pages
label-header-pictures = Pictures
# Pictures a document refers to but that are not shown (on the network, missing, or of a
# kind not drawn).
label-header-pictures-not-shown = Pictures not shown
label-header-tables = Tables
label-header-rows = Rows
label-header-columns = Columns
# The character between a table's fields (",", ";", "|", or "⇥" for a tab).
label-header-delimiter = Delimiter
# The first row of a table, when it names the columns ("city, population, area").
label-header-column-names = Column names
# Subtitle cues in the file.
label-header-cues = Cues
# When the first cue starts and the last one ends ("0:00:01.000 – 1:42:10.500").
label-header-cue-span = Cues from – to

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

group-font = Font
label-font-family = Family
label-font-style = Style
# The face's place in a font collection: "2 of 4".
label-font-face = Face
font-face-of = { $index } of { $count }
label-font-version = Version
label-font-designer = Designer
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Foundry
label-font-glyphs = Glyphs
label-font-outlines = Outlines
font-outlines-truetype = TrueType (quadratic)
font-outlines-cff = PostScript (CFF, cubic)
font-outlines-cff2 = PostScript (CFF2, cubic)
font-outlines-bitmap = Bitmaps only
font-outlines-none = None
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Embedding
font-embedding-installable = May be embedded and installed.
font-embedding-restricted = May not be embedded.
font-embedding-preview-print = May be embedded to view and print only.
font-embedding-editable = May be embedded in documents that are edited.
font-embedding-no-subsetting = Only whole, not subset.
font-embedding-bitmap-only = Only as bitmaps.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Variation axes
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Named styles
label-font-colour = Colour glyphs
font-colour-colr0 = COLR layers
font-colour-colr1 = COLR v1 gradients
font-colour-sbix = Bitmaps (sbix)
font-colour-cbdt = Bitmaps (CBDT)
font-colour-svg = SVG
label-font-weight = Weight
label-font-units-per-em = Units per em
label-font-tables = Tables
label-font-licence = Licence
label-font-licence-url = Licence URL
label-font-copyright = Copyright
label-font-trademark = Trademark

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

group-document = Document
label-document-kind = Kind
document-kind-word = Word document
document-kind-excel = Excel workbook
document-kind-powerpoint = PowerPoint presentation
document-kind-pages = Pages document
document-kind-numbers = Numbers spreadsheet
document-kind-keynote = Keynote presentation
document-kind-odf-text = OpenDocument text
document-kind-odf-spreadsheet = OpenDocument spreadsheet
document-kind-odf-presentation = OpenDocument presentation
document-kind-odf-graphics = OpenDocument drawing
document-kind-odf-other = OpenDocument file
document-kind-playlist = Playlist
label-playlist-items = Items
# Appended to the kind: "Word document, template, with macros".
document-template = template
document-macros = with macros
document-legacy = Office 97–2003 format
label-document-preview = Preview
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format } image
document-preview-none = None inside this file
label-document-subject = Subject
label-document-author = Author
label-document-description = Description
label-document-category = Category
label-document-last-saved-by = Last saved by
label-document-created = Created
label-document-modified = Modified
label-document-printed = Last printed
label-document-pages = Pages
label-document-slides = Slides
label-document-hidden-slides = Hidden slides
label-document-notes = Notes pages
label-document-sheets = Sheets
label-document-words = Words
label-document-characters = Characters
label-document-paragraphs = Paragraphs
label-document-lines = Lines
label-document-editing-time = Editing time
document-minutes = { $minutes } min
label-document-revision = Revision
label-document-application = Saved with
label-document-company = Company
label-document-template = Template

## Books (EPUB)

group-book = Book
label-book-authors = Authors
label-book-publisher = Publisher
label-book-date = Published
label-book-language = Language
label-book-pages = Pages
# Entries in the book's table of contents.
label-book-contents = Contents entries
label-book-layout = Layout
book-layout-reflowable = Reflowable
book-layout-fixed = Fixed layout
book-right-to-left = right to left
label-book-version = Format
book-version = EPUB { $version }
label-book-fonts = Embedded fonts
book-fonts-obfuscated = { $count } ({ $obfuscated } obfuscated)
label-book-subjects = Subjects
label-book-identifier = Identifier
label-book-rights = Rights

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
