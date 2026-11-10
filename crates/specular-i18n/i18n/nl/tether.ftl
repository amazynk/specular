# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Nederlands. machine-drafted, needs native review

palette-tether = Tethered fotograferen…
palette-tether-capture = Foto maken (tethered)

tether-title = Tethered fotograferen
tether-close = Tethered fotograferen sluiten
tether-default-folder = Tethering

## Cameras
tether-none = Geen camera gevonden.
tether-none-hint = Sluit de camera aan met de USB-kabel en zet hem aan. Heeft hij een USB-modus, kies dan PTP of pc-verbinding.
tether-look-again = Opnieuw zoeken
tether-connect = Verbinden
tether-next-camera = Volgende camera
tether-connecting = Verbinden…
tether-connected = { $camera } verbonden
tether-disconnect = Verbinding verbreken
tether-try-again = Opnieuw proberen
tether-gone = De camera is losgekoppeld.
tether-not-connected = Geen camera verbonden. Open eerst Tethered fotograferen.

## Why a camera can't be opened
tether-permission = Deze gebruiker mag het USB-apparaat van de camera niet openen { $node }.
tether-permission-rule = Een udev-regel staat het toe. Zet als beheerder deze regel in { $file }:
tether-permission-reload = Voer daarna "sudo udevadm control --reload" uit en sluit de camera opnieuw aan.
tether-copy-rule = Regel kopiëren
tether-busy-other = Een ander programma gebruikt de camera.
tether-busy-named = De camera wordt gebruikt door { $programs }.
tether-busy-hint = Sluit dat programma of ontkoppel de camera in bestandsbeheer en probeer het opnieuw.
tether-driver = Het systeemstuurprogramma van deze camera laat geen andere programma's toe.

## The camera
tether-battery = Accu { PERCENT($percent) }
tether-card = { $card }: { $free } vrij
tether-card-shots = { $card }: { $free } vrij, { $shots } opnamen
tether-no-card = Geen geheugenkaart.
tether-no-settings = De camera meldt zijn belichtingsinstellingen niet.
tether-no-remote = Deze camera kan niet vanaf hier worden ontspannen. Opnamen gemaakt met de eigen ontspanknop worden gedownload.
tether-capture = Opname maken
tether-step-down = { $setting }: lager ({ $value })
tether-step-up = { $setting }: hoger ({ $value })

## Exposure settings
tether-prop-mode = Stand
tether-prop-shutter = Sluitertijd
tether-prop-aperture = Diafragma
tether-prop-iso = ISO
tether-prop-bias = Belichtingscorrectie
tether-prop-white-balance = Witbalans
tether-auto = Automatisch
tether-bulb = Bulb-stand (B)
tether-camera-value = Camera-instelling { $value }
tether-wb-manual = Handmatig
tether-wb-one-push = Eenmalig meten
tether-wb-daylight = Daglicht
tether-wb-fluorescent = TL-licht
tether-wb-tungsten = Gloeilamp
tether-wb-flash = Flitser
tether-mode-manual = Handmatig
tether-mode-program = Programma
tether-mode-aperture = Diafragmavoorkeuze
tether-mode-shutter = Sluitertijdvoorkeuze
tether-mode-creative = Creatief
tether-mode-action = Sport
tether-mode-portrait = Portret

## Where shots go
tether-folder = Map
tether-names = Namen
tether-names-camera = De namen van de camera
tether-arrival = Develop-stijl
tether-choose-folder = Map…
tether-choose-style = Stijl…
tether-downloading = Downloaden… { PERCENT($percent) }
tether-saved = { $name } opgeslagen
tether-saved-count = { $count ->
    [one] { $count } opname opgeslagen
   *[other] { $count } opnamen opgeslagen
}

## Failures
tether-busy = De camera is bezig. Probeer het opnieuw.
tether-card-full = De geheugenkaart is vol.
tether-set-refused = De camera nam die instelling niet over. Controleer de standknop.
tether-disk = Kon de opname niet opslaan: { $error }
tether-error = Camerafout: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Map voor tethered fotograferen
settings-cull-tether-folder-help = Waar opnamen van een met een kabel verbonden camera worden opgeslagen.
settings-cull-tether-folder-unset = Afbeeldingen › Tethering
settings-cull-tether-rename = Bestandsnamen bij tethered fotograferen
settings-cull-tether-rename-help = Nieuwe namen, met dezelfde codes als Hernoemen bij Importeren. Leeg houdt de namen van de camera.
