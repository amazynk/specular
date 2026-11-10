# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Черга, повтор і перемішування
# machine-drafted, needs native review
palette-next-track = Наступний елемент
# machine-drafted, needs native review
palette-next-track-help = Відтворити наступний елемент черги, а без неї наступний файл того ж типу в теці.
# machine-drafted, needs native review
palette-prev-track = Попередній елемент
# machine-drafted, needs native review
palette-prev-track-help = Відтворити попередній елемент. Після 3 секунд поточний почнеться спочатку.
# machine-drafted, needs native review
palette-cycle-repeat = Повтор
# machine-drafted, needs native review
palette-cycle-repeat-help = Повтор вимкнено, усе або один.
# machine-drafted, needs native review
palette-shuffle = Перемішати
# machine-drafted, needs native review
palette-shuffle-help = Відтворювати у випадковому порядку, кожен елемент раз за коло.
# machine-drafted, needs native review
palette-play-queue = Черга відтворення
# machine-drafted, needs native review
palette-play-queue-help = Показати, що гратиме далі: змінити порядок, вилучити, зберегти.
# machine-drafted, needs native review
palette-queue-play-next = Відтворити наступним
# machine-drafted, needs native review
palette-queue-play-next-help = Відтворити файл (у галереї — вибране) після поточного елемента.
# machine-drafted, needs native review
palette-queue-add = Додати до черги
# machine-drafted, needs native review
palette-queue-add-help = Додати файл (у галереї — вибране) в кінець черги.
# machine-drafted, needs native review
palette-save-queue = Зберегти чергу як список відтворення…
# machine-drafted, needs native review
palette-save-queue-help = Зберегти чергу як список відтворення M3U8 або XSPF.
# machine-drafted, needs native review
transport-prev-track = Попередній елемент
# machine-drafted, needs native review
transport-next-track = Наступний елемент
# machine-drafted, needs native review
queue-title = Черга відтворення
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] { $count } елемент
    [few] { $count } елементи
    [many] { $count } елементів
   *[other] { $count } елемента
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] { $count } елемент
    [few] { $count } елементи
    [many] { $count } елементів
   *[other] { $count } елемента
} · { $time }
# machine-drafted, needs native review
queue-empty = Черга порожня.
    Додайте звукові файли й відео командами «Відтворити наступним» або «Додати до черги» чи відкрийте список відтворення.
# machine-drafted, needs native review
queue-empty-short = Черга порожня.
# machine-drafted, needs native review
queue-playing = Грає
# machine-drafted, needs native review
queue-repeat = Повтор
# machine-drafted, needs native review
queue-repeat-off = Повтор: вимк.
# machine-drafted, needs native review
queue-repeat-all = Повтор: усе
# machine-drafted, needs native review
queue-repeat-one = Повтор: один
# machine-drafted, needs native review
queue-shuffle = Перемішати
# machine-drafted, needs native review
queue-shuffle-on = Перемішування: увімк.
# machine-drafted, needs native review
queue-shuffle-off = Перемішування: вимк.
# machine-drafted, needs native review
queue-save = Зберегти як список відтворення…
# machine-drafted, needs native review
queue-save-tip = Зберегти чергу як список відтворення M3U8 або XSPF
# machine-drafted, needs native review
queue-clear = Очистити…
# machine-drafted, needs native review
queue-clear-tip = Очистити чергу. Файли лишаються на диску.
# machine-drafted, needs native review
queue-remove = Вилучити з черги (файл лишається на диску)
# machine-drafted, needs native review
queue-network-item = Мережевий елемент, не відтворюється: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Наступного елемента немає.
# machine-drafted, needs native review
queue-at-start = Попереднього елемента немає.
# machine-drafted, needs native review
queue-missing = { $name } більше немає. Пропущено.
# machine-drafted, needs native review
queue-network-not-played = Мережеві елементи поки не відтворюються.
# machine-drafted, needs native review
queue-nothing-to-add = Нічого додати. До черги потрапляють лише звукові файли, відео та списки відтворення.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] До черги додано { $count } елемент.
    [few] До черги додано { $count } елементи.
    [many] До черги додано { $count } елементів.
   *[other] До черги додано { $count } елемента.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] { $count } елемент гратиме наступним.
    [few] { $count } елементи гратимуть наступними.
    [many] { $count } елементів гратимуть наступними.
   *[other] { $count } елемента гратимуть наступними.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] { $count } елемент у черзі.
    [few] { $count } елементи в черзі.
    [many] { $count } елементів у черзі.
   *[other] { $count } елемента в черзі.
}
# machine-drafted, needs native review
queue-playlist-nothing = Нічого з { $name } тут не відтворюється. Черга показує, що в ньому є.
# machine-drafted, needs native review
queue-playlist-failed = Не вдається відкрити { $name }: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = файл не читається.
# machine-drafted, needs native review
queue-playlist-damaged = це не список відтворення, який Specular уміє читати.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Очистити чергу ({ $count } елемент)?
    [few] Очистити чергу ({ $count } елементи)?
    [many] Очистити чергу ({ $count } елементів)?
   *[other] Очистити чергу ({ $count } елемента)?
}
# machine-drafted, needs native review
queue-clear-detail = Очищується лише список. Файли лишаються на диску.
# machine-drafted, needs native review
queue-clear-ok = Очистити чергу
# machine-drafted, needs native review
queue-cleared = Чергу очищено.
# machine-drafted, needs native review
queue-save-title = Зберегти чергу як список відтворення
# machine-drafted, needs native review
queue-default-name = Черга
# machine-drafted, needs native review
queue-format-m3u8 = Список відтворення M3U8
# machine-drafted, needs native review
queue-format-xspf = Список відтворення XSPF
# machine-drafted, needs native review
queue-saved = Збережено { $name }.
# machine-drafted, needs native review
queue-save-failed = Не вдається зберегти { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Відтворювати наступний звуковий файл
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Коли звуковий файл закінчується, грає наступний звуковий файл у теці.
# machine-drafted, needs native review
settings-viewing-play-repeat = Повтор
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Для звукових файлів, відео й черги.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Вимк.
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Усе
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Один
# machine-drafted, needs native review
settings-viewing-play-shuffle = Перемішати
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Відтворювати у випадковому порядку, кожен елемент раз за коло.
