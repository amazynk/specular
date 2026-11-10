# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Albums
albums-new = Nieuw album…
albums-new-smart = Nieuw slim album…
albums-save-search = Zoekopdracht bewaren als slim album…
albums-smart = Slim album
# The album the photos go to when added without choosing.
albums-target = Toevoegingen komen hier
albums-group-count = { $count ->
    [one] { $count } album
   *[other] { $count } albums
}
albums-need-library = Albums staan in de bibliotheek. Voeg deze map eerst toe aan de bibliotheek.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = { $album } hernoemen
albums-a11y-edit = { $album } bewerken
albums-a11y-delete = Album { $album } verwijderen
albums-a11y-rename-group = Groep { $group } hernoemen
albums-a11y-delete-group = Groep { $group } verwijderen

## Sheets
albums-name = Naam
albums-unnamed = Nog geen naam
albums-new-title = Nieuw album
albums-new-smart-title = Nieuw slim album
albums-edit-smart-title = Slim album bewerken
albums-rename-title = Album hernoemen
albums-rename-group-title = Groep hernoemen
albums-create = Maken
albums-rename = Hernoemen
albums-save = Bewaren
albums-groups-note = Gebruik / om het in een groep te zetten: Klant X / selectie.
albums-name-empty = Typ een naam.
albums-name-taken = { $album } bestaat al.
albums-name-clash = { $album } is een album en kan dus niet ook een groep zijn.

## Smart album rules
albums-rules = Regels
albums-rules-note = Regels gelden van boven naar beneden. Lege regels worden overgeslagen.
albums-rules-none = Voeg minstens één regel toe.
albums-rule-label = Regel { $number }
albums-rule-empty = Lege regel
albums-rule-add = Een regel toevoegen
albums-rule-combine = Combineren met de regels erboven
albums-rule-first = Foto’s die voldoen aan
albums-rule-and = En
albums-rule-or = Of
albums-rule-except = Behalve

## Adding and removing photos
albums-add-menu = Aan album toevoegen…
albums-remove-menu = Uit album verwijderen
albums-add-title = { $count ->
    [one] { $count } foto aan een album toevoegen
   *[other] { $count } foto’s aan een album toevoegen
}
albums-add = Toevoegen
albums-add-new = Nieuw album
albums-add-list = Albums
albums-add-none = Nog geen albums. Typ een naam om er een te maken.
albums-add-note = Of typ een naam voor een nieuw album.
albums-added = { $count ->
    [one] { $count } foto toegevoegd aan { $album }.
   *[other] { $count } foto’s toegevoegd aan { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } staat niet in de bibliotheek en is dus niet toegevoegd.
   *[other] { $count } staan niet in de bibliotheek en zijn dus niet toegevoegd.
}
albums-created = { $album } gemaakt.
albums-removed = { $count ->
    [one] { $count } foto uit { $album } verwijderd.
   *[other] { $count } foto’s uit { $album } verwijderd.
}
albums-remove = Verwijderen
albums-remove-confirm = { $count ->
    [one] { $count } foto uit { $album } verwijderen?
   *[other] { $count } foto’s uit { $album } verwijderen?
}
albums-remove-confirm-detail = De bestanden blijven op de schijf en in de bibliotheek.
albums-delete = Verwijderen
albums-delete-confirm = Het album { $album } verwijderen?
albums-delete-group-confirm = { $count ->
    [one] De groep { $group } en het { $count } album erin verwijderen?
   *[other] De groep { $group } en de { $count } albums erin verwijderen?
}
albums-delete-confirm-detail = De foto’s blijven op de schijf en in de bibliotheek.
albums-deleted = { $count ->
    [one] Album verwijderd.
   *[other] { $count } albums verwijderd.
}
albums-failed = Kon de albums niet wijzigen: { $reason }

## Date timeline
albums-timeline = Tijdlijn
albums-timeline-description = Kies een periode om alleen de foto’s daaruit te tonen. Shift+klik kiest een reeks.
albums-timeline-picked = { $period } wordt getoond
albums-timeline-cleared = Alle datums worden getoond
