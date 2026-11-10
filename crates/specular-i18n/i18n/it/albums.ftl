# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Album
albums-new = Nuovo album…
albums-new-smart = Nuovo album smart…
albums-save-search = Salva la ricerca come album smart…
albums-smart = Album smart
# The album the photos go to when added without choosing.
albums-target = Le aggiunte vanno qui
albums-group-count = { $count ->
    [one] { $count } album
   *[other] { $count } album
}
albums-need-library = Gli album sono nella Libreria. Aggiungi prima questa cartella alla Libreria.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Rinomina { $album }
albums-a11y-edit = Modifica { $album }
albums-a11y-delete = Elimina l’album { $album }
albums-a11y-rename-group = Rinomina il gruppo { $group }
albums-a11y-delete-group = Elimina il gruppo { $group }

## Sheets
albums-name = Nome
albums-unnamed = Ancora senza nome
albums-new-title = Nuovo album
albums-new-smart-title = Nuovo album smart
albums-edit-smart-title = Modifica album smart
albums-rename-title = Rinomina album
albums-rename-group-title = Rinomina gruppo
albums-create = Crea
albums-rename = Rinomina
albums-save = Salva
albums-groups-note = Usa / per metterlo in un gruppo: Cliente X / selezione.
albums-name-empty = Scrivi un nome.
albums-name-taken = { $album } esiste già.
albums-name-clash = { $album } è un album, quindi non può essere anche un gruppo.

## Smart album rules
albums-rules = Regole
albums-rules-note = Le regole valgono dall’alto verso il basso. Le regole vuote vengono ignorate.
albums-rules-none = Aggiungi almeno una regola.
albums-rule-label = Regola { $number }
albums-rule-empty = Regola vuota
albums-rule-add = Aggiungi una regola
albums-rule-combine = Combina con le regole sopra
albums-rule-first = Foto che corrispondono a
albums-rule-and = E
albums-rule-or = O
albums-rule-except = Tranne

## Adding and removing photos
albums-add-menu = Aggiungi all’album…
albums-remove-menu = Rimuovi dall’album
albums-add-title = { $count ->
    [one] Aggiungi { $count } foto a un album
   *[other] Aggiungi { $count } foto a un album
}
albums-add = Aggiungi
albums-add-new = Nuovo album
albums-add-list = Album
albums-add-none = Ancora nessun album. Scrivi un nome per crearne uno.
albums-add-note = Oppure scrivi il nome di un nuovo album.
albums-added = { $count ->
    [one] { $count } foto aggiunta a { $album }.
   *[other] { $count } foto aggiunte a { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } non è nella Libreria, quindi non è stata aggiunta.
   *[other] { $count } non sono nella Libreria, quindi non sono state aggiunte.
}
albums-created = { $album } creato.
albums-removed = { $count ->
    [one] { $count } foto rimossa da { $album }.
   *[other] { $count } foto rimosse da { $album }.
}
albums-remove = Rimuovi
albums-remove-confirm = { $count ->
    [one] Rimuovere { $count } foto da { $album }?
   *[other] Rimuovere { $count } foto da { $album }?
}
albums-remove-confirm-detail = I file restano sul disco e nella Libreria.
albums-delete = Elimina
albums-delete-confirm = Eliminare l’album { $album }?
albums-delete-group-confirm = { $count ->
    [one] Eliminare il gruppo { $group } e il suo { $count } album?
   *[other] Eliminare il gruppo { $group } e i suoi { $count } album?
}
albums-delete-confirm-detail = Le foto restano sul disco e nella Libreria.
albums-deleted = { $count ->
    [one] Album eliminato.
   *[other] { $count } album eliminati.
}
albums-failed = Impossibile modificare gli album: { $reason }

## Date timeline
albums-timeline = Cronologia
albums-timeline-description = Scegli un periodo per mostrare solo le sue foto. Shift+clic ne sceglie una serie.
albums-timeline-picked = Mostra { $period }
albums-timeline-cleared = Mostra tutte le date
