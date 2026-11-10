# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Italiano. machine-drafted, needs native review

palette-tether = Scatto collegato…
palette-tether-capture = Scatta una foto (collegata)

tether-title = Scatto collegato
tether-close = Chiudi lo scatto collegato
tether-default-folder = Scatto collegato

## Cameras
tether-none = Nessuna fotocamera trovata.
tether-none-hint = Collega la fotocamera con il cavo USB e accendila. Se ha una modalità USB, scegli PTP o collegamento al PC.
tether-look-again = Cerca di nuovo
tether-connect = Collega
tether-next-camera = Fotocamera successiva
tether-connecting = Collegamento…
tether-connected = { $camera } collegata
tether-disconnect = Scollega
tether-try-again = Riprova
tether-gone = La fotocamera è stata scollegata.
tether-not-connected = Nessuna fotocamera collegata. Apri prima lo scatto collegato.

## Why a camera can't be opened
tether-permission = Questo utente non può aprire il dispositivo USB della fotocamera { $node }.
tether-permission-rule = Una regola udev lo consente. Come amministratore, metti questa riga in { $file }:
tether-permission-reload = Poi esegui «sudo udevadm control --reload» e ricollega la fotocamera.
tether-copy-rule = Copia la regola
tether-busy-other = Un altro programma sta usando la fotocamera.
tether-busy-named = La fotocamera è in uso da { $programs }.
tether-busy-hint = Chiudi quel programma o smonta la fotocamera nel gestore file, poi riprova.
tether-driver = Il driver di sistema di questa fotocamera non consente ad altri programmi di usarla.

## The camera
tether-battery = Batteria { PERCENT($percent) }
tether-card = { $card }: { $free } liberi
tether-card-shots = { $card }: { $free } liberi, { $shots } scatti
tether-no-card = Nessuna scheda di memoria.
tether-no-settings = La fotocamera non comunica le impostazioni di esposizione.
tether-no-remote = Questa fotocamera non può essere azionata da qui. Gli scatti fatti con il suo pulsante di scatto vengono scaricati.
tether-capture = Scatta
tether-step-down = { $setting }: diminuisci ({ $value })
tether-step-up = { $setting }: aumenta ({ $value })

## Exposure settings
tether-prop-mode = Modalità
tether-prop-shutter = Tempo
tether-prop-aperture = Diaframma
tether-prop-iso = ISO
tether-prop-bias = Compensazione dell’esposizione
tether-prop-white-balance = Bilanciamento del bianco
tether-auto = Automatico
tether-bulb = Posa B
tether-camera-value = Impostazione della fotocamera { $value }
tether-wb-manual = Manuale
tether-wb-one-push = Misurazione singola
tether-wb-daylight = Luce diurna
tether-wb-fluorescent = Fluorescente
tether-wb-tungsten = Tungsteno
tether-wb-flash = Flash
tether-mode-manual = Manuale
tether-mode-program = Programma
tether-mode-aperture = Priorità diaframma
tether-mode-shutter = Priorità tempi
tether-mode-creative = Creativo
tether-mode-action = Sport
tether-mode-portrait = Ritratto

## Where shots go
tether-folder = Cartella
tether-names = Nomi
tether-names-camera = I nomi della fotocamera
tether-arrival = Stile di Develop
tether-choose-folder = Cartella…
tether-choose-style = Stile…
tether-downloading = Download… { PERCENT($percent) }
tether-saved = { $name } salvato
tether-saved-count = { $count ->
    [one] { $count } scatto salvato
   *[other] { $count } scatti salvati
}

## Failures
tether-busy = La fotocamera è occupata. Riprova.
tether-card-full = La scheda di memoria è piena.
tether-set-refused = La fotocamera non ha accettato l’impostazione. Controlla la ghiera dei modi.
tether-disk = Impossibile salvare lo scatto: { $error }
tether-error = Errore della fotocamera: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Cartella dello scatto collegato
settings-cull-tether-folder-help = Dove vengono salvati gli scatti di una fotocamera collegata via cavo.
settings-cull-tether-folder-unset = Immagini › Scatto collegato
settings-cull-tether-rename = Nomi dei file dello scatto collegato
settings-cull-tether-rename-help = Nuovi nomi, con gli stessi segnaposto di Rinomina in Importa. Vuoto mantiene i nomi della fotocamera.
