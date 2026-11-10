# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Albumy
albums-new = Nowy album…
albums-new-smart = Nowy album inteligentny…
albums-save-search = Zapisz wyszukiwanie jako album inteligentny…
albums-smart = Album inteligentny
# The album the photos go to when added without choosing.
albums-target = Tu trafia to, co dodasz
albums-group-count = { $count ->
    [one] { $count } album
    [few] { $count } albumy
    [many] { $count } albumów
   *[other] { $count } albumu
}
albums-need-library = Albumy są w Bibliotece. Najpierw dodaj ten folder do Biblioteki.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Zmień nazwę { $album }
albums-a11y-edit = Edytuj { $album }
albums-a11y-delete = Usuń album { $album }
albums-a11y-rename-group = Zmień nazwę grupy { $group }
albums-a11y-delete-group = Usuń grupę { $group }

## Sheets
albums-name = Nazwa
albums-unnamed = Jeszcze bez nazwy
albums-new-title = Nowy album
albums-new-smart-title = Nowy album inteligentny
albums-edit-smart-title = Edytuj album inteligentny
albums-rename-title = Zmień nazwę albumu
albums-rename-group-title = Zmień nazwę grupy
albums-create = Utwórz
albums-rename = Zmień nazwę
albums-save = Zapisz
albums-groups-note = Użyj /, aby umieścić go w grupie: Klient X / wybrane.
albums-name-empty = Wpisz nazwę.
albums-name-taken = { $album } już istnieje.
albums-name-clash = { $album } jest albumem, więc nie może być też grupą.

## Smart album rules
albums-rules = Reguły
albums-rules-note = Reguły działają od góry do dołu. Puste reguły są pomijane.
albums-rules-none = Dodaj co najmniej jedną regułę.
albums-rule-label = Reguła { $number }
albums-rule-empty = Pusta reguła
albums-rule-add = Dodaj regułę
albums-rule-combine = Połącz z regułami powyżej
albums-rule-first = Zdjęcia, które pasują do
albums-rule-and = I
albums-rule-or = Lub
albums-rule-except = Oprócz

## Adding and removing photos
albums-add-menu = Dodaj do albumu…
albums-remove-menu = Usuń z albumu
albums-add-title = { $count ->
    [one] Dodaj { $count } zdjęcie do albumu
    [few] Dodaj { $count } zdjęcia do albumu
    [many] Dodaj { $count } zdjęć do albumu
   *[other] Dodaj { $count } zdjęcia do albumu
}
albums-add = Dodaj
albums-add-new = Nowy album
albums-add-list = Albumy
albums-add-none = Nie ma jeszcze albumów. Wpisz nazwę, aby utworzyć album.
albums-add-note = Albo wpisz nazwę nowego albumu.
albums-added = { $count ->
    [one] Dodano { $count } zdjęcie do { $album }.
    [few] Dodano { $count } zdjęcia do { $album }.
    [many] Dodano { $count } zdjęć do { $album }.
   *[other] Dodano { $count } zdjęcia do { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } nie jest w Bibliotece, więc nie zostało dodane.
    [few] { $count } nie są w Bibliotece, więc nie zostały dodane.
    [many] { $count } nie jest w Bibliotece, więc nie zostało dodanych.
   *[other] { $count } nie jest w Bibliotece, więc nie zostało dodane.
}
albums-created = Utworzono { $album }.
albums-removed = { $count ->
    [one] Usunięto { $count } zdjęcie z { $album }.
    [few] Usunięto { $count } zdjęcia z { $album }.
    [many] Usunięto { $count } zdjęć z { $album }.
   *[other] Usunięto { $count } zdjęcia z { $album }.
}
albums-remove = Usuń
albums-remove-confirm = { $count ->
    [one] Usunąć { $count } zdjęcie z { $album }?
    [few] Usunąć { $count } zdjęcia z { $album }?
    [many] Usunąć { $count } zdjęć z { $album }?
   *[other] Usunąć { $count } zdjęcia z { $album }?
}
albums-remove-confirm-detail = Pliki zostają na dysku i w Bibliotece.
albums-delete = Usuń
albums-delete-confirm = Usunąć album { $album }?
albums-delete-group-confirm = { $count ->
    [one] Usunąć grupę { $group } i jej { $count } album?
    [few] Usunąć grupę { $group } i jej { $count } albumy?
    [many] Usunąć grupę { $group } i jej { $count } albumów?
   *[other] Usunąć grupę { $group } i jej { $count } albumu?
}
albums-delete-confirm-detail = Zdjęcia zostają na dysku i w Bibliotece.
albums-deleted = { $count ->
    [one] Usunięto album.
    [few] Usunięto { $count } albumy.
    [many] Usunięto { $count } albumów.
   *[other] Usunięto { $count } albumu.
}
albums-failed = Nie udało się zmienić albumów: { $reason }

## Date timeline
albums-timeline = Oś czasu
albums-timeline-description = Wybierz okres, aby pokazać tylko jego zdjęcia. Shift+klik wybiera kilka po kolei.
albums-timeline-picked = Pokazano { $period }
albums-timeline-cleared = Pokazano wszystkie daty
