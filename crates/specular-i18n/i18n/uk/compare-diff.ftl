# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Порівняти з…
palette-compare-with-help = Порівняти це зображення з іншим, яке ви виберете.
palette-compare-side = Порівняти поруч
palette-compare-side-help = Показати порівнювані зображення поруч.
palette-compare-split = Порівняти з розділювачем
palette-compare-split-help = Показати друге зображення поверх першого, обрізане розділювачем, який можна перетягувати.
palette-compare-split-direction = Повернути розділювач
palette-compare-split-direction-help = Поставити розділювач горизонтально замість вертикально, або назад.
palette-compare-overlay = Порівняти накладенням
palette-compare-overlay-help = Показати друге зображення поверх першого, зі змішуванням.
palette-compare-difference = Порівняти різницю
palette-compare-difference-help = Показати, де зображення відрізняються; ще раз для наступного виду.
palette-compare-flicker = Мерехтіння між порівнюваними зображеннями
palette-compare-flicker-help = Одразу показати інше зображення.
palette-compare-auto-flicker = Мерехтіти автоматично
palette-compare-auto-flicker-help = Самостійно перемикати між двома зображеннями, або зупинити.
palette-compare-align = Вирівняти порівнювані зображення
palette-compare-align-help = Вирівняти зображення різних розмірів: за лівим верхнім кутом, по центру, з масштабуванням або автоматично.
palette-compare-metric-space = Змінити простір вимірювання
palette-compare-metric-space-help = Вимірювати в лінійному світлі або в кодованому sRGB.
palette-compare-whole-image = Вимірювати все зображення
palette-compare-whole-image-help = Вимірювати все зображення замість видимої частини, або назад.
palette-compare-probe = Піксельний зонд порівняння
palette-compare-probe-help = Показати значення обох зображень під вказівником.
palette-compare-link = Зв’язати або розв’язати порівнювані зображення
palette-compare-link-help = Масштабувати й зсувати кожне з двох зображень окремо, або знову разом.
palette-compare-close = Закрити порівняння
palette-compare-close-help = Повернутися до галереї.
disabled-compare-closed = Спершу відкрийте порівняння
disabled-compare-not-two = Працює з двома зображеннями

## Cheat sheet
cheatsheet-compare-split = Розділювач або поруч
cheatsheet-compare-split-direction = Розділювач горизонтально або вертикально
cheatsheet-compare-auto-flicker = Мерехтіти автоматично
cheatsheet-compare-align = Наступне вирівнювання
cheatsheet-compare-space = Простір вимірювання
cheatsheet-compare-whole = Все зображення або видима частина
cheatsheet-compare-probe = Піксельний зонд
cheatsheet-compare-link = Зв’язати або розв’язати два зображення

## Settings
settings-gallery-compare-metric-space = Простір вимірювання порівняння
settings-gallery-compare-metric-space-help = Де порівняння вимірює MAE, MSE, PSNR і SSIM. Лінійне світло — те, що показує екран. sRGB збігається з іншими інструментами для 8-bit файлів.
settings-gallery-compare-metric-space-linear = Лінійне світло
settings-gallery-compare-metric-space-srgb = Кодований sRGB
settings-gallery-compare-flicker-limit = Найшвидше автомерехтіння
settings-gallery-compare-flicker-limit-help = Щонайбільше стільки змін зображення за секунду. Понад 3 мерехтіння може спричинити напади.

## Toolbar and captions
compare-split = Розділення
compare-split-down = Розділювач вертикально
compare-split-across = Розділювач горизонтально
compare-link = Зв’язано
compare-auto-flicker = Авто
compare-flicker-rate = { $rate } змін за секунду
compare-gain = Підсилення ×{ $gain }
compare-probe = Зонд
compare-kind-signed = Похибка зі знаком
compare-kind-on-image = Похибка поверх зображення
compare-align-top-left = Лівий верхній кут
compare-align-centre = По центру
compare-align-scaled = B масштабовано до A
compare-align-auto = Автовирівнювання
compare-space-linear = Лінійне світло
compare-space-srgb = sRGB
compare-space-help-linear = Виміряно в лінійному світлі, як показано після тонального відображення. PSNR тут не звичайний PSNR для 8-bit sRGB.
compare-space-help-srgb = Виміряно на кодованих значеннях sRGB, як це роблять інші інструменти для 8-bit файлів. HDR понад білий SDR рахується як білий.
compare-space-help-common = ΔE2000 завжди в CIELAB (D65), білий SDR як L* 100, HDR обрізано. Зображення понад { $size } пікселів вимірюються з копії на { $size } пікселів.
compare-region-view = Видима частина
compare-region-whole = Все зображення
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM потребує 11 пікселів
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, макс. { $max })
compare-delta-e-hdr = HDR обрізано для ΔE
compare-aligned-centre = По центру, порівняно { $width } × { $height }
compare-aligned-scaled = B масштабовано { $scale }×, порівняно { $width } × { $height }
compare-aligned-auto = Вирівняно { $angle }° { $scale }×, порівняно { $width } × { $height }
compare-aligning = Вирівнювання…
compare-align-failed = Не вдалося вирівняти ці зображення. Повернуто до лівого верхнього кута.
compare-split-label = { $b } поверх { $a }, розділювач на { PERCENT($percent) }
compare-showing-which = Показано { $which }: { $name }
compare-auto-flicker-on = Мерехтіння, { $rate } змін за секунду
compare-auto-flicker-off = Мерехтіння зупинено
compare-linked = Зображення зв’язано
compare-unlinked = Зображення розв’язано: кожне зсувається окремо
compare-with-title = Порівняти з
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Розділювач
compare-a11y-space = Простір вимірювання
compare-a11y-align = Вирівнювання
