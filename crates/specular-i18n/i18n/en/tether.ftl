# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. English.

palette-tether = Tethered capture…
palette-tether-capture = Take a picture (tethered)

tether-title = Tethered capture
tether-close = Close tethered capture
tether-default-folder = Tethered

## Cameras
tether-none = No camera found.
tether-none-hint = Connect the camera with its USB cable and turn it on. Set its USB mode to PTP or PC connection if it has one.
tether-look-again = Look again
tether-connect = Connect
tether-next-camera = Next camera
tether-connecting = Connecting…
tether-connected = { $camera } connected
tether-disconnect = Disconnect
tether-try-again = Try again
tether-gone = The camera was disconnected.
tether-not-connected = No camera connected. Open Tethered capture first.

## Why a camera can't be opened
tether-permission = This user may not open the camera's USB device { $node }.
tether-permission-rule = A udev rule allows it. As administrator, put this line in { $file }:
tether-permission-reload = Then run "sudo udevadm control --reload" and plug the camera in again.
tether-copy-rule = Copy rule
tether-busy-other = Another program is using the camera.
tether-busy-named = The camera is in use by { $programs }.
tether-busy-hint = Close that program, or unmount the camera in the file manager, then try again.
tether-driver = The system's driver for this camera does not let other programs use it.

## The camera
tether-battery = Battery { PERCENT($percent) }
tether-card = { $card }: { $free } free
tether-card-shots = { $card }: { $free } free, { $shots } shots
tether-no-card = No memory card.
tether-no-settings = The camera does not report its exposure settings.
tether-no-remote = This camera can't be triggered from here. Shots taken with its shutter button are downloaded.
tether-capture = Capture
tether-step-down = { $setting }: lower ({ $value })
tether-step-up = { $setting }: higher ({ $value })

## Exposure settings
tether-prop-mode = Mode
tether-prop-shutter = Shutter
tether-prop-aperture = Aperture
tether-prop-iso = ISO
tether-prop-bias = Exposure compensation
tether-prop-white-balance = White balance
tether-auto = Auto
tether-bulb = Bulb
tether-camera-value = Camera setting { $value }
tether-wb-manual = Manual
tether-wb-one-push = One-push
tether-wb-daylight = Daylight
tether-wb-fluorescent = Fluorescent
tether-wb-tungsten = Tungsten
tether-wb-flash = Flash
tether-mode-manual = Manual
tether-mode-program = Program
tether-mode-aperture = Aperture priority
tether-mode-shutter = Shutter priority
tether-mode-creative = Creative
tether-mode-action = Action
tether-mode-portrait = Portrait

## Where shots go
tether-folder = Folder
tether-names = Names
tether-names-camera = The camera's names
tether-arrival = Develop style
tether-choose-folder = Folder…
tether-choose-style = Style…
tether-downloading = Downloading… { PERCENT($percent) }
tether-saved = Saved { $name }
tether-saved-count = { $count ->
    [one] { $count } shot saved
   *[other] { $count } shots saved
}

## Failures
tether-busy = The camera is busy. Try again.
tether-card-full = The memory card is full.
tether-set-refused = The camera did not take that setting. Check its mode dial.
tether-disk = Could not save the shot: { $error }
tether-error = Camera error: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Tethered capture folder
settings-cull-tether-folder-help = Where shots from a tethered camera are saved.
settings-cull-tether-folder-unset = Pictures › Tethered
settings-cull-tether-rename = Tethered file names
settings-cull-tether-rename-help = New names, with the same tokens as Import's Rename. Empty keeps the camera's names.
