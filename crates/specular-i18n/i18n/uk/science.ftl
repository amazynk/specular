# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Українська.

keys-mode-science = Наукові зображення
cheatsheet-science = Наукові зображення
cheatsheet-science-look = Розтягування та межі
cheatsheet-science-planes = Площини та канали
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Показати або сховати канали 1–9
cheatsheet-science-channels-10-16 = Показати або сховати канали 10–16
palette-science-stretch = Наступне розтягування
palette-science-limits = Наступні межі
palette-science-next-plane = Наступна площина
palette-science-prev-plane = Попередня площина
palette-science-next-axis = Гортати іншу вісь
palette-science-composite = Скласти канали в кольорі
palette-science-channel-1 = Показати або сховати канал 1
palette-science-channel-2 = Показати або сховати канал 2
palette-science-channel-3 = Показати або сховати канал 3
palette-science-channel-4 = Показати або сховати канал 4
palette-science-channel-5 = Показати або сховати канал 5
palette-science-channel-6 = Показати або сховати канал 6
palette-science-channel-7 = Показати або сховати канал 7
palette-science-channel-8 = Показати або сховати канал 8
palette-science-channel-9 = Показати або сховати канал 9
# machine-drafted, needs native review
palette-science-channel-10 = Показати або сховати канал 10
palette-science-channel-11 = Показати або сховати канал 11
palette-science-channel-12 = Показати або сховати канал 12
palette-science-channel-13 = Показати або сховати канал 13
palette-science-channel-14 = Показати або сховати канал 14
palette-science-channel-15 = Показати або сховати канал 15
palette-science-channel-16 = Показати або сховати канал 16
palette-science-grid = Показати або сховати координатну сітку
palette-science-reset = Автоматичний вигляд
palette-science-level = Вирівняти карти висот (відняти середню площину)
palette-science-wiggle = Сейсмічні траси wiggle зі змінною площею поверх кольорів
# machine-drafted, needs native review
palette-science-wireframe = Ребра неструктурованої сітки (каркас) поверх кольорів
palette-science-import-lut = Таблиця кольорів із файлу (ImageJ .lut)…
palette-science-hillshade = Відтінити рельєф моделей висот (відмивка рельєфу)
palette-science-fft = Спектр потужності (FFT) площини
palette-science-catalogue = Накласти каталог джерел (VOTable, таблиця FITS, CSV)…
palette-science-clear-catalogues = Прибрати накладені каталоги
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] { $count } джерело на зображенні
        [few] { $count } джерела на зображенні
        [many] { $count } джерел на зображенні
       *[other] { $count } джерела на зображенні
    }
science-catalogue-capped = { $name }: показано { $shown } з { $count } джерел на зображенні
science-catalogue-none = { $name }: немає джерел на зображенні
science-catalogue-failed = Це не каталог із положеннями на небі: { $name }
science-catalogues-cleared = Каталоги прибрано
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Каталоги джерел
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Іонне зображення за m/z (мас-спектрометрія)…
disabled-not-science = Лише для наукових зображень (астрономія, електронна та скануюча зондова мікроскопія)
disabled-one-plane = Зображення має одну площину
disabled-one-axis = Зображення не має іншої осі для гортання
disabled-no-channels = Зображення не має такого каналу для складання
disabled-no-coordinates = Зображення не має небесних координат
disabled-not-height-map = Лише для карт висот (Gwyddion, Igor)
disabled-not-elevation = Лише для геоприв’язаних моделей висот з одним каналом
disabled-not-seismic = Лише для сейсмічних даних (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Лише для зображень мас-спектрометрії (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Лише для неструктурованих сіток (NetCDF UGRID, ICON і MPAS)
disabled-not-bayer = Лише для сирих кадрів кольорових камер (FITS із шаблоном Байєра, відео SER)
palette-debayer = Показувати кадри кольорових камер після дебаєризації або сирими
palette-bayer-pattern = Наступний шаблон Байєра для цього файлу
science-debayer-on = Дебаєризовано ({ $pattern })
science-debayer-off = Сирий масив кольорових фільтрів
science-bayer-pattern = Шаблон Байєра { $pattern }
# machine-drafted, needs native review
palette-annotations = Показати або сховати анотації
disabled-no-annotations = Поруч із зображенням немає анотацій (NDPA, Aperio або ASAP XML)
science-annotations-on = Анотації показано
science-annotations-off = Анотації приховано
science-stretch-auto = Автоматично (для кожного кольору)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Лінійне
science-stretch-sqrt = Квадратний корінь
science-stretch-log = Логарифмічне
science-stretch-asinh = Asinh
science-stretch-histeq = Вирівнювання гістограми
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Мін/макс
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Діапазон із файлу
# Toasts when the look changes.
science-stretch-toast = Розтягування: { $stretch }
science-limits-toast = Межі: { $limits }
science-reset-toast = Автоматичний вигляд
# An imported colour table: $name is its file name.
science-lut-toast = Таблиця кольорів { $name }
science-lut-failed = Не таблиця кольорів: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = Таблиці кольорів ImageJ
science-composite-on = Канали складено в кольорі
science-composite-off = Один канал у відтінках сірого
science-channel-on = Показано: { $channel }
science-channel-off = Приховано: { $channel }
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Канал { $number }
science-grid-on = Координатну сітку показано
science-grid-off = Координатну сітку приховано
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Кільця роздільної здатності показано
science-rings-off = Кільця роздільної здатності приховано
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Траси wiggle увімкнено (збільште, щоб побачити)
science-wiggle-off = Траси wiggle вимкнено
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Ребра сітки увімкнено
science-wireframe-off = Ребра сітки вимкнено
science-wireframe-dense = Ребра сітки увімкнено (наблизьте, щоб побачити)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Вирівнювання площини ввімкнено
science-level-off = Вирівнювання площини вимкнено
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Відмивку рельєфу ввімкнено: сонце на { $azimuth }°, { $altitude }° над обрієм
science-hillshade-off = Відмивку рельєфу вимкнено
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Спектр потужності показано
science-fft-off = Спектр потужності приховано
science-axis-toast = Гортання: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Значення { $value }
science-value-unit = Значення { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Значення { $value } (біти { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Значення { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Немає даних
s2-class-defective = Насичені або дефектні
s2-class-dark = Пікселі темних ділянок
s2-class-cloud-shadow = Тіні хмар
s2-class-vegetation = Рослинність
s2-class-not-vegetated = Без рослинності
s2-class-water = Вода
s2-class-unclassified = Некласифіковано
s2-class-cloud-medium = Хмари середньої ймовірності
s2-class-cloud-high = Хмари високої ймовірності
s2-class-cirrus = Тонкі перисті хмари
s2-class-snow = Сніг або лід
s2-class-clear = Безхмарно
s2-class-opaque-cloud = Непрозорі хмари
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = α { $ra }  δ { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS не підтримується: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Позначки каталогу: ICRS → FK4 перераховано
science-fk4-markers-galactic = Позначки каталогу: галактичні → FK4 перераховано
science-fk4-samp = SAMP: FK4 → ICRS перераховано
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Площину вирівняно
# Shown under the look while the hillshade is on.
science-hillshaded = Відмивка рельєфу: сонце на { $azimuth }°, { $altitude }° над обрієм
# Shown in the readout while the power spectrum is shown.
science-fft = Спектр потужності (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ потужності { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Частота u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Період { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Фаза { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Спектр у точці { $x }, { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (бін { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = Приховані біни недоповнення/переповнення: { $count } (сума { $sum })
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Біни змінної ширини передискретизовано на рівномірну сітку
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } подія
    [few] { $count } події
    [many] { $count } подій
   *[other] { $count } події
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Компенсовано ({ $keyword })
science-cytometry-uncompensated = Не компенсовано
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Інлайн { $inline }  Крослайн { $crossline }
science-seismic-trace = Траса { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Немає траси
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Час { $time } мс
science-seismic-depth = Глибина { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Грань { $face }
science-mesh-face-node = Грань { $face }  Вузол { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Поза сіткою
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } з { $count }
science-plane-world = { $axis } { $index } з { $count } ({ $world })
science-axis-unnamed = Площина
science-axis-extension = Зображення
science-axis-channel = Канал
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Поле
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Бін
science-bin-size = { $size } пкс
science-axis-band = Енергія
science-band-all = усі енергії
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Частина
science-part-magnitude = модуль
science-part-real = дійсна частина
science-part-imaginary = уявна частина
science-part-phase = фаза (радіани)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = інтенсивність (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Кадр
science-readout = Значення та координати
# Accessible names of the plane sliders.
science-sliders = Площини
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Шкала масштабу: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Повний іонний струм
science-ion-window = Іонне зображення m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Іонне зображення за m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab допуск · Enter показати · Esc скасувати
science-ion-bad = Не є значенням m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Точка x { $x }  y { $y }
science-ms-spot-z = Точка x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = У цій точці немає спектра
# machine-drafted, needs native review
palette-science-temperature-unit = Одиниця температури (°C, °F, K)
palette-science-isotherm = Ізотерма вище або нижче температури під вказівником
palette-science-thermal-parameters = Коефіцієнт випромінювання і відбита температура…
palette-science-export-temperatures = Експортувати температури (TIFF з рухомою комою або CSV)…
palette-thermal-visible = Видиме фото до термограми
disabled-not-thermal = Лише для термограм
disabled-not-radiometric = Файл зберігає температури: його параметри не можна змінити
disabled-no-visible-photo = Термограма не має видимого фото
science-temperature-unit-toast = Температури в { $unit }
science-isotherm-above = Ізотерма вище { $level }
science-isotherm-below = Ізотерма нижче { $level }
science-isotherm-off = Ізотерму вимкнено
science-thermal-emissivity-prompt = Коефіцієнт випромінювання: { $text }
science-thermal-reflected-prompt = Відбита температура ({ $unit }): { $text }
science-thermal-prompt-hint = Tab наступне поле · Enter перерахувати · Esc скасувати
science-thermal-bad = Недійсне значення: { $text }
science-thermal-applied = Перераховано для коефіцієнта випромінювання { $emissivity }, відбитої { $reflected }
science-thermal-conditions = Коефіцієнт випромінювання { $emissivity } · відбита { $reflected }
science-thermal-export-tiff = 32-бітний TIFF з рухомою комою
science-thermal-export-csv = CSV
science-thermal-export-done = Температури збережено в { $name }
science-thermal-export-failed = Не вдалося зберегти температури: { $reason }
# machine-drafted, needs native review
science-values-failed = Не вдалося показати значення: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Відтворити або призупинити термографічну послідовність
palette-science-thermal-series-csv = Зберегти температури в часі (CSV)…
disabled-not-thermal-sequence = Лише для термографічних послідовностей
thermal-seq-title = Температури в часі
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Кадр { $frame } з { $frames } · { $time }
thermal-seq-play = Відтворити
thermal-seq-pause = Пауза
thermal-seq-playing = Відтворення
thermal-seq-paused = Призупинено
thermal-seq-save-csv = Зберегти CSV…
thermal-seq-hint = Додайте точку, лінію або область (M), щоб побудувати її графік у часі.
thermal-seq-reading = Читання кадру { $done } з { $total }…
thermal-seq-failed = Не вдалося прочитати кадри: { $reason }
thermal-seq-no-series = Поки нічого зберігати: додайте точку, лінію або область.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Графік: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Звіт (PDF)…
thermal-report-heading = Термографічний звіт
thermal-report-default-title = Звіт про тепловізійне обстеження
thermal-report-title-label = Назва
thermal-report-notes-label = Примітки
thermal-report-photo = Додати видиме фото
thermal-report-folder = Додати інші термограми з цієї теки
thermal-report-save = Зберегти PDF…
thermal-report-busy = Створення звіту…
thermal-report-nothing = Температури ще не показано.
thermal-report-saved = Звіт збережено в { $name } ({ $count ->
    [one] { $count } зображення
    [few] { $count } зображення
    [many] { $count } зображень
   *[other] { $count } зображення
})
thermal-report-failed = Не вдалося зберегти звіт: { $reason }
# In the PDF.
thermal-report-notes = Примітки
thermal-report-measurements = Вимірювання
thermal-report-no-measurements = Немає вимірювань.
thermal-report-thermal-caption = Термограма
thermal-report-visible-caption = Видиме фото
thermal-report-col-measurement = Вимірювання
thermal-report-col-min = Мін.
thermal-report-col-max = Макс.
thermal-report-col-mean = Середнє
thermal-report-col-emissivity = Коеф. випромінювання
thermal-report-whole-image = Усе зображення
thermal-report-camera = Камера
thermal-report-lens = Об’єктив
thermal-report-date = Знято
thermal-report-place = Місце
thermal-report-frame = Кадр
thermal-report-emissivity = Коефіцієнт випромінювання
thermal-report-reflected = Відбита температура
thermal-report-distance = Відстань
thermal-report-humidity = Вологість
thermal-report-range = Діапазон
thermal-report-format = Формат
thermal-report-page = Сторінка { $page } з { $pages }
