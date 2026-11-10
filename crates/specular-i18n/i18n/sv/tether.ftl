# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Svenska. machine-drafted, needs native review

palette-tether = Fotografering via kabel…
palette-tether-capture = Ta en bild (via kabel)

tether-title = Fotografering via kabel
tether-close = Stäng fotografering via kabel
tether-default-folder = Via kabel

## Cameras
tether-none = Ingen kamera hittades.
tether-none-hint = Anslut kameran med USB-kabeln och slå på den. Om den har ett USB-läge, välj PTP eller datoranslutning.
tether-look-again = Sök igen
tether-connect = Anslut
tether-next-camera = Nästa kamera
tether-connecting = Ansluter…
tether-connected = { $camera } ansluten
tether-disconnect = Koppla från
tether-try-again = Försök igen
tether-gone = Kameran kopplades från.
tether-not-connected = Ingen kamera ansluten. Öppna fotografering via kabel först.

## Why a camera can't be opened
tether-permission = Den här användaren får inte öppna kamerans USB-enhet { $node }.
tether-permission-rule = En udev-regel tillåter det. Lägg som administratör till den här raden i { $file }:
tether-permission-reload = Kör sedan "sudo udevadm control --reload" och anslut kameran igen.
tether-copy-rule = Kopiera regel
tether-busy-other = Ett annat program använder kameran.
tether-busy-named = Kameran används av { $programs }.
tether-busy-hint = Stäng det programmet eller avmontera kameran i filhanteraren och försök igen.
tether-driver = Systemets drivrutin för den här kameran släpper inte in andra program.

## The camera
tether-battery = Batteri { PERCENT($percent) }
tether-card = { $card }: { $free } ledigt
tether-card-shots = { $card }: { $free } ledigt, { $shots } bilder
tether-no-card = Inget minneskort.
tether-no-settings = Kameran rapporterar inte sina exponeringsinställningar.
tether-no-remote = Kameran kan inte utlösas härifrån. Bilder tagna med dess avtryckare laddas ned.
tether-capture = Ta bild
tether-step-down = { $setting }: lägre ({ $value })
tether-step-up = { $setting }: högre ({ $value })

## Exposure settings
tether-prop-mode = Läge
tether-prop-shutter = Slutartid
tether-prop-aperture = Bländare
tether-prop-iso = ISO
tether-prop-bias = Exponeringskompensation
tether-prop-white-balance = Vitbalans
tether-auto = Automatisk
tether-bulb = Bulb-läge (B)
tether-camera-value = Kamerainställning { $value }
tether-wb-manual = Manuell
tether-wb-one-push = Engångsmätning
tether-wb-daylight = Dagsljus
tether-wb-fluorescent = Lysrör
tether-wb-tungsten = Glödlampa
tether-wb-flash = Blixt
tether-mode-manual = Manuell
tether-mode-program = Programautomatik
tether-mode-aperture = Bländarprioritet
tether-mode-shutter = Slutarprioritet
tether-mode-creative = Kreativ
tether-mode-action = Sport
tether-mode-portrait = Porträtt

## Where shots go
tether-folder = Mapp
tether-names = Namn
tether-names-camera = Kamerans namn
tether-arrival = Develop-stil
tether-choose-folder = Mapp…
tether-choose-style = Stil…
tether-downloading = Laddar ned… { PERCENT($percent) }
tether-saved = { $name } sparad
tether-saved-count = { $count ->
    [one] { $count } bild sparad
   *[other] { $count } bilder sparade
}

## Failures
tether-busy = Kameran är upptagen. Försök igen.
tether-card-full = Minneskortet är fullt.
tether-set-refused = Kameran tog inte emot inställningen. Kontrollera lägesratten.
tether-disk = Bilden kunde inte sparas: { $error }
tether-error = Kamerafel: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Mapp för fotografering via kabel
settings-cull-tether-folder-help = Var bilder från en kabelansluten kamera sparas.
settings-cull-tether-folder-unset = Bilder › Via kabel
settings-cull-tether-rename = Filnamn vid fotografering via kabel
settings-cull-tether-rename-help = Nya namn, med samma koder som Byt namn i Import. Tomt behåller kamerans namn.
