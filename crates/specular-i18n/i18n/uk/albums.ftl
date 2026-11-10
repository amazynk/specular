# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Альбоми
albums-new = Новий альбом…
albums-new-smart = Новий розумний альбом…
albums-save-search = Зберегти пошук як розумний альбом…
albums-smart = Розумний альбом
# The album the photos go to when added without choosing.
albums-target = Сюди потрапляє додане
albums-group-count = { $count ->
    [one] { $count } альбом
    [few] { $count } альбоми
    [many] { $count } альбомів
   *[other] { $count } альбому
}
albums-need-library = Альбоми є в бібліотеці. Спершу додайте цю теку до бібліотеки.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Перейменувати { $album }
albums-a11y-edit = Редагувати { $album }
albums-a11y-delete = Видалити альбом { $album }
albums-a11y-rename-group = Перейменувати групу { $group }
albums-a11y-delete-group = Видалити групу { $group }

## Sheets
albums-name = Назва
albums-unnamed = Ще без назви
albums-new-title = Новий альбом
albums-new-smart-title = Новий розумний альбом
albums-edit-smart-title = Редагувати розумний альбом
albums-rename-title = Перейменувати альбом
albums-rename-group-title = Перейменувати групу
albums-create = Створити
albums-rename = Перейменувати
albums-save = Зберегти
albums-groups-note = Використайте /, щоб додати його до групи: Клієнт X / вибране.
albums-name-empty = Введіть назву.
albums-name-taken = { $album } уже існує.
albums-name-clash = { $album } — це альбом, тому він не може бути ще й групою.

## Smart album rules
albums-rules = Правила
albums-rules-note = Правила діють згори донизу. Порожні правила пропускаються.
albums-rules-none = Додайте принаймні одне правило.
albums-rule-label = Правило { $number }
albums-rule-empty = Порожнє правило
albums-rule-add = Додати правило
albums-rule-combine = Поєднати з правилами вище
albums-rule-first = Фото, що відповідають
albums-rule-and = І
albums-rule-or = Або
albums-rule-except = Крім

## Adding and removing photos
albums-add-menu = Додати до альбому…
albums-remove-menu = Вилучити з альбому
albums-add-title = { $count ->
    [one] Додати { $count } фото до альбому
    [few] Додати { $count } фото до альбому
    [many] Додати { $count } фото до альбому
   *[other] Додати { $count } фото до альбому
}
albums-add = Додати
albums-add-new = Новий альбом
albums-add-list = Альбоми
albums-add-none = Альбомів ще немає. Введіть назву, щоб створити альбом.
albums-add-note = Або введіть назву нового альбому.
albums-added = { $count ->
    [one] Додано { $count } фото до { $album }.
    [few] Додано { $count } фото до { $album }.
    [many] Додано { $count } фото до { $album }.
   *[other] Додано { $count } фото до { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } немає в бібліотеці, тому його не додано.
    [few] { $count } немає в бібліотеці, тому їх не додано.
    [many] { $count } немає в бібліотеці, тому їх не додано.
   *[other] { $count } немає в бібліотеці, тому їх не додано.
}
albums-created = Створено { $album }.
albums-removed = { $count ->
    [one] Вилучено { $count } фото з { $album }.
    [few] Вилучено { $count } фото з { $album }.
    [many] Вилучено { $count } фото з { $album }.
   *[other] Вилучено { $count } фото з { $album }.
}
albums-remove = Вилучити
albums-remove-confirm = { $count ->
    [one] Вилучити { $count } фото з { $album }?
    [few] Вилучити { $count } фото з { $album }?
    [many] Вилучити { $count } фото з { $album }?
   *[other] Вилучити { $count } фото з { $album }?
}
albums-remove-confirm-detail = Файли залишаються на диску й у бібліотеці.
albums-delete = Видалити
albums-delete-confirm = Видалити альбом { $album }?
albums-delete-group-confirm = { $count ->
    [one] Видалити групу { $group } і її { $count } альбом?
    [few] Видалити групу { $group } і її { $count } альбоми?
    [many] Видалити групу { $group } і її { $count } альбомів?
   *[other] Видалити групу { $group } і її { $count } альбому?
}
albums-delete-confirm-detail = Фото залишаються на диску й у бібліотеці.
albums-deleted = { $count ->
    [one] Альбом видалено.
    [few] Видалено { $count } альбоми.
    [many] Видалено { $count } альбомів.
   *[other] Видалено { $count } альбому.
}
albums-failed = Не вдалося змінити альбоми: { $reason }

## Date timeline
albums-timeline = Часова шкала
albums-timeline-description = Виберіть період, щоб показати лише його фото. Shift+клацання вибирає кілька поспіль.
albums-timeline-picked = Показано { $period }
albums-timeline-cleared = Показано всі дати
