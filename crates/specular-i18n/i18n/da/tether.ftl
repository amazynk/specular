# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Dansk. machine-drafted, needs native review

palette-tether = Fotografering via kabel…
palette-tether-capture = Tag et billede (via kabel)

tether-title = Fotografering via kabel
tether-close = Luk fotografering via kabel
tether-default-folder = Via kabel

## Cameras
tether-none = Intet kamera fundet.
tether-none-hint = Tilslut kameraet med USB-kablet, og tænd det. Har det en USB-tilstand, så vælg PTP eller pc-forbindelse.
tether-look-again = Søg igen
tether-connect = Forbind
tether-next-camera = Næste kamera
tether-connecting = Forbinder…
tether-connected = { $camera } forbundet
tether-disconnect = Afbryd
tether-try-again = Prøv igen
tether-gone = Kameraet blev frakoblet.
tether-not-connected = Intet kamera forbundet. Åbn fotografering via kabel først.

## Why a camera can't be opened
tether-permission = Denne bruger må ikke åbne kameraets USB-enhed { $node }.
tether-permission-rule = En udev-regel tillader det. Tilføj som administrator denne linje i { $file }:
tether-permission-reload = Kør derefter "sudo udevadm control --reload", og tilslut kameraet igen.
tether-copy-rule = Kopiér regel
tether-busy-other = Et andet program bruger kameraet.
tether-busy-named = Kameraet bruges af { $programs }.
tether-busy-hint = Luk det program, eller afmonter kameraet i filhåndteringen, og prøv igen.
tether-driver = Systemets driver til dette kamera lukker ikke andre programmer ind.

## The camera
tether-battery = Batteri { PERCENT($percent) }
tether-card = { $card }: { $free } ledigt
tether-card-shots = { $card }: { $free } ledigt, { $shots } billeder
tether-no-card = Intet hukommelseskort.
tether-no-settings = Kameraet oplyser ikke sine eksponeringsindstillinger.
tether-no-remote = Kameraet kan ikke udløses herfra. Billeder taget med dets udløser hentes.
tether-capture = Tag billede
tether-step-down = { $setting }: lavere ({ $value })
tether-step-up = { $setting }: højere ({ $value })

## Exposure settings
tether-prop-mode = Tilstand
tether-prop-shutter = Lukkertid
tether-prop-aperture = Blænde
tether-prop-iso = ISO
tether-prop-bias = Eksponeringskompensation
tether-prop-white-balance = Hvidbalance
tether-auto = Automatisk
tether-bulb = B-indstilling
tether-camera-value = Kameraindstilling { $value }
tether-wb-manual = Manuel
tether-wb-one-push = Engangsmåling
tether-wb-daylight = Dagslys
tether-wb-fluorescent = Lysstofrør
tether-wb-tungsten = Glødepære
tether-wb-flash = Blitz
tether-mode-manual = Manuel
tether-mode-program = Programautomatik
tether-mode-aperture = Blændeprioritet
tether-mode-shutter = Lukkerprioritet
tether-mode-creative = Kreativ
tether-mode-action = Sport
tether-mode-portrait = Portræt

## Where shots go
tether-folder = Mappe
tether-names = Navne
tether-names-camera = Kameraets navne
tether-arrival = Develop-stil
tether-choose-folder = Mappe…
tether-choose-style = Stil…
tether-downloading = Henter… { PERCENT($percent) }
tether-saved = { $name } gemt
tether-saved-count = { $count ->
    [one] { $count } billede gemt
   *[other] { $count } billeder gemt
}

## Failures
tether-busy = Kameraet er optaget. Prøv igen.
tether-card-full = Hukommelseskortet er fuldt.
tether-set-refused = Kameraet tog ikke imod indstillingen. Tjek tilstandshjulet.
tether-disk = Billedet kunne ikke gemmes: { $error }
tether-error = Kamerafejl: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Mappe til fotografering via kabel
settings-cull-tether-folder-help = Hvor billeder fra et kabelforbundet kamera gemmes.
settings-cull-tether-folder-unset = Billeder › Via kabel
settings-cull-tether-rename = Filnavne ved fotografering via kabel
settings-cull-tether-rename-help = Nye navne, med de samme koder som Omdøb i Import. Tomt beholder kameraets navne.
