# Multiplanar reformatting (MPR).

palette-mpr = Багатоплощинна реконструкція (MPR)
palette-mpr-slab = Наступний шар MPR (тонкий, MIP, MinIP, середнє)
disabled-no-volume = Це не об’єм: відкрийте серію, багатокадровий файл, 3D-об’єм або z-стек
disabled-mpr-closed = Спершу відкрийте багатоплощинні види
mpr-axial = Аксіальна
mpr-coronal = Корональна
mpr-sagittal = Сагітальна
mpr-oblique = Коса
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D-вигляд
mpr-thin = Тонкий
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Середнє
mpr-slab = Шар: { $mode }, { $thickness } мм
# machine-drafted, needs native review
mpr-slab-angstrom = Шар: { $mode }, { $thickness } Å
mpr-slab-unit = Шар: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Шар: { $mode }, вокселів: { $thickness }
mpr-thicker = Товщий шар
mpr-thinner = Тонший шар
mpr-reset = Скинути види
mpr-close = Закрити MPR
mpr-maximise = Розгорнути вид
mpr-export-planes = Експортувати площини
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Лінії струму з { $files }: шар { $thickness } мм
mpr-tracts-off = Лінії струму з { $files }: приховано
mpr-tracts-none = Поруч із цим об’ємом немає ліній струму TrackVis чи MRtrix
mpr-position = { $view } { $position } мм
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = вокселів: { $value }
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } з { $count }
mpr-axis-channel = Канал
mpr-axis-unnamed = Вісь { $axis }
mpr-one-channel = MPR показує один канал за раз
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = Канали в кольорі: { $count }, кожен у своєму діапазоні показу
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Показано один канал: { $reason }
mpr-isotropic = У файлі немає розміру воксела: воксели показано кубами
mpr-isotropic-z = У файлі немає відстані між зрізами: зрізи показано через один піксель
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Інлайн { $inline }
mpr-seismic-crossline = Крослайн { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Завантаження зрізів: { $loaded } з { $count }
mpr-loaded = Зрізи: { $count }, { $width } × { $height }
mpr-downsampled = Показано з роздільністю 1/{ $factor } (обмеження пам’яті або графіки)
mpr-converted = Значення показано як 32-бітні числа з рухомою комою
mpr-opening = Відкриття об’єму…
mpr-gpu-memory = { $size } на відеокарті
mpr-refused = Не вдається побудувати об’єм: { $reason }
mpr-failed = Об’єм не завантажено: { $reason }
mpr-no-gpu = Відеокарта не вміщує цей об’єм: { $reason }
mpr-planes-saved = { $count ->
    [one] Збережено { $count } площину в { $folder }
    [few] Збережено { $count } площини в { $folder }
    [many] Збережено { $count } площин у { $folder }
   *[other] Збережено { $count } площини в { $folder }
}
mpr-planes-failed = Не вдалося зберегти площини: { $reason }
mpr-view-a11y = Вид { $view }, { $position } мм
mpr-crosshair-a11y = Перехрестя в { $x }, { $y }, { $z } мм
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Перехрестя в { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Перехрестя в { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Перехрестя у вокселі { $x }, { $y }, { $z }
mpr-controls = Керування MPR

# 3D view palette entries.
palette-mpr-preset = Наступний 3D-шаблон (КТ кістки, м’які тканини, легені, ангіо; МРТ)
palette-mpr-render = Наступний 3D-рендеринг (затінений, Cinematic, MIP, MinIP)
palette-mpr-clip = Наступне 3D-відсікання (немає, рамка 2D-виглядів, вирізаний кут)
palette-mpr-export-stl = Експортувати 3D-поверхню як STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Наступний шар ліній струму в MPR
palette-mpr-maximise = Розгорнути вид MPR або показати всі види
palette-mpr-export-planes = Експортувати площини MPR як зображення…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] КТ кістки
    [soft] КТ м’які тканини
    [lung] КТ легені
    [angio] КТ-ангіо
   *[mr] МРТ
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Затінений
}
vr-clip = { $clip ->
    [box] Відсікання: рамка виглядів
    [cut] Відсікання: вирізаний кут
   *[off] Без відсікання
}
vr-smooth = Згладити поверхню
vr-export = Експортувати STL
vr-threshold = Поверхня на { $value }
vr-stl-exporting = Побудова поверхні…
vr-stl-saved = { $count ->
    [0] На цьому порозі нічого немає: поверхню не збережено
   *[other] Збережено в { $name }, трикутників: { $count }
}
vr-stl-failed = Не вдалося зберегти поверхню: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, поверхня на { $threshold }. Shift+стрілки обертають, плюс і мінус масштабують.

## Сирий об’єм: як улаштовано файл .raw / .vol без заголовка

# machine-drafted, needs native review
raw-volume-title = Відкрити сирий об’єм
raw-volume-file = { $file } не має заголовка. Вкажіть, як розміщено його відліки.
raw-volume-guess-label = Запропоноване розміщення
raw-volume-guess = { $index } з { $count }
raw-volume-custom = Власне
raw-volume-width = Ширина (воксели)
raw-volume-height = Висота (воксели)
raw-volume-slices = Зрізи
raw-volume-type = Тип відліку
raw-volume-type-unsigned = Беззнакове { $bits }-бітне
raw-volume-type-signed = Знакове { $bits }-бітне
raw-volume-type-float = З рухомою комою { $bits }-бітне
raw-volume-order = Порядок байтів
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Зсув заголовка
raw-volume-bytes = { $count ->
    [one] { $count } байт
    [few] { $count } байти
    [many] { $count } байтів
   *[other] { $count } байта
}
raw-volume-pixel = Розмір пікселя
raw-volume-spacing = Відстань між зрізами
raw-volume-mm = { $value } мм
raw-volume-exact = Точно заповнює файл ({ $size })
raw-volume-fits = Використовує { $size } з { $file }; залишається { $left }
raw-volume-too-big = Потрібно { $size }, але файл має { $file }
raw-volume-cancel = Скасувати
raw-volume-open = Відкрити
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Зв’язність: { $seed }; перемістіть перехрестя на іншу seed-точку
# machine-drafted, needs native review
mpr-measure-hint-edit = Перетягніть кінець, щоб перемістити; на інший кінець — щоб видалити
# machine-drafted, needs native review
mpr-measure-removed = Вимірювання видалено
mpr-measure-lengths-only = MPR вимірює лише довжини
mpr-more = Більше…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Криволінійна MPR уздовж клацнутої кривої
palette-mpr-curve-layout = Криволінійна MPR: випрямлена або розтягнута
palette-mpr-curve-save = Зберегти криву…
palette-mpr-curve-load = Завантажити криву…
palette-mpr-export-segments = Експортувати кожен сегмент як STL…
palette-mpr-dental = Стоматологічний макет (панорама й поперечні зрізи)
palette-mpr-nerve = Простежити нижньощелепний канал
cpr-kind = { $kind ->
    [stretched] Розтягнута
   *[straightened] Випрямлена
}
cpr-button = Криволінійна
cpr-controls = Керування криволінійною MPR
cpr-at = У { $position }
cpr-section = Поперечний зріз
cpr-hint-start = Клацайте точки вздовж структури у виді { $view }
cpr-hint-more = Клацніть ще одну точку
cpr-add-points = Додати точки
cpr-undo-point = Скасувати точку
# machine-drafted, needs native review
cpr-mark = Позначити тут
cpr-mark-set = Позначку { $n } встановлено.
cpr-mark-removed = Позначку видалено.
cpr-mark-here = Позначка { $n }
cpr-diameter = Діаметр { $value }
cpr-diameters = Діаметр від { $min } до { $max }
cpr-mark-span = Позначки { $from }–{ $to }: { $length } уздовж центральної лінії
cpr-mark-total = Усі позначки: { $length }
cpr-clear = Очистити криву
cpr-save = Зберегти криву
cpr-load = Завантажити криву
cpr-done = Готово
cpr-a11y = { $name } завдовжки { $length }, курсор у { $position }. Коліщатко рухає курсор.
cpr-none = Кривої ще немає: спершу клацніть точки
cpr-cleared = Криву очищено
cpr-saved = Криву збережено: { $name }
cpr-save-failed = Не вдалося зберегти криву: { $reason }
cpr-loaded = Криву завантажено
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] Завантажено { $count } центральну лінію
    [few] Завантажено { $count } центральні лінії
    [many] Завантажено { $count } центральних ліній
   *[other] Завантажено { $count } центральної лінії
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] Знайдено { $count } збережену центральну лінію
    [few] Знайдено { $count } збережені центральні лінії
    [many] Знайдено { $count } збережених центральних ліній
   *[other] Знайдено { $count } збереженої центральної лінії
}
# A centreline without a name, by its place in the list.
cpr-centreline = Центральна лінія { $number }
cpr-new = Нова центральна лінія
cpr-rename = Перейменувати
cpr-name = Назва центральної лінії
cpr-delete = Видалити
# $name: the centreline's name.
cpr-delete-title = Видалити центральну лінію «{ $name }»?
cpr-delete-detail = Її точки зникнуть. Файли, збережені на диску, залишаться.
cpr-deleted = Видалено «{ $name }»
cpr-load-failed = Не вдалося завантажити криву: { $reason }
cpr-load-not-json = файл не є JSON
cpr-load-not-curves = файл не є кривою Specular
cpr-load-empty = файл не містить кривої з двох або більше точок
cpr-panoramic = Панорама
cpr-panoramic-slab = Панорама, шар { $thickness } мм
cpr-panoramic-thin = Панорама, тонка

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Стоматологія
dental-not-for-diagnosis = Не для діагностики чи планування лікування
dental-nerve = Нижньощелепний канал
dental-nerve-hint = Клацайте точки вздовж нижньощелепного каналу на панорамі або поперечному зрізі
dental-nerve-done = Простеження нижньощелепного каналу вимкнено
dental-spacing = Кожні { $spacing } мм
dental-section = Зріз { $number }
dental-off = Спершу відкрийте стоматологічний макет
dental-suggesting = Пошук зубної дуги…
dental-suggested = Запропоновано дугу: перетягніть її точки, щоб підлаштувати
dental-no-arch = Дугу не знайдено: клацайте точки вздовж неї в аксіальному виді
dental-vendor = Стоматологічна КПКТ { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Експортувати сегменти
seg-stl-detail = Сегменти: { $count ->
        [one] { $count } трикутник
        [few] { $count } трикутники
        [many] { $count } трикутників
       *[other] { $count } трикутника
    }
seg-stl-detail-full = Сегменти: повна деталізація
seg-stl-none = Немає сегментів для експорту
seg-stl-not-labels = цей об’єм не є картою міток
seg-stl-exporting = Побудова поверхонь сегментів…
seg-stl-saved = { $count ->
    [one] Збережено { $count } сегмент у { $folder }
    [few] Збережено { $count } сегменти в { $folder }
    [many] Збережено { $count } сегментів у { $folder }
   *[other] Збережено { $count } сегмента в { $folder }
}
seg-stl-open = Незамкнених: { $count }.
seg-stl-empty = Сегменти порожні: нічого не збережено
seg-stl-failed = Не вдалося зберегти сегменти: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Три площини (MPR)
toolbar-mpr-close = Закрити три площини (MPR)
settings-viewing-toolbar-items-mpr = Три площини (MPR)
