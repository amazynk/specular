# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Suomi. machine-drafted, needs native review

palette-tether = Kaapelikuvaus…
palette-tether-capture = Ota kuva (kaapelikuvaus)

tether-title = Kaapelikuvaus
tether-close = Sulje kaapelikuvaus
tether-default-folder = Kaapelikuvaus

## Cameras
tether-none = Kameraa ei löytynyt.
tether-none-hint = Liitä kamera USB-kaapelilla ja kytke se päälle. Jos siinä on USB-tila, valitse PTP tai PC-yhteys.
tether-look-again = Etsi uudelleen
tether-connect = Yhdistä
tether-next-camera = Seuraava kamera
tether-connecting = Yhdistetään…
tether-connected = { $camera } yhdistetty
tether-disconnect = Katkaise yhteys
tether-try-again = Yritä uudelleen
tether-gone = Kameran yhteys katkesi.
tether-not-connected = Kameraa ei ole yhdistetty. Avaa ensin kaapelikuvaus.

## Why a camera can't be opened
tether-permission = Tällä käyttäjällä ei ole oikeutta avata kameran USB-laitetta { $node }.
tether-permission-rule = Udev-sääntö sallii sen. Lisää pääkäyttäjänä tämä rivi tiedostoon { $file }:
tether-permission-reload = Suorita sitten "sudo udevadm control --reload" ja liitä kamera uudelleen.
tether-copy-rule = Kopioi sääntö
tether-busy-other = Toinen ohjelma käyttää kameraa.
tether-busy-named = Kameraa käyttää { $programs }.
tether-busy-hint = Sulje se ohjelma tai irrota kamera tiedostonhallinnassa ja yritä uudelleen.
tether-driver = Tämän kameran järjestelmäajuri ei päästä muita ohjelmia käyttämään sitä.

## The camera
tether-battery = Akku { PERCENT($percent) }
tether-card = { $card }: { $free } vapaana
tether-card-shots = { $card }: { $free } vapaana, { $shots } kuvaa
tether-no-card = Ei muistikorttia.
tether-no-settings = Kamera ei kerro valotusasetuksiaan.
tether-no-remote = Tätä kameraa ei voi laukaista täältä. Sen omalla laukaisimella otetut kuvat ladataan.
tether-capture = Ota kuva
tether-step-down = { $setting }: pienemmäksi ({ $value })
tether-step-up = { $setting }: suuremmaksi ({ $value })

## Exposure settings
tether-prop-mode = Tila
tether-prop-shutter = Suljinaika
tether-prop-aperture = Aukko
tether-prop-iso = ISO
tether-prop-bias = Valotuksen korjaus
tether-prop-white-balance = Valkotasapaino
tether-auto = Automaattinen
tether-bulb = Aikavalotus (B)
tether-camera-value = Kameran asetus { $value }
tether-wb-manual = Käsisäätö
tether-wb-one-push = Kertamittaus
tether-wb-daylight = Päivänvalo
tether-wb-fluorescent = Loisteputki
tether-wb-tungsten = Hehkulamppu
tether-wb-flash = Salama
tether-mode-manual = Käsisäätö
tether-mode-program = Ohjelma-automatiikka
tether-mode-aperture = Aukon esivalinta
tether-mode-shutter = Suljinajan esivalinta
tether-mode-creative = Luova
tether-mode-action = Urheilu
tether-mode-portrait = Muotokuva

## Where shots go
tether-folder = Kansio
tether-names = Nimet
tether-names-camera = Kameran nimet
tether-arrival = Develop-tyyli
tether-choose-folder = Kansio…
tether-choose-style = Tyyli…
tether-downloading = Ladataan… { PERCENT($percent) }
tether-saved = { $name } tallennettu
tether-saved-count = { $count ->
    [one] { $count } kuva tallennettu
   *[other] { $count } kuvaa tallennettu
}

## Failures
tether-busy = Kamera on varattu. Yritä uudelleen.
tether-card-full = Muistikortti on täynnä.
tether-set-refused = Kamera ei hyväksynyt asetusta. Tarkista tilanvalitsin.
tether-disk = Kuvaa ei voitu tallentaa: { $error }
tether-error = Kameravirhe: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Kaapelikuvauksen kansio
settings-cull-tether-folder-help = Mihin kaapelilla yhdistetyn kameran kuvat tallennetaan.
settings-cull-tether-folder-unset = Kuvat › Kaapelikuvaus
settings-cull-tether-rename = Kaapelikuvauksen tiedostonimet
settings-cull-tether-rename-help = Uudet nimet, samoilla tunnuksilla kuin tuonnin Nimeä uudelleen. Tyhjä säilyttää kameran nimet.
