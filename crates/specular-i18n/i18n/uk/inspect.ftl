# machine-drafted, needs native review
# Інструменти аналізу: попередження про відсічення, підсвічування фокуса, гістограма, піпетка пікселів.

## Палітра команд
palette-clipping = Показати або сховати відсічення
palette-focus-peaking = Показати або сховати підсвічування фокуса
palette-histogram = Показати або сховати гістограму
palette-pixel-inspector = Показати або сховати інспектор пікселів
# machine-drafted, needs native review
palette-crosshair = Клавіатурний приціл
palette-announce-cursor = Читати значення під курсором

## Панель гістограми
histogram-title = Гістограма
# Частка видимих пікселів на рівнях відсічення або за ними; значення — відсотки.
histogram-clipped = Відсічено: світлі { $high }, тіні { $low }
histogram-empty = Немає видимого зображення
# Підсумок для екранного читача. $mean — середня яскравість (0–255), $high/$low — відсотки.
histogram-a11y = Гістограма видимого зображення: середня яскравість { $mean }, відсічені світлі ділянки { $high }, відсічені тіні { $low }

## Панель інспектора пікселів
inspector-title = Інспектор пікселів
inspector-position = Позиція
inspector-source-rgb = Вихідний RGB
inspector-source-rgba = Вихідний RGBA
inspector-source-ycc = Вихідний Y′CbCr
inspector-source-gray = Вихідний сірий
inspector-source-gray16 = Вихідний сірий (16 біт)
inspector-source-gray-alpha16 = Вихідний сірий + альфа (16 біт)
inspector-source-rgb16 = Вихідний RGB (16 біт)
inspector-source-rgba16 = Вихідний RGBA (16 біт)
inspector-source-pending = Вихідне
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Лінійний
# Показується, коли значення взято зі зменшеного попереднього перегляду, а не з пікселів повної роздільності.
inspector-approximate = Зі зменшеного перегляду; збільште для точних значень
inspector-outside = Наведіть на зображення, щоб оглянути піксель
# Підсумок для екранного читача: позиція пікселя та його значення sRGB.
inspector-a11y = Піксель у { $x }, { $y }: sRGB { $value }

## Налаштування
settings-section-inspection = Аналіз
settings-inspection-clip-highlight = Рівень відсічення світлих
settings-inspection-clip-highlight-help = Пікселі, у яких будь-який канал дорівнює цьому 8-бітовому значенню або перевищує його, позначаються як відсічені світлі.
settings-inspection-clip-shadow = Рівень відсічення тіней
settings-inspection-clip-shadow-help = Пікселі, у яких будь-який канал дорівнює цьому 8-бітовому значенню або менший за нього, позначаються як відсічені тіні.
settings-inspection-highlight-color = Колір попередження для світлих
settings-inspection-highlight-color-help = Колір, яким позначено відсічені світлі ділянки.
settings-inspection-shadow-color = Колір попередження для тіней
settings-inspection-shadow-color-help = Колір, яким позначено відсічені тіні.
settings-inspection-peaking-sensitivity = Чутливість підсвічування фокуса
settings-inspection-peaking-sensitivity-help = Вищі значення позначають як різкі й м’якші краї.
settings-inspection-peaking-color = Колір підсвічування фокуса
settings-inspection-peaking-color-help = Колір, яким позначено різкі краї.
settings-inspection-histogram-channels = Канали гістограми
settings-inspection-histogram-channels-help = Які криві показує гістограма.
settings-inspection-histogram-channels-colour = Червоний, зелений і синій
settings-inspection-histogram-channels-luminance = Яскравість
settings-inspection-histogram-channels-both = Колір і яскравість
settings-inspection-histogram-log = Логарифмічна гістограма
settings-inspection-histogram-log-help = Логарифмічна шкала гістограми, щоб малі значення лишалися помітними.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Показати червоний канал
palette-channel-green = Показати зелений канал
palette-channel-blue = Показати синій канал
palette-channel-alpha = Показати альфа-канал (ще раз: на шаховому тлі, потім на кольорі)
palette-channel-luma = Показати яскравість
channel-normal = Усі канали
channel-red = Червоний канал
channel-green = Зелений канал
channel-blue = Синій канал
channel-alpha = Альфа-канал
channel-alpha-checker = Прозорість на шаховому тлі
channel-alpha-matte = Прозорість на кольорі
channel-luma = Яскравість
disabled-no-alpha = Це зображення не має прозорості
settings-inspection-alpha-matte-color = Колір тла для альфи
settings-inspection-alpha-matte-color-help = Суцільний колір за зображенням у перегляді каналу «Прозорість на кольорі».

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Показати або сховати псевдоколір
palette-next-colour-map = Наступна колірна карта
palette-channel-colours = Наступні кольори каналів (червоний–зелений, пурпуровий–зелений, пурпуровий–жовтий–блакитний)
palette-hilo = Показати або сховати HiLo (пікселі на максимумі й мінімумі бітової глибини)
palette-colour-vision = Наступна симуляція колірного зору
palette-colour-vision-off = Припинити симуляцію колірного зору
palette-colour-vision-protanopia = Симулювати протанопію (без червоних колбочок)
palette-colour-vision-deuteranopia = Симулювати дейтеранопію (без зелених колбочок)
palette-colour-vision-tritanopia = Симулювати тританопію (без синіх колбочок)
palette-colour-vision-achromatopsia = Симулювати ахроматопсію (без кольору)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (не безпечна для дальтоніків)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = сирі значення
# Screen reader summary of the colour bar.
colour-bar-a11y = Колірна шкала { $map }: від { $low } до { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Псевдоколір: { $map }
badge-hilo = HiLo: позначено пікселі на максимумі й мінімумі
badge-colour-vision-protanopia = Симуляція протанопії
badge-colour-vision-deuteranopia = Симуляція дейтеранопії
badge-colour-vision-tritanopia = Симуляція тританопії
badge-colour-vision-achromatopsia = Симуляція ахроматопсії
channel-colours-red-green = Канали: червоний, зелений, синій
channel-colours-magenta-green = Канали: пурпуровий, зелений
channel-colours-magenta-yellow-cyan = Канали: пурпуровий, жовтий, блакитний
settings-inspection-false-colour-map = Колірна карта псевдокольору
settings-inspection-false-colour-map-help = Колірна карта для псевдокольору та його шкали. Усі, крім jet і phase, лишаються читабельними за будь-якого колірного зору.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (розбіжна)
settings-inspection-false-colour-map-vik = Vik (розбіжна)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (розбіжна)
settings-inspection-false-colour-map-curl = Curl (розбіжна)
settings-inspection-false-colour-map-phase = Phase (циклічна)
settings-inspection-false-colour-map-jet = Jet (не безпечна для дальтоніків)
settings-inspection-colour-bar = Колірна шкала
settings-inspection-colour-bar-help = Показувати колірну шкалу зі значеннями у власних одиницях зображення, коли ввімкнено псевдоколір.
settings-inspection-channel-colours = Кольори каналів
settings-inspection-channel-colours-help = Як показувати колірні канали. Пурпуровий–зелений і пурпуровий–жовтий–блакитний лишаються розрізненими за червоно-зеленого дальтонізму.
settings-inspection-channel-colours-red-green = Червоний, зелений, синій
settings-inspection-channel-colours-magenta-green = Пурпуровий, зелений
settings-inspection-channel-colours-magenta-yellow-cyan = Пурпуровий, жовтий, блакитний
settings-inspection-colour-vision = Симуляція колірного зору
settings-inspection-colour-vision-help = Показувати все вікно так, як його бачать із порушенням колірного зору (Machado et al. 2009). На експорт це ніколи не впливає.
settings-inspection-colour-vision-off = Вимк.
settings-inspection-colour-vision-protanopia = Протанопія
settings-inspection-colour-vision-deuteranopia = Дейтеранопія
settings-inspection-colour-vision-tritanopia = Тританопія
settings-inspection-colour-vision-achromatopsia = Ахроматопсія
settings-inspection-auto-range = Автодіапазон для сірих зображень високої розрядності
settings-inspection-auto-range-help = Розтягувати 16-бітні сірі зображення (TIFF, PNG, FITS) від найтемнішого до найяскравішого значення. Зображення DICOM зберігають власне вікно.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Приціл увімкнено. Стрілки переміщують його, Shift для більших кроків. Enter клацає. Esc завершує.
crosshair-off = Приціл вимкнено
announce-cursor-nothing = Під курсором нічого немає
