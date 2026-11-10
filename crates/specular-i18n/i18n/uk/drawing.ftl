# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Українська.

keys-mode-drawing = Креслення
cheatsheet-drawing = Креслення
palette-drawing-layers = Показати або сховати панель шарів
palette-drawing-all-layers = Показати всі шари або шари як збережено
palette-drawing-paper = Поміняти темний і світлий папір
palette-drawing-lineweights = Показати або сховати товщини ліній
disabled-not-drawing = Лише для креслень (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Модель
# machine-drafted, needs native review
cad-pen-name = Перо { $n }
cad-page-name = Сторінка { $n }
cad-layer-name = Шар { $n }
cad-map-name = Мапа
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Шари
cad-board-top-name = Верхній бік
cad-board-bottom-name = Нижній бік
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Рамка
cad-trace-name = Крива { $n }
cad-contours-positive-name = Додатні контури
cad-contours-negative-name = Від’ємні контури
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D-модель
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Шари · показано { $shown } з { $total }
# Toasts.
drawing-layers-all-toast = Показано всі шари
drawing-layers-saved-toast = Шари як збережено в кресленні
drawing-paper-dark-toast = Темний папір
drawing-paper-light-toast = Світлий папір
drawing-lineweights-on = Товщини ліній показано
drawing-lineweights-off = Товщини ліній вимкнено: тонкі лінії

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Додати позначки
palette-markup-help = Хмари, текст, стрілки, штампи, вимірювання й фотомітки, збережені поруч із файлом.
palette-compare-sheets = Порівняти аркуші
palette-compare-sheets-help = Накласти іншу ревізію цього аркуша, вирівняну, з обведеними змінами.
markup-title = Позначки
markup-list = Позначки на цій сторінці
markup-count = { $count ->
    [0] На цій сторінці немає позначок
    [one] { $count } позначка на цій сторінці
    [few] { $count } позначки на цій сторінці
    [many] { $count } позначок на цій сторінці
   *[other] { $count } позначки на цій сторінці
}
markup-empty = Виберіть інструмент і клацніть на аркуші.
markup-item = { $n }. { $kind }
markup-tool-select = Вибрати
markup-tool-cloud = Хмара
markup-tool-text = Текст
markup-tool-arrow = Стрілка
markup-tool-stamp = Штамп
markup-tool-length = Довжина
markup-tool-area = Площа
markup-tool-pin = Фотомітка
markup-hint-select = Клацніть елемент, щоб вибрати його. Перетягніть, щоб перемістити. Enter редагує його текст.
markup-hint-cloud = Перетягніть прямокутник або клацайте кути й натисніть Enter.
markup-hint-text = Клацніть там, де буде текст.
markup-hint-arrow = Перетягніть від підпису до точки або клацніть обидва.
markup-hint-stamp = Клацніть там, де буде штамп.
markup-hint-length = Перетягніть або клацніть обидва кінці.
markup-hint-area = Клацайте кути, потім натисніть Enter.
markup-hint-pin = Клацніть точку, потім виберіть фото.
markup-kind-cloud = Хмара
markup-kind-text = Текст
markup-kind-arrow = Стрілка
markup-kind-stamp = Штамп
markup-kind-length = Довжина
markup-kind-area = Площа
markup-kind-pin = Фото
markup-colour = Колір: { $colour }
markup-colour-red = Червоний
markup-colour-blue = Синій
markup-colour-green = Зелений
markup-colour-orange = Помаранчевий
markup-colour-magenta = Пурпуровий
markup-colour-black = Чорний
markup-width = Лінія: { $width }
markup-width-thin = Тонка
markup-width-medium = Середня
markup-width-thick = Товста
markup-stamp = Штамп: { $words }
markup-stamp-approved = Затверджено
markup-stamp-rejected = Відхилено
markup-stamp-reviewed = Перевірено
markup-stamp-custom = Свій текст
markup-undo = Скасувати
markup-redo = Повторити
markup-delete = Видалити
markup-open-photo = Відкрити фото
markup-from-measurements = Додати вимірювання
markup-from-measurements-done = { $count ->
    [one] { $count } вимірювання додано як позначку
    [few] { $count } вимірювання додано як позначки
    [many] { $count } вимірювань додано як позначки
   *[other] { $count } вимірювання додано як позначки
}
markup-export-pdf = Експортувати PDF…
markup-export-png = Експортувати PNG…
markup-done = Готово
markup-ask-text = Текст (Enter — розмістити, Esc — скасувати)
markup-ask-label = Підпис (необов’язково; Enter — розмістити)
markup-ask-stamp = Текст штампа (Enter — розмістити)
markup-ask-edit = Новий текст (Enter — зберегти)
markup-added = Додано: { $kind }
markup-moved = Переміщено
markup-pin-hint = Клацніть мітку ще раз або натисніть Enter, щоб відкрити її фото.
markup-deleted = Видалено
markup-undone = Скасовано
markup-redone = Повторено
markup-delete-ask = Видалити { $name }?
markup-delete-detail = Елемент буде вилучено з файлу позначок. Саме креслення не змінюється.
markup-delete-ok = Видалити
markup-photo-missing = { $name } більше немає.
markup-load-failed = Позначки не прочитано: { $reason }
markup-save-failed = Позначки не збережено: { $reason }
markup-sidecar-too-large = файл позначок завеликий
markup-pdf-unreadable = не вдалося прочитати PDF
markup-exported = Збережено { $name }
markup-export-failed = Не експортовано: { $reason }
markup-export-same-file = виберіть іншу назву; оригінал ніколи не перезаписується
markup-export-encrypted = PDF зашифровано. Експортуйте натомість PNG.
markup-export-damaged = структуру PDF пошкоджено. Експортуйте натомість PNG.
sheets-title = Порівняти аркуші
sheets-other = Інший аркуш…
sheets-other-named = Інший: { $name }
sheets-page = Сторінка { $page } з { $count }
sheets-auto = Вирівняти
sheets-pick = Вибрати 2 точки
sheets-overlay = Накладання
sheets-difference = Різниця
sheets-blend = Перехід
sheets-blink = Блимання
sheets-slider = Інший аркуш { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Змін немає
    [one] Обвести хмарою { $count } зміну
    [few] Обвести хмарами { $count } зміни
    [many] Обвести хмарами { $count } змін
   *[other] Обвести хмарами { $count } зміни
}
sheets-done = Готово
sheets-choose = Виберіть інший аркуш.
sheets-reading = Читання аркуша…
sheets-aligning = Вирівнювання…
sheets-aligned = Вирівняно: масштаб { $scale }, поворот { $angle }°, відхилення до { $error } px. { $changes ->
    [0] Змін не знайдено.
    [one] { $changes } зміна.
    [few] { $changes } зміни.
    [many] { $changes } змін.
   *[other] { $changes } зміни.
}
sheets-align-failed = Не вдалося вирівняти автоматично. Виберіть 2 точки.
sheets-pick-this = Точка { $n }: клацніть її на цьому аркуші.
sheets-pick-other = Точка { $n }: клацніть ту саму точку на іншому аркуші.
sheets-pick-same = Обидві точки однакові. Виберіть 2 точки знову.
sheets-read-failed = Аркуш не прочитано: { $reason }
sheets-failed = Цей крок не вдався. Спробуйте ще раз або виберіть інший аркуш.
sheets-legend-this = Лише цей аркуш
sheets-legend-other = Лише інший аркуш
sheets-legend-both = Обидва
sheets-cloud-text = Змінено
sheets-clouds-added = { $count ->
    [0] Хмари не додано
    [one] { $count } хмару додано до позначок
    [few] { $count } хмари додано до позначок
    [many] { $count } хмар додано до позначок
   *[other] { $count } хмари додано до позначок
}
sheets-opened = Порівняти аркуші
sheets-closed = Порівняння закрито
markup-ask-new-line = Shift+Enter: новий рядок

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Креслення
settings-drawings-shx-folders = Теки шрифтів SHX
settings-drawings-shx-folders-help = Теки зі шрифтами SHX для тексту в DXF і DWG, розділені символом ;. Спершу шукається тека самого креслення.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Це не тека: { $folder }
