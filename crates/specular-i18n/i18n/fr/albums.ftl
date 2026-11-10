# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Albums
albums-new = Nouvel album…
albums-new-smart = Nouvel album intelligent…
albums-save-search = Enregistrer la recherche comme album intelligent…
albums-smart = Album intelligent
# The album the photos go to when added without choosing.
albums-target = Les ajouts vont ici
albums-group-count = { $count ->
    [one] { $count } album
   *[other] { $count } albums
}
albums-need-library = Les albums se trouvent dans la Bibliothèque. Ajoutez d’abord ce dossier à la Bibliothèque.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Renommer { $album }
albums-a11y-edit = Modifier { $album }
albums-a11y-delete = Supprimer l’album { $album }
albums-a11y-rename-group = Renommer le groupe { $group }
albums-a11y-delete-group = Supprimer le groupe { $group }

## Sheets
albums-name = Nom
albums-unnamed = Pas encore de nom
albums-new-title = Nouvel album
albums-new-smart-title = Nouvel album intelligent
albums-edit-smart-title = Modifier l’album intelligent
albums-rename-title = Renommer l’album
albums-rename-group-title = Renommer le groupe
albums-create = Créer
albums-rename = Renommer
albums-save = Enregistrer
albums-groups-note = Utilisez / pour le placer dans un groupe : Client X / sélection.
albums-name-empty = Saisissez un nom.
albums-name-taken = { $album } existe déjà.
albums-name-clash = { $album } est un album, il ne peut donc pas être aussi un groupe.

## Smart album rules
albums-rules = Règles
albums-rules-note = Les règles s’appliquent de haut en bas. Les règles vides sont ignorées.
albums-rules-none = Ajoutez au moins une règle.
albums-rule-label = Règle { $number }
albums-rule-empty = Règle vide
albums-rule-add = Ajouter une règle
albums-rule-combine = Combiner avec les règles au-dessus
albums-rule-first = Photos qui correspondent à
albums-rule-and = Et
albums-rule-or = Ou
albums-rule-except = Sauf

## Adding and removing photos
albums-add-menu = Ajouter à un album…
albums-remove-menu = Retirer de l’album
albums-add-title = { $count ->
    [one] Ajouter { $count } photo à un album
   *[other] Ajouter { $count } photos à un album
}
albums-add = Ajouter
albums-add-new = Nouvel album
albums-add-list = Albums
albums-add-none = Aucun album pour l’instant. Saisissez un nom pour en créer un.
albums-add-note = Ou saisissez le nom d’un nouvel album.
albums-added = { $count ->
    [one] { $count } photo ajoutée à { $album }.
   *[other] { $count } photos ajoutées à { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } n’est pas dans la Bibliothèque, elle n’a donc pas été ajoutée.
   *[other] { $count } ne sont pas dans la Bibliothèque, elles n’ont donc pas été ajoutées.
}
albums-created = { $album } créé.
albums-removed = { $count ->
    [one] { $count } photo retirée de { $album }.
   *[other] { $count } photos retirées de { $album }.
}
albums-remove = Retirer
albums-remove-confirm = { $count ->
    [one] Retirer { $count } photo de { $album } ?
   *[other] Retirer { $count } photos de { $album } ?
}
albums-remove-confirm-detail = Les fichiers restent sur le disque et dans la Bibliothèque.
albums-delete = Supprimer
albums-delete-confirm = Supprimer l’album { $album } ?
albums-delete-group-confirm = { $count ->
    [one] Supprimer le groupe { $group } et son { $count } album ?
   *[other] Supprimer le groupe { $group } et ses { $count } albums ?
}
albums-delete-confirm-detail = Les photos restent sur le disque et dans la Bibliothèque.
albums-deleted = { $count ->
    [one] Album supprimé.
   *[other] { $count } albums supprimés.
}
albums-failed = Impossible de modifier les albums : { $reason }

## Date timeline
albums-timeline = Chronologie
albums-timeline-description = Choisissez une période pour n’afficher que ses photos. Shift+clic en choisit une suite.
albums-timeline-picked = Affichage de { $period }
albums-timeline-cleared = Affichage de toutes les dates
