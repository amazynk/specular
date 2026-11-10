# machine-drafted, needs native review
# Photographer polish pass: Develop regions, Crop card rows, import choices, keyword rename and move, auto-apply rules.

# Develop: F6 between the picture, the panel and the filmstrip.
palette-develop-next-region = Наступна область
palette-develop-next-region-help = Перевести клавіатуру вперед: зображення, панель, стрічка кадрів.
palette-develop-prev-region = Попередня область
palette-develop-prev-region-help = Перевести клавіатуру назад: стрічка кадрів, панель, зображення.
develop-region-picture = Зображення
develop-region-panel = Панель

# Develop: the Crop card's rows (the crop tool's keys as rows).
develop-crop-aspect-row = Пропорції
develop-crop-aspect-option-free = Вільні
develop-crop-aspect-option-original = Оригінальні
develop-crop-aspect-option-custom = Власні
develop-crop-type-ratio = Ввести пропорції…
develop-crop-swap = Поміняти книжкову й альбомну
develop-crop-turn-left = Повернути ліворуч
develop-crop-turn-right = Повернути праворуч
develop-crop-flip-horizontal = Віддзеркалити зліва направо
develop-crop-flip-vertical = Віддзеркалити згори донизу
develop-crop-ratio-title = Пропорції кадру
develop-crop-ratio-field = Ширина:висота
develop-crop-ratio-ok = Задати
develop-crop-ratio-bad = Введіть пропорції, наприклад 5:4, або одне число, наприклад 1.85.

# Import: what each copied photo gets.
import-template = Шаблон метаданих
import-style = Стиль проявлення
import-choice-none = Немає
# A choice row stepped, read out: "Metadata template: Copyright".
import-choice-said = { $label }: { $value }
import-keywords = Ключові слова
import-keywords-hint = Наприклад Родина; Місця > Париж
import-style-missing = Стилю { $name } більше немає. Фото імпортовано без нього.

# Keywords: rename, move and merge.
keywords-rename = Перейменувати…
keywords-move = Перемістити до…
keywords-rename-of = Нова назва для { $keyword }
keywords-move-of = Розмістити { $keyword } під (порожньо: на верхній рівень)
keywords-move-into-itself = { $keyword } не можна розмістити під самим собою.
keywords-renaming = Перейменування { $from } на { $to }…
keywords-merging = Об’єднання { $from } з { $to }…
keywords-renamed = { $from } тепер { $to } на { $count ->
    [one] { $count } фото
    [few] { $count } фото
    [many] { $count } фото
   *[other] { $count } фото
}
keywords-rename-row = Ключове слово { $from } на { $to }
keywords-rename-finding = Пошук фото…
keywords-rename-row-detail = { $done } з { $total } фото

# Develop: the before/after divider dragged ("Divider at 30 %").
develop-split-moved = Розділювач на { PERCENT($percent) }

# Develop: auto-apply rules, changed row by row.
develop-rules-placeholder-edit = Виберіть правило, щоб змінити або видалити його
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = Виробник: { $value }
develop-rule-model = Модель: { $value }
develop-rule-lens = Об’єктив: { $value }
develop-rule-iso = ISO: { $value }
develop-rule-type = Тип файлу: { $value }
develop-rule-style = Стиль: { $value }
develop-rule-any = будь-який
develop-rule-type-any = будь-який файл
develop-rule-delete = Видалити правило…
develop-rule-field-title = Змінити «{ $rule }»
develop-rule-field-maker = Виробник (досить частини назви; порожньо: будь-який)
develop-rule-field-model = Модель (досить частини назви; порожньо: будь-яка)
develop-rule-field-lens = Об’єктив (досить частини назви; порожньо: будь-який)
develop-rule-field-iso = ISO, наприклад 100-800, 1600- або -400 (порожньо: будь-яке)
develop-rule-field-ok = Задати
develop-rule-iso-bad = Введіть діапазон ISO, наприклад 100-800.
develop-rule-style-placeholder = Виберіть стиль правила
