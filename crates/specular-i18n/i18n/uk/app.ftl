# machine-drafted, needs native review
# Specular — вікно переглядача, палітра команд, помилки, вікно «Про програму». Українська.

-app-name = Specular

## Підказки та мітки доступності панелі інструментів

toolbar-open = Відкрити зображення…
toolbar-prev = Попереднє зображення
toolbar-next = Наступне зображення
toolbar-back-gallery = Назад до галереї
toolbar-zoom-out = Зменшити
toolbar-zoom-in = Збільшити
toolbar-zoom-level = Масштаб і стандартні значення
toolbar-zoom-fit = За розміром вікна
toolbar-zoom-100 = Справжній розмір ({ PERCENT(100) })
toolbar-rotate-left = Повернути проти годинникової стрілки
toolbar-rotate-right = Повернути за годинниковою стрілкою
toolbar-fullscreen-enter = Повноекранний режим
toolbar-fullscreen-exit = Вийти з повноекранного режиму
toolbar-info = Відомості про зображення
toolbar-settings = Налаштування
toolbar-delete = Перемістити в кошик
toolbar-copy = Копіювати зображення
toolbar-palette = Палітра команд
toolbar-go-to = Перейти до зображення…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Налаштувати панель інструментів…
toolbar-more = Більше
toolbar-announce-moved = { $item }: позиція { $index } з { $count }
toolbar-announce-added = Додано { $item }: позиція { $index } з { $count }
toolbar-announce-removed = Прибрано з панелі інструментів: { $item }
toolbar-announce-reset = Панель інструментів скинуто до типової
# Мітка підказки, за якою йде її клавіатурне скорочення.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Підказка елемента, який зараз недоступний; $reason — одне з повідомлень disabled-*.
tooltip-unavailable = { $label } (недоступно: { $reason })
# Чому елемент або команда недоступні (підказки, палітра команд, екранні читачі).
disabled-no-image = Жодне зображення не відкрито
disabled-still-loading = Зображення ще завантажується
disabled-decode-failed = Не вдалося декодувати це зображення
disabled-no-other-images = У цій теці немає інших зображень

## Показники

# Позиція в теці, напр. «12 / 348».
readout-position = { $index } / { $count }
# Озвучується екранними читачами для показника позиції.
readout-position-a11y = Зображення { $index } з { $count }
# $zoom — уже відформатований відсоток, напр. «150%».
readout-zoom = { $zoom }
readout-zoom-a11y = Масштаб { $zoom }
readout-zoom-fit = За розміром ({ $zoom })
zoom-preset-fit = За розміром вікна
zoom-preset-fill = Заповнити вікно
zoom-preset-actual = Справжній розмір
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Повноекранний режим
# $zoom — уже відформатований відсоток.
zoom-preset-percent = { $zoom }
# Розміри зображення в пікселях, напр. «6000 × 4000».
readout-dimensions = { $width } × { $height }
# Кількість зображень у поточній теці.
status-image-count =
    { $count ->
        [one] { $count } зображення
        [few] { $count } зображення
        [many] { $count } зображень
       *[other] { $count } зображення
    }

## Палітра команд

palette-placeholder = Введіть команду або параметр…
palette-no-results = Немає відповідних команд
# Зміна параметра з палітри, напр. «Задати: Коліщатко миші → Наступне зображення».
palette-set-setting = Задати: { $setting } → { $value }
palette-next = Наступне зображення
palette-prev = Попереднє зображення
palette-first = Перше зображення
palette-last = Останнє зображення
palette-go-to = Перейти до зображення…
palette-zoom-in = Збільшити
palette-zoom-out = Зменшити
palette-zoom-fit = Масштаб за розміром вікна
palette-zoom-fill = Масштаб із заповненням
# machine-drafted, needs native review
palette-zoom-fit-width = Масштаб за шириною
palette-zoom-fit-height = Масштаб за висотою
palette-zoom-100 = Масштаб { PERCENT(100) }
palette-zoom-200 = Масштаб { PERCENT(200) }
palette-zoom-300 = Масштаб { PERCENT(300) }
palette-zoom-400 = Масштаб { PERCENT(400) }
palette-zoom-rectangle = Масштаб за виділенням
palette-rotate-left = Повернути проти годинникової стрілки
palette-rotate-right = Повернути за годинниковою стрілкою
palette-flip-horizontal = Віддзеркалити по горизонталі
palette-flip-vertical = Віддзеркалити по вертикалі
palette-toggle-fullscreen = Перемкнути повноекранний режим
palette-navigator = Показати або сховати навігатор
palette-loupe = Лупа
palette-pixel-grid = Показати або сховати сітку пікселів
palette-info-panel = Показати або сховати панель відомостей
palette-copy-image = Копіювати зображення
# machine-drafted, needs native review
palette-read-codes = Прочитати коди
# machine-drafted, needs native review
palette-find-text = Знайти текст
# machine-drafted, needs native review
palette-face-boxes = Показати обличчя
palette-copy-path = Копіювати шлях до файлу
palette-move-to-trash = Перемістити в кошик
palette-undo-trash = Скасувати переміщення в кошик
palette-open = Відкрити зображення…
# machine-drafted, needs native review
palette-open-address = Відкрити адресу…
palette-settings = Налаштування
# machine-drafted, needs native review
palette-customise-toolbar = Налаштувати панель інструментів…
palette-cheat-sheet = Клавіатурні скорочення
palette-hud = Показати або сховати HUD часу кадру
palette-language = Мова
palette-theme = Тема
# machine-drafted, needs native review
palette-interface-larger = Більший інтерфейс
palette-interface-smaller = Менший інтерфейс
palette-interface-reset = Інтерфейс на { PERCENT(100) }
palette-quit = Вийти з { -app-name }

## Довідка зі скорочень

cheatsheet-title = Клавіатурні скорочення
cheatsheet-navigation = Навігація
cheatsheet-zoom-pan = Масштаб і прокручування
cheatsheet-view = Вигляд
cheatsheet-file = Файл
cheatsheet-window = Вікно
cheatsheet-help = Довідка
cheatsheet-hold = Утримувати { $key }
cheatsheet-close-hint = Натисніть { $key } або Esc, щоб закрити

## Помилки

# machine-drafted, needs native review
viewer-no-preview = У файлі немає попереднього перегляду.
error-cannot-open = Не вдалося відкрити { $file }
error-cannot-open-reason = Не вдалося відкрити { $file }: { $reason }
error-unsupported-format = Формат файлу { $file } поки що не підтримується в { -app-name }.
error-file-truncated = Файл { $file } неповний. Показано частину, яку вдалося прочитати.
error-file-too-large = Файл { $file } завеликий ({ $megapixels } Мп; обмеження — { $limit } Мп).
error-not-found = Файл { $file } більше не існує.
error-decoder-stopped = декодер зупинився, не створивши зображення
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = потрібна поверхня мозку (файл .surf.gii з { $vertices } вершинами) у тій самій теці
error-permission-denied = Немає дозволу на читання { $file }.
error-details = Подробиці
crash-last-time = Минулого разу { -app-name } неочікувано закрився; звіт збережено в { $path }
error-trash-failed = Не вдалося перемістити { $file } у кошик: { $reason }
# machine-drafted, needs native review
# Причини, що показуються після «Не вдалося відкрити ФАЙЛ:» та інших повідомлень «Не вдалося …:»,
# тому починаються з малої літери.
error-reason-not-found = його там уже немає. Можливо, його перемістили, перейменували чи видалили.
error-reason-permission = у вас немає дозволу на його читання. Перевірте власника й дозволи.
error-reason-disk-full = диск заповнено. Звільніть трохи місця й спробуйте ще раз.
error-reason-read-only = диск доступний лише для читання.
error-reason-truncated = файл неповний. Можливо, він ще копіюється чи завантажується.
error-reason-is-folder = це тека, а не файл.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = сервер відмовив у доступі. Дані можуть бути приватними; для кошика S3 спершу налаштуйте облікові дані AWS.
error-reason-remote-missing = за цією адресою немає даних Zarr.
# $status is an HTTP status code ("500").
error-reason-remote-status = сервер відповів помилкою { $status }.
error-reason-offline = сервер недоступний. Перевірте адресу й з’єднання.
# machine-drafted, needs native review
error-reason-network-off = мережу вимкнено в Налаштування › Приватність.
error-reason-timed-out = диск або сервер не відповів вчасно. Перевірте з’єднання й спробуйте ще раз.
error-reason-memory = бракує пам’яті. Закрийте інші зображення чи програми й спробуйте ще раз.
error-reason-unsupported = він у форматі, який { -app-name } поки не вміє читати.
# $detail — власні слова декодера, технічні й англійською («12-bit lossless JPEG»).
error-reason-unsupported-detail = він використовує можливість, яку { -app-name } поки не вміє читати ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = він не є жодним із файлів .{ $ext }, які читає { -app-name } ({ $formats }).
error-reason-damaged = файл пошкоджено, або він не той, що вказує його назва ({ $detail }).
error-reason-too-large = він завеликий, щоб його відкрити.
# machine-drafted, needs native review
error-reason-needs-memory = йому потрібно { $needed } вільної оперативної пам’яті (RAM), але зараз вільно лише { $free }. Закрийте деякі інші програми й спробуйте ще раз або відкрийте його на комп’ютері з більшим обсягом пам’яті.
error-needs-memory = Цьому зображенню потрібно { $needed } вільної оперативної пам’яті (RAM), але зараз вільно лише { $free }. Закрийте деякі інші програми й спробуйте ще раз або відкрийте його на комп’ютері з більшим обсягом пам’яті.
error-needs-memory-short = Потрібно { $needed } вільної RAM
error-reason-cancelled = відкриття зупинено.
error-reason-trash-access = його там уже немає, або у вас немає дозволу його змінювати.
error-reason-restore-collision = у його теці вже є файл із такою самою назвою.
error-reason-trash-root = цілий диск не можна перемістити в кошик.
error-reason-not-in-trash = його вже немає в кошику.
error-reason-program-missing = програму не встановлено, або її переміщено. Виберіть її знову чи перевстановіть.
error-reason-program-not-allowed = програмі не дозволено запускатися. Перевірте, чи вона виконувана.
# machine-drafted, needs native review
error-reason-permission-write = у вас немає дозволу його змінювати. Перевірте власника й дозволи.
error-reason-file-has-errors = він містить помилки. Спершу виправте їх, щоб нічого в ньому не втратити.
error-reason-not-dicom = це не файл DICOM.
error-reason-no-image = він не містить зображення.
error-reason-needs-layout = це об’єм без заголовка. Щоб відкрити його, потрібні розмір, тип відліків і порядок байтів.
error-reason-not-archive = це не архів, який { -app-name } уміє читати.
error-reason-needs-sandbox = такий архів відкриває лише захищений зчитувач { -app-name }, який недоступний.
error-reason-encrypted = він зашифрований, і потрібен пароль.
error-reason-wrong-password = пароль неправильний.
error-reason-checksum = файл пошкоджено: його вміст не відповідає контрольній сумі.
error-reason-missing-entry = у ньому бракує файлу.
error-reason-busy = його використовує інша програма. Закрийте його там і спробуйте ще раз.
error-reason-catalogue-damaged = файл каталогу пошкоджено.
error-reason-cannot-open = не вдалося відкрити його файл. Перевірте, що його тека існує і що ви можете в неї записувати.
error-reason-newer-version = його записала новіша версія { -app-name }. Оновіть { -app-name }, щоб змінити його.
error-reason-internal = усередині { -app-name } щось пішло не так. Подробиці — у журналі.
error-reason-volume-too-few = { $count ->
    [one] { $count } зріз — замало, щоб побудувати об’єм.
    [few] { $count } зрізи — замало, щоб побудувати об’єм.
    [many] { $count } зрізів — замало, щоб побудувати об’єм.
   *[other] { $count } зрізу — замало, щоб побудувати об’єм.
}
error-reason-volume-no-geometry = зріз не вказує, де він розташований і куди спрямований.
error-reason-volume-not-parallel = зрізи не паралельні.
error-reason-volume-shared-position = кілька зображень мають спільну кожну з позицій (фази, луни чи часовий ряд); позицій: { $count }.
error-reason-volume-missing = бракує зрізів, приблизно: { $count }.
error-reason-volume-mismatch = зрізи відрізняються розміром або кроком пікселів.
error-reason-volume-not-grey = зрізи кольорові, а не у відтінках сірого.
error-reason-volume-skewed = осі об’єму не перпендикулярні.
error-reason-volume-too-large = він потребує { $need } МіБ, а дозволено лише { $budget } МіБ.
error-reason-url-not-specular = це не посилання { -app-name }.
error-reason-url-too-long = воно задовге.
error-reason-url-damaged = воно пошкоджене. Можливо, його обрізали чи змінили під час копіювання.
error-reason-url-unknown = воно запитує те, чого ця версія { -app-name } не знає. Оновлення { -app-name } може допомогти.
error-reason-url-not-absolute = шляхи до файлів мають бути повними.
error-reason-url-no-paths = воно не вказує жодного файлу.
error-reason-url-too-many = воно вказує забагато файлів.

## Сповіщення

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Не вдалося завершити: { $reason }
toast-moved-to-trash = Переміщено в кошик
toast-moved-to-trash-named = { $file } переміщено в кошик
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } зображення переміщено в кошик
        [few] { $count } зображення переміщено в кошик
        [many] { $count } зображень переміщено в кошик
       *[other] { $count } зображення переміщено в кошик
    }
toast-undo = Скасувати
toast-restored = { $file } відновлено
toast-copied-image = Зображення скопійовано
toast-copied-path = Шлях скопійовано
toast-copied-report = Звіт скопійовано
toast-copied-value = Значення скопійовано
toast-loop-start = Знову перше зображення
toast-loop-end = Перехід до останнього зображення
# machine-drafted, needs native review
toast-interface-size = Розмір інтерфейсу { PERCENT($percent) }
toast-language-changed = Мову змінено на { $language }

## Порожні стани

empty-drop = Перетягніть сюди зображення або натисніть { $shortcut }
empty-open-button = Відкрити зображення…
empty-drop-release = Відпустіть, щоб відкрити
empty-folder = У цій теці немає зображень, які може відкрити { -app-name }.
# machine-drafted, needs native review
# Під «Відкрити зображення…» в порожньому вікні: де знайти все інше.
empty-hint-keys = Натисніть { $cheatsheet }, щоб побачити клавіатурні скорочення, або { $palette }, щоб знайти будь-яку команду.
empty-hint-cheatsheet = Натисніть { $cheatsheet }, щоб побачити клавіатурні скорочення.
empty-hint-palette = Натисніть { $palette }, щоб знайти будь-яку команду.
# Під індикатором завантаження, коли файл відкривається довше, ніж мить.
viewer-opening = Відкриття { $file }…
fullscreen-exit-hint = Натисніть Esc, щоб вийти з повноекранного режиму
# Показується під час входу в повноекранний режим; Esc спершу скидає масштаб.
fullscreen-back-hint = Esc скидає масштаб або виходить із повноекранного режиму
# Довідка та палітра: що робить Esc, по одному кроку за натискання.
action-back = Закрити панель (у діалозі спершу крок назад), скинути масштаб, вийти з повноекранного режиму; у галереї — на теку вище

## Вибір мови

language-picker-title = Мова
# machine-drafted, needs native review
language-picker-search = Пошук мов
# $percent — уже відформатований відсоток, напр. «87%».
language-completeness = Перекладено { $percent }
language-system-default = Як у системі ({ $language })
language-pseudo-accented = Псевдомова: з діакритикою, довша
language-pseudo-rtl = Псевдомова: дзеркально справа наліво

## Про програму

about-title = Про { -app-name }
about-version = Версія { $version }
about-tagline = Швидкий і точний переглядач зображень.
about-copyright = © { $year } Автори { -app-name }
about-license = Ліцензовано за MIT License / Apache License 2.0.
about-geonames = Назви місць із GeoNames (geonames.org), ліцензія Creative Commons Attribution 4.0.
about-unicode = Дані локалей із Unicode CLDR через ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Профілі об’єктивів із бази даних Lensfun (lensfun.github.io), ліцензія Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Ліцензії сторонніх компонентів
about-third-party-file = { -app-name } містить сторонні програми та дані з власними ліцензіями; див. THIRD-PARTY.md, встановлений разом із { -app-name }.
about-copy-info = Копіювати відомості про систему
about-close = Закрити

## Вікно переглядача (застосунок етапу 1)

app-close = Закрити
# Позначка на зображенні, файл якого обірвався.
app-truncated-badge = Неповний файл
app-info-summary = Огляд
app-info-all-fields = Усі поля
app-info-copy-value = Копіювати значення
app-info-copy-field = Копіювати значення поля { $field }
app-info-loading = Читання метаданих…
# machine-drafted, needs native review
# Усі поля, для файлу, що не зберігає метаданих, окрім пікселів.
app-info-no-fields = Цей файл не має полів метаданих.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR-карта підсилення
app-info-gain-map-headroom = До { $headroom }× яскравості SDR
app-copy-failed = Не вдалося скопіювати: { $reason }
app-restore-failed = Не вдалося відновити { $file }: { $reason }
app-open-dialog-title = Відкрити зображення
app-open-dialog-filter = Підтримувані файли
app-open-dialog-all = Усі файли
app-open-group-images = Зображення
app-open-group-raw = RAW з камери
app-open-group-video = Відео
app-open-group-sound = Звук
app-open-group-documents = Документи
app-open-group-drawings = Креслення та CAD
app-open-group-models = 3D-моделі
app-open-group-maps = Мапи й треки
app-open-group-science = Наука й медицина
app-open-group-fonts = Шрифти
app-open-group-other = Інше

## Відомості про рендерер (Про програму, Налаштування › Додатково)

# machine-drafted, needs native review
renderer-title = Відомості про рендерер
renderer-section-adapter = Графічний адаптер
renderer-section-limits = Обмеження й функції
renderer-section-display = Дисплей
renderer-section-memory = Пам’ять
renderer-section-decoders = Необов’язкові декодери
renderer-unknown = Невідомо
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ID виробника й пристрою
# Wayland, X11, Windows or macOS.
renderer-display-server = Сервер відображення
renderer-features = Використані функції
renderer-surface-format = Формат вікна
renderer-surface-formats = Доступні формати
renderer-present-mode = Режим показу
renderer-present-modes = Доступні режими
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Увімкнено (запас { $headroom }×)
renderer-hdr-off = Вимкнено (дисплей може показувати HDR)
renderer-hdr-unavailable = Недоступно на цьому дисплеї
renderer-decode-budget = Декодування може використати
renderer-decode-budget-detail = { PERCENT(80) } вільної пам’яті, спільно для всіх зображень, що відкриваються.
renderer-display-cache = Кеш відображення

## Панель продуктивності (F12)

app-hud-scale = Масштаб
app-hud-level = Рівень
app-hud-level-thumbnail = Вбудована мініатюра
app-hud-level-full = Повна роздільність
# $denominator — 2, 4 або 8: зображення декодовано в 1/2, 1/4 або 1/8 розміру.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Відкриття
app-hud-decode = Декодування
app-hud-upload = Передавання
app-hud-frame = Кадр
app-hud-threads = Потоки
app-hud-threads-value = { $threads } ({ $queued } у черзі)
app-hud-tiles = Плитки
app-hud-tiles-value = { $tiles } з { $capacity }
app-hud-ram = Пам’ять
app-hud-vram = Пам’ять GPU
app-hud-gpu = GPU

# Підтвердження переміщення в кошик
app-confirm-trash-title = Перемістити { $file } у кошик?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Супровідні файли теж підуть у смітник: { $files }
app-cancel = Скасувати
# Попередження перед декодуванням дуже великого зображення. $need і $available — обсяги
# пам’яті («3,2 GiB»).
toast-low-memory = Цьому зображенню може знадобитися близько { $need } пам’яті, а доступно лише { $available }. Воно може завантажуватися повільно або не завантажитися.

# Звіт про обладнання (Налаштування → Додатково). Значення заповнює програма.
hw-report-title = Звіт про обладнання
hw-report-copy = Копіювати звіт
hw-report-version = Версія
hw-report-cpu = CPU
hw-report-cores = Ядра
hw-report-cores-value = логічних: { $logical }, фізичних: { $physical }
hw-report-performance-cores = Продуктивні ядра
hw-report-cpu-features = Швидкі шляхи CPU
hw-report-none = Немає
hw-report-memory = Пам’ять
hw-report-memory-value = усього { $total }, доступно { $available }
hw-report-graphics-api = Графічний API
hw-report-driver = Драйвер
hw-report-max-texture = Найбільша текстура
hw-report-pixels = { $width } × { $height } пкс
hw-report-decode-threads = Потоки декодування
hw-report-ram-cache = Кеш зображень у пам’яті
hw-report-vram-cache = Кеш у пам’яті GPU
hw-report-processing = Обробка
hw-report-path-jpeg-decode = Декодування JPEG
hw-report-path-ycbcr-to-rgb = Перетворення кольорів
hw-report-path-png-unfilter = Декодування PNG
hw-report-path-heic-decode = Декодування HEIC
hw-report-path-inference = Аналіз зображень
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, паралельно
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = апаратний декодер
hw-report-engine-npu = NPU
hw-report-build-debug = налагоджувальна збірка
hw-report-build-release = випускна збірка

## Доступність (екранні читачі)
# Зображення, як його оголошує екранний читач: назва файлу й розмір у пікселях.
a11y-image = { $name }, { $width } × { $height } пікселів
# Ділянка, де з’являються короткі сповіщення.
a11y-notifications = Сповіщення
# machine-drafted, needs native review
a11y-transport-playback = Відтворення
a11y-transport-pages = Сторінки
a11y-settings-sections = Розділи

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Коди на цьому зображенні
codes-looking = Пошук кодів…
codes-none = На цьому зображенні кодів не знайдено.
codes-failed = Не вдалося прочитати зображення: { $reason }
codes-copy = Копіювати «{ $text }»
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Відкрити посилання на { $host }…
codes-network-off = Жодну мережу не ввімкнено, тому посилання не відкриваються.
codes-open-title = Відкрити посилання на { $host }?
codes-open-detail = Воно відкриється у веббраузері. Відкривайте лише посилання, яким довіряєте.
codes-open = Відкрити в браузері
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Текст на цьому зображенні
# machine-drafted, needs native review
ocr-looking = Читання тексту…
# machine-drafted, needs native review
ocr-none = На цьому зображенні тексту не знайдено.
# machine-drafted, needs native review
ocr-failed = Не вдалося прочитати текст: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Для функції «Знайти текст» потрібні текстові моделі пакета локального ШІ (Налаштування › Локальний ШІ).
# machine-drafted, needs native review
ocr-loading = Завантаження текстових моделей…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Копіювати «{ $text }»
# machine-drafted, needs native review
ocr-copy-all = Копіювати весь текст
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Упевненість: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Система
hw-report-section-gpu = GPU
hw-report-section-decoding = Декодування
hw-report-operating-system = Операційна система
hw-report-adapter = Адаптер
hw-report-gpu-type = Тип
hw-report-gpu-init = Час запуску
hw-report-max-layers = Шари текстур
hw-report-compression = Стиснення текстур
hw-report-unified-memory = Пам’ять, спільна з CPU
hw-report-display = Дисплей
hw-report-hw-decode = Апаратне декодування
hw-report-core-cluster = Група ядер
hw-report-power = Живлення
hw-report-calibration = Вимірювання швидкості
hw-report-codec-others = Інші
hw-report-performance-cores-value = { $count } з { $total }
hw-report-cluster-value = { $name } — фізичних: { $physical }, логічних: { $logical }
hw-report-thermal-nominal = Нормальна температура
hw-report-thermal-fair = Тепло
hw-report-thermal-serious = Гаряче, сповільнюється
hw-report-thermal-critical = Перегрів
hw-report-power-battery = Від батареї ({ $percent })
hw-report-power-battery-unknown = Від батареї
hw-report-power-mains = Від мережі
hw-report-power-mains-percent = Від мережі ({ $percent })
hw-report-power-saving = Енергозбереження ввімкнено
hw-report-gpu-discrete = Дискретний
hw-report-gpu-integrated = Інтегрований
hw-report-gpu-virtual = Віртуальний
hw-report-gpu-software = Програмний, на CPU
hw-report-gpu-other = Інший
hw-report-gpu-unavailable = Недоступно
hw-report-ms = { $ms } мс
hw-report-yes = Так
hw-report-no = Ні
hw-report-unified-platform-differs = Система повідомляє інакше
hw-report-display-value = { $fps } Hz, запас HDR зараз { $now }, до { $max }
hw-report-hw-decode-value = { $api } на { $device }
hw-report-hw-decode-checking = Ще перевіряється…
hw-report-codec-up-to = До { $width } × { $height }
hw-report-codec-none = Недоступно
hw-report-codec-h264-encode = Кодування H.264
hw-report-supported = підтримується: { $list }
hw-report-unsupported = не підтримується: { $list }
hw-report-calibration-done = Виміряно на цьому комп’ютері
hw-report-calibration-pending = Ще не виміряно
hw-report-paths-power-saving = Енергозбереження: перевагу мають шляхи, що споживають менше енергії
hw-report-paths-caption = Шляхи для зображення на { $mpx } Мп
hw-report-col-task = Завдання
hw-report-col-path = Шлях
hw-report-col-time = Час
hw-report-col-cpu = Час CPU (усі потоки)
hw-report-col-runs = Вимірювання
hw-report-col-use = Використання
hw-report-runs-untested = Не перевірено
hw-report-use-viewing = Перегляд ({ $reason })
hw-report-use-background = У фоні ({ $reason })
hw-report-reason-only = єдиний варіант
hw-report-reason-faster = найшвидший
hw-report-reason-cheaper = найменше часу CPU
hw-report-reason-lower-power = найменше енергії
hw-report-reason-explore = випробовується
hw-report-reason-forced = задано в Налаштуваннях
hw-report-reason-prefer-gpu = Надавати перевагу GPU
hw-report-reason-cpu-only = Лише CPU
hw-report-reason-calibrate = вимірюється
hw-report-reason-scaled = зменшений розмір
hw-report-trust-unverified = Ще не перевірено
hw-report-trust-blocked = Вимкнено після помилки
hw-report-trust-absent = Немає на цьому комп’ютері

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Галерея
cheatsheet-mode-compare = Порівняння
cheatsheet-gallery-browse = Перегляд
cheatsheet-gallery-select = Виділення
cheatsheet-gallery-mark = Оцінки й позначки
cheatsheet-gallery-move = Перехід між зображеннями
cheatsheet-gallery-page = Сторінка вгору або вниз
cheatsheet-gallery-ends = Перше або останнє зображення
cheatsheet-gallery-up = Вгору до батьківської теки
cheatsheet-gallery-history-back = Назад до попередньої теки
cheatsheet-gallery-history-forward = Вперед до наступної теки
cheatsheet-gallery-back = Закрити верхній вигляд, потім назад
cheatsheet-gallery-viewer = Назад до переглядача
cheatsheet-gallery-album-add = Додати до останнього альбому
cheatsheet-gallery-select-one = Виділити або зняти виділення
cheatsheet-gallery-quick-look = Швидкий перегляд
cheatsheet-gallery-filter = Фільтр
cheatsheet-gallery-bigger = Більші мініатюри
cheatsheet-gallery-smaller = Менші мініатюри
cheatsheet-gallery-default-size = Стандартний розмір мініатюр
cheatsheet-gallery-refresh = Оновити теку
cheatsheet-gallery-menu = Контекстне меню
cheatsheet-compare-views = Порівняння
cheatsheet-compare-overlay = Накладення або поруч
cheatsheet-compare-difference = Різниця (ще раз: наступний вид)
cheatsheet-compare-step-down = Менше змішування або підсилення різниці
cheatsheet-compare-step-up = Більше змішування або підсилення різниці
cheatsheet-compare-pick = Зображення, на яке діють клавіші
cheatsheet-compare-close = Закрити порівняння
cheatsheet-channels = Канали
cheatsheet-medical-window = Вікно й рівень
cheatsheet-medical-measure = Вимірювання й анотації
cheatsheet-medical-frames = Кадри й серії
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D і реконструкції
cheatsheet-medical-analysis = Сегментація, суміщення й ПЕТ
# machine-drafted, needs native review
cheatsheet-medical-studies = Дослідження й поширення
cheatsheet-palette-only-title = У палітрі команд ({ $key })
cheatsheet-palette-only-title-nokey = У палітрі команд
cheatsheet-palette-only-tag = Палітра
cheatsheet-palette-only-line = { $action }: немає клавіші; у палітрі команд ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: немає клавіші; у палітрі команд
# Esc у шпаргалці (довгий опис із палітри не вміщається).
cheatsheet-back = Закрити або повернутися

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Експорт відео
hw-report-engine-hw-encoder = апаратний кодер

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (на одну мініатюру { $size })
hw-report-task-frame = { $task } (на кадр { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Лише програмно

about-natural-earth = Дані мапи з Natural Earth (naturalearthdata.com), суспільне надбання.
app-info-show-on-map = Показати на мапі
