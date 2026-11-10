# machine-drafted, needs native review
# Specular — Звіт про відкриття: чому зображення відкривалося стільки часу (інформаційна панель,
# розділ «Продуктивність»; DESIGN.md §12.7).

open-report-title = Продуктивність

## Headlines
open-report-fast = Відкрито за { $ms } мс
open-report-display-cache = Показано з кешу відображення
open-report-slow-read = Повільне сховище: { $percent } часу пішло на читання файлу ({ $speed } МБ/с)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Повільне сховище: { $percent } часу пішло на читання файлу (малий файл: переважно очікування відповіді сховища)
open-report-archive-read = { $percent } часу пішло на розпакування архіву
open-report-startup = { $percent } часу пішло на запуск { -app-name }
open-report-checked = Декодовано двічі: перший результат апаратного декодера порівняно з результатом CPU ({ $percent } часу)
open-report-hardware = Декодовано апаратним декодером, що зайняло { $percent } часу
open-report-decode = Велике зображення: декодування { $mpx } мегапікселів зайняло { $percent } часу
open-report-upload = { $percent } часу пішло на передавання зображення до GPU
open-report-waiting = { $percent } часу пішло на очікування іншої роботи
open-report-stage = { $stage }: { $percent } часу
open-report-memory-short = Бракувало пам’яті, тож системі, можливо, довелося використовувати своп

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Декодовано програмно: апаратний декодер ще визначався
open-report-sw-no-device = Декодовано програмно: { $api } недоступний на цьому комп’ютері
open-report-sw-no-codec = Декодовано програмно: { $api } на цьому GPU не має декодера { $codec }
open-report-sw-blocked-mismatch = Декодовано програмно: апаратний декодер вимкнено, бо його результат не збігся з результатом CPU
open-report-sw-blocked-error = Декодовано програмно: апаратний декодер вимкнено після помилки
open-report-sw-blocked-lost = Декодовано програмно: апаратний декодер вимкнено, бо він перестав відповідати
open-report-sw-failed = Декодовано програмно: апаратний декодер не впорався з цим зображенням
open-report-sw-busy = Декодовано програмно: апаратний декодер був зайнятий
open-report-sw-unsupported = Декодовано програмно: апаратний декодер не підтримує це зображення
open-report-sw-unavailable = Декодовано програмно: апаратне декодування вимкнено до перезапуску { -app-name }
open-report-sw-cpu-only = Декодовано програмно: обробку налаштовано на «Лише CPU»
open-report-sw-setting-off = Декодовано програмно: апаратне декодування вимкнено в налаштуваннях
open-report-sw-unchecked = Декодовано програмно: апаратний декодер ще не перевірено (його спершу випробовують на зображенні, що готується у фоні)
open-report-sw-lower-power = Декодовано програмно: від батареї CPU витрачає на це менше енергії
open-report-sw-faster = Декодовано програмно: на цьому комп’ютері так швидше (близько { $cpu } мс проти { $hw } мс на апаратному декодері)
open-report-sw-scaled = Декодовано програмно: апаратний декодер декодує зображення лише в повному розмірі

## Hints
open-report-hint-copy-local = Скопіюйте файл на локальний диск, щоб він відкривався швидше.
open-report-hint-extract = Розпакуйте архів у теку, щоб його сторінки відкривалися швидше.
open-report-hint-try-again = Щоб знову спробувати апаратний декодер: Налаштування → Продуктивність → Спробувати ще раз.
open-report-hint-automatic = Встановіть «Де обробляти зображення» на «Автоматично» в Налаштування → Продуктивність.
open-report-hint-turn-on = Встановіть «Апаратне декодування» на «Автоматично» в Налаштування → Продуктивність.
open-report-hint-free-memory = Закрийте інші програми, щоб звільнити пам’ять.
open-report-hint-once-only = Так перевіряється лише перше зображення.

## Rows
open-report-total = До повної чіткості
open-report-first-pixels = Перші пікселі
# machine-drafted, needs native review
open-report-whole = Усе зображення показано
open-report-stage-startup = Запуск
open-report-stage-read = Читання файлу
open-report-stage-parse = Читання заголовка
open-report-stage-colour = Підготовка керування кольором
open-report-stage-decode = Декодування
open-report-stage-upload = Передавання до GPU
open-report-stage-waiting = Очікування іншої роботи
open-report-decoder = Декодер
open-report-decoder-checked = { $engine }, звірено з CPU
open-report-decoder-cache = Кеш відображення
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, у 1/{ $scale } розміру
open-report-decoder-scaled-full = { $engine }, у повному розмірі, потім зменшено до 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, у 1/{ $scale } розміру, без індексу швидкого старту
open-report-colour-conversion = Перетворення кольорів

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Повторюється
open-report-advice-change = Допомогло б
open-report-advice-gain = Очікуваний виграш
open-report-advice-software = Для { $count } з останніх { $total } повільних зображень { $format } з цієї причини використано програмне декодування
open-report-advice-slow-storage = Для { $count } з останніх { $total } повільних зображень затримку спричинило читання файлу (близько { $speed } МБ/с)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = Для { $count } з останніх { $total } повільних зображень затримку спричинило читання файлу (малі файли: переважно очікування відповіді сховища)
open-report-advice-archive = Для { $count } з останніх { $total } повільних зображень затримку спричинило розпакування архіву
open-report-advice-hardware-decoder = GPU з апаратним декодером { $codec }
open-report-advice-gain-range = Приблизно в { $low }–{ $high } раза швидше
open-report-advice-gain-one = Приблизно в { $low } раза швидше

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Поради щодо продуктивності
open-report-insights-indicator = Чому це повільно?
open-report-insights-announce = Порада щодо продуктивності: зображення раз у раз відкриваються повільно з тієї самої причини. Відкрийте «Чому це повільно?», щоб побачити, що допомогло б.
open-report-insights-hide = Сховати
open-report-insights-last = Останнє повільне зображення: { $total } до повної чіткості
open-report-insights-now = Що ви можете зробити зараз
open-report-insights-hardware = Обладнання, яке допомогло б
open-report-insights-none = Немає повторюваного сповільнення
open-report-insights-none-detail = Поради з’являються, коли кілька зображень відкриваються повільно з тієї самої причини. Щоб знайти їх, { -app-name } зберігає на цьому комп’ютері повільні відкриття за тиждень; нічого нікуди не надсилається.
open-report-insights-software = Зображення { $format } декодуються на CPU
open-report-insights-storage = Читання файлів затримує зображення
open-report-insights-archive = Розпакування архівів затримує зображення
open-report-insights-nothing-now = Жодне налаштування { -app-name } цього не змінить.
open-report-insights-display-cache = Увімкніть кеш відображення: раніше відкриті зображення тоді відкриватимуться зі швидкої копії на цьому комп’ютері.
open-report-insights-prefetch = Тримайте напоготові більше сусідніх зображень, щоб наступні готувалися, поки ви дивитеся.
open-report-insights-hw-ssd = Швидкий внутрішній диск (NVMe SSD) для цих файлів
open-report-insights-action-turn-on = Увімкнути
open-report-insights-action-try-again = Спробувати ще раз
open-report-insights-action-automatic = Встановити «Автоматично»
open-report-insights-dismiss-storage = Більше не показувати для сховища
open-report-insights-dismiss-decoder = Більше не показувати для декодування
open-report-insights-off = Вимкнути поради
open-report-insights-on = Увімкнути поради
open-report-insights-restore = Знову показати приховані поради
open-report-insights-power = Цей комп’ютер заощаджує енергію (працює від батареї або в режимі енергозбереження), що сповільнює декодування на CPU. Якщо під’єднати живлення, зображення можуть відкриватися швидше.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Порада щодо продуктивності: мініатюри раз у раз з’являються повільно з тієї самої причини. Відкрийте «Чому це повільно?», щоб побачити, що допомогло б.
open-report-insights-last-thumbnail = Остання повільна мініатюра: { $total } на створення
open-report-insights-thumbnails-software = Мініатюри зображень { $format } декодуються на CPU
open-report-insights-thumbnails-storage = Читання файлів затримує мініатюри
open-report-insights-thumbnails-archive = Розпакування архівів затримує мініатюри
open-report-insights-thumbnail-cache = Мініатюри не можна зберігати на цьому комп’ютері (кеш мініатюр недоступний), тому вони створюються заново щоразу, коли показано теку.
open-report-advice-thumbnails-software = Для { $count } з останніх { $total } повільних мініатюр { $format } з цієї причини використано програмне декодування
open-report-advice-thumbnails-slow-storage = Для { $count } з останніх { $total } повільних мініатюр затримку спричинило читання файлу (близько { $speed } МБ/с)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = Для { $count } з останніх { $total } повільних мініатюр затримку спричинило читання файлу (малі файли: переважно очікування відповіді сховища)
open-report-advice-thumbnails-archive = Для { $count } з останніх { $total } повільних мініатюр затримку спричинило розпакування архіву
