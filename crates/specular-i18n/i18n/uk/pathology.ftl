# machine-drafted, needs native review
# Specular — Патологія: розділення барвників, експорт та імпорт анотацій, об’єми міток поверх об’ємів. Українська.

## Палітра
palette-stain-separation = Розділити барвники
palette-stain-1 = Показати або сховати барвник 1
palette-stain-2 = Показати або сховати барвник 2
palette-stain-3 = Показати або сховати барвник 3
palette-stain-mode = Барвники як світло або оптична густина
palette-stain-estimate = Оцінити барвники за ділянкою
palette-stain-normalise = Нормалізувати кольори барвників
palette-stain-reference = Використати ділянку як еталон барвників
palette-stain-reset = Опубліковані вектори барвників
palette-export-annotations = Експортувати анотації…
palette-import-annotations = Імпортувати анотації…
palette-label-volume = Накласти об’єм міток…

## Причини недоступності
disabled-not-colour-image = Потрібне кольорове зображення
disabled-stains-off = Спершу увімкніть розділення барвників
disabled-no-region = Спершу намалюйте ділянку
disabled-no-slide-annotations = Немає анотацій, ділянок чи точок для експорту
disabled-no-label-target = Відкрийте об’єм у багатоплощинних видах або медичне зображення

## Назви барвників
stain-hematoxylin = Гематоксилін
stain-eosin = Еозин
stain-dab = DAB
stain-residual = Залишок
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Повідомлення й легенда барвників
stain-off = Розділення барвників вимкнено
# { $set }: H&E, H-DAB або H&E-DAB.
stain-on = Барвники: { $set }
stain-shown = { $stain }: показано
stain-hidden = { $stain }: сховано
stain-mode-light = Барвники як світло
stain-mode-od = Барвники як оптична густина
# Заголовок легенди: «H&E · світло», «H-DAB · оптична густина».
stain-legend-light = { $set } · світло
stain-legend-od = { $set } · оптична густина
stain-legend-estimated = Оцінено за ділянкою
stain-legend-normalised = Нормалізовано
stain-legend-published = Опубліковані вектори
# { $pixels }: використані забарвлені пікселі.
stain-estimated = Барвники оцінено за { $pixels } пікселями
stain-estimate-failed = Не вдалося оцінити барвники: { $reason }
stain-estimate-background = ділянка здебільшого фон
stain-estimate-one-colour = ділянка має один колір
stain-estimate-three = для оцінки потрібен H&E або H-DAB
stain-estimate-no-pixels = пікселі ділянки ще не завантажено
stain-reference-set = Ділянку задано як еталон барвників
stain-normalise-on = Кольори барвників нормалізовано
stain-normalise-off = Кольори барвників як при скануванні
stain-reset-done = Опубліковані вектори барвників

## Вікно експорту анотацій
annot-export-title = Експорт анотацій
annot-export-format = Формат
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Маски міток
annot-export-output = Маски як
annot-export-output-png = Тайли PNG
annot-export-output-tiff = Тайли TIFF
annot-export-output-ome = Піраміда OME-TIFF
annot-export-region = Ділянка
annot-export-region-slide = Увесь препарат
annot-export-region-view = Видима ділянка
annot-export-region-drawn = Остання намальована ділянка
annot-export-level = Рівень
# { $level }: 0 — повна роздільність; { $width } × { $height } — його розмір у пікселях.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Розмір тайла
annot-export-empty = Порожні тайли
annot-export-empty-skip = Пропустити
annot-export-empty-keep = Записати
annot-export-what = { $count ->
    [one] { $count } фігура
    [few] { $count } фігури
    [many] { $count } фігур
   *[other] { $count } фігури
}, { $classes ->
    [one] { $classes } клас
    [few] { $classes } класи
    [many] { $classes } класів
   *[other] { $classes } класу
}
annot-export-lines = Лінії не потрапляють до COCO і масок.
annot-export-export = Експортувати…
annot-export-writing = Запис…
# { $files }: записані файли.
annot-export-done = { $files ->
    [one] Експортовано { $files } файл
    [few] Експортовано { $files } файли
    [many] Експортовано { $files } файлів
   *[other] Експортовано { $files } файлу
}
annot-export-failed = Не вдалося експортувати: { $reason }
annot-import-done = { $count ->
    [one] Імпортовано { $count } анотацію
    [few] Імпортовано { $count } анотації
    [many] Імпортовано { $count } анотацій
   *[other] Імпортовано { $count } анотації
}
annot-import-failed = Не вдалося імпортувати: { $reason }
annot-import-skipped = { $count ->
    [one] { $count } фігура не мала придатного контуру
    [few] { $count } фігури не мали придатного контуру
    [many] { $count } фігур не мали придатного контуру
   *[other] { $count } фігури не мали придатного контуру
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Об’єми міток
label-volume-file = Об’єм міток
label-volume-table = Таблиця кольорів
# { $file }: назва файлу об’єму міток; { $count }: його мітки.
label-volume-loaded = { $count ->
    [one] { $file }: { $count } мітка
    [few] { $file }: { $count } мітки
    [many] { $file }: { $count } міток
   *[other] { $file }: { $count } мітки
}
label-volume-failed = Не вдалося накласти { $file }: { $reason }
label-volume-outside = { $file } не перетинається з цим об’ємом
label-volume-not-whole = його значення не є цілими числами
label-volume-colour = він містить колір, а не мітки
label-volume-empty = у ньому немає міток
label-volume-too-many = у ньому забагато міток
label-volume-too-large = він завеликий для передискретизації
label-volume-degenerate = його орієнтація недійсна
# Мітка без назви в таблиці кольорів.
label-volume-unnamed = Мітка { $value }
