# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# Українська.

keys-mode-map = Мапи
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Шари
cheatsheet-map-coordinates = Координати
cheatsheet-map-measure = Вимірювання

## Palette entries and their one-line descriptions.
palette-map-layers = Показати або сховати панель шарів
palette-map-layers-help = Шари мапи: порядок, непрозорість, видимість і стиль.
palette-map-add-layer = Додати шар із файлу…
palette-map-add-layer-help = Накласти інший файл мапи на цей, наприклад трек GPX на shapefile.
palette-map-zoom-to-layer = Масштабувати до шару
palette-map-zoom-to-layer-help = Показати весь шар, вибраний на панелі шарів.
palette-map-attribute-table = Показати або сховати таблицю атрибутів
palette-map-attribute-table-help = Об’єкти вибраного шару як таблиця: сортування, фільтр, пошук і вибір.
palette-map-coordinate-format = Наступний формат координат
palette-map-coordinate-format-help = Десяткові градуси, градуси мінути секунди, UTM або MGRS.
palette-map-units = Метричні або імперські одиниці
palette-map-units-help = Довжини й площі в метрах або у футах і милях.
palette-map-copy-coordinate = Копіювати координату
palette-map-copy-coordinate-help = Скопіювати позицію під вказівником або центру мапи.
palette-map-go-to-coordinate = Перейти до координати…
palette-map-go-to-coordinate-help = Введіть позицію в будь-якому форматі, щоб відцентрувати там мапу.
palette-map-measure-distance = Виміряти відстань
palette-map-measure-distance-help = Клацайте точки на мапі: довжина по поверхні Землі й азимут.
palette-map-measure-area = Виміряти площу
palette-map-measure-area-help = Клацайте кути ділянки: її площа й периметр по поверхні Землі.
palette-map-elevation-profile = Профіль висот
palette-map-elevation-profile-help = Проведіть лінію по моделі висот, щоб побачити графік висот.
palette-map-overview = Показати або сховати оглядову мапу
palette-map-overview-help = Мала мапа всього охоплення з позначеною частиною на екрані.

## Why a map command is unavailable.
disabled-not-map = Лише для мап, з увімкненими інструментами мап
disabled-no-lonlat = Мапа не має відомої системи координат
disabled-no-features = Мапа не має об’єктів
disabled-no-heights = Мапа не має висот

## Settings › Maps.
settings-section-maps = Мапи
settings-maps-coordinate-format = Формат координат
settings-maps-coordinate-format-help = Як позиції показуються й копіюються. Перехід до координати читає будь-який формат.
settings-maps-coordinate-format-dd = Десяткові градуси
settings-maps-coordinate-format-dms = Градуси, мінути, секунди
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Одиниці
settings-maps-units-help = Довжини й площі на мапах: масштабна лінійка й вимірювання. Автоматично — як у Метадані › Одиниці.
settings-maps-units-auto = Автоматично
settings-maps-units-metric = Метричні
settings-maps-units-imperial = Імперські

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Шир., довг. { $position }
}
map-height = Висота { $height }
map-height-label = Висота
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Масштабна лінійка: { $length }
map-north-label = Стрілка півночі: північ на { $degrees }° за годинниковою стрілкою від верху
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] градуси, мінути, секунди
    [utm] UTM
    [mgrs] MGRS
   *[dd] десяткові градуси
}
map-format-toast = Координати: { $format }
map-units-metric = Одиниці: метри й кілометри
map-units-imperial = Одиниці: фути й милі
toast-copied-coordinate = Координату скопійовано

## Go to coordinate.
map-goto-hint = Десяткові градуси, DMS, UTM або MGRS. Enter переходить.
map-goto-empty = Введіть позицію.
map-goto-unrecognised = Це не позиція у відомому форматі.
map-goto-range = Широта або довгота поза межами.
map-goto-no-way = Ця мапа не може розмістити позицію.
map-goto-outside = Ця позиція поза мапою.
map-goto-toast = Відцентровано на { $position }

## The layer panel.
map-layers-title = Шари: показано { $shown } з { $total }
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } об’єкт
    [few] { $features } об’єкти
    [many] { $features } об’єктів
   *[other] { $features } об’єкта
}, непрозорість { PERCENT($opacity) }
map-kind-area = Площі
map-kind-line = Лінії
map-kind-point = Точки
map-kind-empty = Немає об’єктів
map-layer-visible = Показати або сховати шар
map-layer-opacity-down = Менш непрозоро
map-layer-opacity-up = Більш непрозоро
map-layer-colour = Наступний колір
map-layer-width = Наступна товщина лінії
map-layer-point = Наступний розмір точки
map-layer-up = Вгору
map-layer-down = Вниз
map-layer-remove = Вилучити
map-layer-keep = Залишити
map-layer-remove-ask = Вилучити { $name } з мапи? Файл залишиться на диску.
map-layer-adding = Додавання шару…
map-removed = { $name } вилучено з мапи
map-added = { $layers ->
    [one] Додано { $name }
    [few] Додано { $name }: { $layers } шари
    [many] Додано { $name }: { $layers } шарів
   *[other] Додано { $name }: { $layers } шару
}
map-add-failed = Не вдається додати { $name }: { $reason }
map-add-filter = Файли мап
map-add-not-map = це не мапа.
map-add-not-vector = у ньому немає об’єктів.
map-add-no-crs = у ньому немає файлу проєкції (.prj).

## Identify.
map-identify-more = і ще { $count } у таблиці атрибутів
map-cell = Комірка { $x }, { $y }
map-band = Канал { $n }
map-no-data = Немає даних

## The attribute table.
map-table-title = { $layer }: об’єктів { $shown } з { $total }, вибрано { $selected }
map-table-search = Пошук або поле = значення
map-table-selected-only = Лише вибрані
map-table-empty = Немає відповідних об’єктів.
map-table-many = багато
map-table-stats-number = { $field }: значень { $count }, порожніх { $empty } · мін. { $min } · макс. { $max } · середнє { $mean } · сума { $sum } · станд. відх. { $sd }
map-table-stats-text = { $field }: значень { $count }, порожніх { $empty } · різних { $distinct }
map-table-stats-text-top = { $field }: значень { $count }, порожніх { $empty } · різних { $distinct } · найчастіше { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Клацайте точки вздовж шляху.
map-measure-hint-area = Клацайте кути ділянки.
map-measure-hint-profile = Клацайте точки вздовж лінії.
map-measure-keys = Enter завершує. Backspace вилучає останню точку. Esc закриває.
map-measure-distance = Відстань { $length }
map-measure-bearing = Азимут { $bearing }
map-measure-segment = Остання частина { $length }, азимут { $bearing }
map-measure-area = Площа { $area }
map-measure-perimeter = Периметр { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Довжина { $length } · найнижче { $min } · найвище { $max } · підйом { $climb } · спуск { $descent }
map-profile-no-heights = Довжина { $length }. Немає висот уздовж лінії.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: немає висоти
