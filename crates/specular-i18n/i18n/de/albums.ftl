# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Alben
albums-new = Neues Album…
albums-new-smart = Neues intelligentes Album…
albums-save-search = Suche als intelligentes Album sichern…
albums-smart = Intelligentes Album
# The album the photos go to when added without choosing.
albums-target = Hierhin wird hinzugefügt
albums-group-count = { $count ->
    [one] { $count } Album
   *[other] { $count } Alben
}
albums-need-library = Alben gibt es in der Bibliothek. Fügen Sie diesen Ordner zuerst der Bibliothek hinzu.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = { $album } umbenennen
albums-a11y-edit = { $album } bearbeiten
albums-a11y-delete = Album { $album } löschen
albums-a11y-rename-group = Gruppe { $group } umbenennen
albums-a11y-delete-group = Gruppe { $group } löschen

## Sheets
albums-name = Name
albums-unnamed = Noch kein Name
albums-new-title = Neues Album
albums-new-smart-title = Neues intelligentes Album
albums-edit-smart-title = Intelligentes Album bearbeiten
albums-rename-title = Album umbenennen
albums-rename-group-title = Gruppe umbenennen
albums-create = Erstellen
albums-rename = Umbenennen
albums-save = Sichern
albums-groups-note = Mit / kommt es in eine Gruppe: Kunde X / Auswahl.
albums-name-empty = Geben Sie einen Namen ein.
albums-name-taken = { $album } gibt es schon.
albums-name-clash = { $album } ist ein Album und kann daher nicht auch eine Gruppe sein.

## Smart album rules
albums-rules = Regeln
albums-rules-note = Regeln gelten von oben nach unten. Leere Regeln werden übergangen.
albums-rules-none = Fügen Sie mindestens eine Regel hinzu.
albums-rule-label = Regel { $number }
albums-rule-empty = Leere Regel
albums-rule-add = Regel hinzufügen
albums-rule-combine = Mit den Regeln darüber verknüpfen
albums-rule-first = Fotos, für die gilt
albums-rule-and = Und
albums-rule-or = Oder
albums-rule-except = Außer

## Adding and removing photos
albums-add-menu = Zu Album hinzufügen…
albums-remove-menu = Aus Album entfernen
albums-add-title = { $count ->
    [one] { $count } Foto zu einem Album hinzufügen
   *[other] { $count } Fotos zu einem Album hinzufügen
}
albums-add = Hinzufügen
albums-add-new = Neues Album
albums-add-list = Alben
albums-add-none = Noch keine Alben. Geben Sie einen Namen ein, um eines anzulegen.
albums-add-note = Oder geben Sie einen Namen für ein neues Album ein.
albums-added = { $count ->
    [one] { $count } Foto zu { $album } hinzugefügt.
   *[other] { $count } Fotos zu { $album } hinzugefügt.
}
albums-added-missing = { $count ->
    [one] { $count } ist nicht in der Bibliothek und wurde daher nicht hinzugefügt.
   *[other] { $count } sind nicht in der Bibliothek und wurden daher nicht hinzugefügt.
}
albums-created = { $album } erstellt.
albums-removed = { $count ->
    [one] { $count } Foto aus { $album } entfernt.
   *[other] { $count } Fotos aus { $album } entfernt.
}
albums-remove = Entfernen
albums-remove-confirm = { $count ->
    [one] { $count } Foto aus { $album } entfernen?
   *[other] { $count } Fotos aus { $album } entfernen?
}
albums-remove-confirm-detail = Die Dateien bleiben auf dem Datenträger und in der Bibliothek.
albums-delete = Löschen
albums-delete-confirm = Das Album { $album } löschen?
albums-delete-group-confirm = { $count ->
    [one] Die Gruppe { $group } und ihr { $count } Album löschen?
   *[other] Die Gruppe { $group } und ihre { $count } Alben löschen?
}
albums-delete-confirm-detail = Die Fotos bleiben auf dem Datenträger und in der Bibliothek.
albums-deleted = { $count ->
    [one] Album gelöscht.
   *[other] { $count } Alben gelöscht.
}
albums-failed = Die Alben konnten nicht geändert werden: { $reason }

## Date timeline
albums-timeline = Zeitleiste
albums-timeline-description = Wählen Sie einen Zeitraum, um nur seine Fotos zu zeigen. Shift+Klick wählt mehrere am Stück.
albums-timeline-picked = { $period } wird gezeigt
albums-timeline-cleared = Alle Zeiträume werden gezeigt
