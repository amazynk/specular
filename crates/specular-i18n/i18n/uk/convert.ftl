# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Конвертувати…

## Settings → Convert
settings-section-convert = Конвертування
settings-convert-format = Формат
settings-convert-format-help = Формат, у якому записує команда «Конвертувати…». PNG і TIFF — без втрат; WebP і JPEG XL можуть бути як з втратами, так і без.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = Якість JPEG
settings-convert-jpeg-quality-help = Вище значення зберігає більше деталей у більших файлах. При 90 різницю з оригіналом важко помітити.
settings-convert-jpeg-full-chroma = Колір у повній роздільності в JPEG
settings-convert-jpeg-full-chroma-help = Зберігає колір у повній роздільності (4:4:4) замість половинної (4:2:0): чіткіші кольорові краї, більші файли.
settings-convert-png-effort = Стиснення PNG
settings-convert-png-effort-help = Вище значення дає менші файли, але повільніше. 1 — найшвидше.
settings-convert-jxl-effort = Зусилля JPEG XL
settings-convert-jxl-effort-help = Вище значення дає менші файли, але повільніше.
settings-convert-tiff-compression = Стиснення TIFF
settings-convert-tiff-compression-help = Deflate — стиснення без втрат, яке читає будь-яка програма для TIFF.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Немає
settings-convert-depth = Бітова глибина
settings-convert-depth-help = «Автоматично» записує 16 бітів на відлік, коли зображення має більше 8 і формат їх підтримує (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Автоматично
settings-convert-depth-8 = 8 бітів
settings-convert-depth-16 = 16 бітів
settings-convert-resize = Зміна розміру
settings-convert-resize-help = Зображення зменшуються в лінійному світлі якісним фільтром.
settings-convert-resize-none = Початковий розмір
settings-convert-resize-fit = Вписати в
settings-convert-resize-long-edge = Довша сторона
settings-convert-resize-percent = Відсоток
settings-convert-width = Ширина
settings-convert-width-help = Найбільша ширина, яку може мати зображення з «Вписати в».
settings-convert-height = Висота
settings-convert-height-help = Найбільша висота, яку може мати зображення з «Вписати в».
settings-convert-long-edge = Довша сторона
settings-convert-long-edge-help = Довжина довшої сторони зображення з «Довша сторона».
settings-convert-percent = Відсоток
settings-convert-percent-help = Розмір відносно оригіналу з «Відсоток».
settings-convert-upscale = Збільшувати малі зображення
settings-convert-upscale-help = Якщо вимкнено, зображення, менші за потрібний розмір, зберігають свій розмір.
settings-convert-colour = Колір
settings-convert-colour-help = «Зберегти профіль» записує колірний профіль зображення разом із ним. «Перетворити на sRGB» підходить для інтернету й пристроїв без керування кольором.
settings-convert-colour-keep = Зберегти профіль
settings-convert-colour-srgb = Перетворити на sRGB
settings-convert-hdr = HDR-зображення
settings-convert-hdr-help = PNG і JPEG XL зберігають HDR як є; інші формати завжди тонально відображаються в SDR.
settings-convert-hdr-keep = Зберігати, де можливо
settings-convert-hdr-tone-map = Тонально відобразити в SDR
settings-convert-metadata = Метадані
settings-convert-metadata-help = Exif і XMP, як-от камера, дата зйомки й місце. Колірний профіль зберігається завжди.
settings-convert-metadata-all = Зберегти всі
settings-convert-metadata-no-location = Усі, крім місця
settings-convert-metadata-strip = Видалити
settings-convert-folder = Тека призначення
settings-convert-folder-help = Куди потрапляють конвертовані файли. Якщо не задано, вони потрапляють у підтеку поруч із кожним зображенням.
settings-convert-folder-unset = Підтека поруч із зображеннями
settings-convert-subfolder = Назва підтеки
settings-convert-subfolder-help = Підтека, яка використовується, коли теку призначення не задано. Порожньо: власна тека зображень.
settings-convert-pattern = Назви файлів
settings-convert-pattern-help = {"{"}name{"}"} — початкова назва, {"{"}n:03{"}"} — номер у пакеті, {"{"}date:%Y-%m-%d{"}"} — дата зйомки, {"{"}w{"}"} і {"{"}h{"}"} — новий розмір, {"{"}ext{"}"} — розширення формату.
settings-convert-conflicts = Коли назва зайнята
settings-convert-conflicts-help = Наявні файли ніколи не замінюються.
settings-convert-conflicts-suffix = Додати номер
settings-convert-conflicts-skip = Пропустити зображення

## The dialog
convert-title = Конвертувати
convert-images = Зображення
convert-images-current = Це зображення
convert-images-selection = Виділення ({ $count })
convert-images-folder = Тека ({ $count })
convert-images-picked = Вибрані ({ $count })
convert-quality = Якість
convert-chroma = Деталізація кольору
convert-chroma-half = 4:2:0 (менше)
convert-chroma-full = 4:4:4 (чіткіше)
convert-effort = Зусилля
convert-on = Увімк.
convert-off = Вимк.
convert-pixels = { $value } пкс
convert-effort-fastest = { $value } (найшвидше)
convert-folder-subfolder = Підтека «{ $name }»
convert-folder-same = Та сама тека, що й зображення
convert-choose-folder = Вибрати теку…
convert-name = Назва
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Дужку {"{"} не закрито.
convert-name-error-stray = Дужка {"}"} не має пари {"{"}. Щоб вставити фігурну дужку, напишіть {"}}"}.
convert-name-error-unknown = { $token } не є частиною назви. Використовуйте name, n, date, w, h або ext.
convert-name-error-format = { $token } має неправильний формат.
convert-name-error-empty = Введіть шаблон назви.
convert-start = Конвертувати
convert-stop = Зупинити
convert-hide = Сховати
convert-close = Закрити
convert-progress = Конвертування { $done } з { $total }
convert-progress-eta = Конвертування { $done } з { $total } · залишилося близько { $time }
convert-eta-seconds = { $count ->
    [one] { $count } секунда
    [few] { $count } секунди
    [many] { $count } секунд
   *[other] { $count } секунди
}
convert-eta-minutes = { $count ->
    [one] { $count } хвилина
    [few] { $count } хвилини
    [many] { $count } хвилин
   *[other] { $count } хвилини
}
convert-done = { $count ->
    [one] Конвертовано { $count } зображення
    [few] Конвертовано { $count } зображення
    [many] Конвертовано { $count } зображень
   *[other] Конвертовано { $count } зображення
}
convert-done-skipped = Пропущено: { $count } (назва була зайнята)
convert-done-failed = Не вдалося: { $count }
convert-stopped = Зупинено після { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = не вдається відкрити ({ $error })
convert-failure-decode = не вдається прочитати ({ $error })
convert-failure-encode = не вдається записати в цьому форматі ({ $error })
convert-failure-write = не вдається зберегти ({ $error })
convert-failure-same = новий файл замінив би оригінал
convert-report-more = і ще { $count }
convert-running = Конвертування вже триває.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = WebP без втрат
settings-convert-webp-lossless-help = Зберігає кожен піксель точно. Якщо вимкнено, файли WebP значно менші за вибраної якості WebP.
settings-convert-webp-quality = Якість WebP
settings-convert-webp-quality-help = Вище значення зберігає більше деталей у більших файлах. Використовується, коли WebP записується з втратами.
settings-convert-webp-effort = Зусилля WebP
settings-convert-webp-effort-help = Вище значення дає менші файли, але повільніше. Використовується, коли WebP записується з втратами.
settings-convert-jxl-lossless = JPEG XL без втрат
settings-convert-jxl-lossless-help = Зберігає кожен піксель точно. Якщо вимкнено, файли JPEG XL значно менші за вибраної якості JPEG XL.
settings-convert-jxl-quality = Якість JPEG XL
settings-convert-jxl-quality-help = Вище значення зберігає більше деталей у більших файлах. При 90 різницю з оригіналом важко помітити. Використовується, коли JPEG XL записується з втратами.
settings-convert-avif-quality = Якість AVIF
settings-convert-avif-quality-help = Вище значення зберігає більше деталей у більших файлах.
settings-convert-avif-effort = Зусилля AVIF
settings-convert-avif-effort-help = Вище значення дає менші файли, але повільніше. 1 — найшвидше.
convert-lossless = Без втрат
