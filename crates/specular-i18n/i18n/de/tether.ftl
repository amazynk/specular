# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Deutsch. machine-drafted, needs native review

palette-tether = Tethering…
palette-tether-capture = Foto aufnehmen (Tethering)

tether-title = Tethering
tether-close = Tethering schließen
tether-default-folder = Tethering

## Cameras
tether-none = Keine Kamera gefunden.
tether-none-hint = Kamera per USB-Kabel anschließen und einschalten. Hat sie einen USB-Modus, PTP oder PC-Verbindung wählen.
tether-look-again = Erneut suchen
tether-connect = Verbinden
tether-next-camera = Nächste Kamera
tether-connecting = Verbinde…
tether-connected = { $camera } verbunden
tether-disconnect = Trennen
tether-try-again = Erneut versuchen
tether-gone = Die Kamera wurde getrennt.
tether-not-connected = Keine Kamera verbunden. Zuerst Tethering öffnen.

## Why a camera can't be opened
tether-permission = Dieser Benutzer darf das USB-Gerät der Kamera nicht öffnen { $node }.
tether-permission-rule = Eine udev-Regel erlaubt es. Als Administrator diese Zeile in { $file } eintragen:
tether-permission-reload = Dann „sudo udevadm control --reload“ ausführen und die Kamera neu anschließen.
tether-copy-rule = Regel kopieren
tether-busy-other = Ein anderes Programm verwendet die Kamera.
tether-busy-named = Die Kamera wird von { $programs } verwendet.
tether-busy-hint = Dieses Programm schließen oder die Kamera im Dateimanager aushängen, dann erneut versuchen.
tether-driver = Der Systemtreiber dieser Kamera lässt keine anderen Programme zu.

## The camera
tether-battery = Akku { PERCENT($percent) }
tether-card = { $card }: { $free } frei
tether-card-shots = { $card }: { $free } frei, { $shots } Aufnahmen
tether-no-card = Keine Speicherkarte.
tether-no-settings = Die Kamera meldet ihre Belichtungseinstellungen nicht.
tether-no-remote = Diese Kamera lässt sich von hier nicht auslösen. Mit ihrem Auslöser gemachte Aufnahmen werden geladen.
tether-capture = Aufnehmen
tether-step-down = { $setting }: niedriger ({ $value })
tether-step-up = { $setting }: höher ({ $value })

## Exposure settings
tether-prop-mode = Modus
tether-prop-shutter = Verschlusszeit
tether-prop-aperture = Blende
tether-prop-iso = ISO
tether-prop-bias = Belichtungskorrektur
tether-prop-white-balance = Weißabgleich
tether-auto = Automatisch
tether-bulb = Langzeit (B)
tether-camera-value = Kameraeinstellung { $value }
tether-wb-manual = Manuell
tether-wb-one-push = Einmal-Messung
tether-wb-daylight = Tageslicht
tether-wb-fluorescent = Leuchtstofflampe
tether-wb-tungsten = Glühlampe
tether-wb-flash = Blitz
tether-mode-manual = Manuell
tether-mode-program = Programmautomatik
tether-mode-aperture = Zeitautomatik
tether-mode-shutter = Blendenautomatik
tether-mode-creative = Kreativ
tether-mode-action = Sport
tether-mode-portrait = Porträt

## Where shots go
tether-folder = Ordner
tether-names = Namen
tether-names-camera = Die Namen der Kamera
tether-arrival = Develop-Stil
tether-choose-folder = Ordner…
tether-choose-style = Stil…
tether-downloading = Wird geladen… { PERCENT($percent) }
tether-saved = { $name } gespeichert
tether-saved-count = { $count ->
    [one] { $count } Aufnahme gespeichert
   *[other] { $count } Aufnahmen gespeichert
}

## Failures
tether-busy = Die Kamera ist beschäftigt. Erneut versuchen.
tether-card-full = Die Speicherkarte ist voll.
tether-set-refused = Die Kamera hat diese Einstellung nicht übernommen. Das Moduswahlrad prüfen.
tether-disk = Die Aufnahme konnte nicht gespeichert werden: { $error }
tether-error = Kamerafehler: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Tethering-Ordner
settings-cull-tether-folder-help = Wo Aufnahmen einer per Kabel verbundenen Kamera gespeichert werden.
settings-cull-tether-folder-unset = Bilder › Tethering
settings-cull-tether-rename = Dateinamen beim Tethering
settings-cull-tether-rename-help = Neue Namen, mit denselben Platzhaltern wie beim Umbenennen im Import. Leer behält die Namen der Kamera.
