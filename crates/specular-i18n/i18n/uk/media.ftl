# machine-drafted, needs native review
# Відтворення анімації та режим читання (архіви, комікси).

disabled-not-animated = Це зображення не анімоване
disabled-in-archive = Сторінки в архіві не можна перемістити в кошик

## Палітра команд і довідка

palette-play-pause = Відтворити або призупинити анімацію
palette-next-frame = Наступний кадр
palette-prev-frame = Попередній кадр
palette-faster = Відтворювати швидше
palette-slower = Відтворювати повільніше
palette-normal-speed = Звичайна швидкість відтворення
palette-loop-forever = Повторювати анімацію безкінечно
palette-right-to-left = Порядок сторінок справа наліво
# machine-drafted, needs native review
palette-contents = Зміст
palette-spreads = Розвороти з двох сторінок

## Керування анімацією (панель над панеллю інструментів)

# Назва групи елементів відтворення для доступності.
transport-region = Керування анімацією
transport-play = Відтворити
transport-pause = Пауза
transport-prev-frame = Попередній кадр
transport-next-frame = Наступний кадр
transport-slower = Повільніше
transport-faster = Швидше
# $index — показаний кадр, $count — кількість кадрів.
transport-frame = Кадр { $index } / { $count }
# Кількість кадрів невідома, доки анімацію не відтворено один раз.
transport-frame-unknown = Кадр { $index }
# $speed — відформатоване число, напр. 1,5.
transport-speed = { $speed }×
transport-speed-name = Швидкість відтворення { $speed }×
# machine-drafted, needs native review
motion-badge = Рух
motion-badge-live = Live
motion-play = Відтворити рухомий кліп
motion-play-live = Відтворити Live Photo
motion-stop = Назад до фото
toast-motion-failed = Не вдається відтворити рухомий кліп: { $reason }

## Сповіщення

toast-speed = Швидкість відтворення { $speed }×
toast-loop-forever-on = Безкінечне повторення
toast-loop-forever-off = Повторення як у файлі
toast-right-to-left-on = Сторінки справа наліво
toast-right-to-left-off = Сторінки зліва направо
toast-spreads-on = Розвороти з двох сторінок
toast-spreads-off = По одній сторінці

## Архіви

# Заголовок вікна для сторінки архіву.
title-archive-page = { $page } — { $archive }
# Показник позиції, коли показано дві сторінки.
readout-position-spread = { $first }–{ $last } / { $count }
# $format — RAR.
archive-needs-sandbox = Архіви { $format } може відкривати лише ізольований процес декодування, якого ця збірка поки що не містить.
archive-empty = У цьому архіві немає зображень.
archive-failed = Не вдалося відкрити архів: { $error }
archive-encrypted = Цей архів зашифровано. Відкрийте його знову, щоб ввести пароль.
archive-wrong-password = Пароль до цього архіву неправильний.
password-title = Потрібен пароль
password-archive = Архів «{ $archive }» зашифровано.
password-label = Пароль
password-show = Показати пароль
password-wrong = Неправильний пароль. Спробуйте ще раз.
password-open = Відкрити
password-cancel = Скасувати

## Налаштування

settings-viewing-animation-autoplay = Автоматично відтворювати анімацію
settings-viewing-animation-autoplay-help = Запускати анімовані зображення (GIF, APNG, WebP…) одразу після відкриття.
settings-viewing-animation-loop = Повторення анімації
settings-viewing-animation-loop-help = Скільки разів повторюється анімація.
settings-viewing-animation-loop-file = Як у файлі
settings-viewing-animation-loop-forever = Безкінечно
settings-viewing-animation-loop-once = Один раз
settings-viewing-animation-short-delays = Дуже короткі затримки кадрів
settings-viewing-animation-short-delays-help = Кадри тривалістю 10 мс або менше: сповільнити до 100 мс, як у веббраузерах, або відтворювати як записано.
settings-viewing-animation-short-delays-browser = Як у веббраузерах (100 мс)
settings-viewing-animation-short-delays-as-stored = Як записано
settings-browsing-reading-order = Порядок сторінок в архівах
settings-browsing-reading-order-help = Архіви коміксів читаються справа наліво, якщо так зазначено в їхньому ComicInfo.xml (манґа), якщо тут не задано інше.
settings-browsing-reading-order-auto = З архіву
settings-browsing-reading-order-left-to-right = Зліва направо
settings-browsing-reading-order-right-to-left = Справа наліво
settings-browsing-page-layout = Сторінки в архівах
settings-browsing-page-layout-help = Показувати одну сторінку або дві поруч; обкладинка завжди показується окремо.
settings-browsing-page-layout-auto = З архіву
settings-browsing-page-layout-single = Одна сторінка
settings-browsing-page-layout-spreads = Розвороти з двох сторінок

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Наступна сторінка
palette-prev-page = Попередня сторінка
palette-first-page = Перша сторінка
palette-last-page = Остання сторінка
disabled-one-page = Зображення має лише одну сторінку
page-indicator = Сторінка { $index } з { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Шар { $index }
page-label-face = Грань { $face }
page-label-sphere = Сферична карта
page-label-slice = Зріз { $index }
page-label-join = { $a } · { $b }
page-label-slide = Препарат
page-label-slide-label = Етикетка
page-label-slide-macro = Огляд
page-label-slide-thumbnail = Мініатюра
# machine-drafted, needs native review
page-label-thermal = Термограма
page-label-visible = Видиме фото
# machine-drafted, needs native review
page-label-left-view = Лівий вид
# machine-drafted, needs native review
page-label-right-view = Правий вид
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } з { $count }
page-label-t = T { $index } з { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Типові канали
page-label-product-bands = Усі канали
page-label-product-true-colour = Природні кольори
page-label-product-false-colour = Псевдокольори (ближній інфрачервоний)
page-label-product-swir = Короткохвильовий інфрачервоний
page-label-product-agriculture = Сільське господарство
page-label-product-scene-class = Класифікація сцени
page-label-product-cloud = Імовірність хмарності
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Сторінка
info-page-position = Позиція
info-page-contents = Вміст

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO не використовується: { $reason }
ocio-builtin-config = $OCIO вказує на вбудовану конфігурацію (ocio://); збережіть її як файл і виберіть цей файл.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits }-бітний

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Об’єднане зображення
page-label-from-layers = Складено з шарів
page-label-layer-hidden = { $name } (прихований)
page-label-layer-unnamed = Шар без назви { $index }
