# machine-drafted, needs native review
# Specular — галерея (режим мініатюр, DESIGN §10) і Налаштування → Галерея.

## Палітра й меню

palette-gallery = Галерея
gallery-palette-view = Вигляд галереї…
gallery-palette-layout = Компонування
gallery-palette-sort = Сортувати за
gallery-palette-group = Групувати за
gallery-palette-filter = Фільтр…
gallery-palette-quick-filters = Швидкі фільтри
gallery-palette-details = Подання «Подробиці»
gallery-palette-info-overlay = Відомості на мініатюрах
gallery-palette-stacks = Групувати RAW+JPEG і серії
gallery-palette-subfolders = Включати підтеки
gallery-palette-compare = Порівняти вибрані
gallery-palette-rate = Оцінити…
gallery-palette-settings = Налаштування галереї…
gallery-view-menu = Вигляд
gallery-more-menu = Більше
gallery-open = Відкрити
gallery-new-window = Відкрити в новому вікні
gallery-reveal = Показати в теці
gallery-properties = Властивості
gallery-copy-paths = Копіювати шляхи
gallery-select-all = Вибрати все
gallery-select-none = Зняти виділення
gallery-select-invert = Інвертувати виділення
gallery-next-unrated = Наступне без оцінки
gallery-stack-toggle = Розгорнути або згорнути стос
gallery-run-on = { $count ->
    [one] { $name } ({ $count } файл)
    [few] { $name } ({ $count } файли)
    [many] { $name } ({ $count } файлів)
   *[other] { $name } ({ $count } файлу)
}
gallery-depth-off = Лише ця тека
gallery-depth-all = Усі підтеки
gallery-depth-levels = { $depth ->
    [one] { $depth } рівень підтек
    [few] { $depth } рівні підтек
    [many] { $depth } рівнів підтек
   *[other] { $depth } рівня підтек
}

## Стан і порожні стани

gallery-items = { $count ->
    [one] { $count } елемент
    [few] { $count } елементи
    [many] { $count } елементів
   *[other] { $count } елемента
}
gallery-selected = Вибрано { $count } з { $total }
gallery-shown-of = Показано { $count } з { $total }
gallery-listing = Читання теки… { $count }
gallery-empty = У цій теці немає зображень.
gallery-empty-filter = Жодне зображення не відповідає фільтру.
gallery-list-error = Не вдалося прочитати теку: { $reason }
gallery-no-folder = Жодну теку не відкрито
gallery-kind-folder = Тека
gallery-drag-count = { $count ->
    [one] { $count } файл
    [few] { $count } файли
    [many] { $count } файлів
   *[other] { $count } файлу
}
gallery-badge-pages = { $count ->
    [one] { $count } сторінка
    [few] { $count } сторінки
    [many] { $count } сторінок
   *[other] { $count } сторінки
}

## Панель фільтра

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Назва або rating>=3 camera:X-T5 …
gallery-filter-clear = Очистити фільтр
gallery-filter-rating = { $stars ->
    [one] ★ { $stars } і більше
    [few] ★ { $stars } і більше
    [many] ★ { $stars } і більше
   *[other] ★ { $stars } і більше
}
gallery-filter-label = Мітка: { $label }
gallery-filter-picks = Відібрані
gallery-filter-hide-rejects = Сховати відхилені
gallery-filter-unrated = Без оцінки
gallery-filter-raw = RAW
gallery-filter-animated = Анімовані
gallery-filter-hdr = HDR
gallery-filter-landscape = Альбомні
gallery-filter-portrait = Книжкові

## Query errors and facets

gallery-query-unknown-field = Невідоме поле «{ $field }»
gallery-query-unknown-field-suggest = Невідоме поле «{ $field }»: можливо, ви мали на увазі «{ $suggestion }»?
gallery-query-missing-value = «{ $field }» потребує значення
gallery-query-bad-number = «{ $value }» не є числом ({ $field })
gallery-query-bad-date = «{ $value }» не є датою ({ $field }): використовуйте 2024, 2024-05 або 2024-05-12
gallery-query-bad-word = «{ $value }» не підходить для { $field }: використовуйте { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = «{ $value }» не є назвою тегу Exif
gallery-query-no-compare = { $field } тут не можна порівнювати за допомогою < або >
gallery-query-unterminated = Бракує закривальної лапки
gallery-query-unavailable = «{ $field }» потребує бібліотеки (тут недоступно)
gallery-facets = Фасети
gallery-facets-counting = Підрахунок…
gallery-facets-empty = Нема чого рахувати
gallery-facets-search = Пошук у фасетах
gallery-facets-no-match = Немає відповідних значень
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Початковий екран
gallery-facet-format = Формат
gallery-facet-camera = Камера
gallery-facet-lens = Об’єктив
gallery-facet-rating = Оцінка
gallery-facet-year = Рік
gallery-facet-orientation = Орієнтація
gallery-facet-label = Мітка
gallery-facet-properties = Властивості
gallery-facet-square = Квадратні
gallery-facet-alpha = Прозорість
gallery-facet-gps = Місце
gallery-facet-multipage = Кілька сторінок
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } файл
    [few] { $count } файли
    [many] { $count } файлів
   *[other] { $count } файлу
}

## Групи

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Ця тека
gallery-group-unknown = Невідомо
gallery-group-unrated = Без оцінки

## Відбір

gallery-stars = { $stars ->
    [one] { $stars } зірка
    [few] { $stars } зірки
    [many] { $stars } зірок
   *[other] { $stars } зірки
}
gallery-rate-none = Без оцінки
gallery-pick = Відібрати
gallery-reject = Відхилити
gallery-unflag = Без позначки
gallery-label-red = Червона
gallery-label-yellow = Жовта
gallery-label-green = Зелена
gallery-label-blue = Синя
gallery-label-none = Без мітки
gallery-compare-hint = Виберіть від 2 до 4 зображень, щоб порівняти їх.

## Файли

gallery-trash-count = { $count ->
    [one] Перемістити { $count } файл у кошик
    [few] Перемістити { $count } файли в кошик
    [many] Перемістити { $count } файлів у кошик
   *[other] Перемістити { $count } файлу в кошик
}
gallery-moved-to = { $count ->
    [one] { $count } файл переміщено до { $folder }
    [few] { $count } файли переміщено до { $folder }
    [many] { $count } файлів переміщено до { $folder }
   *[other] { $count } файлу переміщено до { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } файл скопійовано до { $folder }
    [few] { $count } файли скопійовано до { $folder }
    [many] { $count } файлів скопійовано до { $folder }
   *[other] { $count } файлу скопійовано до { $folder }
}
gallery-move-failed = Не вдалося перемістити { $file }: { $reason }
gallery-drop-move = Перемістити до { $folder }
gallery-drop-copy = Копіювати до { $folder }
gallery-move-here = Перемістити вибрані сюди
gallery-copy-here = Копіювати вибрані сюди
gallery-not-an-image = { $file } не є зображенням, яке може відкрити { -app-name }.
gallery-cache-cleared = Кеш мініатюр очищено
# machine-drafted, needs native review
gallery-thumb-failed = Немає мініатюри: { $reason }
gallery-cache-clear-failed = Не вдалося очистити кеш мініатюр: { $reason }

## Властивості

gallery-prop-name = Назва
gallery-prop-folder = Тека
gallery-prop-size = Розмір файлу
gallery-prop-dimensions = Розміри
gallery-prop-format = Формат
gallery-prop-date = Дата
gallery-prop-camera = Камера
gallery-prop-lens = Об’єктив
gallery-prop-rating = Оцінка
gallery-prop-label = Мітка
gallery-prop-items = Елементи

## Екранні читачі

gallery-a11y-list = { $count ->
    [one] { $folder }, { $count } елемент
    [few] { $folder }, { $count } елементи
    [many] { $folder }, { $count } елементів
   *[other] { $folder }, { $count } елемента
}
gallery-a11y-dims = { $width } на { $height }
gallery-a11y-folder = тека
gallery-a11y-up = Вгору до { $folder }
gallery-a11y-archive = архів
gallery-a11y-stack = стос: { $count }
gallery-a11y-filter-bar = Фільтр
gallery-a11y-filter-field = Фільтрувати за назвою або полем
gallery-a11y-chip = Умова фільтра { $term }
gallery-a11y-chip-remove = Натисніть, щоб прибрати
gallery-a11y-results = { $count ->
    [0] Немає відповідних елементів
    [one] { $count } відповідний елемент
    [few] { $count } відповідні елементи
    [many] { $count } відповідних елементів
   *[other] { $count } відповідного елемента
}

## Налаштування → Галерея

settings-section-gallery = Галерея
settings-gallery-layout = Компонування
settings-gallery-layout-help = Квадрати обтинають мініатюри за сіткою; вирівняні рядки зберігають пропорції кожного зображення; без обтинання — цілі зображення в однакових клітинках.
settings-gallery-layout-squares = Квадрати
settings-gallery-layout-justified = Вирівняні рядки
settings-gallery-layout-uncropped = Без обтинання
settings-gallery-layout-details = Подробиці
settings-gallery-thumbnail-size = Розмір мініатюр
settings-gallery-thumbnail-size-help = Також Ctrl+коліщатко миші, зведення пальців або Ctrl+плюс і мінус у галереї.
settings-gallery-captions = Показувати назви файлів
settings-gallery-captions-help = Підпис під кожною мініатюрою.
settings-gallery-badges = Показувати позначки
settings-gallery-badges-help = Невеликі позначки для RAW, анімації, HDR, багатосторінкових файлів, оцінки та мітки.
settings-gallery-sort-order = Сортувати за
settings-gallery-sort-order-help = Порядок у галереї. Теки завжди йдуть першими.
settings-gallery-sort-order-name = Назвою
settings-gallery-sort-order-date-taken = Датою зйомки
settings-gallery-sort-order-modified = Датою зміни
settings-gallery-sort-order-size = Розміром файлу
settings-gallery-sort-order-type = Типом файлу
settings-gallery-sort-order-dimensions = Розмірами
settings-gallery-sort-descending = Зворотний порядок
settings-gallery-sort-descending-help = Сортувати від останнього до першого.
settings-gallery-group-by = Групувати за
settings-gallery-group-by-help = Заголовки розділів, які лишаються вгорі під час прокручування.
settings-gallery-group-by-none = Не групувати
settings-gallery-group-by-day = Днем
settings-gallery-group-by-month = Місяцем
settings-gallery-group-by-year = Роком
settings-gallery-group-by-event = Подією
settings-gallery-group-by-folder = Текою
settings-gallery-group-by-camera = Камерою
settings-gallery-group-by-format = Форматом
settings-gallery-group-by-rating = Оцінкою
settings-gallery-group-by-target = Об’єктом
settings-gallery-group-by-filter = Фільтром
settings-gallery-per-folder-view = Запам’ятовувати вигляд для кожної теки
settings-gallery-per-folder-view-help = Компонування, розмір, сортування й групування зберігаються окремо для кожної теки; інакше всюди використовується один вигляд.
settings-gallery-open-folders-in-gallery = Відкривати теки в галереї
settings-gallery-open-folders-in-gallery-help = Під час відкриття теки спершу показувати її мініатюри, а не перше зображення.
settings-gallery-escape-opens-gallery = Esc відкриває галерею
settings-gallery-escape-opens-gallery-help = Коли більше нічого закривати, Esc у переглядачі переходить до галереї, а не нічого не робить.
settings-gallery-wrap-around = Стрілки переходять по колу
settings-gallery-wrap-around-help = Наприкінці списку стрілки переходять на початок, а з початку — в кінець. Утримувана клавіша все одно зупиняється в кінці.
settings-gallery-show-hidden = Показувати приховані файли
settings-gallery-show-hidden-help = Включати файли й теки, назви яких починаються з крапки.
settings-gallery-show-other-files = Показувати інші файли
settings-gallery-show-other-files-help = Включати файли, які { -app-name } не може відкрити, зі звичайним значком.
settings-gallery-show-archives = Показувати архіви
settings-gallery-show-archives-help = Включати комікси й інші архіви (zip, cbz, rar, 7z…) як плитки. Відкрити архів напряму все одно можна.
settings-gallery-subfolder-depth = Рівні підтек
settings-gallery-subfolder-depth-help = Показувати також зображення з підтек до вказаної глибини (0: лише ця тека).
settings-gallery-stacks = Групувати пов’язані файли
settings-gallery-stacks-help = Показувати пари RAW+JPEG і серії однією плиткою. Натисніть S, щоб розгорнути стос.
settings-gallery-info-overlay = Відомості на мініатюрах
settings-gallery-info-overlay-help = Розміри й експозиція поверх кожної мініатюри.
settings-gallery-prefer-embedded = Надавати перевагу вбудованим мініатюрам
settings-gallery-prefer-embedded-help = Використовувати мініатюру, збережену у файлі, якщо вона достатньо велика. Значно швидше, трохи менш точно.
settings-gallery-sharper-raw-thumbnails = Чіткіші мініатюри RAW
settings-gallery-sharper-raw-thumbnails-help = Змінені фото RAW проявляються для мініатюр у 1024 пкс. Чіткіше, трохи повільніше. Вимкнено: 512 пкс.
settings-gallery-rating-sidecars = Записувати оцінки в супровідні файли XMP
settings-gallery-rating-sidecars-help = Оцінки, мітки та позначки також записуються у файл .xmp поруч із зображенням, який читають інші фотопрограми.
settings-gallery-cache-size = Розмір кешу мініатюр
settings-gallery-cache-size-help = Коли кеш заповнено, найстаріші мініатюри вилучаються.
settings-gallery-cache-location = Розташування кешу мініатюр
settings-gallery-cache-location-help = Найкраще підходить тека на швидкому диску.
settings-gallery-clear-cache = Очистити кеш мініатюр
settings-gallery-clear-cache-help = Видаляє збережені мініатюри. Під час перегляду вони створюються знову.

## Compare
# machine-drafted, needs native review

compare-side = Поруч
compare-overlay = Накладення
compare-difference = Різниця
compare-flicker = Мерехтіння
compare-blend = Змішування
compare-kind-absolute = Абсолютна похибка
compare-kind-squared = Квадратична похибка
compare-kind-relative = Відносна похибка
compare-scale-down = Менш чутливо
compare-scale-up = Чутливіше
compare-scale = Повна шкала { $value }
compare-space = Лінійне світло, як показано
compare-space-help = Похибки та метрики вимірюються в лінійному світлі так, як показано зображення, після тонального відображення, для кожного каналу разом з альфою.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = вибірка { $width } × { $height }
compare-aligned = Вирівняно за лівим верхнім кутом, порівняно { $width } × { $height }
compare-vs = { $b } порівняно з { $a }
compare-overlay-label = { $b } поверх { $a }, { PERCENT($percent) }
compare-pending = Вимірювання…
# machine-drafted, needs native review
compare-diff-failed = Не вдалося показати різницю: { $reason }
compare-no-overlap = У видимій області нема чого порівнювати
compare-zoom-fit = Умістити
compare-zoom = { PERCENT($percent) }
compare-showing = Показано: { $name }
compare-a11y-group = Порівняння
compare-a11y-blend = Змішування між зображеннями
compare-a11y-zoom = Масштаб
gallery-palette-compare-side = Порівняти поруч
gallery-palette-compare-overlay = Порівняти накладенням
gallery-palette-compare-difference = Порівняти різницю: { $kind }
gallery-palette-compare-flicker = Мерехтіння між порівнюваними зображеннями
gallery-palette-compare-actual = Порівняти при { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Пробіл або Ctrl+Enter вибирає кілька

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Колірний профіль
gallery-facet-date = Дата зйомки
gallery-facet-picked = Відібрані
gallery-facet-rejected = Відхилені
gallery-facet-flagged = Відібрані або відхилені
gallery-facet-rated = З оцінкою
gallery-facet-labelled = З міткою

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Бітова глибина
gallery-facet-pages = Сторінки
gallery-facet-width = Ширина
gallery-facet-height = Висота
gallery-facet-megapixels = Мегапікселі
gallery-picker-bits = { $bits }-бітний
gallery-picker-pages = { $pages ->
    [one] { $pages } сторінка
    [few] { $pages } сторінки
    [many] { $pages } сторінок
   *[other] { $pages } сторінки
}
gallery-picker-pixels = { $px } пкс
gallery-picker-megapixels = { $mp } Мп
gallery-picker-group-any = Будь-який у групі
gallery-picker-group-keep = Пропозиція: залишити
gallery-picker-group-discard = Пропозиція: вилучити
gallery-picker-ops = Порівняти
gallery-picker-op-eq = Дорівнює
gallery-picker-op-ge = Щонайменше
gallery-picker-op-le = Щонайбільше
gallery-picker-ge = щонайменше { $value }
gallery-picker-gt = більше ніж { $value }
gallery-picker-le = щонайбільше { $value }
gallery-picker-lt = менше ніж { $value }

# Map view.
gallery-facet-place = Місце
gallery-palette-map = Мапа
map-title = Мапа
map-summary = Фото з місцем: { $count }
map-photos = Фото: { $count }
map-zoom-in = Наблизити
map-zoom-out = Віддалити
map-show-all = Показати все
map-done = Готово
map-credit = Дані мапи: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Мапа: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Онлайн-плитки мапи
map-basemap-consent-server = Плитки мапи надходять із { $host }.
map-basemap-consent-sees = Цей сервер бачить вашу інтернет-адресу та області, які ви переглядаєте.
map-basemap-consent-private = Ваші фото, їхні назви й місця не надсилаються.
map-basemap-consent-cache = Плитки зберігаються на цьому комп’ютері, до { $size }.
map-basemap-allow = Використовувати онлайн-мапу
map-basemap-decline = Залишити офлайн-мапу
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Базову мапу не показано: { $reason }
map-basemap-bad-address = адреса сервера плиток має містити {"{"}z{"}"}, {"{"}x{"}"} і {"{"}y{"}"}.
map-basemap-unknown-tiles = плитки у файлі мапи невідомого типу.
map-loading = Читання місць фото…
map-empty = Жодне з цих фото не має місця
map-hidden = Місця фото приховано (Налаштування › Метадані й приватність)
map-a11y-group = Мапа місць фото
map-a11y-status = Показано місць: { $count }, рівень масштабу { $zoom }
