# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Fernsteuerung und Ereignisse
settings-integration-ipc-help = Andere Programme auf diesem Computer, die unter Ihrem Konto laufen, können { -app-name } über einen privaten Socket (specular ipc) steuern und erfahren, was darin geschieht. Nichts lauscht im Netzwerk.

## The hooks panel under "Remote control and events".
hooks-panel-title = Ereignis-Hooks
hooks-panel-intro = Programme, die settings.toml bei einem Ereignis ausführt. Jedes fragt einmal vor seinem ersten Lauf.
hooks-panel-none = Keine Hooks. Fügen Sie sie als [[hooks]] in settings.toml hinzu.
hooks-panel-keywords = Hooks Ereignisse Automatisierung Skripte Programme
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Wenn { $events }
hooks-panel-untrusted = Fragt vor dem ersten Lauf
hooks-panel-allowed = Erlaubt
hooks-panel-remove = Entfernen…
hooks-panel-unused = Nicht verwendet

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = „{ $name }“ ({ $program }) ausführen, wenn { $events }?
hook-consent-detail = Das Programm wird mit den Namen der Fotos gestartet: { $args }
hook-consent-trust = Erlauben Sie nur Programme, denen Sie vertrauen. Ändert sich der Hook, wird erneut gefragt.
hook-consent-allow = Erlauben
hook-consent-not-now = Nicht jetzt
hook-or = { " " }oder{ " " }
hook-event-opened = ein Foto geöffnet wird
hook-event-marked = Fotos markiert werden
hook-event-keywords = sich Stichwörter ändern
hook-event-exported = Fotos exportiert werden
hook-event-imported = Fotos importiert werden
hook-event-captured = eine Kamera ein Foto sendet
hook-event-develop-saved = eine Develop-Bearbeitung gespeichert wird
hook-event-any = irgendetwas geschieht

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Den Hook „{ $name }“ entfernen?
hook-remove-detail = Er wird aus settings.toml entfernt. Das Programm und Ihre Dateien bleiben auf dem Datenträger.
hook-remove-ok = Entfernen
hook-removed = Hook „{ $name }“ entfernt.
hook-settings-unwritable = settings.toml konnte nicht geändert werden: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = Der Hook „{ $name }“ hat nicht funktioniert: { $reason }
hook-failed-exit = er endete mit Code { $code }
hook-failed-timeout = er wurde nach { $seconds } s gestoppt
hook-failed-start = er konnte nicht starten ({ $reason })
hook-dropped = Der Hook „{ $name }“ ist beschäftigt: { $count ->
    [one] ein Ereignis wurde übersprungen.
   *[other] { $count } Ereignisse wurden übersprungen.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] eine Datei, die ersetzt würde, existiert: nichts wurde exportiert
   *[other] { $count } Dateien, die ersetzt würden, existieren: nichts wurde exportiert
}
export-recipes-remote-skipped = { $file } existiert
