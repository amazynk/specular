# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Порівняти серії (пов’язані вигляди)
# machine-drafted, needs native review
palette-compare-layout = Наступне компонування видів
# machine-drafted, needs native review
palette-next-viewport = Наступний вид
# machine-drafted, needs native review
palette-prev-viewport = Попередній вид
# machine-drafted, needs native review
palette-reference-lines = Показати або сховати опорні лінії
# machine-drafted, needs native review
palette-sync-window = Наступна синхр. вікна
# machine-drafted, needs native review
palette-sync-zoom = Спільне масштабування й панорамування за розміром
# machine-drafted, needs native review
palette-link-series = Наступний зв’язок серії
# machine-drafted, needs native review
palette-align-series = Вирівняти тут інші серії
# machine-drafted, needs native review
palette-compare-series-help = Показати кілька серій поруч, що прокручуються разом.
# machine-drafted, needs native review
palette-compare-layout-help = Показати більше або менше видів.
# machine-drafted, needs native review
palette-next-viewport-help = Зробити наступний вид активним.
# machine-drafted, needs native review
palette-prev-viewport-help = Зробити попередній вид активним.
# machine-drafted, needs native review
palette-reference-lines-help = Намалювати, де активний зріз перетинає інші види.
# machine-drafted, needs native review
palette-sync-window-help = Вибрати, які види спільно змінюють вікно/рівень: вимк., та сама модальність або всі.
# machine-drafted, needs native review
palette-sync-zoom-help = Показати всі види в однаковому розмірі й панорамувати їх разом.
# machine-drafted, needs native review
palette-link-series-help = Вибрати, як ця серія слідує за іншими.
# machine-drafted, needs native review
palette-align-series-help = Вирівняти попередню серію з цією на показаних зрізах.
# machine-drafted, needs native review
disabled-not-comparing = Спершу порівняйте серії

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Компонування: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Опорні лінії
# machine-drafted, needs native review
linked-zoom = Синхронізація масштабу
# machine-drafted, needs native review
linked-window-sync = Синхронізація вікна: { $mode ->
    [off] вимк.
    [modality] та сама модальність
   *[all] усі
}
# machine-drafted, needs native review
linked-link = Зв’язок: { $mode ->
    [frame] система відліку
    [position] положення пацієнта
   *[off] вимк.
}
# machine-drafted, needs native review
linked-align = Вирівняти тут

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Вид { $n } з { $count }, активний
   *[no] Вид { $n } з { $count }
}
# machine-drafted, needs native review
linked-empty = Перетягніть сюди серію
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Зріз { $n } з { $count } · { $position } мм
# machine-drafted, needs native review
linked-in-gap = У проміжку: найближчий зріз на відстані { $distance } мм
# machine-drafted, needs native review
linked-outside = Поза цією серією ({ $distance } мм)
# machine-drafted, needs native review
linked-not-linked = Не пов’язано
# machine-drafted, needs native review
linked-aligned = Вирівняно вручну ({ $offset } мм)
# machine-drafted, needs native review
linked-cursor = 3D-курсор у { $x }, { $y }, { $z } мм
# machine-drafted, needs native review
linked-chip = { $label }, зображень: { $count }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Тут лише одна серія: перетягніть іншу з оглядача досліджень
    [few] Порівняння { $count } серій
    [many] Порівняння { $count } серій
   *[other] Порівняння { $count } серії
}
# machine-drafted, needs native review
linked-no-geometry = Ця серія не має положень зрізів: її не можна пов’язати
# machine-drafted, needs native review
linked-active = Вид { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Нічого вирівнювати: інші серії мають ту саму систему відліку
    [one] Вирівняно 1 серію з активною, для цього сеансу
    [few] Вирівняно { $count } серії з активною, для цього сеансу
    [many] Вирівняно { $count } серій з активною, для цього сеансу
   *[other] Вирівняно { $count } серії з активною, для цього сеансу
}
