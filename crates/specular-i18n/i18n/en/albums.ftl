# Specular — Library albums, smart albums and the date timeline. English.

## Sidebar
albums-title = Albums
albums-new = New album…
albums-new-smart = New smart album…
albums-save-search = Save search as smart album…
albums-smart = Smart album
# The album the photos go to when added without choosing.
albums-target = Adds go here
albums-group-count = { $count ->
    [one] { $count } album
   *[other] { $count } albums
}
albums-need-library = Albums are in the Library. Add this folder to the Library first.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Rename { $album }
albums-a11y-edit = Edit { $album }
albums-a11y-delete = Delete album { $album }
albums-a11y-rename-group = Rename group { $group }
albums-a11y-delete-group = Delete group { $group }

## Sheets
albums-name = Name
albums-unnamed = No name yet
albums-new-title = New album
albums-new-smart-title = New smart album
albums-edit-smart-title = Edit smart album
albums-rename-title = Rename album
albums-rename-group-title = Rename group
albums-create = Create
albums-rename = Rename
albums-save = Save
albums-groups-note = Use / to put it in a group: Client X / selects.
albums-name-empty = Type a name.
albums-name-taken = { $album } already exists.
albums-name-clash = { $album } is an album, so it can't be a group too.

## Smart album rules
albums-rules = Rules
albums-rules-note = Rules apply top to bottom. Empty rules are left out.
albums-rules-none = Add at least one rule.
albums-rule-label = Rule { $number }
albums-rule-empty = Empty rule
albums-rule-add = Add a rule
albums-rule-combine = Combine with the rules above
albums-rule-first = Photos that match
albums-rule-and = And
albums-rule-or = Or
albums-rule-except = Except

## Adding and removing photos
albums-add-menu = Add to album…
albums-remove-menu = Remove from album
albums-add-title = { $count ->
    [one] Add { $count } photo to an album
   *[other] Add { $count } photos to an album
}
albums-add = Add
albums-add-new = New album
albums-add-list = Albums
albums-add-none = No albums yet. Type a name to make one.
albums-add-note = Or type a name for a new album.
albums-added = { $count ->
    [one] Added { $count } photo to { $album }.
   *[other] Added { $count } photos to { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } isn't in the Library, so it wasn't added.
   *[other] { $count } aren't in the Library, so they weren't added.
}
albums-created = Created { $album }.
albums-removed = { $count ->
    [one] Removed { $count } photo from { $album }.
   *[other] Removed { $count } photos from { $album }.
}
albums-remove = Remove
albums-remove-confirm = { $count ->
    [one] Remove { $count } photo from { $album }?
   *[other] Remove { $count } photos from { $album }?
}
albums-remove-confirm-detail = The files stay on disk and in the Library.
albums-delete = Delete
albums-delete-confirm = Delete the album { $album }?
albums-delete-group-confirm = { $count ->
    [one] Delete the group { $group } and its { $count } album?
   *[other] Delete the group { $group } and its { $count } albums?
}
albums-delete-confirm-detail = The photos stay on disk and in the Library.
albums-deleted = { $count ->
    [one] Album deleted.
   *[other] { $count } albums deleted.
}
albums-failed = Couldn't change the albums: { $reason }

## Date timeline
albums-timeline = Timeline
albums-timeline-description = Pick a period to show only its photos. Shift+click picks a run of them.
albums-timeline-picked = Showing { $period }
albums-timeline-cleared = Showing all dates
