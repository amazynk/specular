# machine-drafted, needs native review
# Specular — Фонова робота: індикатор, що показується, поки щось виконується у фоні, і панель, яку він відкриває. Українська.

## Панель

work-title = Фонова робота
work-show = Показати фонову роботу
work-empty = У фоні нічого не виконується.

## Завдання

work-kind-library = Бібліотека фото
work-kind-ai-search = Пошук ШІ
work-kind-faces = Люди
work-kind-text = Текст на фото
work-kind-culling = Оцінки для відбору
work-kind-download = Завантаження моделі ШІ
work-kind-similar = Схожі знімки

## Кроки

work-step-finding = Пошук фото
work-step-reading = Читання відомостей про фото
work-step-signatures = Порівняння фото
work-step-grouping = Групування схожих знімків
work-step-text-index = Створення пошукового індексу
work-step-embedding = Індексування фото для пошуку ШІ
work-step-switching = Повторне індексування для нового розміру моделі
work-step-faces = Пошук облич
work-step-ocr = Читання тексту на фото
work-step-scoring = Оцінювання різкості й експозиції
work-step-downloading = Завантаження
work-step-focus = Перевірка різкості
work-step-item = { $step } · { $item }

## Поступ

work-count-photos = { $done } з { $total } { $count ->
    [one] фото
    [few] фото
    [many] фото
   *[other] фото
}
work-count = { $done } з { $total }
work-count-left = { $count } · залишилося { $left }
work-so-far-photos = Поки що { $done } { $count ->
    [one] фото
    [few] фото
    [many] фото
   *[other] фото
}
work-so-far = Поки що { $done }

## Залишок часу

work-eta-estimating = Оцінювання…
work-eta-seconds = Залишилося близько { $n ->
    [one] { $n } секунди
    [few] { $n } секунд
    [many] { $n } секунд
   *[other] { $n } секунди
}
work-eta-minutes = Залишилося близько { $n ->
    [one] { $n } хвилини
    [few] { $n } хвилин
    [many] { $n } хвилин
   *[other] { $n } хвилини
}
work-eta-hours = Залишилося близько { $h ->
    [one] { $h } години
    [few] { $h } годин
    [many] { $h } годин
   *[other] { $h } години
}
work-eta-hours-minutes = Залишилося близько { $h ->
    [one] { $h } години
    [few] { $h } годин
    [many] { $h } годин
   *[other] { $h } години
} і { $m ->
    [one] { $m } хвилини
    [few] { $m } хвилин
    [many] { $m } хвилин
   *[other] { $m } хвилини
}

## Чому завдання чекає

work-paused-browsing = Призупинено, поки ви переглядаєте фото
work-paused-battery = Призупинено під час роботи від батареї
work-paused-low-power = Призупинено в режимі енергозбереження
work-paused-heat = Призупинено, поки комп’ютер гарячий
work-paused-memory = Призупинено, поки бракує пам’яті
work-paused-user = Призупинено

## Що далі

work-then = Далі: { $steps }

## Кнопки

work-pause = Призупинити
work-resume = Продовжити
work-settings = Налаштування…

## Індикатор

work-chip = { $job } · { $done } з { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Фонова робота: { $status }

## Оголошення

work-started = { $job }: розпочато у фоні
work-finished = { $job }: завершено
