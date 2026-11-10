# machine-drafted, needs native review
# Specular — вікно налаштувань. Українська.
#
# Схема ідентифікаторів, згенерована зі схеми налаштувань у crates/specular-settings:
#   settings-section-<section>              назва розділу
#   settings-<section>-<key>                мітка параметра
#   settings-<section>-<key>-help           довідковий текст під міткою
#   settings-<section>-<key>-<value>        один варіант параметра з вибором
# Ключі та значення — це назви з settings.toml, де «_» записано як «-».

## Елементи вікна

settings-title = Налаштування
settings-search-placeholder = Пошук налаштувань
settings-search-no-results = Немає параметрів, що відповідають { $query }
settings-reset = Скинути
settings-reset-section = Скинути розділ
settings-modified = Змінено
settings-default-value = Типово: { $value }
settings-restart-required = Набуде чинності після перезапуску
settings-restart-now = Перезапустити
settings-undo = Скасувати
settings-redo = Повторити
settings-value-on = Увімкнено
settings-value-off = Вимкнено
settings-value-auto = Автоматично
settings-value-system = Як у системі
settings-value-system-language = Мова системи
settings-value-follow-language = Як мова інтерфейсу
settings-value-default-location = Типове розташування
settings-choose = Вибрати…
settings-clear = Очистити
settings-file-reloaded = Файл settings.toml змінено поза { -app-name }, його перезавантажено.
settings-warning-unknown-key = Невідомий параметр { $key } у settings.toml. Він лишається у файлі, але ігнорується.
settings-warning-invalid-value = Неприпустиме значення для { $key }: { $reason } Натомість використано типове.
settings-warning-newer-version = Файл settings.toml записано новішою версією { -app-name }. Невідомі цій версії параметри збережено.
settings-warning-syntax = Не вдалося прочитати settings.toml (рядок { $line }): { $reason } Доки файл не виправлено, діють типові значення.

## Помилки перевірки (також виводить `specular --set`)

settings-error-unknown-key = Параметра { $key } не існує.
settings-error-unknown-key-suggestion = Параметра { $key } не існує. Можливо, ви мали на увазі { $suggestion }?
settings-error-not-bool = Очікується on або off (true або false).
settings-error-not-integer = Очікується ціле число.
settings-error-not-number = Очікується число.
settings-error-out-of-range = Має бути від { $min } до { $max }.
settings-error-out-of-range-auto = Має бути 0 (автоматично) або від { $min } до { $max }.
settings-error-unknown-option = Очікується одне з: { $options }.
settings-error-bad-colour = Очікується колір, напр. #1E1E1E або #1E1E1E80.
settings-error-bad-language = Очікується мовний тег, напр. de, pt-BR чи zh-Hans, або «system».
settings-error-bad-path = Очікується шлях до файлу або теки.
settings-error-not-settable = Цей пункт — команда, він не має значення.
settings-error-syntax = Очікується ключ=значення, напр. viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Не може містити табуляцій, розривів рядків чи інших керівних символів.
settings-error-invalid = { $reason }
settings-error-tile-url = Потрібна адреса http або https з {"{"}z{"}"}, {"{"}x{"}"} і {"{"}y{"}"}.

## Одиниці

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } мс
unit-seconds = { $value } с
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } зображення
        [few] { $value } зображення
        [many] { $value } зображень
       *[other] { $value } зображення
    }
unit-threads =
    { $value ->
        [one] { $value } потік
        [few] { $value } потоки
        [many] { $value } потоків
       *[other] { $value } потоку
    }
unit-bytes =
    { $value ->
        [one] { $value } байт
        [few] { $value } байти
        [many] { $value } байтів
       *[other] { $value } байта
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = мм
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = мм²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Розділи

settings-section-general = Загальні
settings-section-viewing = Перегляд
settings-section-browsing = Гортання
settings-section-colour = Колір
settings-section-metadata = Метадані й приватність
# machine-drafted, needs native review
settings-section-privacy = Приватність
settings-section-performance = Продуктивність
settings-section-accessibility = Доступність
settings-section-keys = Клавіші
settings-section-advanced = Додатково

## Загальні

settings-general-language = Мова інтерфейсу
settings-general-language-help = Зміни діють одразу. «Мова системи» відповідає мові вашої операційної системи.
settings-general-region-formats = Регіональні формати
settings-general-region-formats-help = Як записуються дати, час, числа й розміри. Типово — як у мові інтерфейсу.
settings-general-theme = Тема
settings-general-theme-help = Світлий або темний вигляд вікон і елементів керування. Зображення ніколи не змінюються.
settings-general-theme-system = Як у системі
settings-general-theme-light = Світла
settings-general-theme-dark = Темна
# machine-drafted, needs native review
settings-general-on-start = Коли { -app-name } відкривається без файлу
settings-general-on-start-help = Що показувати, коли { -app-name } відкрито без файлу. Файл, відкритий із файлового менеджера чи командного рядка, завжди показується в переглядачі.
settings-general-on-start-start-screen = Початковий екран
settings-general-on-start-gallery = Галерея останньої теки
settings-general-on-start-library = Бібліотека
settings-general-start-tips = Показувати «Варто спробувати» на початковому екрані
settings-general-start-tips-help = Ряд карток із корисними функціями та їхніми клавішами. «Показати вітання» в палітрі команд повертає його.
settings-general-tips = Показувати підказки
settings-general-tips-help = Коротка підказка, один раз, коли функція вперше може допомогти, як-от клавіша, що показує теку як галерею. Ніколи під час слайд-шоу.
settings-general-tips-shown = Уже показані підказки
settings-general-tips-shown-help = Одноразові підказки, які вже було показано, щоб вони не повторювалися.
# machine-drafted, needs native review
settings-general-your-name = Ваше ім’я
settings-general-your-name-help = Показується на штампах і позначках, які ви додаєте. Зберігається лише у файлах позначок, які ви створюєте.
settings-general-single-instance = Відкривати файли в наявному вікні
settings-general-single-instance-help = Якщо { -app-name } вже запущено, інше зображення відкривається в тому самому вікні, а не в новому.
settings-general-file-associations = Асоціації файлів…
settings-general-file-associations-help = Зробити { -app-name } типовим переглядачем для підтримуваних форматів зображень.

## Перегляд

settings-viewing-initial-zoom = Початковий масштаб
settings-viewing-initial-zoom-help = Яким завбільшки показувати щойно відкрите зображення.
settings-viewing-initial-zoom-fit = За розміром вікна
settings-viewing-initial-zoom-fit-if-larger = Великі — за вікном, малі — у { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Справжній розмір ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Заповнити вікно
settings-viewing-wheel-behaviour = Коліщатко миші
settings-viewing-wheel-behaviour-help = Що робить прокручування коліщатка. Утримуйте Ctrl для іншої дії.
settings-viewing-wheel-behaviour-help-macos = Що робить прокручування коліщатка. Утримуйте ⌘ для іншої дії.
settings-viewing-wheel-behaviour-zoom = Масштаб
settings-viewing-wheel-behaviour-navigate = Наступне й попереднє зображення
settings-viewing-keep-zoom = Зберігати масштаб між зображеннями
settings-viewing-keep-zoom-help = Коли зображення збільшено, наступне й попереднє відкриваються з тим самим масштабом і показують ту саму частину кадру: зручно для перевірки фокуса в серії.
settings-viewing-zoom-animation = Анімувати масштабування
settings-viewing-zoom-animation-help = Плавне масштабування замість стрибків. Вимикається автоматично, коли ввімкнено «Зменшити рух».
settings-viewing-zoom-animation-speed = Швидкість анімації масштабу
settings-viewing-zoom-animation-speed-help = Як швидко завершується анімоване масштабування.
settings-viewing-zoom-animation-speed-relaxed = Повільна
settings-viewing-zoom-animation-speed-normal = Звичайна
settings-viewing-zoom-animation-speed-fast = Швидка
settings-viewing-background = Тло
settings-viewing-background-help = Колір позаду зображення у вікні.
settings-viewing-background-theme = Як у темі
settings-viewing-background-black = Чорне
settings-viewing-background-dark-grey = Темно-сіре
settings-viewing-background-light-grey = Світло-сіре
settings-viewing-background-white = Біле
settings-viewing-background-custom = Власний колір
settings-viewing-background-color = Власний колір тла
settings-viewing-background-color-help = Використовується, коли для тла вибрано «Власний колір».
settings-viewing-fullscreen-background = Тло в повноекранному режимі
settings-viewing-fullscreen-background-help = Колір позаду зображення в повноекранному режимі.
settings-viewing-transparency = Прозорі ділянки
settings-viewing-transparency-help = Як малюються прозорі частини зображення.
settings-viewing-transparency-checkerboard = Шахівниця
settings-viewing-transparency-background = Колір тла
settings-viewing-transparency-black = Чорний
settings-viewing-transparency-white = Білий
settings-viewing-pixel-grid-threshold = Сітка пікселів від
settings-viewing-pixel-grid-threshold-help = Малювати сітку між пікселями, якщо масштаб не менший за цей.
settings-viewing-smoothing-high-zoom = Збільшені пікселі
settings-viewing-smoothing-high-zoom-help = Як виглядають пікселі, коли зображення показано більшим за { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Згладжено, різко від { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Завжди згладжено
settings-viewing-smoothing-high-zoom-pixelated = Завжди різкі квадрати
settings-viewing-double-click = Подвійне клацання на зображенні
settings-viewing-double-click-help = Подвійне клацання на порожньому тлі завжди перемикає повноекранний режим.
settings-viewing-double-click-toggle-zoom = Перемикати між «За вікном» і { PERCENT(100) }
settings-viewing-double-click-fullscreen = Перемкнути повноекранний режим
settings-viewing-double-click-nothing = Нічого не робити
settings-viewing-fullscreen-on-open = Відкривати зображення на весь екран
settings-viewing-fullscreen-on-open-help = Запускати в повноекранному режимі, коли зображення відкрито ззовні { -app-name }.
settings-viewing-fullscreen-mode = Стиль повноекранного режиму
settings-viewing-fullscreen-mode-help = Системний повноекранний режим використовує окремий простір; миттєвий покриває поточний без анімації.
settings-viewing-fullscreen-mode-native = Системний (окремий простір)
settings-viewing-fullscreen-mode-borderless = Миттєвий
settings-viewing-toolbar-autohide = Автоматично ховати панель інструментів
settings-viewing-toolbar-autohide-help = Панель інструментів і стрілки на краях зникають, коли вказівник нерухомий, і з’являються, коли він рухається.
settings-viewing-toolbar-autohide-delay = Ховати панель інструментів через
settings-viewing-toolbar-autohide-delay-help = Скільки часу вказівник має бути нерухомим, перш ніж панель зникне.
settings-viewing-toolbar-items = Кнопки панелі інструментів
settings-viewing-toolbar-items-help = Які кнопки показує панель інструментів і в якому порядку.
settings-viewing-toolbar-drag = Упорядковувати панель інструментів перетягуванням
settings-viewing-toolbar-drag-help = Перетягніть кнопку вздовж панелі інструментів, щоб перемістити її. Короткий клік і далі натискає її; Esc повертає перетягувану кнопку на місце.
settings-viewing-toolbar-items-open = Відкрити
settings-viewing-toolbar-items-prev = Попереднє зображення
settings-viewing-toolbar-items-next = Наступне зображення
settings-viewing-toolbar-items-zoom-out = Зменшити
settings-viewing-toolbar-items-zoom-level = Масштаб
settings-viewing-toolbar-items-zoom-in = Збільшити
settings-viewing-toolbar-items-fit = За розміром вікна
settings-viewing-toolbar-items-rotate-left = Повернути проти годинникової стрілки
settings-viewing-toolbar-items-rotate-right = Повернути за годинниковою стрілкою
settings-viewing-toolbar-items-info = Відомості
settings-viewing-toolbar-items-copy = Копіювати зображення
settings-viewing-toolbar-items-delete = Перемістити в кошик
settings-viewing-toolbar-items-palette = Палітра команд
settings-viewing-toolbar-items-settings = Налаштування
settings-viewing-toolbar-items-fullscreen = Повний екран
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Гнучкий проміжок
settings-viewing-toolbar-items-separator = Роздільник
settings-toolbar-preview = Попередній перегляд панелі
settings-toolbar-hidden = Приховані кнопки
settings-toolbar-add-spacer = Додати гнучкий проміжок
settings-toolbar-add-separator = Додати роздільник
settings-toolbar-reset = Скинути до типових
settings-toolbar-remove = Прибрати: { $item }
settings-toolbar-hint = Перетягніть рядок, щоб перемістити його, або виберіть його й натисніть Alt+↑ чи Alt+↓. Tab перемикає між полем пошуку та списком.
settings-toolbar-move-up = Перемістити вгору
settings-toolbar-move-down = Перемістити вниз
settings-toolbar-move-start = Перемістити на початок
settings-toolbar-move-end = Перемістити в кінець
settings-toolbar-menu-hide = Сховати
settings-toolbar-menu-show = Показати
settings-toolbar-menu-remove = Вилучити
settings-toolbar-row-menu = Інші дії: { $item }
settings-toolbar-position = Позиція { $index } з { $count }
settings-toolbar-hidden-state = Приховано

## Гортання

settings-browsing-sort-order = Сортувати зображення за
settings-browsing-sort-order-help = Порядок під час гортання теки. Сортування за назвою ставить «img2» перед «img10» і враховує абетку вашої мови.
settings-browsing-sort-order-natural = Назвою
settings-browsing-sort-order-date-taken = Датою зйомки
settings-browsing-sort-order-modified = Датою зміни
settings-browsing-sort-order-size = Розміром файлу
settings-browsing-sort-order-type = Типом файлу
settings-browsing-sort-descending = Зворотний порядок
settings-browsing-sort-descending-help = Сортувати від останнього до першого.
settings-browsing-loop-folder = Зациклювати в кінці теки
settings-browsing-loop-folder-help = Після останнього зображення переходити до першого, і навпаки.
settings-browsing-include-subfolders = Включати підтеки
settings-browsing-include-subfolders-help = Гортати також зображення в підтеках.
settings-browsing-hold-acceleration = Прискорення під час утримання
settings-browsing-hold-acceleration-help = Як швидко пришвидшується гортання, поки клавішу чи кнопку утримують.
settings-browsing-hold-acceleration-off = Без прискорення
settings-browsing-hold-acceleration-gentle = Плавне
settings-browsing-hold-acceleration-normal = Звичайне
settings-browsing-hold-acceleration-fast = Швидке
settings-browsing-confirm-trash = Питати перед переміщенням у кошик
settings-browsing-confirm-trash-help = Переміщення в кошик завжди можна скасувати, тому { -app-name } типово не питає.
settings-browsing-arrows-follow-reading-direction = Стрілки відповідають напрямку читання
settings-browsing-arrows-follow-reading-direction-help = У мовах із письмом справа наліво клавіша зі стрілкою вліво переходить до наступного зображення.

## Колір

settings-colour-management = Керування кольором
settings-colour-management-help = Показувати кольори такими, якими їх задумано, за колірним профілем зображення та профілем дисплея.
settings-colour-display-profile = Профіль дисплея
settings-colour-display-profile-help = Колірний профіль вашого дисплея. «Автоматично» використовує профіль, який повідомляє операційна система.
settings-colour-rendering-intent = Метод відтворення
settings-colour-rendering-intent-help = Як кольори, яких дисплей не може показати, переводяться в його діапазон.
settings-colour-rendering-intent-perceptual = Перцептивний
settings-colour-rendering-intent-relative-colorimetric = Відносний колориметричний
settings-colour-rendering-intent-saturation = Насиченість
settings-colour-rendering-intent-absolute-colorimetric = Абсолютний колориметричний
settings-colour-black-point-compensation = Компенсація точки чорного
settings-colour-black-point-compensation-help = Зберігати деталі в тінях, коли чорний колір дисплея світліший, ніж у зображенні.
settings-colour-untagged-images = Зображення без колірного профілю
settings-colour-untagged-images-help = Як показувати зображення, які не вказують, які кольори використовують.
settings-colour-untagged-images-srgb = Вважати sRGB
settings-colour-untagged-images-display = Показувати без змін
settings-colour-dithering = Дизеринг
settings-colour-dithering-help = Додавати дрібний синій шум під час зменшення глибини кольору, що запобігає смугам на небі й градієнтах.
settings-colour-hdr-output = Виведення HDR
settings-colour-hdr-output-help = Показувати зображення HDR з повною яскравістю на дисплеях, які це підтримують. «Автоматично» вмикає HDR, коли дисплей повідомляє про додаткову яскравість; інакше яскраві ділянки м’яко стискаються.
settings-colour-hdr-output-auto = Автоматично
settings-colour-hdr-output-on = Увімкнено
settings-colour-hdr-output-off = Вимкнено
# machine-drafted, needs native review
settings-colour-ocio-config = Конфігурація OCIO
settings-colour-ocio-config-help = Конфігурація OpenColorIO (.ocio) для зображень EXR, Radiance HDR і PFM. Якщо порожньо, використовується та, яку вказує $OCIO.
settings-colour-ocio-display = Дисплей OCIO
settings-colour-ocio-display-help = Дисплей конфігурації OCIO, що відповідає вашому монітору.
settings-colour-ocio-display-default = Перший у конфігурації
settings-colour-ocio-view = Вигляд OCIO
settings-colour-ocio-view-help = Як зображення scene-linear показуються на цьому дисплеї, наприклад вигляд із тональним відображенням.
settings-colour-ocio-view-default = Перший для дисплея
settings-colour-ocio-input = Вхідний колірний простір OCIO
settings-colour-ocio-input-help = Колірний простір значень зображень.
settings-colour-ocio-input-default = Роль scene-linear у конфігурації
# machine-drafted, needs native review
settings-colour-proof-profile = Профіль екранної проби
settings-colour-proof-profile-help = Профіль принтера, який екранна проба в Проявленні використала востаннє.
settings-colour-proof-intent = Метод рендерингу екранної проби
settings-colour-proof-intent-help = Як підганяються кольори, яких принтер не надрукує.
settings-colour-proof-intent-perceptual = Перцептивний
settings-colour-proof-intent-relative = Відносний колориметричний
settings-colour-proof-bpc = Компенсація точки чорного в пробі
settings-colour-proof-bpc-help = Відображає чорний на чорний принтера.
settings-colour-proof-paper = Білий папір у пробі
settings-colour-proof-paper-help = Показує власний колір паперу.
settings-colour-proof-ink = Чорне чорнило в пробі
settings-colour-proof-ink-help = Показує, наскільки темний чорний колір чорнила.

## Метадані й приватність

settings-metadata-show-info-on-open = Показувати панель відомостей під час відкриття
settings-metadata-show-info-on-open-help = Відкривати панель із даними про камеру, дату й місце щоразу, коли відкривається зображення.
settings-metadata-info-detail = Панель відомостей показує
settings-metadata-info-detail-help = Огляд пояснює найкорисніші дані простими словами; «Усі поля» показує все, що збережено у файлі.
settings-metadata-info-detail-summary = Огляд
settings-metadata-info-detail-all = Усі поля
settings-metadata-reverse-geocoding = Показувати назви місць для GPS-позицій
settings-metadata-reverse-geocoding-help = Знаходити найближчий населений пункт за вбудованими даними. Працює офлайн: позиції ніколи не залишають ваш комп’ютер.
settings-metadata-hide-gps = Приховувати GPS-позиції на екрані
settings-metadata-hide-gps-help = Не показувати координати й місця, напр. під час демонстрації екрана. Файли не змінюються.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Зберігати зміни проявлення поруч із фото
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Записує IMG_1234.CR2.specular.xmp поруч із файлом, тож зміни переміщуються разом із ним. Вимкнено: зміни лишаються тільки в бібліотеці.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Імпортувати налаштування Lightroom у Проявленні
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Фото з налаштуваннями Lightroom і без обробки відкривається з ними, приблизно. Вимкнено: Проявлення пропонує кнопку.
settings-metadata-units = Одиниці
settings-metadata-units-help = Одиниці для висоти, відстаней і швидкостей.
settings-metadata-units-auto = Як у регіональних форматах
settings-metadata-units-metric = Метричні
settings-metadata-units-imperial = Імперські
# machine-drafted, needs native review
settings-metadata-basemap = Базова мапа
settings-metadata-basemap-help = Мапа під місцями фото. Онлайн-мапи завантажують плитки для області, яку ви переглядаєте.
settings-metadata-basemap-off = Лише вбудована мапа (офлайн)
settings-metadata-basemap-openstreetmap = OpenStreetMap (онлайн)
settings-metadata-basemap-custom = Інший сервер плиток (онлайн)
settings-metadata-basemap-file = Власний файл мапи
settings-metadata-basemap-url = Адреса сервера плиток
settings-metadata-basemap-url-help = Адреса з {"{"}z{"}"}, {"{"}x{"}"} і {"{"}y{"}"}, напр. https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Файл мапи
settings-metadata-basemap-path-help = Файл MBTiles або PMTiles. Працює офлайн.
settings-metadata-basemap-cache = Кеш плиток мапи
settings-metadata-basemap-cache-help = Місце на диску для завантажених плиток. Найстаріші вилучаються першими.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Дозволені сервери плиток
settings-metadata-basemap-allowed-help = Сервери мап, з яких ви погодилися завантажувати плитки.

## Продуктивність

settings-performance-ram-cache = Кеш у пам’яті
settings-performance-ram-cache-help = Пам’ять для зберігання готових декодованих зображень. «Автоматично» вибирає обсяг залежно від пам’яті комп’ютера.
settings-performance-vram-cache = Кеш у пам’яті GPU
settings-performance-vram-cache-help = Пам’ять GPU для зображень і плиток. «Автоматично» вибирає обсяг залежно від GPU.
settings-performance-prefetch-depth = Зображень для підготовки наперед
settings-performance-prefetch-depth-help = Скільки зображень у напрямку гортання декодується заздалегідь.
settings-performance-decoder-threads = Потоки декодера
settings-performance-decoder-threads-help = Потоки CPU для декодування зображень. «Автоматично» використовує всі продуктивні ядра й підлаштовується під файл.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Який GPU використовувати, якщо їх у комп’ютері кілька.
settings-performance-gpu-adapter-high-performance = Найвища продуктивність
settings-performance-gpu-adapter-low-power = Найменше енергоспоживання
settings-performance-processing = Де обробляти зображення
settings-performance-processing-help = Частина роботи може виконуватися на CPU, GPU або апаратному декодері. «Автоматично» вимірює цей комп’ютер і вибирає найшвидший спосіб для зображення, яке ви переглядаєте, та найекономніший для фонової роботи. Що вибрано на цьому комп’ютері й чому, показано нижче.
settings-performance-processing-automatic = Автоматично
settings-performance-processing-prefer-gpu = Надавати перевагу GPU
settings-performance-processing-cpu-only = Лише CPU
settings-performance-drag-preview = Перегляд під час перетягування
settings-performance-drag-preview-help = Як проявлення показує фото, поки ви тягнете повзунок. Автоматично бере менше зображення лише тоді, коли комп’ютер не встигає.
settings-performance-drag-preview-automatic = Автоматично
settings-performance-drag-preview-sharp = Завжди чітко
settings-performance-drag-preview-fast = Завжди швидко

## Налаштування → Продуктивність → Як { -app-name } працює на цьому комп’ютері (вибір шляхів,
## DESIGN.md §12.3.1). $engine і $other — назви hw-report-engine-* (з малої літери, у називному
## відмінку посеред речення); paths-engine-* — окремі назви.
settings-paths-title = Як { -app-name } працює на цьому комп’ютері
settings-paths-intro = { -app-name } вимірює час кожного способу виконання цих завдань на цьому комп’ютері й використовує найкращий. Час наведено для фото на { $mpx } Мп.
settings-paths-intro-measuring = Вимірювання ще не завершено: { -app-name } вимірює, коли комп’ютер простоює.
settings-paths-intro-battery = Живлення від батареї: перевагу надано меншому енергоспоживанню.
settings-paths-keywords = GPU CPU NPU відеокарта графічний процесор апаратне прискорення декодер тест швидкість продуктивність
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, усі ядра
paths-engine-gpu = GPU
paths-engine-hw-decoder = Апаратний декодер
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (ваш вибір)
settings-paths-measured-faster = Виміряно на цьому комп’ютері: у { $ratio } раза швидше, ніж { $other }.
settings-paths-measured-similar = Виміряно на цьому комп’ютері: приблизно так само швидко, як { $other }.
settings-paths-measured-power = Від батареї цей спосіб витрачає найменше енергії (виміряно на цьому комп’ютері).
settings-paths-not-measured = Ще не виміряно: звичайний вибір для такого типу комп’ютерів.
settings-paths-set-by = Вибрано вашим параметром «{ $setting }: { $option }».
settings-paths-blocked-check = { $engine }: вимкнено { $date }, бо результат не збігся з результатом CPU.
settings-paths-blocked-error = { $engine }: вимкнено { $date } після помилки.
settings-paths-blocked-lost = { $engine }: вимкнено { $date }, бо пристрій перестав відповідати.
settings-paths-blocked-undated = { $engine }: вимкнено після помилки.
settings-paths-checking = { $engine }: перед використанням для поточного зображення результат звіряється з CPU.
settings-paths-exploring = { $engine }: зараз триває пробне використання у фоні.
settings-paths-unavailable = На цьому комп’ютері для цього немає такого пристрою: { $engine }.
settings-paths-background-cheaper = У фоні: { $engine }; за вимірюваннями, на { $percent } менше часу CPU.
settings-paths-background-power = У фоні: { $engine }, для економії енергії.
settings-paths-background = У фоні: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } мс, { $cpu } мс часу CPU, { $runs ->
    [one] { $runs } вимірювання
    [few] { $runs } вимірювання
    [many] { $runs } вимірювань
   *[other] { $runs } вимірювання
} ({ $confidence })
settings-paths-detail-guess = { $engine }: близько { $time } мс (оцінка)
settings-paths-confidence-low = низька достовірність
settings-paths-confidence-medium = середня достовірність
settings-paths-confidence-high = висока достовірність
settings-paths-machine = Виміряно для { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Спробувати ще раз
settings-paths-measure-again = Виміряти знову
settings-paths-measure-again-question = Забути результати вимірювань на цьому комп’ютері, зокрема вимкнені шляхи, і виміряти знову, коли комп’ютер простоюватиме?
settings-paths-measure-again-confirm = Забути вимірювання
settings-performance-display-cache = Зберігати кеш відображення на диску
settings-performance-display-cache-help = Зберігати копії зображень розміром з екран, щоб переглянуті теки відкривалися миттєво.
settings-performance-display-cache-size = Розмір кешу відображення
settings-performance-display-cache-size-help = Коли кеш заповнено, найстаріші записи вилучаються.
settings-performance-display-cache-location = Розташування кешу відображення
settings-performance-display-cache-location-help = Найкраще підходить тека на швидкому диску.
settings-performance-fast-simd = Використовувати інструкції SIMD
settings-performance-fast-simd-help = Використовувати векторні інструкції CPU для декодування. Вимикайте лише для діагностики проблем.
settings-performance-fast-dct-scaling = Декодувати в розмірі екрана
settings-performance-fast-dct-scaling-help = Декодувати великі JPEG одразу в розмірі показу. Вимикайте лише для діагностики проблем.
settings-performance-fast-parallel-decode = Декодувати на кількох ядрах
settings-performance-fast-parallel-decode-help = Розподіляти великі зображення між ядрами CPU. Вимикайте лише для діагностики проблем.
settings-performance-fast-restart-index = Індексувати великі JPEG для масштабування
settings-performance-fast-restart-index-help = Запам’ятовувати, де починається кожна частина JPEG, щоб збільшені ділянки декодувалися швидко. Вимикайте лише для діагностики проблем.
settings-performance-fast-yuv-upload = Перетворювати кольори на GPU
settings-performance-fast-yuv-upload-help = Перетворювати кольори JPEG на GPU або CPU. «Автоматично» вибирає швидший варіант на цьому комп’ютері. Вимикайте лише для діагностики проблем.
settings-performance-fast-yuv-upload-auto = Автоматично
settings-performance-fast-yuv-upload-on = Увімкнено
settings-performance-fast-yuv-upload-off = Вимкнено
settings-performance-fast-async-io = Асинхронне читання файлів
settings-performance-fast-async-io-help = Читати наперед через найшвидший файловий інтерфейс операційної системи. Вимикайте лише для діагностики проблем.
settings-performance-fast-mmap = Відображення файлів у пам’ять
settings-performance-fast-mmap-help = Читати файли без копіювання. Вимикайте лише для діагностики проблем.
settings-performance-hardware-decode = Апаратне декодування
settings-performance-hardware-decode-help = Дозволити вбудованому декодеру GPU відкривати фотографії JPEG. «Автоматично» використовує його там, де це допомагає, зазвичай для зображень, що готуються у фоні. Спершу його звіряють зі звичайним декодером, а все, з чим він не впорається, відкривається звичайним способом.
settings-performance-hardware-decode-auto = Автоматично
settings-performance-hardware-decode-on = Увімкнено
settings-performance-hardware-decode-off = Вимкнено
settings-performance-frame-time-hud = HUD часу кадру
settings-performance-frame-time-hud-help = Показувати час декодування й кадру та задіяні швидкі шляхи.
settings-performance-stall-warning = Повідомляти про завислі завантаження через
settings-performance-stall-warning-help = Якщо зображення досі завантажується після цього часу, записати стан завантаження й завдання в черзі до журналу (стандартний потік помилок або SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Поради щодо продуктивності
settings-performance-insights-help = Коли зображення раз у раз відкриваються повільно з тієї самої причини, показувати в куті «Чому це повільно?» з тим, що допомогло б. Зібрані дані залишаються на цьому комп’ютері.
settings-performance-insights-storage = Поради щодо повільного сховища
settings-performance-insights-storage-help = Показувати поради, коли читання файлів (із мережевої папки, повільного диска чи архівів) раз у раз затримує відкриття зображень.
settings-performance-insights-decoder = Поради щодо декодування
settings-performance-insights-decoder-help = Показувати поради, коли зображення раз у раз декодуються на CPU, хоча апаратний декодер міг би допомогти.
# machine-drafted, needs native review
settings-performance-insights-restore = Знову показати приховані поради
settings-performance-insights-restore-help = Повертає поради, які ви приховали кнопкою × у «Чому це повільно?», і для відкривання зображень, і для мініатюр. Недоступно, поки нічого не приховано.

## Доступність

settings-accessibility-reduce-motion = Зменшити рух
settings-accessibility-reduce-motion-help = Замінити анімації масштабування, зсуву й згасання миттєвими змінами.
settings-accessibility-reduce-motion-system = Як у системі
settings-accessibility-reduce-motion-on = Увімкнено
settings-accessibility-reduce-motion-off = Вимкнено
settings-accessibility-increase-contrast = Підвищити контрастність
settings-accessibility-increase-contrast-help = Чіткіші межі й текст, менше прозорості.
settings-accessibility-increase-contrast-system = Як у системі
settings-accessibility-increase-contrast-on = Увімкнено
settings-accessibility-increase-contrast-off = Вимкнено
settings-accessibility-text-scale = Розмір тексту
settings-accessibility-text-scale-help = Розмір тексту відносно стандартного.
settings-accessibility-ui-scale = Масштаб інтерфейсу
settings-accessibility-ui-scale-help = Розмір усього інтерфейсу, додатково до масштабування дисплея в операційній системі.
settings-accessibility-focus-ring-always = Завжди показувати рамку фокуса
settings-accessibility-focus-ring-always-help = Показувати, який елемент має фокус клавіатури, навіть під час роботи з мишею.
settings-accessibility-screen-reader-verbosity = Докладність екранного читача
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Скільки оголошує екранний читач під час переходу між зображеннями і скільки з показань під курсором він озвучує.
settings-accessibility-screen-reader-verbosity-concise = Стисло
settings-accessibility-screen-reader-verbosity-normal = Звичайно
settings-accessibility-screen-reader-verbosity-verbose = Докладно
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Читати значення під курсором
settings-accessibility-announce-cursor-help = Екранний читач озвучує значення пікселя, HU, температуру або позицію під курсором чи клавіатурним прицілом, коли він перестає рухатися.

## Приватність

# machine-drafted, needs native review
settings-privacy-no-network = Без мережі
# machine-drafted, needs native review
settings-privacy-no-network-help = Вимкнено: з’єднуються лише ці функції, кожна лише після того, як ви її ввімкнете або щось нею відкриєте: перевірка оновлень, завантаження моделей локального ШІ, онлайн-карти, зображення з вебадреси чи хмарного сховища та додані вами сервери (DICOMweb, OMERO, карти). Увімкнено: ніщо не з’єднується.

## Додатково

settings-advanced-portable-mode = Портативний режим
settings-advanced-portable-mode-help = Зберігати налаштування й кеші в теці поруч із програмою, а не в профілі користувача.
settings-advanced-log-level = Докладність журналу
settings-advanced-log-level-help = Скільки { -app-name } записує до свого журналу.
settings-advanced-log-level-error = Лише помилки
settings-advanced-log-level-warn = Попередження
settings-advanced-log-level-info = Відомості
settings-advanced-log-level-debug = Налагодження
settings-advanced-log-level-trace = Усе
settings-advanced-developer-mode = Режим розробника
settings-advanced-developer-mode-help = Показувати інструменти для розробки й тестування { -app-name }, як-от псевдомови в списку мов.
# machine-drafted, needs native review
settings-advanced-audit-log = Локальний журнал аудиту
# machine-drafted, needs native review
settings-advanced-audit-log-help = Записувати відкриті медичні зображення, експорт, анонімізацію, друк і надсилання в PACS в audit.jsonl у теці даних. Журнал залишається на цьому комп’ютері.
settings-advanced-hw-report = Звіт про обладнання…
settings-advanced-hw-report-help = Показати, які можливості CPU і GPU виявила програма { -app-name } і які швидкі шляхи вона використовує.
# machine-drafted, needs native review
settings-advanced-renderer-info = Відомості про рендерер…
settings-advanced-renderer-info-help = Показує графічний адаптер, драйвер, формати відображення й обмеження пам’яті, які використовує { -app-name }, готові для копіювання у звіт про помилку.
settings-advanced-edit-settings-file = Редагувати settings.toml
settings-advanced-edit-settings-file-help = Редагуйте файл налаштувань у вбудованому редакторі { -app-name }. Він перевіряє файл під час введення і зберігає його, лише коли в ньому немає помилок.
settings-advanced-import = Імпортувати налаштування…
settings-advanced-import-help = Завантажити налаштування з файлу, експортованого з { -app-name }.
settings-advanced-export = Експортувати налаштування…
settings-advanced-export-help = Зберегти всі налаштування в один файл — для резервної копії або іншого комп’ютера.
settings-advanced-reset-all = Скинути всі налаштування…
settings-advanced-reset-all-help = Повернути кожному параметру типове значення. Це можна скасувати.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Зберегти
config-editor-revert = Повернути
config-editor-close = Закрити
config-editor-modified = Змінено
config-editor-text = Файл налаштувань
config-editor-position = Рядок { $line }, стовпець { $column }
config-editor-saved = settings.toml збережено й застосовано.
config-editor-cant-save = Не вдається зберегти. Проблем до виправлення: { $count }.
config-editor-save-failed = Не вдалося зберегти settings.toml: { $reason }
config-editor-conflict = settings.toml змінено деінде.
config-editor-reload = Перезавантажити
config-editor-keep-mine = Залишити мої
config-editor-discard-question = Відкинути незбережені зміни в settings.toml?
config-editor-discard = Відкинути
config-editor-keep-editing = Продовжити редагування
config-editor-find-placeholder = Знайти у файлі
config-editor-find-count = { $current } з { $total }
config-editor-find-none = Немає збігів
config-editor-find-previous = Попередній збіг
config-editor-find-next = Наступний збіг
config-editor-find-close = Закрити пошук
config-editor-problems = Проблеми
config-editor-problems-count =
    { $count ->
        [0] Проблем немає
       *[other] Проблеми: { $count }
    }
config-editor-no-problems = Проблем немає. Файл можна зберегти.
config-editor-problem-at = Рядок { $line }: { $message }
config-editor-severity-error = Помилка
config-editor-severity-warning = Попередження
config-editor-severity-info = Примітка
config-editor-problem-syntax = Некоректний TOML: { $reason }
config-editor-problem-duplicate = { $key } задано двічі в одній таблиці. Видаліть одне.
config-editor-problem-unknown-section = Розділу налаштувань [{ $key }] не існує.
config-editor-problem-unknown-section-suggestion = Розділу налаштувань [{ $key }] не існує. Можливо, ви мали на увазі [{ $suggestion }]?
config-editor-problem-unknown-key = Налаштування { $key } не існує.
config-editor-problem-unknown-key-suggestion = Налаштування { $key } не існує. Можливо, ви мали на увазі { $suggestion }?
config-editor-problem-invalid = Некоректне значення для { $key }: { $reason }
config-editor-problem-migrated = { $key } записано так, як записувала старіша версія { -app-name }. Під час читання воно перетворюється.
config-editor-problem-newer-version = Цей файл записано новішою версією { -app-name } (формат { $found }; ця версія читає { $supported }). Невідомі налаштування зберігаються.
config-editor-problem-command = Команда { $index } у [[commands]]: { $reason }
config-editor-problem-hook = Хук { $index } у [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Файл призначень клавіш
config-editor-keymap-saved = keymap.toml збережено й застосовано.
config-editor-keymap-save-failed = Не вдається зберегти keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml змінено деінде.
config-editor-keymap-discard-question = Відкинути незбережені зміни в keymap.toml?
config-editor-new-file = новий файл, буде створено під час збереження
config-editor-no-folder = немає теки налаштувань, щоб зберегти його.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Скинути всі налаштування до типових значень?
settings-reset-all-confirm-detail = Це стосується всіх розділів Налаштувань. Згодом це можна скасувати.
settings-reset-all-confirm-ok = Скинути все
settings-reset-all-done = Усі налаштування скинуто до типових значень.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Апаратний кодер

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = { $engine }: перед використанням результат звіряється з CPU.
settings-paths-unit-thumbnail = Час наведено для однієї мініатюри { $size }
settings-paths-unit-frame = Час наведено на один відеокадр { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Тека калібрувальних кадрів
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flat- і dark-кадри для Проявлення. Назви файлів або тек містять «flat» чи «dark».

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Білий
settings-colour-light-grey = Світло-сірий
settings-colour-grey = Сірий
settings-colour-dark-grey = Темно-сірий
settings-colour-black = Чорний
settings-colour-yellow = Жовтий
settings-colour-orange = Помаранчевий
settings-colour-red = Червоний
settings-colour-magenta = Пурпуровий
settings-colour-blue = Синій
settings-colour-cyan = Блакитний
settings-colour-green = Зелений
