# Структуровані звіти DICOM, криві, доза променевої терапії та офтальмологічна томографія. machine-drafted, needs native review

## Дерево вмісту структурованого звіту (переглядач тегів у режимі звіту)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Елемент
sr-col-concept = Поняття
sr-col-type = Тип
sr-col-relationship = Зв’язок
sr-col-value = Значення
sr-show-image = Показати зображення
sr-jump-missing = Зображення, на яке посилається звіт, немає в цій теці
sr-jump-frame = Жодне зображення цієї системи координат не містить цієї точки

## Графіки (гістограми доза–об’єм, записи ЕКГ)
plot-caliper-time = { $ms } мс · { $rate } /хв
plot-caliper-time-zero = 0 мс
plot-caliper-hint = Протягніть по графіку, щоб виміряти
plot-adjust-hint = + і − змінюють підсилення, [ і ] — швидкість

## Запис ЕКГ
ecg-paper = { $speed } мм/с · { $gain } мм/мВ · { $rate } Гц
ecg-groups-more = { $count ->
    [one] Ще { $count } група каналів не показана
    [few] Ще { $count } групи каналів не показані
    [many] Ще { $count } груп каналів не показано
   *[other] Ще { $count } групи каналів не показано
}

## Доза променевої терапії поверх MPR (ізодози, кольорова карта, легенда, показ значення)
dose-value = Доза { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Дозу приховано (Shift+U показує її)
dose-on-rx = Доза: призначено { $dose } { $unit } · макс. { $max } { $unit }
dose-on-max = Доза: відсоток від максимуму, { $max } { $unit }
dose-colour = Доза в { $colormap } при { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: гістограми доза–об’єм { $count } структури
    [few] Shift+K: гістограми доза–об’єм { $count } структур
    [many] Shift+K: гістограми доза–об’єм { $count } структур
   *[other] Shift+K: гістограми доза–об’єм { $count } структури
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Доза як ізодози
dose-mode-colour = Доза як кольорова карта

## Гістограми доза–об’єм (аркуш графіків)
dvh-title = Гістограми доза–об’єм
dvh-col-roi = Структура
dvh-col-volume = Об’єм
dvh-axis-dose = Доза ({ $unit })
dvh-axis-volume = Об’єм (%)
dvh-prescription = Призначена доза { $dose } Гр · кумулятивна
dvh-max = Максимальна доза { $dose } { $unit } · кумулятивна
dvh-none = Немає набору структур цієї системи координат, за яким можна побудувати гістограму дози

## Офтальмологічна томографія (повзунок B-сканів; оглядове зображення в куті)
oct-bscan = B-скан
oct-depth = глибина { $depth } мкм
