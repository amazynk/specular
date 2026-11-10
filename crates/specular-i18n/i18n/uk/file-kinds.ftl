# Specular — File kinds: which format a shared extension is, Open as…, remembered choices. Українська.

## Which kind of file? Files whose extension several formats share.

# machine-drafted, needs native review
file-kind-title = Що це за файл { $file }?
# machine-drafted, needs native review
file-kind-why-content = Його вміст підходить більш ніж до одного формату.
# machine-drafted, needs native review
file-kind-why-name = Його вміст цього не каже. Ці формати використовують файли .{ $ext }.
# machine-drafted, needs native review
file-kind-why-none = Жоден формат не розпізнає його вміст.
# machine-drafted, needs native review
file-kind-certain = Напевно
# machine-drafted, needs native review
file-kind-likely = Імовірно
# machine-drafted, needs native review
file-kind-possible = Можливо
# machine-drafted, needs native review
file-kind-guess = Лише за назвою
# machine-drafted, needs native review
file-kind-hint-same = як інші файли .{ $ext } тут
# machine-drafted, needs native review
file-kind-hint-companions = його звичні супровідні файли поруч
# machine-drafted, needs native review
file-kind-scope = Використати для
# machine-drafted, needs native review
file-kind-scope-file = Цей файл
# machine-drafted, needs native review
file-kind-scope-folder = Файли .{ $ext } у цій теці
# machine-drafted, needs native review
file-kind-scope-everywhere = Усі файли .{ $ext }
# machine-drafted, needs native review
file-kind-later = Змінити пізніше можна через «Відкрити як…» у меню файлу.
# machine-drafted, needs native review
file-kind-open = Відкрити
# machine-drafted, needs native review
file-kind-raw = Сирі відліки
# machine-drafted, needs native review
file-kind-open-as-none = Ніщо інше не читає { $file }.
# machine-drafted, needs native review
error-reason-ambiguous = Це може бути один із кількох форматів. Виберіть один, щоб відкрити його.

## Open as… (the palette and the gallery's file menu)

# machine-drafted, needs native review
palette-open-as = Відкрити як…
# machine-drafted, needs native review
palette-open-as-help = Виберіть, у якому форматі читати файл.

## Settings › Optional decoders › File kinds

# machine-drafted, needs native review
settings-decoders-file-kinds = Забути всі типи файлів…
# machine-drafted, needs native review
settings-decoders-file-kinds-help = Формати, які ви вибрали для розширень, спільних для кількох форматів. Наступного разу буде запитано знову.
# machine-drafted, needs native review
file-kind-rules-intro = Кожне правило відкриває файли з розширенням в одному форматі.
# machine-drafted, needs native review
file-kind-rules-none = Правил ще немає. Коли буде запитано, виберіть «Файли .ext у цій теці» або «Усі файли .ext».
# machine-drafted, needs native review
file-kind-rule = Файли .{ $ext }: { $format }
# machine-drafted, needs native review
file-kind-rule-folder = У { $folder }
# machine-drafted, needs native review
file-kind-rule-everywhere = Усюди
# machine-drafted, needs native review
file-kind-forget = Забути…
# machine-drafted, needs native review
file-kind-forget-ask = Забути «Файли .{ $ext }: { $format }»?
# machine-drafted, needs native review
file-kind-forget-detail = { -app-name } запитає знову наступного разу. Жоден файл не змінюється й не видаляється.
# machine-drafted, needs native review
file-kind-forget-ok = Забути
# machine-drafted, needs native review
file-kind-forgotten = Забуто: файли .{ $ext }: { $format }.
# machine-drafted, needs native review
file-kind-forget-all-ask = { $count ->
    [one] Забути правило типу файлу?
    [few] Забути всі { $count } правила типів файлів?
    [many] Забути всі { $count } правил типів файлів?
   *[other] Забути всі { $count } правила типів файлів?
}
# machine-drafted, needs native review
file-kind-forgotten-all = { $count ->
    [one] Правило типу файлу забуто.
    [few] Забуто { $count } правила типів файлів.
    [many] Забуто { $count } правил типів файлів.
   *[other] Забуто { $count } правила типів файлів.
}
# machine-drafted, needs native review
file-kind-keywords = розширення, формат, відкрити як, який тип, тип файлу

## One line on what each format is, under its name in the prompt.

# machine-drafted, needs native review
file-kind-about-other = Інший формат, що використовує файли .{ $ext }.
# machine-drafted, needs native review
file-kind-about-raw-volume = Відліки без заголовка. Ви задаєте ширину, висоту, зрізи й тип відліку.
# machine-drafted, needs native review
file-kind-about-raw = Сирий знімок камери.
# machine-drafted, needs native review
file-kind-about-flir-fff = Тепловізійне зображення FLIR із температурами.
# machine-drafted, needs native review
file-kind-about-fits = Астрономічне зображення (FITS).
# machine-drafted, needs native review
file-kind-about-fit = Трек активності Garmin: точки GPS і дані датчиків.
# machine-drafted, needs native review
file-kind-about-mif = Медичний об’єм MRtrix (МРТ).
# machine-drafted, needs native review
file-kind-about-map-info-mif = Векторна карта MapInfo з даними .mid.
# machine-drafted, needs native review
file-kind-about-net-cdf = Наукові сіткові дані (клімат, океан).
# machine-drafted, needs native review
file-kind-about-hdf5 = Наукові дані HDF5.
# machine-drafted, needs native review
file-kind-about-playlist = Аудіоплейлист (PLS).
# machine-drafted, needs native review
file-kind-about-gerber = Шар друкованої плати (Gerber), наприклад паяльна паста.
# machine-drafted, needs native review
file-kind-about-ser = Астрономічне відео з планетарної камери (SER).
# machine-drafted, needs native review
file-kind-about-tia = Серія з електронного мікроскопа (FEI TIA).
file-kind-about-pcd = 3D-хмара точок (PCD).
file-kind-about-photo-cd = Скан Kodak Photo CD.
file-kind-about-zx-screen = Знімок екрана ZX Spectrum.
file-kind-about-cpc-screen = Знімок екрана Amstrad CPC.
file-kind-about-msx-screen = Знімок екрана MSX.
file-kind-about-scc = Приховані субтитри Scenarist.
# machine-drafted, needs native review
file-kind-about-pict = Класичне зображення Mac (PICT).
# machine-drafted, needs native review
file-kind-about-hdr = Зображення Radiance HDR.
# machine-drafted, needs native review
file-kind-about-analyze = Медичний об’єм Analyze 7.5 з файлом .hdr.
# machine-drafted, needs native review
file-kind-about-envi = Растр дистанційного зондування (ENVI) з файлом .hdr.
# machine-drafted, needs native review
file-kind-about-pds3 = Зображення з планетарної місії (PDS3) з міткою.
# machine-drafted, needs native review
file-kind-about-mpeg-ps = Відео з відеокамери (програмний потік MPEG, MOD/TOD).
# machine-drafted, needs native review
file-kind-about-tracker = Музичний модуль трекера (MOD, S3M, XM, IT, 669, MTM, STM, OKT, MED, ULT, FAR).
# machine-drafted, needs native review
file-kind-about-softimage-pic = 3D-рендер Softimage (PIC).
# machine-drafted, needs native review
file-kind-about-dr-halo = Зображення Dr. Halo з палітрою .pal.
# machine-drafted, needs native review
file-kind-about-wal = Текстура Quake 2, розфарбована палітрою гри.
# machine-drafted, needs native review
file-kind-about-quick-time = Фільм QuickTime.
file-kind-about-pro-res-raw = Відео Apple ProRes RAW: кадри з сенсора камери.
file-kind-about-cinema-raw-light = Відео Canon Cinema RAW Light: кадри з сенсора камери.
# machine-drafted, needs native review
file-kind-about-gimp-brush = Пензель GIMP.
# machine-drafted, needs native review
file-kind-about-swatch-exchange = Палітра кольорів Adobe Swatch Exchange.
# machine-drafted, needs native review
file-kind-about-aseprite = Піксельний спрайт Aseprite.
