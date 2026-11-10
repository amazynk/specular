# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# Українська.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Рельєф
cheatsheet-map-time = Час і погода
cheatsheet-map-online = Онлайн-сервіси
cheatsheet-map-photos = Фото й GPS
cheatsheet-map-output = Вивід

## Palette entries and their one-line descriptions.
palette-map-slope = Кольори крутизни
palette-map-slope-help = Розфарбувати модель висот за крутизною схилів.
palette-map-aspect = Кольори експозиції
palette-map-aspect-help = Розфарбувати модель висот за напрямком, куди звернені схили.
palette-map-contours = Показати або сховати горизонталі
palette-map-contours-help = Горизонталі поверх моделі висот, з підписом кожні кілька ліній.
palette-map-terrain-3d = Рельєф у 3D
palette-map-terrain-3d-help = Показати модель висот як 3D-поверхню з накладеними кольорами.
palette-map-time = Показати або сховати повзунок часу
palette-map-time-help = Крокувати часами сітки або часами вздовж треку.
palette-map-play = Відтворити або призупинити час
palette-map-play-help = Повзунок часу рухається сам, крок за кроком.
palette-map-wind = Показати або сховати стрілки вітру
palette-map-wind-help = Стрілки швидкості й напрямку вітру з компонент u і v сітки.
palette-map-weather-colours = Наступна погодна колірна мапа
palette-map-weather-colours-help = Колірні мапи для температури, опадів, вітру й тиску.
palette-map-services = Онлайн-сервіси мап…
palette-map-services-help = Додати шари із серверів WMS, WMTS, WFS, OGC API або STAC.
palette-map-photos-on-track = Показати або сховати фото на треку
palette-map-photos-on-track-help = Розмістити фото з теки вздовж треку GPS за часом зйомки.
palette-map-geotag = Геотеги для фото з треку…
palette-map-geotag-help = Записати позиції з треку в супровідні файли XMP фото з теки.
palette-map-export = Експортувати мапу…
palette-map-export-help = Зберегти мапу як PNG або PDF у масштабі, з легендою, масштабною лінійкою й стрілкою півночі.
palette-map-print = Друкувати мапу в масштабі…
palette-map-print-help = Друкувати мапу у вибраному масштабі, наприклад 1:{ NUMBER(25000) }.
palette-map-copy-extent = Копіювати охоплення
palette-map-copy-extent-help = Скопіювати область на екрані як обмежувальну рамку або як WKT.

## Why a map command is unavailable.
disabled-no-time = Мапа не має часів
disabled-no-wind = Мапа не має компонент вітру (u і v)
disabled-no-track = Немає треку GPS із часами
disabled-not-grid = Мапа не є сіткою значень

## Settings › Maps and Metadata.
settings-maps-services = Онлайн-сервіси мап
settings-maps-services-help = Додавати шари із серверів WMS, WMTS, WFS, OGC API і STAC. Вимкнено: нічого не завантажується.
settings-maps-services-allowed = Дозволені сервери мап
settings-maps-services-allowed-help = Сервери, на які ви погодилися після пояснення, що вони бачать.
settings-metadata-basemap-service = Шар сервісу мап
settings-metadata-basemap-service-help = Шар, вибраний на панелі онлайн-сервісів мапи.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] ПнСх
    [e] Сх
    [se] ПдСх
    [s] Пд
    [sw] ПдЗх
    [w] Зх
    [nw] ПнЗх
   *[n] Пн
}
map-slope-aspect = Крутизна { $slope }° · звернений на { $direction } ({ $aspect }°)
map-slope-flat = Крутизна { $slope }° · рівно
map-slope-on = Кольори крутизни
map-aspect-on = Кольори експозиції
map-relief-off = Кольори висот
map-contours-on = Горизонталі увімкнено
map-contours-off = Горизонталі вимкнено
map-legend-slope = Крутизна
map-legend-aspect = Експозиція: куди звернені схили
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° і більше
map-terrain-3d-on = Рельєф у 3D. Esc повертає до мапи.
map-terrain-3d-on-exaggerated = Рельєф у 3D, висоти × { $times }. Esc повертає до мапи.
map-terrain-exaggeration = Висоти перебільшено в { $times } раз.
map-terrain-drape = Кольори мапи

## Time, weather and wind.
map-time-grid = { $when } · крок { $step } з { $steps }
map-time-track = { $when } · від початку { $elapsed }
map-time-speed = × { $times }
map-time-play = Відтворити
map-time-pause = Пауза
map-weather-map = Кольори: { $name }
map-weather-file = Кольори з файлу
map-wind-on = Стрілки вітру увімкнено
map-wind-off = Стрілки вітру вимкнено
map-wind-at = Вітер { $speed } { $unit } з { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Обмежувальну рамку скопійовано. Скопіюйте ще раз для WKT.
map-extent-copied-wkt = Охоплення скопійовано як WKT

## Online map services.
map-services-title = Онлайн-сервіси мап
map-service-off = Онлайн-сервіси мап вимкнено.
map-service-off-why = Увімкніть їх, щоб додавати шари із серверів в інтернеті.
map-service-turn-on = Увімкнути
map-service-close = Закрити
map-service-back = Назад
map-service-cancel = Скасувати
map-service-connect = Під’єднатися
map-service-allow = Дозволити
map-service-remove = Вилучити
map-service-download = Завантажити
map-service-list-hint = Enter відкриває сервер. Delete вилучає його.
map-service-add = Додати сервер…
map-service-add-sub = WMS, WMTS, WFS, OGC API або STAC
map-service-add-hint = Адреса сервера. Enter під’єднує.
map-service-bad-address = Це не вебадреса (http або https, без імені користувача).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Онлайн-сервіс мап
map-service-consent-server = Specular звернеться до { $host }.
map-service-consent-sees = Цей сервер бачить вашу інтернет-адресу й запитану область мапи.
map-service-consent-private = Ваші файли та їхні назви не надсилаються.
map-service-asking = Запит до { $host }…
map-service-downloading = Завантаження з { $host }…
map-service-layers-hint = Шари { $kind }. Enter використовує один.
map-service-no-layers = Сервер не перелічує жодних шарів.
map-service-offer-tiles = Тайли: стає базовою мапою фотомапи
map-service-offer-features = Об’єкти: додаються до цієї мапи для області на екрані
map-service-offer-search = Зображення: пошук в області на екрані
map-service-refused-mercator = Не пропонується у Web Mercator (EPSG:3857), потрібному базовій мапі.
map-service-refused-format = Не пропонується як зображення PNG або JPEG.
map-service-refused-geojson = Не пропонується як GeoJSON, формат, який тут читається.
map-service-items-hint = Знайдено зображень: { $count }. Enter завантажує одне.
map-service-no-items = Тут не знайдено зображень.
map-service-item-cloud = { $when } · хмарність { PERCENT($cloud) }
map-service-item-no-cog = Немає Cloud-Optimized GeoTIFF
map-service-download-ask = Завантажити { $size } з { $host }?
map-service-download-ask-unknown = Завантажити це зображення з { $host }? Його розмір невідомий.
map-service-download-cache = Його буде збережено в кеші мап і відкрито.
map-service-size-mb = { $size } МБ
map-service-size-gb = { $size } ГБ
map-service-too-big = Зображення має { $size }, завелике для завантаження.
map-service-opened = Відкриття завантаженого зображення
map-service-basemap = { $layer } тепер базова мапа фотомапи. Фотомапа запитає перед першим завантаженням.
map-service-status = Сервер відповів { $status }.
map-service-unknown = Ця версія не читає такий сервіс мап.
map-service-exception = Сервер повідомив: { $message }
map-service-malformed = Відповідь сервера пошкоджена: { $reason }
map-service-remove-ask = Вилучити «{ $server }» зі збережених серверів?
map-service-remove-detail = Нічого на диску не втрачається. Шари, уже додані до мапи, лишаються, доки її не закрито.
map-service-removed = { $server } вилучено
map-service-saved = Збережені сервери мап
map-service-saved-none = Ще немає. Додайте сервер на панелі онлайн-сервісів мапи.
map-service-keywords = WMS WMTS WFS OGC API STAC сервери мап

## Photos and geotagging.
map-geotag-title = Геотеги для фото з треку
map-photos-on = Фото на треку
map-photos-off = Фото на треку сховано
map-geotag-offset = Годинник камери: { $offset } від часу GPS. ← → змінюють на хвилину, із Shift на годину, з Alt на секунду.
map-geotag-reading = Читання фото…
map-geotag-summary = Позицію отримують { $count } з { $total } фото.
map-geotag-sidecars = Позиції записуються в супровідні файли XMP поруч із фото. Самі файли фото не змінюються.
map-geotag-writing = Запис…
map-geotag-write = Записати супровідні файли…
map-geotag-guess = Вгадати зсув
map-geotag-will-write = Отримає { $position }
map-geotag-has-position = Уже має позицію: без змін
map-geotag-no-time = Немає часу зйомки: без змін
map-geotag-outside = Знято за { $offset } поза треком: без змін
map-geotag-gap = У розриві треку ({ $seconds } с від точки): без змін
map-geotag-nothing = За цього зсуву жодне фото не отримує позиції.
map-geotag-confirm = Записати позиції в { $count ->
    [one] { $count } супровідний файл XMP
    [few] { $count } супровідні файли XMP
    [many] { $count } супровідних файлів XMP
   *[other] { $count } супровідного файлу XMP
}? Самі фото не змінюються.
map-geotag-confirm-ok = Записати супровідні файли
map-geotag-done = { $count ->
    [one] Записано { $count } супровідний файл
    [few] Записано { $count } супровідні файли
    [many] Записано { $count } супровідних файлів
   *[other] Записано { $count } супровідного файлу
}
map-geotag-done-failed = Записано супровідних файлів: { $count }; не вдалося: { $failed }: { $reason }
map-geotag-not-sidecar = Це не назва супровідного файлу.

## Export and printing.
map-export-title = Експортувати мапу
map-print-title = Друкувати мапу в масштабі
map-export-keys = ↑ ↓ вибір, ← → зміна, Enter виконує.
map-export-format = Формат
map-export-scale = Масштаб
map-export-scale-value = 1:{ $n }
map-export-resolution = Роздільність
map-export-dpi = { $dpi } dpi
map-export-paper = Папір
map-export-size = { $width } × { $height } пікс. · { $across } × { $down } км на місцевості
map-export-save = Зберегти…
map-export-print = Друкувати…
map-export-too-large = Завелике для цього масштабу: { $width } × { $height } пікс. Виберіть менший масштаб або роздільність.
map-export-no-scale = Ця мапа не має масштабу на місцевості.
map-export-no-gpu = Для експорту потрібен графічний процесор.
map-export-subtitle = Масштаб { $scale } · { $crs }
map-export-data = Дані: { $sources }
map-export-saved = Збережено { $name }
map-export-printing = Друк…
map-export-printed = Надіслано на принтер
map-export-print-failed = Не надруковано: { $reason }
