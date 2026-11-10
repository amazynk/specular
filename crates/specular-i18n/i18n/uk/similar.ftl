# machine-drafted, needs native review
# Specular — дублікати й схожі знімки (категорії галереї). Українська.

similar-duplicates = Дублікати
similar-similar = Схожі знімки

## Пункти меню «Вигляд» і палітри
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } група
    [few] { $groups } групи
    [many] { $groups } груп
   *[other] { $groups } групи
}, можна звільнити { $size }
similar-menu-scanning = { $name } — сканування ({ PERCENT($percent) })
similar-menu-none = { $name } — нічого не знайдено

## Заголовок
similar-summary = { $groups ->
    [one] { $groups } група для перегляду
    [few] { $groups } групи для перегляду
    [many] { $groups } груп для перегляду
   *[other] { $groups } групи для перегляду
}, { $files ->
    [one] { $files } файл
    [few] { $files } файли
    [many] { $files } файлів
   *[other] { $files } файлу
} на вилучення, можна звільнити { $size }
similar-progress = Розглянуто { $resolved } з { $total }
similar-skipped = Пропущено: { $count }
similar-scanning = Пошук дублікатів і схожих знімків… { $done } з { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Перевірка різкості… { $done } з { $total }
# machine-drafted, needs native review
# Список під час першого сканування (його поступ — у заголовку вище).
similar-waiting = Групи з’являться тут, коли сканування завершиться.
# machine-drafted, needs native review
similar-stopped = Сканування зупинено: нічого не згруповано
similar-stopped-sharpness = Перевірку різкості зупинено: найкращі знімки вибрано за мініатюрами
similar-stopped-empty = Сканування зупинено. Виміряне збережено: «Сканувати знову» продовжить звідти.
similar-scan-again = Сканувати знову
similar-empty-duplicates = У цій теці немає дублікатів.
similar-empty-similar = У цій теці немає схожих знімків.
similar-all-done = Усі групи переглянуто.

## Схожі знімки: наскільки подібними мають бути фото для групування
similar-sensitivity = Чутливість
similar-sensitivity-strict = Суворо
similar-sensitivity-loose = Вільно

## Нижня частина списку груп
similar-accept-all = Прийняти всі пропозиції
similar-show-skipped = Показати пропущені групи
similar-discard-with = Вилучення: { $action }
similar-action-trash = Перемістити в кошик
similar-action-move = Перемістити до теки…
similar-action-move-to = Перемістити до «{ $folder }»
similar-action-reject = Лише позначити як відхилені
similar-choose-folder = Вибрати теку для вилучених файлів…

## Масове підтвердження
similar-confirm = Прийняти пропозиції для { $groups ->
    [one] { $groups } групи
    [few] { $groups } груп
    [many] { $groups } груп
   *[other] { $groups } групи
}? { $files ->
    [one] { $files } файл
    [few] { $files } файли
    [many] { $files } файлів
   *[other] { $files } файлу
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Прийняти й вилучити { $files } файл
    [few] Прийняти й вилучити { $files } файли
    [many] Прийняти й вилучити { $files } файлів
   *[other] Прийняти й вилучити { $files } файлу
}
similar-cancel = Скасувати

## Група
similar-group-title = Група { $index } з { $total }
similar-group-photos = { $count ->
    [one] { $count } фото
    [few] { $count } фото
    [many] { $count } фото
   *[other] { $count } фото
}
similar-to-free = Можна звільнити { $size }

## Дії перегляду та стани
similar-keep = Залишити
similar-discard = Вилучити
similar-undecided = Не вирішено
similar-accept-next = Прийняти й далі
similar-not-similar = Не схожі
similar-skip = Пропустити
similar-compare = Порівняти
similar-zoom = { PERCENT(100) }
similar-back = Назад
similar-done = Готово
similar-suggested-keep = Пропозиція: залишити
similar-suggested-discard = Пропозиція: вилучити

## Сповіщення
similar-toast-accepted = { $count ->
    [one] Прийнято { $count } групу
    [few] Прийнято { $count } групи
    [many] Прийнято { $count } груп
   *[other] Прийнято { $count } групи
}
similar-toast-rejected = { $count ->
    [one] { $count } файл позначено як відхилений
    [few] { $count } файли позначено як відхилені
    [many] { $count } файлів позначено як відхилені
   *[other] { $count } файлу позначено як відхилені
}
similar-toast-protected = Фото з оцінкою, позначкою, міткою, ключовими словами чи змінами завжди залишаються.
# machine-drafted, needs native review
similar-zoom-already = Це фото вже показано в масштабі { PERCENT(100) } або більше

## Доступність
similar-a11y-member = { $name }: { $state }, { $reason }

## Підказки клавіш
similar-hint-list = ↑↓ вибір · Enter перегляд · A прийняти все · Tab перемкнути · Esc закрити

## Чому фото залишається
similar-reason-rated = Має оцінку
similar-reason-flagged = Позначено як відібране
similar-reason-labelled = Має кольорову мітку
similar-reason-tagged = Має ключові слова
similar-reason-edited = Має зміни
similar-reason-original = Оригінал
similar-reason-raw-original = Оригінал RAW
similar-reason-highest-resolution = Найвища роздільність
similar-reason-deeper-colour = Глибший колір
similar-reason-least-compressed = Найменше стиснення
similar-reason-has-metadata = Зберігає метадані
similar-reason-oldest = Найстаріша копія
similar-reason-sharpest = Найрізкіше
similar-reason-steadiest = Найменше змазування
similar-reason-best-exposure = Найкраща експозиція
similar-reason-least-noise = Найменше шуму
similar-reason-also-good = Теж вдале

## Чому фото пропонується вилучити
similar-reason-same-file = Ідентичний файл
similar-reason-derivative = Похідна копія
similar-reason-rendering-of-raw = Обробка RAW
similar-reason-smaller-copy = Менша копія
similar-reason-shallower-colour = Менша глибина кольору
similar-reason-more-compressed = Сильніше стиснення
similar-reason-no-metadata = Метадані видалено
similar-reason-newer-copy = Новіша копія
similar-reason-blurrier = Менш різке
similar-reason-motion-blur = Змазування від руху
similar-reason-overexposed = Переекспоновано
similar-reason-underexposed = Недоекспоновано
similar-reason-noisier = Більше шуму
similar-reason-lower-resolution = Нижча роздільність
