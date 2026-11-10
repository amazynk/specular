# machine-drafted, needs native review
# Specular — Import von Kameras, Smartphones und Speicherkarten. Deutsch.
# Die Karte wird nur gelesen; Dateien werden nur auf Wunsch und nach geprüfter Kopie gelöscht.

palette-import = Fotos importieren …
disabled-no-import-source = Keine Speicherkarte, Kamera und kein Smartphone verbunden

## Einstellungen → Import
settings-section-import = Import
settings-import-on-card = Beim Einlegen einer Karte
settings-import-on-card-help = Was passiert, wenn eine Speicherkarte, Kamera oder ein Smartphone mit Fotos verbunden wird.
settings-import-on-card-ask = Fragen
settings-import-on-card-open = Import öffnen
settings-import-on-card-nothing = Nichts tun
settings-import-destination = Importieren nach
settings-import-destination-help = Der Ordner für importierte Fotos und Videos. Ohne Angabe Ihr Ordner „Bilder“.
settings-import-destination-unset = Ordner „Bilder“
settings-import-folder-pattern = Ordner
settings-import-folder-pattern-help = Die Ordner darin: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} und {"{"}event{"}"}, getrennt durch /.
settings-import-rename-pattern = Umbenennen
settings-import-rename-pattern-help = Neue Dateinamen, wie bei Konvertieren: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Leer behält die Namen der Kamera.
settings-import-keep-pairs = RAW und JPEG zusammenhalten
settings-import-keep-pairs-help = Eine RAW-Datei und das zugehörige JPEG landen im selben Ordner mit demselben Namen, auch beim Umbenennen.
settings-import-low-res-videos = Videokopien in niedriger Auflösung importieren
settings-import-low-res-videos-help = Manche Kameras speichern zu jedem Video eine kleine Vorschaukopie (.LRV). Aus: Sie bleiben auf der Karte.
settings-import-eject = Danach auswerfen
settings-import-eject-help = Wirft die Karte aus, sobald alles kopiert ist, damit sie entnommen werden kann.

## Angebot beim Einlegen einer Karte
import-offer = Von { $name } importieren?
import-offer-button = Importieren

## Die Importansicht
import-title = Import
import-from = von { $name }
import-source-next = Nächste Quelle
import-close = Schließen
import-scanning = Suche nach neuen Fotos … { $done } von { $total }
import-scanning-start = Suche nach neuen Fotos …
import-none-new = Alles auf { $name } wurde bereits importiert.
import-empty = Auf { $name } sind keine Fotos oder Videos.
import-no-source = Keine Speicherkarte, Kamera und kein Smartphone verbunden.
import-summary = { $count ->
    [one] 1 neues Objekt
   *[other] { $count } neue Objekte
} · { $size }
import-already = { $count } bereits importiert
import-unreadable = { $count ->
    [one] 1 Datei konnte nicht gelesen werden
   *[other] { $count } Dateien konnten nicht gelesen werden
}
import-selected = { $count } von { $total } ausgewählt
import-select-all = Alles auswählen
import-select-none = Nichts auswählen
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Importieren nach
import-choose = Wählen …
import-folder-pattern = Ordner
import-event = Ereignis
import-event-hint = Optional, für {"{"}event{"}"}
import-rename = Umbenennen
import-rename-hint = Leer behält die Namen der Kamera
import-preview = Erste Datei: { $path }
import-pattern-error = { $error }
import-keep-pairs = RAW und JPEG zusammenhalten
import-eject = Danach auswerfen
import-delete = Nach dem Import von der Karte löschen
import-free = { $size } zu kopieren · { $free } frei
import-free-unknown = { $size } zu kopieren
import-no-space = Nicht genug freier Speicher in { $folder }: Dieser Import braucht { $need }, frei sind nur { $free }. Geben Sie Speicher frei oder wählen Sie einen anderen Ordner.
import-start = { $count ->
    [one] 1 Objekt importieren
   *[other] { $count } Objekte importieren
}
import-nothing-selected = Fotos zum Importieren auswählen

import-confirm-delete = { $count ->
    [one] Nach Kopieren und Prüfen wird 1 Datei von { $name } gelöscht. Das lässt sich nicht rückgängig machen.
   *[other] Nach Kopieren und Prüfen werden { $count } Dateien von { $name } gelöscht. Das lässt sich nicht rückgängig machen.
}
import-confirm-yes = Importieren und löschen
import-confirm-no = Zurück

import-progress = Importiere { $done } von { $total } · { $size }
import-progress-eta = Importiere { $done } von { $total } · { $size } · { $time }
import-paused = Angehalten · { $done } von { $total }
import-pause = Anhalten
import-resume = Fortsetzen
import-cancel = Abbrechen
import-hide = Ausblenden
import-stopping = Wird angehalten …
import-running = Es läuft bereits ein Import.

import-done = { $count ->
    [one] 1 Datei
   *[other] { $count } Dateien
} nach { $folder } importiert
import-done-existed = { $count ->
        [one] { $count } war schon vorhanden
       *[other] { $count } waren schon vorhanden
    }
import-done-deleted = { $count } von der Karte gelöscht
import-done-failed = { $count ->
    [one] 1 Datei konnte nicht importiert werden
   *[other] { $count } Dateien konnten nicht importiert werden
}
import-done-cancelled = Import nach { $count ->
        [one] { $count } Datei
       *[other] { $count } Dateien
    } abgebrochen. Ein neuer Import macht dort weiter.
import-done-nothing = Nichts Neues importiert.
import-ejected = { $name } kann entnommen werden.
import-eject-failed = { $name } konnte nicht ausgeworfen werden: { $error }
import-failed-destination = Der Ordner { $folder } kann nicht verwendet werden: { $error }
import-ledger-failed = Die Liste der importierten Dateien konnte nicht gelesen werden ({ $error }); bereits importierte Dateien werden eventuell erneut angeboten.

## Bildschirmleser
import-a11y-group = Import
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, ausgewählt
