## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = Optional decoders
settings-decoders-look-again = Look again
settings-decoders-look-again-help = Look for decoders installed since.
    { -app-name } also looks each time its window comes to the front.
settings-decoders-braw = Use the Blackmagic RAW SDK
settings-decoders-braw-help = Blackmagic RAW clips open with it when it is installed.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Leave empty to find it where its installer puts it.
settings-decoders-dwg = Use the ODA File Converter
settings-decoders-dwg-help = DWG drawings open with it when it is installed.
    A copy of the drawing is converted in a private folder, which is then deleted.
settings-decoders-dwg-path = ODA File Converter program
settings-decoders-dwg-path-help = Leave empty to find it where its installer puts it.
settings-decoders-occt = Use OpenCASCADE
settings-decoders-occt-help = STEP and IGES models open with it when it is installed.
settings-decoders-occt-path = OpenCASCADE libraries
settings-decoders-occt-path-help = Leave empty to find them where the system keeps libraries.
settings-decoders-czi = Use libCZI
settings-decoders-czi-help = Zeiss CZI files open with it when it is installed.
    It runs in { -app-name }’s sandboxed helper, with no access to your other files.
settings-decoders-czi-path = libCZI library folder
settings-decoders-czi-path-help = Leave empty to find it where the system keeps libraries.

## The panel: one block per decoder.

decoders-intro = Some formats open with free software from their makers.
    Install it once: { -app-name } finds it and uses it. It never downloads it.
decoders-keywords = optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = Found
decoders-state-off = Off
decoders-state-not-found = Not installed
decoders-state-broken = Can’t be used
decoders-state-refused = Not possible
decoders-state-planned = Not available yet
decoders-state-checking = Looking…
# The row's lines.
decoders-uses = Opens with the { $decoder } from { $vendor }.
decoders-not-found = It’s free. Install it, then press Look again.
decoders-off = Switched off: these files don’t open.
decoders-version = Version { $version }.
decoders-r3d-why = RED’s licence lets only programs built with its SDK, under RED’s own terms, read R3D files.
    { -app-name } can’t accept those terms, so R3D files don’t open.
decoders-ecw-why = Hexagon’s licence lets only programs that ship its ECW SDK use it, not a copy you installed.
    So ECW files don’t open. JPEG 2000 files from the ECW SDK do.
decoders-mrsid-why = The MrSID SDK’s licence lets only programs that ship the SDK use it, not a copy you installed.
    So MrSID files don’t open.
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = DJI’s SDK licence covers only programs you build yourself, not a copy you installed. The photo and its metadata still open.
decoders-occt-why = STEP and IGES solids need OpenCASCADE, which this build of { -app-name } can’t use.
# Rows with one button each, under a decoder.
decoders-get-row = The maker’s download page
decoders-get = Get it
decoders-locate-row = Installed somewhere else?
decoders-locate = Locate…
decoders-forget-row = You picked this one by hand.
decoders-forget = Forget…
# Asked before Forget… does anything.
decoders-forget-ask = Forget the { $decoder } you picked?
decoders-forget-detail = { -app-name } goes back to looking for it by itself. Nothing on disk is removed.
decoders-forget-ok = Forget
# The title of the Locate… file dialog.
decoders-locate-title = Locate the { $decoder }
# Toasts.
decoders-locate-wrong = That isn’t the { $decoder }.
decoders-located = Using the { $decoder } you picked.
decoders-forgotten = { -app-name } looks for the { $decoder } by itself again.
decoders-still-missing = The { $decoder } is still not found.
decoders-now-found = Found the { $decoder }.
decoders-looked = Looked for decoders again.

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = { $format } needs { $vendor }’s free decoder
decoders-card-what = { $file } opens with the { $decoder }. It’s free.
decoders-card-how = Install it, then come back: { -app-name } finds it by itself.
decoders-card-get = Get it
decoders-card-installed = I’ve installed it
decoders-card-locate = Locate…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = Needs decoder
gallery-needs-decoder = Opens with the free { $decoder } from { $vendor }, which isn’t installed.

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = { $format } files need the free { $decoder } from { $vendor }, which isn’t installed.
