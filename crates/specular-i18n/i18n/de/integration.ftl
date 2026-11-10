# Integration: open with, user commands, paste (Stage 2).
palette-open-with = Öffnen mit …
palette-paste = Einfügen
open-with-placeholder = Öffnen mit …
open-with-loading = Programme werden gesucht …
open-with-default = { $name } (Standard)
open-with-choose-other = Anderes Programm wählen …
open-with-choose-title = Programm wählen
open-with-failed = Öffnen mit { $name } fehlgeschlagen: { $reason }
command-missing = { $program } nicht gefunden
command-failed = { $name } konnte nicht gestartet werden: { $reason }
command-exited = { $name } wurde mit einem Fehler beendet: { $status }
paste-nothing = Die Zwischenablage enthält nichts zum Öffnen
paste-failed = Bild konnte nicht eingefügt werden: { $reason }
drag-failed = Datei konnte nicht gezogen werden: { $reason }
about-storage = Einstellungen: { $settings } · Cache: { $cache }
about-storage-portable = Portabler Modus: Einstellungen und Cache in { $dir }

## Scripting and recipes

palette-copy-view-command = Ansicht als Shell-Befehl kopieren
palette-copy-view-python = Ansicht als Python kopieren
toast-copied-view-command = Befehl kopiert
script-copy-no-image = Kein Bild zum Beschreiben
recipe-confirm-title = Das Rezept „{ $name }“ zum ersten Mal ausführen? Es führt aus: { $command }
recipe-confirm-run = Ausführen
recipe-no-file = Das Rezept braucht eine Bilddatei
recipe-running = { $name } läuft…
recipe-done = { $name }: { $shapes ->
    [one] 1 Form
   *[other] { $shapes } Formen
} hinzugefügt
recipe-failed = { $name } fehlgeschlagen: { $reason }
recipe-timeout = nach { $seconds } s angehalten
recipe-no-output = { $name } hat keine { $file } geschrieben
