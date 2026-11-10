# machine-drafted, needs native review
# Панель метаданих Specular — українська.
#
# Домовленості
# - Числа надходять уже відформатованими для локалі (ICU4X) як рядки, напр.
#   $num = "2 456 789". Де потрібна форма множини, сире число також передається
#   як $n (число) і використовується лише як селектор.
# - Дати й час надходять уже відформатованими ICU4X ($date, $time).
# - Fluent обгортає кожне підставлене значення в ізолятори Unicode.
# - Ідентифікатори повідомлень стабільні; ніколи не використовуйте id повторно з іншим значенням.

## Назви груп огляду

group-camera = Камера
group-exposure = Експозиція
group-lens = Об’єктив
group-date = Дата й час
group-location = Місце
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Дрон
label-gimbal = Підвіс
label-flight = Апарат
label-relative-altitude = Над точкою зльоту
drone-attitude = рискання { $yaw }, тангаж { $pitch }, крен { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Температури
drone-thermal-refused = Не показано: ліцензія SDK від DJI цього не дозволяє
drone-thermal-refused-detail = Щоб прочитати температури DJI, потрібен DJI Thermal SDK, ліцензія якого дозволяє використовувати його лише програмам, які його постачають. Зображення та його метадані показано.
group-image = Зображення
group-file = Файл
group-rights = Права й опис
group-credentials = Облікові дані вмісту
# machine-drafted, needs native review
group-medical = Медичне зображення
# machine-drafted, needs native review
group-inspection = Деталь і контроль
group-nde-image = Зображення контролю (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Зображення контролю
group-science = Наукове зображення
group-em = Електронна мікроскопія
group-spm = Скануюча зондова мікроскопія
group-slide = Препарат
group-microscopy = Мікроскопія
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Дифракція
group-surface = Топографія поверхні
group-ebsd = Дифракція зворотно розсіяних електронів
group-seismic = Сейсмічні дані
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = Зображення CASA
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Трактографія
# machine-drafted, needs native review
group-annotations = Анотації
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Мас-спектрометрична візуалізація
# machine-drafted, needs native review
group-thermal = Термограма
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Проточна цитометрія
# machine-drafted, needs native review
group-spectroscopy = Спектроскопія
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Неструктурована сітка
group-drawing = Креслення
# machine-drafted, needs native review
group-volume = Об’єм

## Мітки пунктів огляду

label-camera = Камера
label-software = Програма
label-application = Програма
label-shutter = Витримка
label-aperture = Діафрагма
label-iso = ISO
label-focal-length = Фокусна відстань
label-exposure-bias = Експокорекція
label-program = Режим
label-metering = Замір експозиції
label-white-balance = Баланс білого
label-flash = Спалах
label-lens = Об’єктив
label-lens-make = Виробник об’єктива
label-lens-range = Діапазон об’єктива
# machine-drafted, needs native review
label-focus-mode = Режим фокусування
label-stabilisation = Стабілізація зображення
label-taken = Знято
label-digitised = Оцифровано
label-edited = Останнє редагування
label-place = Місце
label-coordinates = Координати
label-altitude = Висота
label-direction = Напрямок
label-accuracy = Точність
label-gps-time = Час GPS
label-dimensions = Розміри
label-encoding = Кодування
# machine-drafted, needs native review
label-motion = Рух
motion-kind-motion-photo = Рухоме фото
motion-kind-micro-video = Рухоме фото (MicroVideo)
motion-kind-samsung = Рухоме фото Samsung
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } с
label-colour-profile = Колірний профіль
label-orientation = Орієнтація
label-file-name = Назва
label-folder = Тека
label-file-size = Розмір
label-format = Формат
label-modified = Змінено
label-title = Заголовок
label-description = Опис
label-creator = Автор
label-copyright = Авторське право
label-keywords = Ключові слова
label-rating = Оцінка
label-colour-label = Мітка
label-credentials = Облікові дані вмісту

## Назви груп «Усі поля» (самі назви тегів лишаються як у стандартах)

fields-ifd0 = IFD0 (основне зображення)
fields-exif = Exif
fields-gps = GPS
fields-interop = Сумісність
fields-ifd1 = IFD1 (мініатюра)
fields-icc = Заголовок профілю ICC
# $namespace: з xmp-namespace; $prefix: напр. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Основні
    [xmpRights] Права
    [xmpMM] Керування медіа
    [xmpDM] Динамічні медіа
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Аксесуари камери
    [photoshop] Photoshop
    [crs] Параметри Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] Ліцензування PLUS
    [GPano] Фотосфера
    [GCamera] Камера Google
    [hdrgm] Карта підсилення HDR
    [mwg-rs] Області
    [pdf] PDF
    [stEvt] Події ресурсу
    [stRef] Посилання на ресурси
   *[other] Інше
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Невідомий тег { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } байт)
    [few] ({ $num } байти)
    [many] ({ $num } байтів)
   *[other] ({ $num } байта)
}
maker-note-bytes = ({ $num ->
        [one] { $num } байт
        [few] { $num } байти
        [many] { $num } байтів
       *[other] { $num } байта
    }, примітка виробника, не декодовано)
more-values = { $shown }, … (значень: { $total })

## Загальні з’єднувачі

# Розділяє частини списку, побудованого в коді (опис спалаху, адміністративні одиниці).
list-separator = {", "}
# Розділяє витримку · діафрагму · ISO · фокусну відстань в однорядковому огляді.
one-line-separator = {" · "}

## Експозиція

exposure-time-fraction = 1/{ $denominator } с
exposure-time-seconds = { $seconds } с
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } мм
focal-length-with-equiv = { $mm } мм (екв. { $equiv } мм)
focal-length-equiv-only = екв. { $equiv } мм
# $ev — абсолютне значення; знак задає селектор.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Ручний
    [2] Програмний (авто)
    [3] Пріоритет діафрагми
    [4] Пріоритет витримки
    [5] Творчий (глибина різкості)
    [6] Спорт (коротка витримка)
    [7] Портрет
    [8] Пейзаж
   *[other] Невідомо
}
metering-mode = { $value ->
    [1] Середній
    [2] Центрально-зважений
    [3] Точковий
    [4] Багатоточковий
    [5] Матричний (оцінювальний)
    [6] Частковий
   *[other] Інший
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Ручне фокусування
    [4] Одиночний
    [5] Безперервний
    [6] Ручне фокусування
    [16] Pan focus
    [256] One-Shot AF (живий перегляд)
    [257] AI Servo AF (живий перегляд)
    [258] AI Focus AF (живий перегляд)
    [512] Фокусування Movie Snap
    [519] Movie Servo AF
   *[other] Інший
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Вимкнено
    [1] Увімкнено
    [2] Лише під час зйомки
    [3] Панорамування
    [4] Динамічна
    [256] Вимкнено
    [257] Увімкнено
    [258] Лише під час зйомки
    [259] Панорамування
    [260] Динамічна
   *[other] Інший
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Авто
    [2] Ручне фокусування
    [4] Авто (кнопка фокусування)
    [5] Авто (безперервне)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Інше
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Увімк. (оптична)
    [3] Вимк.
    [4] Увімк. (режим 2)
    [5] Увімк. (оптична, панорамування)
    [6] Увімк. (лише корпус)
    [7] Увімк. (лише корпус, панорамування)
    [9] Dual IS
    [10] Dual IS (панорамування)
    [11] Dual IS 2
    [12] Dual IS 2 (панорамування)
   *[other] Інше
}
label-comment = Коментар
fields-tags = Теги { $kind }
group-picture = Налаштування зображення
fields-fujifilm = Дані виробника Fujifilm
label-film-simulation = Симуляція плівки
label-dynamic-range = Динамічний діапазон
label-highlight-tone = Світла
label-shadow-tone = Тіні
label-colour-setting = Колір
label-sharpness = Різкість
label-noise-reduction = Шумозаглушення
label-clarity = Чіткість
label-grain-effect = Ефект зернистості
label-colour-chrome = Кольоровий хром
label-colour-chrome-blue = Кольоровий хром синій
label-shutter-type = Тип затвора
label-drive-mode = Режим зйомки
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (стандартний)
    [256] Студійний портрет
    [272] Студійний портрет (більше насиченості)
    [288] Astia (м’який)
    [304] Студійний портрет (різкіший)
    [512] Velvia (яскравий)
    [768] Студійний портрет Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Інше
}
fuji-monochrome = { $value ->
    [768] Монохром
    [769] Монохром, червоний фільтр
    [770] Монохром, жовтий фільтр
    [771] Монохром, зелений фільтр
    [784] Сепія
    [1280] Acros
    [1281] Acros, червоний фільтр
    [1282] Acros, жовтий фільтр
    [1283] Acros, зелений фільтр
   *[other] Монохром
}
fuji-white-balance = { $value ->
    [0] Авто
    [1] Авто (пріоритет білого)
    [2] Авто (пріоритет атмосфери)
    [256] Денне світло
    [512] Хмарно
    [768] Лампа денного світла
    [769] Денна біла люмінесцентна
    [770] Біла люмінесцентна
    [771] Тепла біла люмінесцентна
    [772] Тепла біла люмінесцентна для житла
    [1024] Лампа розжарювання
    [1280] Спалах
    [1536] Під водою
    [3840] Власний 1
    [3841] Власний 2
    [3842] Власний 3
    [3843] Власний 4
    [3844] Власний 5
    [4080] Колірна температура
   *[other] Інше
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Авто (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Авто
    [256] Стандартний (DR100)
    [512] Широкий 1 (DR230)
    [513] Широкий 2 (DR400)
   *[other] Інше
}
fuji-effect = { $value ->
    [0] Вимк.
    [32] Слабкий
    [64] Сильний
   *[other] Інше
}
fuji-grain-size = { $value ->
    [16] дрібне
    [32] велике
   *[other] інше
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Механічний
    [1] Електронний
    [2] Електронний (довгі витримки)
    [3] Електронна перша шторка
   *[other] Інше
}
fuji-drive-mode = { $value ->
    [0] Покадрова
    [1] Серійна, повільна
    [2] Серійна, швидка
   *[other] Інше
}
fuji-focus-mode = { $value ->
    [0] Ручний
    [1] AF-S
    [2] AF-C
    [100] Авто
    [101] Ручний
   *[other] Інше
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Немає
    [1] Оптична
    [2] Зсув матриці
    [3] Об’єктив
    [258] Зсув матриці й цифрова
    [512] Цифрова
   *[other] Інше
}
fuji-stabilisation-mode = { $value ->
    [0] Вимк.
    [1] увімк., безперервно
    [2] увімк., під час зйомки
   *[other] Інше
}
light-source = { $value ->
    [1] Денне світло
    [2] Люмінесцентне
    [3] Лампа розжарювання
    [4] Спалах
    [9] Ясна погода
    [10] Хмарно
    [11] Тінь
    [12] Люмінесцентне денного світла
    [13] Люмінесцентне денне біле
    [14] Люмінесцентне холодне біле
    [15] Люмінесцентне біле
    [16] Люмінесцентне тепле біле
    [17] Стандартне світло A
    [18] Стандартне світло B
    [19] Стандартне світло C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Студійна лампа розжарювання
   *[other] Інше
}
white-balance-auto = Авто
white-balance-manual = Ручний
white-balance-manual-with-light = Ручний ({ $light })
# Опис спалаху складається з цих частин, з’єднаних через list-separator.
flash-none = Немає функції спалаху
flash-fired = Спалах спрацював
flash-not-fired = Спалах не спрацював
flash-forced-on = примусово ввімкнено
flash-forced-off = примусово вимкнено
flash-auto = автоматичний режим
flash-return-not-detected = відбите світло не виявлено
flash-return-detected = відбите світло виявлено
flash-red-eye = усунення червоних очей

## Об’єктив

lens-focal-single = { $mm } мм
lens-focal-range = { $min }–{ $max } мм
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Дата й час

# $date: "вівторок, 14 травня 2024 р."; $time: "18:32"
# $weekday: "вівторок" (ICU); $date: "14 травня 2024 р." (ICU)
date-with-weekday = { $weekday }, { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Часовий пояс визначено за годинником GPS

## Місце

# $lat і $lon без знака; півкулю задає селектор.
coordinates = { $lat }° { $latRef ->
    [south] пд. ш.
   *[north] пн. ш.
}, { $lon }° { $lonRef ->
    [west] зх. д.
   *[east] сх. д.
}
altitude = { $ref ->
    [above] { $m } м над рівнем моря
    [below] { $m } м нижче рівня моря
   *[sea] На рівні моря
}
compass-point = { $point ->
    [n] північ
    [nne] північ-північний схід
    [ne] північний схід
    [ene] схід-північний схід
    [e] схід
    [ese] схід-південний схід
    [se] південний схід
    [sse] південь-південний схід
    [s] південь
    [ssw] південь-південний захід
    [sw] південний захід
    [wsw] захід-південний захід
    [w] захід
    [wnw] захід-північний захід
    [nw] північний захід
   *[nnw] північ-північний захід
}
# $point: з compass-point; $degrees: "287"
facing = Напрямок: { $point } ({ $degrees }°)
detail-true-north = Відносно географічної півночі
detail-magnetic-north = Відносно магнітної півночі
accuracy = ±{ $m } м
distance-km = { $km } км
place = { $city }, { $country }
place-city-only = { $city }
# Понад 30 км від знайденого населеного пункту.
place-near = поблизу { $place }
place-detail = { $areas } · { $distance } від { $city }
place-detail-distance = { $distance } від { $city }

## Зображення

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } Мп)
encoding-bit-depth = { $bits } біт
# $format: "8 біт YCbCr 4:2:0"
encoding-progressive = { $format }, прогресивне
encoding-baseline = { $format }, базове
encoding-hierarchical = { $format }, ієрархічне
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] з попередньо помноженою альфою
    [transparency] з прозорістю
   *[alpha] з альфа-каналом
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Відтінки сірого
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Індексовані кольори
    [Multichannel] Багатоканальний
    [Duotone] Дуплекс
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Яскравість/кольоровість (YC)
    [Bayer] RAW Bayer
    [X-Trans] RAW X-Trans
    [CFA] RAW з матрицею кольорових фільтрів
    [LinearRaw] Лінійний RAW
    [RGBE] RGBE (спільний показник степеня)
    [XYZE] XYZE (спільний показник степеня)
    [R] Один канал (R)
    [RG] Два канали (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (з тегу EXIF; вбудованого профілю немає)
orientation = { $value ->
    [1] Пряма (поворот не потрібен)
    [2] Віддзеркалено по горизонталі (застосовано)
    [3] Повернуто на 180° (застосовано)
    [4] Віддзеркалено по вертикалі (застосовано)
    [5] Віддзеркалено по горизонталі й повернуто на 90° проти годинникової стрілки (застосовано)
    [6] Повернуто на 90° за годинниковою стрілкою (застосовано)
    [7] Віддзеркалено по горизонталі й повернуто на 90° за годинниковою стрілкою (застосовано)
    [8] Повернуто на 90° проти годинникової стрілки (застосовано)
   *[other] Невідомо ({ $value })
}
# Коротка форма для попереджень.
orientation-short = { $value ->
    [1] Пряма
    [2] Віддзеркалено по горизонталі
    [3] Повернуто на 180°
    [4] Віддзеркалено по вертикалі
    [5] Віддзеркалено по горизонталі й повернуто на 90° проти годинникової стрілки
    [6] Повернуто на 90° за годинниковою стрілкою
    [7] Віддзеркалено по горизонталі й повернуто на 90° за годинниковою стрілкою
    [8] Повернуто на 90° проти годинникової стрілки
   *[other] значення { $value }
}

## Файл

file-size-bytes = { $num ->
    [one] { $num } байт
    [few] { $num } байти
    [many] { $num } байтів
   *[other] { $num } байта
}
# Десяткові одиниці (1 KB = 1000 байтів).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Права

rating-rejected = Відхилено
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n } з 5)

## Облікові дані вмісту

c2pa-present = Наявні (не перевірено)
c2pa-detail = Цей файл містить маніфест C2PA; його підпис не перевірено.

## Значення заголовка ICC

icc-class = { $class ->
    [scnr] Пристрій введення
    [mntr] Пристрій відображення
    [prtr] Пристрій виведення
    [link] Зв’язок пристроїв
    [spac] Колірний простір
    [abst] Абстрактний
    [nmcl] Іменований колір
   *[other] Невідомо
}
icc-intent = { $intent ->
    [0] Перцептивний
    [1] Відносний колориметричний
    [2] Насиченість
    [3] Абсолютний колориметричний
   *[other] Невідомо
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Не вказано
   *[other] Невідомо
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Модальність
label-body-part = Ділянка тіла
label-study = Дослідження
label-series = Серія
label-slice-thickness = Товщина зрізу
label-pixel-spacing = Відстань між пікселями
label-imager-pixel-spacing = Відстань між пікселями на детекторі
label-frames = Кадри
label-transfer-syntax = Синтаксис передавання
label-document = Документ
label-document-type = Тип документа
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (стиснення з втратами)
detail-pixel-spacing = Відстань між центрами пікселів у тілі пацієнта (рядки × стовпці).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Відстань між центрами пікселів в об’єкті (рядки × стовпці).
detail-imager-pixel-spacing = Виміряно на детекторі без поправки на збільшення: відстані в тілі пацієнта зазвичай менші.

## Промисловий контроль (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Форма деталі
label-nde-component-manufacturer = Виробник деталі
label-nde-material = Матеріал
label-nde-thickness = Товщина матеріалу
label-nde-pipe-diameter = Діаметр труби
label-nde-source = Джерело
label-nde-environment = Середовище
label-nde-iqi = Індикатор якості зображення
label-nde-indications = Індикації
label-nde-standard = Стандарт

## Дослідницькі об’єми (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Розмір вокселя
label-data-type = Тип даних
label-time-points = Часові точки
label-orientation-source = Орієнтація з
label-intent = Значення
label-compression = Стиснення
volume-voxel-size = { $x } × { $y } × { $z } мм
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Заголовок Analyze (ненадійний)
volume-orient-space = Просторові напрямки NRRD
volume-orient-transform = Перетворення MetaImage
volume-orient-spacings = Лише крок вокселів
volume-orient-none = Не вказано
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = Типово у FreeSurfer (ненадійно)
volume-orient-minc = Напрямні косинуси вимірів MINC
volume-orient-object = Координати об’єкта (без орієнтації пацієнта)
volume-content-projections = Проєкції (сире сканування)
volume-content-reconstruction = Реконструйований об’єм
volume-coords-rectilinear = Прямолінійна (нерівномірний крок, показано рівномірно)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Час повторення
label-volume-te = Час луни
label-volume-ti = Час інверсії
label-volume-flip = Кут відхилення
label-volume-fov = Поле огляду
label-volume-field = Індукція поля
label-volume-pedir = Фазове кодування
label-volume-cmdline = Командний рядок
label-volume-xform = Перетворення Talairach
label-volume-content = Вміст
label-volume-projections = Проєкції
label-volume-angles = Діапазон обертання
label-volume-angle-step = Крок обертання
label-volume-kv = Напруга трубки
label-volume-current = Струм трубки
label-volume-power = Потужність джерела
label-volume-exposure = Експозиція
label-volume-sod = Джерело–об’єкт
label-volume-odd = Об’єкт–детектор
label-volume-sdd = Джерело–детектор
label-volume-magnification = Оптичне збільшення
label-volume-binning = Бінінг
label-volume-objective = Об’єктив
label-volume-filter = Фільтр джерела
label-volume-camera = Детектор
label-volume-instrument = Прилад
label-volume-software = Програмне забезпечення
label-volume-date = Отримано
label-volume-ring = Корекція кільцевих артефактів
label-volume-smoothing = Згладжування
label-volume-coords = Сітка
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Око
volume-eye-right = Праве (OD)
volume-eye-left = Ліве (OS)
detail-volume-unreliable = Файл не вказує, куди в тілі пацієнта спрямовані його осі: ліве й праве можуть бути переставлені, тому мітки орієнтації не показано.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Осі належать сканеру, а не пацієнту, тому мітки орієнтації не показано.

## Наукові зображення (FITS, XISF)

label-object = Об’єкт
label-telescope = Телескоп
label-instrument = Прилад
label-filter = Фільтр
label-observed = Спостереження
label-exposure-time = Експозиція
label-sample = Відліки
label-unit = Одиниця
label-hdu = Розширення
label-centre = Центр
label-centre-deg = Центр (градуси)
label-frame = Система координат
# machine-drafted, needs native review
science-frame-galactic = Галактична
science-frame-ecliptic = Екліптична
label-projection = Проєкція
label-pixel-scale = Масштаб пікселя
science-seconds = { $seconds } с
science-arcsec-px = { $arcsec }″ на піксель
label-distortion = Дисторсія
label-wcs = Небесні координати
science-wcs-unsupported = Не підтримується: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Наближені (лише за наведенням, без дисторсії)
label-observer = Спостерігач
label-bayer = Шаблон Байєра
label-capture-start = Початок запису (UTC)
label-capture-end = Кінець запису (UTC)
label-frame-rate = Частота кадрів
science-fps = { $fps } кадрів/с
science-frames-cut = { $frames } з { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Набір даних
label-pixel-size = Розмір пікселя
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } на піксель
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Мікроскоп
label-voltage = Прискорювальна напруга
label-magnification = Збільшення
label-imaging-mode = Режим
label-detector = Детектор
label-acquired = Отримано
label-field-of-view = Поле зору
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Виробник
label-scanner = Сканер
label-objective = Об’єктив
label-pyramid-levels = Рівні
# machine-drafted, needs native review
label-emissivity = Коефіцієнт випромінювання
label-object-distance = Відстань до об’єкта
label-reflected-temperature = Відбита температура
label-atmospheric-temperature = Температура атмосфери
label-humidity = Відносна вологість
label-raw-range = Сирий діапазон
label-temperature-range = Діапазон температур
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Довжина хвилі
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Енергія фотона
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Відстань до детектора
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Центр пучка
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = Кут 2θ детектора
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Фі (φ)
label-omega = Омега (ω)
label-chi = Хі (χ)
label-kappa = Каппа (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Осциляція
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° навколо { $axis }
# The count at which a pixel saturates.
label-overload = Поріг насичення
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Роздільна здатність
xtal-resolution = { $edge } Å на краю, { $corner } Å у кутах
label-geometry = Геометрія
xtal-curved = Вигнута запам’ятовувальна пластина: без q, d і кілець роздільної здатності
xtal-incomplete = Неповний заголовок: без q, d і кілець роздільної здатності
# All-fields group title: "FITS header".
fields-science-header = Заголовок { $format }
# machine-drafted, needs native review
fields-asdf-tree = Дерево ASDF
label-asdf-model = Модель даних
label-asdf-version = Версія файлу
label-asdf-standard = Стандарт ASDF
label-asdf-library = Записано
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = Ключі ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = Версія ROOT
label-root-compression = Стиснення
label-root-keys = Ключі
label-root-class = Клас
label-root-name = Ім’я
label-root-title = Заголовок
label-root-entries = Записи
label-root-sum-weights = Сума ваг
# A graph's number of points.
label-root-points = Точки
# $axis is "x", "y" or "z".
label-root-axis = Вісь { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } бін
        [few] { $bins } біни
        [many] { $bins } бінів
       *[other] { $bins } біна
    }, від { $min } до { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } бін змінної ширини
        [few] { $bins } біни змінної ширини
        [many] { $bins } бінів змінної ширини
       *[other] { $bins } біна змінної ширини
    }, від { $min } до { $max }
label-root-mean-rms = Середнє та RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Дерева
root-tree = { $name } ({ $entries ->
        [one] { $entries } запис
        [few] { $entries } записи
        [many] { $entries } записів
       *[other] { $entries } запису
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Тип зображення
casa-type-image = Відновлене зображення
casa-type-residual = Залишок
casa-type-model = Модель
casa-type-pb = Первинна діаграма
casa-type-mask = Маска очищення
casa-type-psf = Функція розсіювання точки
casa-type-sumwt = Сума ваг
casa-type-weight = Ваги
label-casa-beam = Відновлювальна діаграма
casa-beam = { $major }″ × { $minor }″, ПК { $pa }°
label-casa-plane-beams = Діаграми окремих площин
casa-plane-beams = { $count ->
        [one] { $count } діаграма
        [few] { $count } діаграми
        [many] { $count } діаграм
       *[other] { $count } діаграми
    }, велика вісь від { $min }″ до { $max }″
label-casa-rest-frequency = Частота спокою
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Спектральна система відліку
label-casa-velocity = Визначення швидкості
casa-velocity-radio = Радіо
casa-velocity-optical = Оптична
casa-velocity-relativistic = Релятивістська
label-casa-mask = Типова маска
label-casa-storage = Зберігання
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Сітка
label-ugrid-convention = Конвенція
label-ugrid-nodes = Вузли
label-ugrid-faces = Грані
label-ugrid-edges = Ребра
label-ugrid-shapes = Форми граней
label-ugrid-coordinates = Координати
label-ugrid-values = Значення на
label-ugrid-split = Розділено для малювання
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } трикутник
    [few] { $count } трикутники
    [many] { $count } трикутників
   *[other] { $count } трикутника
}
ugrid-quads = { $count ->
    [one] { $count } чотирикутник
    [few] { $count } чотирикутники
    [many] { $count } чотирикутників
   *[other] { $count } чотирикутника
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } многокутник із { $max } вузлами
    [few] { $count } многокутники з не більш ніж { $max } вузлами
    [many] { $count } многокутників з не більш ніж { $max } вузлами
   *[other] { $count } многокутника з не більш ніж { $max } вузлами
}
ugrid-lonlat = Довгота й широта
ugrid-projected = Проєктовані x і y
ugrid-projected-unit = Проєктовані x і y ({ $unit })
ugrid-on-faces = Грані (суцільне зафарбування)
ugrid-on-nodes = Вузли (інтерпольовано по гранях)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Немає: зображення показує кількість вузлів на грань
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } грань на антимеридіані або полюсі
    [few] { $count } грані на антимеридіані або полюсі
    [many] { $count } граней на антимеридіані або полюсі
   *[other] { $count } грані на антимеридіані або полюсі
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Файл
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Фігури
label-annotation-groups = Групи
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Пропущено
annotation-polygon = Багатокутник
annotation-polyline = Лінія
annotation-rectangle = Прямокутник
annotation-ellipse = Еліпс
annotation-point = Точка
annotation-points = Точки
annotation-ruler = Лінійка
annotation-arrow = Стрілка
# The list of annotations is cut short: "and 1,200 more".
annotation-more = і ще { $count }
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Анотації в { $file }
label-drawing-version = Версія
label-drawing-units = Одиниці
label-drawing-extents = Межі
label-drawing-sheet = Аркуш
label-drawing-layouts = Макети
label-drawing-layers = Шари
label-drawing-entities = Об’єкти
label-drawing-codepage = Кодова сторінка
label-drawing-missing-fonts = Замінені шрифти
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (стилі { $styles }), замінено на { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (стилі { $styles }), замінено резервними шрифтами
# Survey data (LandXML).
label-survey-crs = Система координат
survey-crs-none = Не вказано
label-survey-project = Назва проєкту
label-survey-application = Записано
label-survey-surface = Модель рельєфу
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: точок { $points }, трикутників { $triangles }; площа в плані { $area } { $unit }², фактична площа { $area3d } { $unit }²; { $volume } { $unit }³ над найнижчою точкою; позначки { $zmin }–{ $zmax } { $unit }; горизонталі через { $interval } { $unit }
label-survey-alignment = Вісь траси
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, пікетаж від { $start } до { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = Шрифти SHX
label-drawing-warnings = Проблеми
label-plot-paper = Папір
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } з { $pages }
label-plot-page = Сторінка
label-plot-pens = Використані пера
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Завдання друку
# What a plot draws: strokes, fills and labels.
label-plot-items = Намальовані елементи
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Модель
# "24, 3 off or frozen"
drawing-layers-off = { $num }, з них { $off } вимкнено або заморожено
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Дюйми
    [2] Фути
    [3] Милі
    [4] Міліметри
    [5] Сантиметри
    [6] Метри
    [7] Кілометри
    [8] Мікродюйми
    [9] Тисячні дюйма
    [10] Ярди
    [11] Ангстреми
    [12] Нанометри
    [13] Мікрони
    [14] Дециметри
    [15] Декаметри
    [16] Гектометри
    [17] Гігаметри
    [18] Астрономічні одиниці
    [19] Світлові роки
    [20] Парсеки
    [21] Геодезичні фути США
   *[other] Без одиниць
}
# All-fields group title: "DXF entities".
fields-drawing-entities = Об’єкти { $format }

## Топології ІС (GDSII)

# machine-drafted, needs native review
label-layout-library = Бібліотека
label-layout-units = Одиниці
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } — одиниця бази даних, { $user } — одиниця користувача
# The top cell shown (each top cell is a page).
label-layout-cell = Комірка
label-layout-cells = Комірки
# Cells no other cell places.
label-layout-top-cells = Комірки верхнього рівня
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } та ще { $more }
# Layer/datatype pairs.
label-layout-layers = Шари
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Фігури
label-layout-texts = Мітки
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Посилання
layout-references = { $refs } ({ $placements } { $placements ->
    [one] розміщення
    [few] розміщення
    [many] розміщень
   *[other] розміщення
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Фігури після сплощення
label-layout-bounds = Межі
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } до { $x1 }, { $y1 } µm

## Друковані плати й схеми (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Друкована плата
group-schematic = Схема
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Вигляд
label-pcb-project = Проєкт
label-pcb-revision = Ревізія
label-pcb-date = Дата
# The CAD program that wrote the file.
label-pcb-generator = Створено в
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Розмір плати
label-pcb-thickness = Товщина
label-pcb-copper-layers = Шари міді
label-pcb-sheets = Аркуші
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Шари
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Посадкові місця
label-pcb-symbols = Символи
label-pcb-nets = Кола
label-pcb-tracks = Доріжки
label-pcb-wires = Провідники
label-pcb-vias = Перехідні отвори
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Отвори
pcb-holes = { $holes } у { $sizes } { $sizes ->
    [one] розмірі
    [few] розмірах
    [many] розмірах
   *[other] розмірах
}
label-pcb-apertures = Апертури
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Внутрішня мідь { $n }
pcb-role-top-copper = Верхня мідь
pcb-role-bottom-copper = Нижня мідь
pcb-role-top-solder-mask = Верхня паяльна маска
pcb-role-bottom-solder-mask = Нижня паяльна маска
pcb-role-top-silkscreen = Верхнє маркування
pcb-role-bottom-silkscreen = Нижнє маркування
pcb-role-top-paste = Верхня паста
pcb-role-bottom-paste = Нижня паста
pcb-role-top-solder-paste = Верхня паяльна паста
pcb-role-bottom-solder-paste = Нижня паяльна паста
pcb-role-top-adhesive = Верхній клей
pcb-role-bottom-adhesive = Нижній клей
pcb-role-top-courtyard = Верхня зона компонента
pcb-role-bottom-courtyard = Нижня зона компонента
pcb-role-top-fabrication = Верхній виробничий шар
pcb-role-bottom-fabrication = Нижній виробничий шар
pcb-role-board-outline = Контур плати
pcb-role-plated-drill = Металізовані отвори
pcb-role-non-plated-drill = Неметалізовані отвори
pcb-role-drill = Свердління
pcb-role-drill-holes = Свердлені отвори
pcb-role-mechanical = Механічний
pcb-role-margin = Поле
pcb-role-drawings = Креслення
pcb-role-comments = Коментарі
pcb-role-engineering-change-1 = Технічна зміна 1
pcb-role-engineering-change-2 = Технічна зміна 2
pcb-role-vias = Перехідні отвори
pcb-role-other = Інший
pcb-role-unknown = Невідомий
label-pcb-macros = Макроси апертур

## Спектри (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Спектр
label-spectrum-format = Формат
label-spectrum-technique = Метод
label-spectrum-title = Заголовок
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Сторінка
spectrum-page-of = { $page } з { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = Вісь X
label-spectrum-y = Вісь Y
label-spectrum-nucleus = Ядро
label-spectrum-frequency = Частота спектрометра
label-spectrum-solvent = Розчинник
label-spectrum-pulse-program = Імпульсна програма
label-spectrum-spectra = Спектри
label-spectrum-points = Точки
label-spectrum-instrument = Прилад
# Who or what lab measured it.
label-spectrum-origin = Походження
label-spectrum-date = Виміряно
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Обробка
spectrum-technique-ir = Інфрачервоний
spectrum-technique-raman = Раманівський
spectrum-technique-uv-vis = УФ-видимий
spectrum-technique-fluorescence = Флуоресценція
spectrum-technique-nmr = ЯМР
spectrum-technique-ms = Мас-спектр
spectrum-technique-chromatogram = Хроматограма
spectrum-technique-other = Спектр

## 3D-моделі (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D-модель
label-triangles = Трикутники
label-vertices = Вершини
label-materials = Матеріали
label-textures = Текстури
label-up-axis = Вертикальна вісь
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } (бракує: { $missing })

## Молекулярні структури (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Структура
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Запис
label-structure-classification = Класифікація
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Метод
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Роздільна здатність
structure-resolution = { $value } Å
label-structure-deposited = Депоновано
label-structure-formula = Формула
label-structure-chains = Ланцюги
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Залишки
label-structure-atoms = Атоми
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } в асиметричній одиниці)
label-structure-bonds = Зв’язки
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Моделі
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Записи
structure-first-of = { $count } (показано перший)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ліганди
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } та ще { $more }
label-structure-cell = Елементарна комірка
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Просторова група
label-structure-secondary = Вторинна структура
structure-ss-file = З файлу
structure-ss-computed = Обчислено за основним ланцюгом

## HDF4 і HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Бібліотека
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Набори даних
label-hdf4-images = Растрові зображення
label-hdf4-vgroups = Групи
label-hdf4-vdatas = Таблиці
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Версія
# The grids, swaths and point structures the file defines.
label-eos-structures = Структури
eos-grids = { $count ->
    [one] { $count } сітка ({ $names })
    [few] { $count } сітки ({ $names })
    [many] { $count } сіток ({ $names })
   *[other] { $count } сітки ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } смуга огляду ({ $names })
    [few] { $count } смуги огляду ({ $names })
    [many] { $count } смуг огляду ({ $names })
   *[other] { $count } смуги огляду ({ $names })
}
eos-points = { $count ->
    [one] { $count } точкова структура
    [few] { $count } точкові структури
    [many] { $count } точкових структур
   *[other] { $count } точкової структури
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Структура
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Проєкція
label-eos-grid-size = Розмір сітки
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Кути
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Продукт
# When the data was acquired.
label-eos-time = Час
eos-time = з { $begin } до { $end }

## Планетні зображення (VICAR, PDS3, PDS4: дані місій NASA та ESA)

# machine-drafted, needs native review
group-planetary = Планетне зображення
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Ціль
label-planetary-mission = Місія
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Космічний апарат
label-planetary-start = Час початку
label-planetary-stop = Час завершення
# The product's identifier in the archive.
label-planetary-product = ID продукту
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Картографічна проєкція
label-planetary-resolution = Роздільна здатність
planetary-metres-per-pixel = { $metres } м на піксель
planetary-pixels-per-degree = { $pixels } px на градус
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Центр проєкції
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Охоплення
planetary-coverage = широта від { $south }° до { $north }°, довгота від { $west }° до { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Радіус тіла
planetary-km = { $km } км

## OMERO (зображення, відкриті із сервера OMERO або Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Сервер
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (як { $user })
omero-server-public = { $server } (публічний)
label-omero-image = Зображення
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Проєкт
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Скрин
label-omero-owner = Власник
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Отримано
label-omero-dimensions = Розміри
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } площина Z
    [few] { $count } площини Z
    [many] { $count } площин Z
   *[other] { $count } площини Z
}
omero-c = { $count ->
    [one] { $count } канал
    [few] { $count } канали
    [many] { $count } каналів
   *[other] { $count } каналу
}
omero-t = { $count ->
    [one] { $count } часова точка
    [few] { $count } часові точки
    [many] { $count } часових точок
   *[other] { $count } часової точки
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Розмір пікселя
label-omero-channels = Канали
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (приховано)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Отримується як
omero-fetched-tiles = плитки { $encoding } { $width } × { $height }, { $levels ->
    [one] { $levels } рівень
    [few] { $levels } рівні
    [many] { $levels } рівнів
   *[other] { $levels } рівня
}
omero-fetched-whole = Цілі площини як { $encoding }

## Зображення перетворення відбиття (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Перетворення відбиття
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Базис
label-rti-order = Порядок
# Coefficients per colour channel.
label-rti-terms = Коефіцієнти
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Розміщення кольорів
# Coefficient images stored in the file.
label-rti-planes = Площини
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Масштаб
label-rti-bias = Зміщення

## Хмари точок

# machine-drafted, needs native review
group-points = Хмара точок
label-points = Точки
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } з { $total }
# The LAS point data record format number (0–10).
label-point-format = Формат точок
# The coordinate reference system.
label-crs = Система координат
# What each point carries besides its position.
label-attributes = Атрибути
points-channel-rgb = RGB
points-channel-intensity = Інтенсивність
points-channel-classification = Класифікація
points-channel-returns = Відбиття
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Класи
# E57 scans, with point counts.
label-scans = Скани
point-class-0 = Ніколи не класифіковано
point-class-1 = Некласифіковано
point-class-2 = Ґрунт
point-class-3 = Низька рослинність
point-class-4 = Середня рослинність
point-class-5 = Висока рослинність
point-class-6 = Будівля
point-class-7 = Низька точка (шум)
point-class-8 = Ключова точка моделі
point-class-9 = Вода
point-class-10 = Залізниця
point-class-11 = Дорожнє покриття
point-class-12 = Перекриття
point-class-13 = Грозозахисний трос
point-class-14 = Фазний провід
point-class-15 = Опора ЛЕП
point-class-16 = З’єднувач проводів
point-class-17 = Мостове полотно
point-class-18 = Високий шум
point-class-19 = Надземна конструкція
point-class-20 = Ігнорований ґрунт
point-class-21 = Сніг
point-class-22 = Часове виключення
# A class without a standard name, by number.
point-class-other = Клас { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Гаусові плями (Gaussian splats)
label-splats = Плями
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } з { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Колір залежно від напрямку погляду
splats-harmonics-none = Немає
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, навчено зі згладжуванням

## Попередження

# machine-drafted, needs native review
warn-not-diagnostic = Не для діагностики. Specular — загальний переглядач зображень, а не медичний виріб: його відображення не калібровано для діагностики.

warn-gps = Цей файл містить місце зйомки. Якщо воно має лишитися приватним, вилучіть його перед поширенням.
warn-null-island = GPS-позиція точно 0°, 0°; ймовірно, камера не мала сигналу, тож місце, найпевніше, хибне.
warn-date-future = Дата зйомки в майбутньому; ймовірно, годинник камери було налаштовано неправильно.
warn-icc-damaged = Вбудований колірний профіль пошкоджено, його не вдалося прочитати.
warn-metadata-failed = Не вдалося прочитати метадані через помилку в Specular. Саме зображення в порядку.
warn-orientation-conflict = EXIF і XMP вказують різну орієнтацію (EXIF: { $exif }, XMP: { $xmp }). Використано значення EXIF.
warn-exif-damaged = Частину даних EXIF не вдалося прочитати ({ $error }); решту показано.
warn-xmp-damaged = Метадані XMP мають неправильний формат ({ $error }); вдалося прочитати лише частину.

## Вікно «Про програму»

geonames-attribution = Назви місць: GeoNames (www.geonames.org), ліцензія Creative Commons Attribution 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Офлайн-пошук за допомогою крейту reverse_geocoder від Grant Miner (MIT або Apache-2.0). Дані перетворено на список міст; вони надаються «як є», без жодних гарантій.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Зображення в повній роздільній здатності
    [1] Зображення в зменшеній роздільній здатності
    [2] Одна сторінка багатосторінкового зображення
    [4] Маска прозорості
   *[other] Інше
}
predictor = { $value ->
    [1] Немає
    [2] Горизонтальна різниця
    [3] Різниця з рухомою комою
   *[other] Інше
}
extra-samples = { $value ->
    [0] Не вказано
    [1] Попередньо помножена альфа
    [2] Пряма альфа
   *[other] Інше
}
sample-format = { $value ->
    [1] Ціле без знака
    [2] Ціле зі знаком
    [3] Рухома кома
    [4] Не визначено
   *[other] Інше
}

## Геопросторові дані (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Геопросторові дані
label-geo-crs = Система координат
label-geo-datum = Геодезичний датум
label-geo-projection = Проєкція
label-geo-unit = Одиниці
label-geo-bounds = Охоплення
label-geo-lonlat-bounds = Довгота і широта
label-geo-pixel-size = Розмір пікселя
label-geo-overviews = Огляди
label-geo-nodata = Значення NoData
label-geo-raster-type = Тип растра
label-geo-vertical = Висотна система
label-geo-citation = Джерело
label-geo-features = Об’єкти
label-geo-vertices = Вершини
label-geo-layer = Шар
label-geo-layers = Шари
label-geo-tiles = Тайли
label-geo-encoding = Кодування тексту
label-geo-warnings = Проблеми
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Супровідні файли
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (координати не підтримуються)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = До WGS 84
geo-to-wgs84-same = Не потрібно
geo-to-wgs84-helmert = Зміна датуму (Helmert)
geo-to-wgs84-by-area = Зміна датуму (Helmert, вибрано за областю)
geo-to-wgs84-unknown = Невідомо: координати у власному датумі файлу
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Не вказано
# Map bounds: the lower-left and upper-right corners.
geo-bounds = від { $x0 }, { $y0 } до { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = довгота від { $west }° до { $east }°, широта від { $south }° до { $north }°
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } огляд: { $sizes }
    [few] { $n } огляди: { $sizes }
    [many] { $n } оглядів: { $sizes }
   *[other] { $n } огляду: { $sizes }
}
geo-pixel-is-point = Пікселі є точками
geo-layer = { $features ->
    [one] { $features } об’єкт
    [few] { $features } об’єкти
    [many] { $features } об’єктів
   *[other] { $features } об’єкта
}, { $fields ->
    [one] { $fields } поле
    [few] { $fields } поля
    [many] { $fields } полів
   *[other] { $fields } поля
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, рівень масштабу { $zoom }, { $tiles ->
    [one] { $tiles } тайл
    [few] { $tiles } тайли
    [many] { $tiles } тайлів
   *[other] { $tiles } тайла
}
fields-geo-geokeys = Ключі GeoTIFF
fields-geo-gdal = Метадані GDAL
fields-geo-wkt = Система координат (WKT)
fields-geo-projjson = Система координат (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Набір плиток
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Джерело даних
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Заголовок
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = Версія GIFTI
label-gifti-role = Містить
gifti-role-surface = Поверхню
gifti-role-labels = Мітки для вершин
gifti-role-data = Значення для вершин
label-gifti-arrays = Масиви даних
label-gifti-encoding = Кодування
label-gifti-vertices = Вершини
label-gifti-triangles = Трикутники
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Структура
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Геометричний тип
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Координати
gifti-transform = { $from } (перетворення в { $to } не потрібне)
gifti-transform-applied = { $from } → { $to } (застосовано)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Показано на
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Масив
label-gifti-intent = Intent
label-gifti-data-type = Тип даних
# Entries of the label table (names and colours of regions).
label-gifti-labels = Таблиця міток
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = Версія CIFTI
label-cifti-intent = Intent
label-cifti-matrix = Матриця
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Карти
cifti-maps-scalars = Скалярні карти: { $count }
cifti-maps-labels = Карти міток: { $count }
cifti-maps-series = Часові точки: { $count }
cifti-maps-parcels = Парцели: { $count }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Зв’язність
cifti-dense = Seed-точки: { $seeds }, { $size } читається на точку
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Серія
cifti-series = від { $start }, крок { $step } ({ $unit })
label-cifti-models = Моделі мозку
label-cifti-parcels = Парцели
label-cifti-structures = Структури
label-cifti-voxels = Підкіркові воксели
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Поверхня
cifti-surface-found = { $structure }: вершин: { $vertices }, { $file }
cifti-surface-missing = { $structure }: потрібна поверхня з { $vertices ->
        [one] { $vertices } вершиною
        [few] { $vertices } вершинами
        [many] { $vertices } вершинами
       *[other] { $vertices } вершини
    } (поруч із файлом немає)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Показано як
cifti-shown-surfaces = Розфарбовані поверхні
cifti-shown-voxels = Об’єм підкіркових вокселів
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Зв’язність вибраної seed-точки
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Пропущені карти
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Формат
label-tract-streamlines = Лінії струму
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } (показано { $shown })
label-tract-points = Точки
label-tract-mean-length = Середня довжина
# A length in millimetres.
tract-mm = { $value } мм
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Скаляри для точок
label-tract-properties = Властивості для ліній струму
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Сітка
label-tract-voxel-size = Розмір вокселя
label-tract-voxel-order = Порядок вокселів
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Координати
tract-space-vox-to-ras = З матриці воксель–RAS
tract-space-voxel-order = З порядку вокселів (без матриці воксель–RAS)
tract-space-scanner = Простір сканера
# MRtrix: the tracking step size.
label-tract-step = Розмір кроку

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Заголовок файлу
label-header-created = Створено
# The program or person that wrote the file.
label-header-creator = Автор
label-header-copyright = Авторське право
label-header-project = Проєкт
label-header-comment = Коментар
# The scanner, camera or other device the image came from.
label-header-device = Пристрій
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Тайм-код
# A film frame's edge code (KeyKode).
label-header-edge-code = Краєвий код
label-header-frame-rate = Частота кадрів
# Where the frame sits in its sequence.
label-header-frame = Кадр
# Film stock and slate.
label-header-film = Плівка
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Передавальна функція
# The colour primaries the file states.
label-header-colorimetric = Колориметрія
# The X window a screen dump was taken of.
label-header-window = Вікно
label-header-resolution = Роздільна здатність
label-header-compression = Стиснення
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Профіль
# machine-drafted, needs native review
label-header-level = Рівень
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Рівні вейвлет-розкладу
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Колірне перетворення
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Монтажні області
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Не відображено
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Відображено приблизно

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Шрифт
label-font-family = Гарнітура
label-font-style = Накреслення
# The face's place in a font collection: "2 of 4".
label-font-face = Шрифт у колекції
font-face-of = { $index } з { $count }
label-font-version = Версія
label-font-designer = Дизайнер
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Словолитня
label-font-glyphs = Гліфи
label-font-outlines = Контури
font-outlines-truetype = TrueType (квадратичні)
font-outlines-cff = PostScript (CFF, кубічні)
font-outlines-cff2 = PostScript (CFF2, кубічні)
font-outlines-bitmap = Лише растри
font-outlines-none = Немає
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Вбудовування
font-embedding-installable = Можна вбудовувати й встановлювати.
font-embedding-restricted = Не можна вбудовувати.
font-embedding-preview-print = Можна вбудовувати лише для перегляду й друку.
font-embedding-editable = Можна вбудовувати в документи, які редагуються.
font-embedding-no-subsetting = Лише повністю, не підмножиною.
font-embedding-bitmap-only = Лише як растри.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Осі варіацій
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Іменовані стилі
label-font-colour = Кольорові гліфи
font-colour-colr0 = Шари COLR
font-colour-colr1 = Градієнти COLR v1
font-colour-sbix = Растри (sbix)
font-colour-cbdt = Растри (CBDT)
font-colour-svg = SVG
label-font-weight = Насиченість
label-font-units-per-em = Одиниць на em
label-font-tables = Таблиці
label-font-licence = Ліцензія
label-font-licence-url = URL ліцензії
label-font-copyright = Авторське право
label-font-trademark = Торговельна марка

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Документ
label-document-kind = Тип
document-kind-word = Документ Word
document-kind-excel = Книга Excel
document-kind-powerpoint = Презентація PowerPoint
document-kind-pages = Документ Pages
document-kind-numbers = Таблиця Numbers
document-kind-keynote = Презентація Keynote
document-kind-odf-text = Текст OpenDocument
document-kind-odf-spreadsheet = Електронна таблиця OpenDocument
document-kind-odf-presentation = Презентація OpenDocument
document-kind-odf-graphics = Малюнок OpenDocument
document-kind-odf-other = Файл OpenDocument
# machine-drafted, needs native review
document-kind-playlist = Список відтворення
# machine-drafted, needs native review
label-playlist-items = Елементи
# Appended to the kind: "Word document, template, with macros".
document-template = шаблон
document-macros = з макросами
document-legacy = формат Office 97–2003
label-document-preview = Попередній перегляд
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = Зображення { $format }
document-preview-none = Немає у файлі
label-document-subject = Тема
label-document-author = Автор
label-document-description = Опис
label-document-category = Категорія
label-document-last-saved-by = Ким збережено
label-document-created = Створено
label-document-modified = Змінено
label-document-printed = Надруковано
label-document-pages = Сторінки
label-document-slides = Слайди
label-document-hidden-slides = Приховані слайди
label-document-notes = Сторінки нотаток
label-document-sheets = Аркуші
label-document-words = Слова
label-document-characters = Символи
label-document-paragraphs = Абзаци
label-document-lines = Рядки
label-document-editing-time = Час редагування
document-minutes = { $minutes } хв
label-document-revision = Редакція
label-document-application = Збережено в
label-document-company = Організація
label-document-template = Шаблон

## Books (EPUB)

# machine-drafted, needs native review
group-book = Книга
label-book-authors = Автори
label-book-publisher = Видавець
label-book-date = Видано
label-book-language = Мова
label-book-pages = Сторінки
# Entries in the book's table of contents.
label-book-contents = Пункти змісту
label-book-layout = Верстка
book-layout-reflowable = Адаптивна
book-layout-fixed = Фіксована верстка
book-right-to-left = справа наліво
label-book-version = Формат
book-version = EPUB { $version }
label-book-fonts = Вбудовані шрифти
book-fonts-obfuscated = { $count } (приховано: { $obfuscated })
label-book-subjects = Теми
label-book-identifier = Ідентифікатор
label-book-rights = Права

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

# machine-drafted, needs native review
volume-orient-parrec = Кут і зміщення Philips
volume-orient-ecat = Розміри вокселів і зміщення ECAT (ненадійні)
label-volume-technique = Методика
label-volume-version = Версія формату
label-volume-isotope = Ізотоп
label-volume-tracer = Трейсер
label-volume-units = Одиниці

# Text, tables and subtitles.
label-header-encoding = Кодування
label-header-lines = Рядки
label-header-line-ends = Кінці рядків
label-header-words = Слова
label-header-pages = Сторінки
label-header-pictures = Зображення
label-header-pictures-not-shown = Не показані зображення
label-header-tables = Таблиці
label-header-rows = Рядки таблиці
label-header-columns = Стовпці
label-header-delimiter = Роздільник
label-header-column-names = Назви стовпців
label-header-cues = Субтитри
label-header-cue-span = Субтитри від – до
