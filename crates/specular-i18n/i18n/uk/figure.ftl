# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Експортувати рисунок…
# machine-drafted, needs native review
palette-export-figure-help = Зберегти вигляд як рисунок із масштабною лінійкою, колірною шкалою та підписами.
# machine-drafted, needs native review
palette-toggle-key-image = Позначити ключове зображення
# machine-drafted, needs native review
palette-toggle-key-image-help = Позначити це зображення як ключове або зняти позначку.
# machine-drafted, needs native review
palette-copy-key-images = Копіювати ключові зображення
# machine-drafted, needs native review
palette-copy-key-images-help = Скопіювати ключові зображення як одне зображення.
# machine-drafted, needs native review
palette-export-key-images = Експортувати ключові зображення…
# machine-drafted, needs native review
palette-export-key-images-help = Зберегти ключові зображення як слайди або теку з PNG.
# machine-drafted, needs native review
palette-export-teaching-file = Експортувати навчальний файл…
# machine-drafted, needs native review
palette-export-teaching-file-help = Зберегти ключові зображення без даних пацієнта, для навчання.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Експортувати рисунок
# machine-drafted, needs native review
figure-format = Формат
# machine-drafted, needs native review
figure-format-png = PNG, 16 біт
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16 біт
# machine-drafted, needs native review
figure-region = Ділянка
# machine-drafted, needs native review
figure-region-visible = Видима область
# machine-drafted, needs native review
figure-region-whole = Усе зображення
# machine-drafted, needs native review
figure-size = Розмір
# machine-drafted, needs native review
figure-size-pixels = Пікселі зображення (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = Ширина { $width } px
# machine-drafted, needs native review
figure-scale-bar = Масштабна лінійка
# machine-drafted, needs native review
figure-scale-bar-none = Без калібрування
# machine-drafted, needs native review
figure-colour-bar = Колірна шкала
# machine-drafted, needs native review
figure-colour-bar-none = Без кольорової карти чи вікна
# machine-drafted, needs native review
figure-inset = Врізка
# machine-drafted, needs native review
figure-inset-rectangle = Останній прямокутник
# machine-drafted, needs native review
figure-inset-none = Спершу намалюйте прямокутник
# machine-drafted, needs native review
figure-channels = Канали
# machine-drafted, needs native review
figure-channels-merged = Об’єднані
# machine-drafted, needs native review
figure-channels-split = Окремо й об’єднані
# machine-drafted, needs native review
figure-channels-one = Один канал
# machine-drafted, needs native review
figure-measurements = Вимірювання
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Увімкнено ({ $count })
    [few] Увімкнено ({ $count })
    [many] Увімкнено ({ $count })
   *[other] Увімкнено ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Немає
# machine-drafted, needs native review
figure-title-row = Заголовок
# machine-drafted, needs native review
figure-provenance = Походження
# machine-drafted, needs native review
figure-provenance-metadata = У файлі
# machine-drafted, needs native review
figure-provenance-drawn = У файлі й намальоване
# machine-drafted, needs native review
figure-file = Файл
# machine-drafted, needs native review
figure-choose = Вибрати…
# machine-drafted, needs native review
figure-on = Увімкнено
# machine-drafted, needs native review
figure-off = Вимкнено
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Увімкнено · { $source }
# machine-drafted, needs native review
figure-export = Експортувати
# machine-drafted, needs native review
figure-working = Експорт…
# machine-drafted, needs native review
figure-not-full-res = Пікселі в повній роздільності не завантажено. Збільште до { PERCENT(100) } або виберіть розмір.
# machine-drafted, needs native review
figure-channel-red = Червоний
# machine-drafted, needs native review
figure-channel-green = Зелений
# machine-drafted, needs native review
figure-channel-blue = Синій
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, від { $lo } до { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Вікно від { $lo } до { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Кольори каналів { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Канал { $channel }
# machine-drafted, needs native review
figure-display-hilo = Позначки HiLo
# machine-drafted, needs native review
figure-display-as-stored = Як збережено
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Калібрування: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Відображення: { $value }
# machine-drafted, needs native review
figure-not-image = Рисунки створюються із зображення у переглядачі.
# machine-drafted, needs native review
figure-not-ready = Зображення ще завантажується.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Завелике: { $width } × { $height } px. Виберіть видиму область або розмір.

## Key images
# machine-drafted, needs native review
figure-keys-title = Експортувати ключові зображення
# machine-drafted, needs native review
figure-teaching-title = Експортувати навчальний файл
# machine-drafted, needs native review
figure-keys-images = Зображення
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } ключове зображення
    [few] { $count } ключові зображення
    [many] { $count } ключових зображень
   *[other] { $count } ключового зображення
}
# machine-drafted, needs native review
figure-keys-pptx = Слайди (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Тека з PNG
# machine-drafted, needs native review
figure-keys-preset = Шаблон
# machine-drafted, needs native review
figure-keys-standard = Стандартний
# machine-drafted, needs native review
figure-keys-teaching = Навчальний файл
# machine-drafted, needs native review
figure-keys-overlays-on = Намальовані й у нотатках
# machine-drafted, needs native review
figure-keys-masks = Вбудований текст
# machine-drafted, needs native review
figure-keys-masks-auto = Зачорнений
# machine-drafted, needs native review
figure-keys-notes = Нотатки до випадку
# machine-drafted, needs native review
figure-keys-destination = Зберегти в
# machine-drafted, needs native review
figure-keys-empty = Ключових зображень ще немає. Спершу позначте зображення як ключове.
# machine-drafted, needs native review
figure-keys-teaching-note = Дані пацієнта пропускаються, а вбудований текст зачорнюється. Перевірте кожне зображення, перш ніж ділитися.
# machine-drafted, needs native review
figure-keys-none-left = Жодне зображення не вдалося експортувати без даних пацієнта.
# machine-drafted, needs native review
figure-keys-copied = Ключові зображення скопійовано
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Позначено як ключове зображення ({ $count })
    [few] Позначено як ключове зображення ({ $count })
    [many] Позначено як ключове зображення ({ $count })
   *[other] Позначено як ключове зображення ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Більше не ключове зображення (залишилося { $count })
    [few] Більше не ключове зображення (залишилося { $count })
    [many] Більше не ключове зображення (залишилося { $count })
   *[other] Більше не ключове зображення (залишилося { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Зображення { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Ключове зображення { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = Збережено: { $name }
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] Збережено: { $name }. Пропущено { $count } зображення: не знайдено вбудованого в нього тексту.
    [few] Збережено: { $name }. Пропущено { $count } зображення: не знайдено вбудованого в них тексту.
    [many] Збережено: { $name }. Пропущено { $count } зображень: не знайдено вбудованого в них тексту.
   *[other] Збережено: { $name }. Пропущено { $count } зображення: не знайдено вбудованого в них тексту.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Не вдалося експортувати: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } уже є в цій теці. Виберіть порожню теку.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Ключове зображення
