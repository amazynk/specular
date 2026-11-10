# machine-drafted, needs native review
# Dateioperationen: Umbenennen, Stapelumbenennung, In Ordner kopieren und verschieben, Drehung in
# der Datei speichern, Rückgängig und Wiederholen, Hintergrundbild, Teilen, per E-Mail senden,
# Einstellungen exportieren und importieren.

## Palette, Spickzettel und Menüs

palette-undo = Rückgängig
palette-undo-help = Die letzte Änderung an Dateien zurücknehmen: Papierkorb, Umbenennen, Kopieren oder Verschieben, Drehen.
palette-redo = Wiederholen
palette-redo-help = Das zuletzt Rückgängig gemachte erneut ausführen.
palette-rename = Umbenennen …
palette-rename-help = Der Datei einen neuen Namen geben; Begleitdateien und RAW+JPEG-Partner folgen.
palette-batch-rename = Dateien umbenennen …
palette-batch-rename-help = Die ausgewählten Dateien oder den ganzen Ordner nach einem Muster umbenennen, mit Vorschau.
palette-copy-to-folder = In Ordner kopieren …
palette-copy-to-folder-help = Die Datei oder die Auswahl in einen anderen Ordner kopieren.
palette-move-to-folder = In Ordner verschieben …
palette-move-to-folder-help = Die Datei oder die Auswahl in einen anderen Ordner verschieben.
palette-rotate-file-left = Datei nach links drehen
palette-rotate-file-left-help = Die Datei selbst um eine Vierteldrehung nach links drehen, verlustfrei, wo das Format es erlaubt.
palette-rotate-file-right = Datei nach rechts drehen
palette-rotate-file-right-help = Die Datei selbst um eine Vierteldrehung nach rechts drehen, verlustfrei, wo das Format es erlaubt.
palette-set-wallpaper-help = Das Bild als Schreibtischhintergrund verwenden.
palette-share = Teilen …
palette-share-help = Die Datei über das Teilen-Menü des Systems an eine andere App senden.
palette-send-by-email = Per E-Mail senden …
palette-send-by-email-help = Eine neue E-Mail in Ihrer Mail-App mit der Datei als Anhang beginnen.
# machine-drafted, needs native review
palette-share-with-zynk = Mit Zynk teilen …
palette-share-with-zynk-help = Einen Weblink zur Datei in der Zynk-App erstellen.
palette-send-with-zynk = Per Zynk senden …
palette-send-with-zynk-help = Die Datei mit Zynk an einen Kontakt oder auf eines Ihrer Geräte senden.
palette-export-settings = Einstellungen exportieren …
palette-export-settings-help = Alle Einstellungen und Tastenbelegungen in einer Datei speichern.
palette-import-settings = Einstellungen importieren …
palette-import-settings-help = Einstellungen aus einer Datei lesen, nachdem die Änderungen gezeigt wurden.

disabled-in-archive-file = Diese Seite liegt in einem Archiv und ist keine eigene Datei
disabled-not-rotatable = Diese Dateiart kann keine Drehung speichern
disabled-not-on-this-desktop = Ihr Desktop bietet das nicht an

## Einstellungen

settings-browsing-rotate-trim-edges = JPEG-Ränder für verlustfreies Drehen beschneiden
settings-browsing-rotate-trim-edges-help = Ein JPEG, dessen Größe kein Vielfaches von 8 oder 16 Pixeln ist, lässt sich nur verlustfrei drehen, wenn die letzten Pixel abgeschnitten werden. Ohne diese Option wird stattdessen das Ausrichtungs-Tag geändert, und das Beschneiden wird jedes Mal angeboten.
settings-browsing-rotate-keep-modified = Änderungsdatum beim Drehen beibehalten
settings-browsing-rotate-keep-modified-help = Eine gedrehte Datei behält ihr Änderungsdatum und bleibt bei Sortierung nach Datum an ihrem Platz.
settings-browsing-recent-folders = Zuletzt verwendete Ordner
settings-browsing-recent-folders-help = Die zuletzt zum Kopieren und Verschieben verwendeten Ordner.

## Dialoge

fileop-cancel = Abbrechen
rename-title = Umbenennen
rename-field-label = Name
rename-button = Umbenennen
rename-sidecars-follow = Ebenfalls umbenannt: { $files }
rename-extension-changed = Eine andere Endung wandelt die Datei nicht um; andere Apps öffnen sie vielleicht nicht.
rename-problem-empty = Geben Sie einen Namen ein.
rename-problem-invalid-char = Namen dürfen „{ $char }“ nicht enthalten.
rename-problem-reserved = „{ $name }“ ist ein Name, den Windows für Geräte reserviert.
rename-problem-too-long = Der Name ist zu lang; erlaubt sind bis zu 255 Byte.
rename-problem-dot = „.“ und „..“ sind als Namen nicht erlaubt.
rename-problem-trailing = Namen dürfen nicht auf Leerzeichen oder Punkt enden.
rename-problem-exists = Eine andere Datei hat bereits diesen Namen.
rename-problem-read-only = Dieser Ordner kann nicht geändert werden.
rename-problem-duplicate = Zwei Dateien würden diesen Namen erhalten.

batch-rename-title =
    { $count ->
        [one] { $count } Datei umbenennen
       *[other] { $count } Dateien umbenennen
    }
batch-rename-field-label = Muster
batch-rename-tokens = Verwenden Sie { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } und { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } Datei wird umbenannt
       *[other] { $count } Dateien werden umbenannt
    }{ $problems ->
        [0] .
        [one] ; { $problems } geht nicht und bleibt unverändert.
       *[other] ; { $problems } gehen nicht und bleiben unverändert.
    }
batch-rename-reading = Daten und Kameras werden gelesen …
batch-rename-preview = Vorschau
batch-rename-unchanged = behält den Namen
batch-rename-more =
    { $count ->
        [one] { $count } weitere darunter
       *[other] { $count } weitere darunter
    }

transfer-copy-title-one = „{ $file }“ in einen Ordner kopieren
transfer-copy-title =
    { $count ->
        [one] { $count } Datei in einen Ordner kopieren
       *[other] { $count } Dateien in einen Ordner kopieren
    }
transfer-move-title-one = „{ $file }“ in einen Ordner verschieben
transfer-move-title =
    { $count ->
        [one] { $count } Datei in einen Ordner verschieben
       *[other] { $count } Dateien in einen Ordner verschieben
    }
transfer-recent = Zuletzt verwendete Ordner
transfer-folder = Ordner
transfer-choose-folder = Ordner wählen …
transfer-copy-dialog = In Ordner kopieren
transfer-move-dialog = In Ordner verschieben
transfer-sidecars =
    { $count ->
        [one] { $count } Begleitdatei kommt mit.
       *[other] { $count } Begleitdateien kommen mit.
    }
transfer-taken =
    { $count ->
        [one] { $count } Name ist in { $folder } bereits vergeben.
       *[other] { $count } Namen sind in { $folder } bereits vergeben.
    }
transfer-when-taken = Wenn ein Name bereits vergeben ist
transfer-skip = Überspringen
transfer-keep-both = Beide behalten
transfer-replace-option = Ersetzen
transfer-copy = Kopieren
transfer-move = Verschieben
transfer-replace-title =
    { $count ->
        [one] { $count } Datei in { $folder } ersetzen?
       *[other] { $count } Dateien in { $folder } ersetzen?
    }
transfer-replace-note = Die dortigen Dateien kommen in den Papierkorb. Rückgängig holt sie zurück.
transfer-replace = Ersetzen
transfer-back = Zurück
transfer-stop = Anhalten

settings-file-import-title = Einstellungen importieren
settings-file-import-dialog = Einstellungen importieren
settings-file-export-dialog = Einstellungen exportieren
settings-file-import-changes = Änderungen
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = keine Taste
settings-file-import-unknown = Keine Einstellung dieser Version; ausgelassen.
settings-file-import-keymap = Tastenbelegung
settings-file-import-nothing = Nichts würde sich ändern: Diese Einstellungen gelten bereits.
settings-file-import-summary =
    { $count ->
        [one] { $count } Änderung. Der Import lässt sich in den Einstellungen rückgängig machen.
       *[other] { $count } Änderungen. Der Import lässt sich in den Einstellungen rückgängig machen.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } Eintrag sieht nach Passwort oder Schlüssel aus und wird nie importiert.
       *[other] { $count } Einträge sehen nach Passwörtern oder Schlüsseln aus und werden nie importiert.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } Benutzerbefehl wird nicht importiert: Befehle starten Programme. Kopieren Sie ihn selbst in Ihre Einstellungsdatei, wenn Sie ihm vertrauen.
       *[other] { $count } Benutzerbefehle werden nicht importiert: Befehle starten Programme. Kopieren Sie sie selbst in Ihre Einstellungsdatei, wenn Sie ihnen vertrauen.
    }
settings-file-import-button = Importieren

## Ergebnisse

fileop-renamed = Umbenannt in { $file }
fileop-renamed-count =
    { $count ->
        [one] { $count } Datei umbenannt
       *[other] { $count } Dateien umbenannt
    }
fileop-copied =
    { $count ->
        [one] { $count } Datei nach { $folder } kopiert
       *[other] { $count } Dateien nach { $folder } kopiert
    }
fileop-moved =
    { $count ->
        [one] { $count } Datei nach { $folder } verschoben
       *[other] { $count } Dateien nach { $folder } verschoben
    }
fileop-skipped =
    { $count ->
        [one] { $count } übersprungen: Der Name ist dort bereits vergeben.
       *[other] { $count } übersprungen: Die Namen sind dort bereits vergeben.
    }
fileop-stopped = Angehalten; was erledigt ist, bleibt erledigt.
fileop-failed-file =
    { $count ->
        [one] { $file } konnte nicht kopiert oder verschoben werden: { $reason }
       *[other] { $count } Dateien konnten nicht kopiert oder verschoben werden; die erste, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] { $count } Datei wird kopiert … { PERCENT($percent) }
       *[other] { $count } Dateien werden kopiert … { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] { $count } Datei wird verschoben … { PERCENT($percent) }
       *[other] { $count } Dateien werden verschoben … { PERCENT($percent) }
    }
fileop-done-status = Fertig
fileop-busy = Die letzte Dateiänderung läuft noch; versuchen Sie es gleich noch einmal.
fileop-nothing-to-redo = Nichts zu wiederholen
fileop-undone = Rückgängig gemacht: { $what }
fileop-redone = Wiederholt: { $what }
fileop-kind-rename = Umbenennen
fileop-kind-batch-rename = Umbenennen mehrerer Dateien
fileop-kind-move = Verschieben in Ordner
fileop-kind-copy = Kopieren in Ordner
fileop-kind-rotate = Drehung
rename-failed = Umbenennen fehlgeschlagen: { $reason }

undo-problem-changed = Rückgängig nicht möglich: { $file } wurde seitdem geändert, und diese Änderung ginge verloren.
undo-problem-missing = Rückgängig nicht möglich: { $file } ist nicht mehr da.
undo-problem-taken = Rückgängig nicht möglich: Eine andere Datei heißt jetzt { $file }.
undo-problem-io = Die Änderung an { $file } lässt sich nicht rückgängig machen: { $reason }

rotate-done-lossless = { $file } verlustfrei gedreht.
rotate-done-lossless-trimmed = { $file } verlustfrei gedreht; der Rand wurde auf { $width } × { $height } beschnitten.
rotate-done-tag = { $file } über das Ausrichtungs-Tag gedreht.
rotate-done-sidecar = Drehung von { $file } in { $sidecar } gespeichert; die RAW-Datei selbst wird nie geändert.
rotate-tag-unreadable = Die Bilddaten ließen sich nicht weit genug lesen, um sie zu drehen.
rotate-tag-multi-picture = Sie enthält mehrere Bilder; das erste zu drehen, würde die anderen beschädigen.
rotate-tag-trailer = Nach dem Bild folgen Daten, die das Drehen verschieben würde.
rotate-tag-not-aligned = Verlustfreies Drehen der Pixel würde sie auf { $width } × { $height } beschneiden.
rotate-trim-offer = Beschneiden und drehen
rotate-problem-unsupported = { $file } ist eine Dateiart, die keine Drehung speichern kann.
rotate-problem-too-large = { $file } ist zu groß zum Drehen.
rotate-problem-damaged = { $file } ist an der Stelle der Ausrichtung beschädigt: { $reason }
rotate-problem-io = { $file } konnte nicht gedreht werden: { $reason }

share-failed = Teilen fehlgeschlagen: { $reason }
email-failed = Mail-App konnte nicht gestartet werden: { $reason }
# machine-drafted, needs native review
zynk-opening = Zynk wird geöffnet…
zynk-failed = Zynk ist nicht erreichbar: { $reason }
zynk-get-title-send = „Per Zynk senden“ braucht die Zynk-App
zynk-get-title-share = „Mit Zynk teilen“ braucht die Zynk-App
zynk-get-about = Zynk ist eine kostenlose App für Mac, Windows, Linux, iPhone und Android. Sie sendet Fotos, Videos und ganze Ordner direkt an Ihre anderen Geräte und an andere Menschen, in jeder Größe. Übertragungen sind Ende-zu-Ende-verschlüsselt und machen dort weiter, wo sie aufgehört haben, wenn die Verbindung abbricht. Für Empfänger ohne Zynk erstellt sie einen Weblink, über den die Dateien im Browser heruntergeladen werden können.
zynk-get-macos = Sie erhalten sie im Mac App Store. Sobald sie installiert ist, übergibt { -app-name } Ihre Dateien direkt an sie.
zynk-get-windows = Sie erhalten sie im Microsoft Store. Sobald sie installiert ist, übergibt { -app-name } Ihre Dateien direkt an sie.
zynk-get-linux = Zynk für Linux ist ein AppImage, es muss nichts installiert werden: Laden Sie es herunter, legen Sie es in ~/Applications ab und machen Sie es ausführbar (in seinen Eigenschaften oder mit chmod +x). { -app-name } findet es dort und überall sonst, solange Zynk läuft.
zynk-get = Zynk holen
zynk-send-old-windows = „Per Zynk senden“ erfordert Windows 10 Version 2004 oder neuer. „Mit Zynk teilen“ funktioniert unter diesem Windows.
settings-file-export-done = Einstellungen in { $file } gespeichert
settings-file-export-failed = Einstellungen konnten nicht gespeichert werden: { $reason }
settings-file-import-unreadable = { $file } konnte nicht gelesen werden: { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } Änderung importiert
       *[other] { $count } Änderungen importiert
    }
