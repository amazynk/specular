# machine-drafted, needs native review
# Specular — Режим відбору (DESIGN.md §6.3).

palette-cull = Увійти в режим відбору або вийти з нього
cheatsheet-cull = Режим відбору
settings-section-cull = Режим відбору
settings-cull-auto-enter = Відбирати теки RAW
settings-cull-auto-enter-help = Відкривати теку, де переважають файли RAW, одразу в режимі відбору.
settings-cull-auto-advance = Наступне зображення після позначки
settings-cull-auto-advance-help = Переходити далі після вибору, відхилення чи оцінки зірками. Мітки й зняття позначки лишаються на зображенні.
settings-cull-show = Показувати для пар RAW+JPEG
settings-cull-show-help = RAW і JPEG з однаковою назвою відбираються як одне зображення, а позначки застосовуються до обох. JPEG відкривається швидше.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Групувати серії
settings-cull-bursts-help = Знімки, зроблені швидко один за одним однією камерою, показуються як один стос. S відкриває або закриває стос, B залишає показане зображення й відхиляє решту стосу.
settings-cull-burst-gap = Пауза в серії
settings-cull-burst-gap-help = Найдовша пауза між двома знімками однієї серії.
settings-cull-watch = Показувати нові файли
settings-cull-watch-help = Файли, що з’являються в теці під час відбору (підключена камера, імпорт), додаються в кінець, щойно повністю записані.
settings-cull-follow-newest = Стежити за найновішим файлом
settings-cull-follow-newest-help = Переходити до кожного нового файлу, щойно він з’являється. T перемикає це під час відбору.
settings-cull-slot-1 = Тека 1
settings-cull-slot-1-help = Alt+1 копіює зображення (з його RAW чи JPEG і супровідними файлами) до цієї теки, Alt+Shift+1 переміщує його туди.
settings-cull-slot-1-unset = Не задано
settings-cull-slot-2 = Тека 2
settings-cull-slot-2-help = Alt+2 копіює зображення (з його RAW чи JPEG і супровідними файлами) до цієї теки, Alt+Shift+2 переміщує його туди.
settings-cull-slot-2-unset = Не задано
settings-cull-slot-3 = Тека 3
settings-cull-slot-3-help = Alt+3 копіює зображення (з його RAW чи JPEG і супровідними файлами) до цієї теки, Alt+Shift+3 переміщує його туди.
settings-cull-slot-3-unset = Не задано
settings-cull-collisions = Коли назва зайнята
settings-cull-collisions-help = Файли в теках ніколи не замінюються.
settings-cull-collisions-suffix = Додати номер
settings-cull-collisions-skip = Пропустити зображення
# machine-drafted, needs native review
# Режим відбору вибрано без відкритої теки чи зображень галереї.
cull-nothing = Нема чого відбирати — спершу відкрийте теку з фото
cull-slot-unset = Теку { $slot } не задано — виберіть її в Налаштування → Режим відбору
cull-slot-missing = Тека { $slot } недоступна: { $reason }
cull-copied = Скопійовано до { $folder }
cull-moved = Переміщено до { $folder }
cull-slot-renamed = Збережено як { $name } у { $folder }
cull-slot-skipped = { $folder } уже містить { $name } — пропущено
cull-slot-same = Зображення вже в { $folder }
cull-slot-failed = { $op ->
    [move] Не вдалося перемістити { $file }
   *[copy] Не вдалося скопіювати { $file }
}: { $reason }
cull-copy-kept = Залишено копії, змінені відтоді: { $count }
cull-undo-failed = Не вдалося скасувати для { $file }: { $reason }
cull-burst-open = Серія { $index } з { $count }
cull-burst-closed = Серія: { $count }
cull-burst-toggle = Відкрити або закрити серію
cull-burst-best = Залишено 1, відхилено: { $count }
cull-burst-none = Не входить до серії
cull-follow-on = Стеження за новими файлами
cull-follow-off = Без стеження за новими файлами
cull-new-files = Нові файли: { $count }
cull-key-copy = Копіювати до теки 1–3
cull-key-move = Перемістити до теки 1–3
cull-key-burst = Відкрити або закрити серію
cull-key-best = Залишити це, відхилити решту серії
cull-key-follow = Стежити за новими файлами
cull-key-subframes = Показати або сховати таблицю субкадрів
cull-key-sort = Сортувати за наступним стовпцем
cull-key-sort-reverse = Обернути порядок
cull-key-metric = Наступна метрика на графіку
cull-key-sigma-down = Автовідхилення: зменшити k
cull-key-sigma-up = Автовідхилення: збільшити k
cull-key-auto-reject = Відхилити запропоновані кадри
cull-key-move-rejects = Перемістити відхилені кадри до підтеки
cull-key-same-stretch = Однакове розтягування для всіх кадрів

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Відкрити серію
cull-tip-burst-close = Закрити серію
cull-tip-exit = Вийти з режиму відбору
cull-tip-label = Мітка: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Субкадри
cull-astro-title = { $count ->
    [one] { $count } субкадр
    [few] { $count } субкадри
    [many] { $count } субкадрів
   *[other] { $count } субкадру
}
cull-astro-pending = вимірювання: { $count }
cull-astro-col-name = Кадр
cull-astro-col-filter = Фільтр
cull-astro-col-exposure = Витр.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Ексц.
cull-astro-col-stars = Зірки
cull-astro-col-bg = Фон
cull-astro-col-snr = SNR
cull-astro-col-noise = Шум
cull-astro-same-stretch = Однакове розтягування
cull-astro-own-stretch = Власне розтягування
cull-astro-suggest-high = { $metric } понад { $value } (медіана + { $k }σ): { $count ->
    [one] { $count } кадр
    [few] { $count } кадри
    [many] { $count } кадрів
   *[other] { $count } кадру
}
cull-astro-suggest-low = { $metric } нижче { $value } (медіана − { $k }σ): { $count ->
    [one] { $count } кадр
    [few] { $count } кадри
    [many] { $count } кадрів
   *[other] { $count } кадру
}
cull-astro-suggest-none = Жоден кадр не виходить за { $k }σ
cull-astro-suggest-few = Автовідхиленню потрібно 3 виміряні кадри
cull-astro-suggested = запропоновано відхилити
cull-astro-k = k { $k }
cull-astro-reject = Відхилити { $count }
cull-astro-move = Перемістити відхилені…
cull-astro-chart = { $metric }, кадрів: { $count }, медіана { $median }.
cull-astro-sort-by = Сортувати за: { $column }
cull-astro-ascending = за зростанням
cull-astro-descending = за спаданням
cull-astro-sorted = Відсортовано за: { $column }
cull-astro-reversed = Порядок обернено
cull-astro-metric = Графік: { $metric }
cull-astro-k-set = Автовідхилення при { $k }σ
cull-astro-none = Тут немає субкадрів
cull-astro-same-on = Однакове розтягування для всіх кадрів
cull-astro-same-off = Кожен кадр розтягнуто окремо
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = ексцентриситет { $value }
cull-astro-line-stars = { $count ->
    [one] { $count } зірка
    [few] { $count } зірки
    [many] { $count } зірок
   *[other] { $count } зірки
}
cull-astro-measuring = вимірювання зірок…
cull-astro-reject-confirm = { $count ->
    [one] Відхилити { $count } запропонований кадр?
    [few] Відхилити { $count } запропоновані кадри?
    [many] Відхилити { $count } запропонованих кадрів?
   *[other] Відхилити { $count } запропонованого кадру?
}
cull-astro-reject-detail = { $rule }. Скасування повертає позначки.
cull-astro-reject-ok = Відхилити
cull-astro-nothing-to-reject = Немає кадрів для відхилення
cull-astro-rejected = { $count ->
    [one] Відхилено { $count } кадр
    [few] Відхилено { $count } кадри
    [many] Відхилено { $count } кадрів
   *[other] Відхилено { $count } кадру
}
cull-move-rejects-confirm = { $count ->
    [one] Перемістити { $count } відхилений кадр до теки «{ $folder }»?
    [few] Перемістити { $count } відхилені кадри до теки «{ $folder }»?
    [many] Перемістити { $count } відхилених кадрів до теки «{ $folder }»?
   *[other] Перемістити { $count } відхиленого кадру до теки «{ $folder }»?
}
cull-move-rejects-detail = Файли переміщуються, а не видаляються. Скасування повертає їх на місце.
cull-move-rejects-ok = Перемістити
cull-move-rejects-none = Немає відхилених кадрів для переміщення
cull-moved-rejects = { $count ->
    [one] Переміщено { $count } кадр до «{ $folder }»
    [few] Переміщено { $count } кадри до «{ $folder }»
    [many] Переміщено { $count } кадрів до «{ $folder }»
   *[other] Переміщено { $count } кадру до «{ $folder }»
}
