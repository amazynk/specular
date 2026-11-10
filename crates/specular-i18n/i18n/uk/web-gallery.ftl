# Specular — Галереї для клієнтів: статична HTML-тека з фотографіями та зчитування вибору клієнта.

# machine-drafted, needs native review

palette-client-gallery = Галерея для клієнта…
palette-client-gallery-help = Записати фотографії як теку вебсторінки для клієнта, з вибором, який він може надіслати назад.
palette-import-client-picks = Імпортувати вибір клієнта…
palette-import-client-picks-help = Позначити фотографії, які клієнт вибрав у своїй галереї.

webgallery-heading = Галерея для клієнта
webgallery-subtitle = { $count ->
    [one] { $count } фотографія
    [few] { $count } фотографії
    [many] { $count } фотографій
   *[other] { $count } фотографії
}
webgallery-title-label = Назва
webgallery-title-default = Пробні знімки
webgallery-watermark-label = Текст водяного знака (порожньо: немає)
webgallery-source = Фотографії
webgallery-source-current = Ця фотографія
webgallery-source-selection = Вибране
webgallery-source-album = Альбом «{ $name }»
webgallery-source-search = Результати пошуку
webgallery-source-folder = Ця тека
webgallery-source-picks = Відібрані
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Тема
webgallery-theme-light = Світла
webgallery-theme-dark = Темна
webgallery-theme-minimal = Мінімальна
webgallery-size = Довга сторона
webgallery-size-value = { $px } пкс
webgallery-format = Формат
webgallery-captions = Під кожною фотографією
webgallery-captions-off = Нічого
webgallery-captions-name = Назва файлу
webgallery-captions-title = Назва
webgallery-captions-all = Назва файлу, назва й підпис
webgallery-exif = Показувати камеру й експозицію
webgallery-location = Зберігати місце (GPS) у фотографіях
webgallery-note-private = Статичну галерею неможливо захистити паролем.
webgallery-note-share = Будь-хто, хто має теку або її адресу, бачить усі фотографії. Діліться посиланням лише з клієнтом або використовуйте керування доступом свого хостингу.
webgallery-note-picks = Клієнти вибирають сердечком. Вибір лишається в їхньому браузері, доки вони не надішлють вам список.
webgallery-write = Створити галерею…
webgallery-busy = Записування { $done } з { $total }…
webgallery-stop = Зупинити
webgallery-done = { $count ->
    [one] Галерею створено: { $count } фотографія в { $folder }
    [few] Галерею створено: { $count } фотографії в { $folder }
    [many] Галерею створено: { $count } фотографій у { $folder }
   *[other] Галерею створено: { $count } фотографії в { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Не вдалося записати { $failed } фотографію.
    [few] Не вдалося записати { $failed } фотографії.
    [many] Не вдалося записати { $failed } фотографій.
   *[other] Не вдалося записати { $failed } фотографії.
}
webgallery-stopped = Галерею зупинено. Нічого не збережено.
webgallery-failed = Галерею не створено: { $reason }
webgallery-nothing = Не вдалося записати жодної фотографії.

webgallery-page-count = { $count ->
    [one] { $count } фотографія
    [few] { $count } фотографії
    [many] { $count } фотографій
   *[other] { $count } фотографії
}
webgallery-page-hint = Торкніться сердечка на фотографіях, які хочете. Ваш вибір лишається в цьому браузері, доки ви його не надішлете.
webgallery-page-picked = Вибрано: {"{"}n{"}"}
webgallery-page-pick = Вибрати
webgallery-page-copy = Копіювати мій вибір
webgallery-page-copied = Скопійовано. Вставте список у повідомлення.
webgallery-page-copy-by-hand = Скопіюйте виділений список і вставте його в повідомлення.
webgallery-page-download-txt = Завантажити picks.txt
webgallery-page-download-json = Завантажити picks.json
webgallery-page-clear = Очистити вибір
webgallery-page-clear-ask = Очистити весь ваш вибір у цьому браузері?
webgallery-page-no-storage = Цей браузер не збереже ваш вибір. Надішліть його, перш ніж закрити сторінку.
webgallery-page-noscript = Для вибору потрібен JavaScript. Фотографії все одно відкриваються.
webgallery-page-previous = Попередня
webgallery-page-next = Наступна
webgallery-page-close = Закрити
webgallery-page-position = {"{"}i{"}"} з {"{"}n{"}"}
webgallery-page-viewer = Переглядач фотографій
webgallery-page-picks = Ваш вибір

clientpicks-heading = Імпортувати вибір клієнта
clientpicks-subtitle = { $count ->
    [one] Зіставлено з { $count } показаною фотографією
    [few] Зіставлено з { $count } показаними фотографіями
    [many] Зіставлено з { $count } показаними фотографіями
   *[other] Зіставлено з { $count } показаної фотографії
}
clientpicks-paste-label = Назви (вставте список клієнта)
clientpicks-choose = Вибрати файл вибору…
clientpicks-from-file = З { $name }
clientpicks-apply-as = Позначити їх
clientpicks-as-pick = Як відібрані
clientpicks-as-stars = { $stars ->
    [one] { $stars } зірка
    [few] { $stars } зірки
    [many] { $stars } зірок
   *[other] { $stars } зірки
}
clientpicks-as-label = Мітка { $label }
clientpicks-empty = Вставте список, який надіслав клієнт, або виберіть його picks.txt чи picks.json.
clientpicks-matched = { $matched } з { $total } назв відповідають тутешнім фотографіям.
clientpicks-missing = Не знайдено: { $names }
clientpicks-ambiguous = У кількох теках, залишено без змін: { $names }
clientpicks-more = { $names } і ще { $more }
clientpicks-apply = Застосувати
clientpicks-applied = { $count ->
    [one] Позначено { $count } фотографію.
    [few] Позначено { $count } фотографії.
    [many] Позначено { $count } фотографій.
   *[other] Позначено { $count } фотографії.
}
clientpicks-unchanged = Ці фотографії вже були позначені.
clientpicks-read-failed = Не вдалося прочитати { $name }: { $reason }
