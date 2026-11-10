# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Українська.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] { $count } модуль цього редагування оновлено до новішої версії. Результат може виглядати трохи інакше.
    [few] { $count } модулі цього редагування оновлено до новішої версії. Результат може виглядати трохи інакше.
    [many] { $count } модулів цього редагування оновлено до новішої версії. Результат може виглядати трохи інакше.
   *[other] { $count } модуля цього редагування оновлено до новішої версії. Результат може виглядати трохи інакше.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] { $count } модуль цього редагування потребує новішої версії Specular. Його збережено, але не показано.
    [few] { $count } модулі цього редагування потребують новішої версії Specular. Їх збережено, але не показано.
    [many] { $count } модулів цього редагування потребують новішої версії Specular. Їх збережено, але не показано.
   *[other] { $count } модуля цього редагування потребують новішої версії Specular. Їх збережено, але не показано.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Точка { $index } з { $count }: вхід { $input }, вихід { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Стрілки переміщують точку. Page Up і Page Down вибирають іншу. Insert додає точку, Delete вилучає її.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Нейтральний
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Тон { $hue }°, сила { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Стрілки переміщують колір. Delete скидає його.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Проявлення
# machine-drafted, needs native review
develop-undo = Скасувати
# machine-drafted, needs native review
develop-redo = Повторити
# machine-drafted, needs native review
develop-before-after = До і після
# machine-drafted, needs native review
develop-close = Вийти з проявлення
# machine-drafted, needs native review
develop-history = Історія
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module }: увімкнено
develop-card-menu = { $module }: параметри
develop-card-changed = Змінено
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Стрічка кадрів

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Експозиція
develop-module-light = Світло
develop-look-profile = Профіль
develop-look-standard = Стандартний
develop-look-film = Плівка
develop-look-natural = Природний
develop-module-grading = Кольорова градація
develop-add-tool = Додати інструмент
# machine-drafted, needs native review
develop-module-white-balance = Баланс білого
# machine-drafted, needs native review
develop-module-tone = Тон
# machine-drafted, needs native review
develop-module-contrast = Контраст і діапазон
# machine-drafted, needs native review
develop-module-colour = Колір
# machine-drafted, needs native review
develop-module-curve = Крива
# machine-drafted, needs native review
develop-module-crop = Кадрування та вирівнювання
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Інший модуль
# machine-drafted, needs native review
develop-exposure = Експозиція
# machine-drafted, needs native review
develop-black-level = Рівень чорного
# machine-drafted, needs native review
develop-temperature = Температура
# machine-drafted, needs native review
develop-tint = Відтінок
# machine-drafted, needs native review
develop-highlights = Світла
# machine-drafted, needs native review
develop-shadows = Тіні
# machine-drafted, needs native review
develop-contrast = Контраст
# machine-drafted, needs native review
develop-whites = Білі
# machine-drafted, needs native review
develop-blacks = Чорні
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = Межа HDR
# machine-drafted, needs native review
develop-hdr-preview = Показати
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Соковитість
# machine-drafted, needs native review
develop-saturation = Насиченість
# machine-drafted, needs native review
develop-colour-wheels = Колірні кола
# machine-drafted, needs native review
develop-curve = Тонова крива
# machine-drafted, needs native review
develop-module-local-contrast = Локальний контраст
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Автоекспозиція
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Згладжування
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Текстура
# machine-drafted, needs native review
develop-clarity = Чіткість
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Точка чорного
# machine-drafted, needs native review
develop-levels-mid = Середня точка
# machine-drafted, needs native review
develop-levels-white = Точка білого
# machine-drafted, needs native review
develop-module-detail = Деталі
# machine-drafted, needs native review
develop-module-denoise = Зменшення шуму
# machine-drafted, needs native review
develop-module-sharpen = Різкість
# machine-drafted, needs native review
develop-noise-luminance = Шум яскравості
# machine-drafted, needs native review
develop-noise-detail = Деталі шуму
# machine-drafted, needs native review
develop-noise-colour = Кольоровий шум
# machine-drafted, needs native review
develop-sharpen-amount = Сила різкості
# machine-drafted, needs native review
develop-sharpen-radius = Радіус різкості
# machine-drafted, needs native review
develop-sharpen-detail = Деталі різкості
# machine-drafted, needs native review
develop-sharpen-masking = Маскування
# machine-drafted, needs native review
develop-capture-amount = Різкість знімання
# machine-drafted, needs native review
develop-capture-radius = Радіус знімання
# machine-drafted, needs native review
develop-straighten = Вирівнювання
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Кадрування
# machine-drafted, needs native review
develop-crop-turn = Повернути
# machine-drafted, needs native review
develop-crop-flip = Віддзеркалити
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Корекція об’єктива
develop-module-optics = Оптика
# machine-drafted, needs native review
develop-lens-distortion = Дисторсія
# machine-drafted, needs native review
develop-lens-vignette = Віньєтування
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Червона облямівка
# machine-drafted, needs native review
develop-lens-tca-blue = Синя облямівка
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Профіль об’єктива
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Автоматично
# machine-drafted, needs native review
develop-lensfun-off = Немає
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Немає профілю об’єктива.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Немає профілю Lensfun для { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Немає профілю Lensfun для цього об’єктива.
# machine-drafted, needs native review
develop-lensfun-no-name = Файл не називає об’єктив. Виберіть його вище.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = Підходящі об’єктиви: { $count }. Виберіть один вище.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Кадрування. Перетягніть краї або Ctrl+перетягніть уздовж лінії, яка має бути горизонтальною. Enter зберігає, Esc повертає назад.
# machine-drafted, needs native review
develop-crop-kept = Кадр збережено
# machine-drafted, needs native review
develop-crop-cancelled = Кадрування скасовано
# machine-drafted, needs native review
develop-crop-reset = Кадрування та вирівнювання скинуто
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Вирівняно на { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Пропорції { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = вільні
# machine-drafted, needs native review
develop-crop-aspect-original = оригінальні
# machine-drafted, needs native review
develop-crop-turned-right = Повернуто праворуч
# machine-drafted, needs native review
develop-crop-turned-left = Повернуто ліворуч
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Віддзеркалено зліва направо
# machine-drafted, needs native review
develop-crop-flipped-vertical = Віддзеркалено згори донизу
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Пропорції { $aspect } · Напрямні: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A пропорції · X поміняти · O напрямні · [ ] повернути · H V віддзеркалити · Enter зберегти · Esc назад
# machine-drafted, needs native review
develop-crop-guide-thirds = третини
# machine-drafted, needs native review
develop-crop-guide-golden = золотий перетин
# machine-drafted, needs native review
develop-crop-guide-diagonal = діагоналі
# machine-drafted, needs native review
develop-crop-guide-grid = сітка

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Перспектива
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Випрямлення
# machine-drafted, needs native review
develop-persp-mode-off = Вимк.
# machine-drafted, needs native review
develop-persp-mode-auto = Автоматично
# machine-drafted, needs native review
develop-persp-mode-level = Горизонт
# machine-drafted, needs native review
develop-persp-mode-vertical = Вертикалі
# machine-drafted, needs native review
develop-persp-mode-full = Повне
# machine-drafted, needs native review
develop-persp-mode-guided = За напрямними
# machine-drafted, needs native review
develop-persp-vertical = Вертикаль
# machine-drafted, needs native review
develop-persp-horizontal = Горизонталь
# machine-drafted, needs native review
develop-persp-rotate = Поворот
# machine-drafted, needs native review
develop-persp-aspect = Пропорції
# machine-drafted, needs native review
develop-persp-scale = Масштаб
# machine-drafted, needs native review
develop-persp-offset-x = Убік
# machine-drafted, needs native review
develop-persp-offset-y = Угору чи вниз
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Без порожніх країв
# machine-drafted, needs native review
develop-persp-guides = Напрямні
# machine-drafted, needs native review
develop-persp-focal = Фокусна відстань
# machine-drafted, needs native review
develop-persp-draw-guides = Накреслити напрямні
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Накреслити напрямні ({ $n } з { $max })
# machine-drafted, needs native review
develop-persp-reset = Скинути перспективу
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Випрямлення: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Пошук ліній…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Скинути перспективу? Ваші напрямні ({ $n }) буде вилучено. Скасування поверне їх.
# machine-drafted, needs native review
develop-persp-reset-ok = Скинути
# machine-drafted, needs native review
develop-persp-reset-done = Перспективу скинуто
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Вилучити напрямну { $n }? Скасування поверне її.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Вилучити
# machine-drafted, needs native review
develop-persp-guide-removed = Напрямну вилучено
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Вертикаль { $vertical }, горизонталь { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Не вдалося випрямити: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: нахил { $vertical }°, поворот убік { $horizontal }°, обертання { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = Режиму { $mode } потрібні щонайменше дві прямі лінії, які він може розрізнити. Спробуйте інший режим або накресліть напрямні.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Найбільша кількість напрямних: { $n }. Перетягніть кінець, щоб пересунути одну.
# machine-drafted, needs native review
develop-persp-guides-on = Напрямні. Перетягніть уздовж лінії, яка має бути вертикальною або горизонтальною.
# machine-drafted, needs native review
develop-persp-guides-off = Кадрування
# machine-drafted, needs native review
develop-persp-guide-moved = Напрямну пересунуто
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Напрямну { $n } додано
# machine-drafted, needs native review
develop-persp-guides-keys = Перетягніть, щоб додати · Перетягніть кінець, щоб пересунути · Delete вилучає · Shift+R кадрування
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Напрямні: { $n } з { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Тіні
# machine-drafted, needs native review
develop-wheel-midtones = Середні тони
# machine-drafted, needs native review
develop-wheel-highlights = Світла
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Яскравість тіней
# machine-drafted, needs native review
develop-luminance-midtones = Яскравість середніх тонів
# machine-drafted, needs native review
develop-luminance-highlights = Яскравість світел
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Загалом
# machine-drafted, needs native review
develop-luminance-global = Загальна яскравість
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Інтенсивність кольору
# machine-drafted, needs native review
develop-brilliance = Яскравість кольору

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Мікшер кольорів
# machine-drafted, needs native review
develop-colour-red = Червоний
# machine-drafted, needs native review
develop-colour-orange = Помаранчевий
# machine-drafted, needs native review
develop-colour-yellow = Жовтий
# machine-drafted, needs native review
develop-colour-green = Зелений
# machine-drafted, needs native review
develop-colour-aqua = Аквамариновий
# machine-drafted, needs native review
develop-colour-blue = Синій
# machine-drafted, needs native review
develop-colour-purple = Фіолетовий
# machine-drafted, needs native review
develop-colour-magenta = Пурпуровий
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Тон – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Насиченість – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Яскравість – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Точковий колір
# machine-drafted, needs native review
develop-point-pick = Вибрати колір
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Вибрати інший колір
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Клацніть колір, який треба змінити. Esc скасовує.
# machine-drafted, needs native review
develop-point-picked = Колір вибрано
# machine-drafted, needs native review
develop-point-cancelled = Вибір скасовано
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Діапазон
# machine-drafted, needs native review
develop-point-hue-range = Діапазон відтінку
# machine-drafted, needs native review
develop-point-saturation-range = Діапазон насиченості
# machine-drafted, needs native review
develop-point-lightness-range = Діапазон світлості
# machine-drafted, needs native review
develop-point-hue = Зсув тону
# machine-drafted, needs native review
develop-point-saturation = Зсув насиченості
# machine-drafted, needs native review
develop-point-lightness = Зсув світлості

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Чорно-біле
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Яскравість – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Вибрати файл LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Файл: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Не вдається прочитати { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Вибрати файл LUT
# machine-drafted, needs native review
develop-lut-kind = Файли LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Створено для
# machine-drafted, needs native review
develop-lut-space-linear = Лінійний (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES лінійний (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Фото (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Відео (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Метод
# machine-drafted, needs native review
develop-lut-tetrahedral = Точний
# machine-drafted, needs native review
develop-lut-trilinear = Простий
# machine-drafted, needs native review
develop-lut-strength = Сила

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Оригінал
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module }: увімкнено
# machine-drafted, needs native review
develop-history-off = { $module }: вимкнено
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Назад до оригіналу
# machine-drafted, needs native review
develop-at-step = Назад до: { $step }
# machine-drafted, needs native review
develop-undone = Скасовано: { $step }
# machine-drafted, needs native review
develop-redone = Повторено: { $step }
# machine-drafted, needs native review
develop-entered = Проявлення. Редагування ніколи не змінює файл.
# machine-drafted, needs native review
develop-left = Вихід із проявлення
# machine-drafted, needs native review
develop-split-on = До і після поруч
# machine-drafted, needs native review
develop-split-off = Лише редагування
# machine-drafted, needs native review
develop-showing-before = Показано оригінал
# machine-drafted, needs native review
develop-showing-after = Показано редагування
# machine-drafted, needs native review
# Labels over the image.
develop-before = До
# machine-drafted, needs native review
develop-after = Після
# machine-drafted, needs native review
develop-working = Проявлення…
# machine-drafted, needs native review
develop-preview-failed = Не вдається показати редагування для цього файлу

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Проявлення
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Швидке редагування: утримувати, потім прокручувати
# machine-drafted, needs native review
palette-develop = Проявлення
# machine-drafted, needs native review
palette-develop-help = Редагувати це фото, не змінюючи файл. Ще раз, щоб вийти.
# machine-drafted, needs native review
palette-develop-before-after = До і після
# machine-drafted, needs native review
palette-develop-before-after-help = Показати оригінал або знову редагування.
# machine-drafted, needs native review
palette-develop-split = До і після поділено
# machine-drafted, needs native review
palette-develop-split-help = Показати оригінал і редагування поруч.
# machine-drafted, needs native review
palette-develop-panel = Перейти до панелі проявлення
# machine-drafted, needs native review
palette-develop-panel-help = Перенести клавіатуру на панель. Esc повертає.
# machine-drafted, needs native review
palette-develop-speed-exposure = Швидко змінити експозицію
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Утримувати, потім прокручувати або натискати стрілки, щоб змінити експозицію.
# machine-drafted, needs native review
palette-develop-speed-contrast = Швидко змінити контраст
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Утримувати, потім прокручувати або натискати стрілки, щоб змінити контраст.
# machine-drafted, needs native review
palette-develop-speed-highlights = Швидко змінити світла
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Утримувати, потім прокручувати або натискати стрілки, щоб змінити світла.
# machine-drafted, needs native review
palette-develop-speed-shadows = Швидко змінити тіні
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Утримувати, потім прокручувати або натискати стрілки, щоб змінити тіні.
# machine-drafted, needs native review
palette-develop-speed-whites = Швидко змінити білі
# machine-drafted, needs native review
palette-develop-speed-whites-help = Утримувати, потім прокручувати або натискати стрілки, щоб змінити білі.
# machine-drafted, needs native review
palette-develop-speed-blacks = Швидко змінити чорні
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Утримувати, потім прокручувати або натискати стрілки, щоб змінити чорні.
# machine-drafted, needs native review
palette-develop-speed-temperature = Швидко змінити температуру
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Утримувати, потім прокручувати або натискати стрілки, щоб змінити температуру балансу білого.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Швидко змінити соковитість
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Утримувати, потім прокручувати або натискати стрілки, щоб змінити соковитість.
# machine-drafted, needs native review
palette-develop-crop = Кадрування та вирівнювання
# machine-drafted, needs native review
palette-develop-crop-help = Кадрувати, вирівняти, повернути або віддзеркалити фото. Enter зберігає, Esc повертає назад.
# machine-drafted, needs native review
disabled-not-photo = Лише для фотографій
# machine-drafted, needs native review
disabled-develop-off = Проявлення вимкнено в профілях «Медицина» і «Наука»
# machine-drafted, needs native review
develop-module-corrections = Виправлення
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Вимк.
# machine-drafted, needs native review
develop-on = Увімк.
# machine-drafted, needs native review
develop-highlight-method = Перетримані світла
# machine-drafted, needs native review
develop-highlight-opposed = Відновити колір
# machine-drafted, needs native review
develop-highlight-rebuild = Відновити великі ділянки
# machine-drafted, needs native review
develop-highlight-clip = Обрізати до білого
# machine-drafted, needs native review
develop-highlight-threshold = Рівень обрізання
# machine-drafted, needs native review
develop-module-hot-pixels = Гарячі пікселі
# machine-drafted, needs native review
develop-hot-threshold = Поріг гарячих пікселів
# machine-drafted, needs native review
develop-module-ca = Хроматична аберація
# machine-drafted, needs native review
develop-ca-mode = Облямівки
# machine-drafted, needs native review
develop-ca-auto = Автоматично
# machine-drafted, needs native review
develop-ca-manual = Вручну
# machine-drafted, needs native review
develop-ca-red = Зсув червоного
# machine-drafted, needs native review
develop-ca-blue = Зсув синього
# machine-drafted, needs native review
develop-module-defringe = Прибрати облямівки
# machine-drafted, needs native review
develop-defringe = Прибрати облямівки
# machine-drafted, needs native review
develop-defringe-purple = Фіолетовий тон
# machine-drafted, needs native review
develop-defringe-green = Зелений тон
# machine-drafted, needs native review
develop-defringe-range = Діапазон відтінку
# machine-drafted, needs native review
develop-defringe-edge = Контраст країв
# machine-drafted, needs native review
develop-module-flat-field = Калібрувальні кадри
# machine-drafted, needs native review
develop-calibration = Калібрування
# machine-drafted, needs native review
develop-calibration-auto = З теки
# machine-drafted, needs native review
develop-calibration-chosen = Вибрані файли
# machine-drafted, needs native review
develop-flat-choose = Вибрати flat-кадр…
# machine-drafted, needs native review
develop-dark-choose = Вибрати dark-кадр…
# machine-drafted, needs native review
develop-flat-file = Flat-кадр: { $name }
# machine-drafted, needs native review
develop-dark-file = Dark-кадр: { $name }
# machine-drafted, needs native review
develop-flat-use = Flat-кадр
# machine-drafted, needs native review
develop-flat-use-full = Віньєтування й пил
# machine-drafted, needs native review
develop-flat-use-vignetting = Лише віньєтування
# machine-drafted, needs native review
develop-flat-dialog = Вибрати flat-кадр
# machine-drafted, needs native review
develop-dark-dialog = Вибрати dark-кадр
# machine-drafted, needs native review
develop-raw-kind = Файли raw
# machine-drafted, needs native review
develop-flat-frame = Flat-кадр
# machine-drafted, needs native review
develop-dark-frame = Dark-кадр

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Додати маску
# machine-drafted, needs native review
develop-mask-kind-ellipse = Еліпс
# machine-drafted, needs native review
develop-mask-kind-rectangle = Прямокутник
# machine-drafted, needs native review
develop-mask-kind-polygon = Багатокутник
# machine-drafted, needs native review
develop-mask-kind-linear = Лінійний градієнт
# machine-drafted, needs native review
develop-mask-kind-radial = Радіальний градієнт
# machine-drafted, needs native review
develop-mask-kind-brush = Пензель
# machine-drafted, needs native review
develop-mask-kind-luminance = Діапазон яскравості
# machine-drafted, needs native review
develop-mask-kind-hue = Діапазон відтінку
# machine-drafted, needs native review
develop-mask-kind-chroma = Діапазон насиченості
# machine-drafted, needs native review
develop-mask-kind-colour = Діапазон кольору
# machine-drafted, needs native review
develop-mask-kind-other = Інша маска
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Змішування
# machine-drafted, needs native review
develop-blend-normal = Звичайне
# machine-drafted, needs native review
develop-blend-multiply = Множення
# machine-drafted, needs native review
develop-blend-screen = Екран
# machine-drafted, needs native review
develop-blend-overlay = Перекриття
# machine-drafted, needs native review
develop-blend-soft-light = М’яке світло
# machine-drafted, needs native review
develop-blend-lightness = Лише світлота
# machine-drafted, needs native review
develop-blend-colour = Лише колір
# machine-drafted, needs native review
develop-mask-amount = Сила маски
# machine-drafted, needs native review
develop-mask-invert = Інвертувати маску
# machine-drafted, needs native review
develop-mask-off = Вимк.
# machine-drafted, needs native review
develop-mask-on = Увімк.
# machine-drafted, needs native review
develop-mask-refine = Уточнити краї
# machine-drafted, needs native review
develop-mask-show = Показати маску
# machine-drafted, needs native review
develop-mask-show-red = Червона
# machine-drafted, needs native review
develop-mask-show-grey = Сіра
# machine-drafted, needs native review
develop-mask-remove = Вилучити маску
# machine-drafted, needs native review
develop-mask-combine-add = Додати
# machine-drafted, needs native review
develop-mask-combine-subtract = Відняти
# machine-drafted, needs native review
develop-mask-combine-intersect = Перетин
# machine-drafted, needs native review
develop-mask-row-opacity = Непрозорість
# machine-drafted, needs native review
develop-mask-row-invert = Інвертувати
# machine-drafted, needs native review
develop-mask-feather = Розтушовування
# machine-drafted, needs native review
develop-mask-roundness = Заокруглення
# machine-drafted, needs native review
develop-mask-brush-size = Розмір пензля
# machine-drafted, needs native review
develop-mask-brush-feather = М’якість пензля
# machine-drafted, needs native review
develop-mask-brush-flow = Потік
# machine-drafted, needs native review
develop-mask-brush-mode = Пензель
# machine-drafted, needs native review
develop-mask-brush-paint = Малювати
# machine-drafted, needs native review
develop-mask-brush-erase = Стирати
# machine-drafted, needs native review
develop-mask-range-from = Від
# machine-drafted, needs native review
develop-mask-range-full-from = Повністю від
# machine-drafted, needs native review
develop-mask-range-full-to = Повністю до
# machine-drafted, needs native review
develop-mask-range-to = До
# machine-drafted, needs native review
develop-mask-range-test = Перевіряти
# machine-drafted, needs native review
develop-mask-range-input = Вхід
# machine-drafted, needs native review
develop-mask-range-output = Вихід
# machine-drafted, needs native review
develop-mask-colour-pick = Узяти колір
# machine-drafted, needs native review
develop-mask-colour-pick-more = Узяти ще колір
# machine-drafted, needs native review
develop-mask-colour-range = Діапазон
# machine-drafted, needs native review
develop-mask-colour-falloff = Згасання
# machine-drafted, needs native review
develop-mask-edit = Редагувати на знімку
# machine-drafted, needs native review
develop-mask-row-remove = Вилучити { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Вилучити маску модуля { $module }? Її фігури зникнуть. Фото на диску залишиться.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Вилучити { $name } з модуля { $module }? Інші маски й фото на диску залишаться.
# machine-drafted, needs native review
develop-mask-remove-ok = Вилучити
# machine-drafted, needs native review
develop-mask-removed = Маску вилучено.
# machine-drafted, needs native review
develop-mask-added = Додано: { $name }.
# machine-drafted, needs native review
develop-mask-full = Ця маска вже має стільки частин, скільки вміщує.
# machine-drafted, needs native review
develop-mask-editing = Редагування: { $name }. Перетягуйте маркери або натисніть Tab і стрілки.
# machine-drafted, needs native review
develop-mask-painting = Малювання: { $name }. Перетягуйте по знімку.
# machine-drafted, needs native review
develop-mask-picking = Клацніть знімок, щоб узяти колір.
# machine-drafted, needs native review
develop-mask-picked = Колір узято.
# machine-drafted, needs native review
develop-mask-done = Редагування маски завершено.
# machine-drafted, needs native review
develop-mask-handles = Маркери маски
# machine-drafted, needs native review
develop-handle-centre = Центр
# machine-drafted, needs native review
develop-handle-width = Ширина
# machine-drafted, needs native review
develop-handle-height = Висота
# machine-drafted, needs native review
develop-handle-rotate = Обертання
# machine-drafted, needs native review
develop-handle-feather = Розтушовування
# machine-drafted, needs native review
develop-handle-inner = Внутрішній край
# machine-drafted, needs native review
develop-handle-start = Початок
# machine-drafted, needs native review
develop-handle-end = Кінець
# machine-drafted, needs native review
develop-handle-point = Точка { $n }
# machine-drafted, needs native review
develop-instance = Шар
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Шар { $n }.
# machine-drafted, needs native review
develop-instance-new = Новий шар
# machine-drafted, needs native review
develop-instance-duplicate = Дублювати шар
# machine-drafted, needs native review
develop-instance-delete = Видалити шар
# machine-drafted, needs native review
develop-instance-earlier = Перемістити раніше
# machine-drafted, needs native review
develop-instance-later = Перемістити пізніше
# machine-drafted, needs native review
develop-instance-delete-confirm = Видалити шар { $n } модуля { $module }? Його параметри й маски зникнуть. Фото на диску залишиться.
# machine-drafted, needs native review
develop-instance-delete-ok = Видалити
# machine-drafted, needs native review
develop-instance-deleted = Шар видалено.
# machine-drafted, needs native review
develop-history-mask = Маска: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Фігури маски: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Новий: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Видалено: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Порядок: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Інші програми
# machine-drafted, needs native review
develop-darktable-edited = Змінено в darktable:
# machine-drafted, needs native review
develop-darktable-open = Відкрити в darktable
# machine-drafted, needs native review
develop-darktable-opening = Відкривається в darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable не запустився: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Імпорт з Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] Імпортовано налаштувань Lightroom: 1. Результат приблизний.
    [few] Імпортовано налаштувань Lightroom: { $count }. Результат приблизний.
    [many] Імпортовано налаштувань Lightroom: { $count }. Результат приблизний.
   *[other] Імпортовано налаштувань Lightroom: { $count }. Результат приблизний.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Нічого імпортувати: ці налаштування тут нічого не змінюють.
# machine-drafted, needs native review
develop-history-lightroom = Імпортовано з Lightroom (приблизно)
# machine-drafted, needs native review
develop-conflict-title = Обробку { $file } змінено поза Specular
# machine-drafted, needs native review
develop-conflict-file = Супровідний файл: збережено { $time }
# machine-drafted, needs native review
develop-conflict-library = Бібліотека: збережено { $time }
# machine-drafted, needs native review
develop-conflict-writer = Супровідний файл записав { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Залиште одну. Іншу буде замінено.
# machine-drafted, needs native review
develop-conflict-use-file = Взяти з файлу
# machine-drafted, needs native review
develop-conflict-keep-library = Залишити з бібліотеки
# machine-drafted, needs native review
develop-conflict-used-file = Використано обробку із супровідного файлу
# machine-drafted, needs native review
develop-conflict-kept-library = Залишено обробку з бібліотеки
# machine-drafted, needs native review
toast-develop-edit-newer-process = Цю обробку зроблено в новішому Specular. Тут вона може виглядати інакше.
# machine-drafted, needs native review
info-darktable-edited = Змінено в darktable
# machine-drafted, needs native review
info-darktable-no-modules = Немає увімкнених модулів
# machine-drafted, needs native review
info-darktable-group = Файл
# machine-drafted, needs native review
toast-develop-edit-partial = Цю обробку зроблено в новішому Specular. Частини, яких ця версія не показує, пропущено. Зміни тут не зберігаються.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Не вдається прочитати файл обробки поруч із фото. Його залишено як є. Зміни тут не зберігаються.
# machine-drafted, needs native review
develop-demosaic = Деталі raw
develop-demosaic-directional = Стандартний
develop-demosaic-dual = Згладжувати рівні ділянки
develop-demosaic-fast = Швидкий
develop-dual-threshold = Контраст рівних ділянок
