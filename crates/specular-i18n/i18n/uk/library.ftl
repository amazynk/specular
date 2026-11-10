# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Бібліотека

## Top bar
library-title = Бібліотека
library-locations-button = Розташування

## Locations sidebar
library-locations = Розташування
library-all = Усі розташування
library-add = Додати розташування…
library-remove = Вилучити { $location } з бібліотеки
library-offline = Офлайн
library-location-pending = Залишилося індексувати: { $count }
library-a11y-location = { $location }, { $count ->
    [one] { $count } зображення
    [few] { $count } зображення
    [many] { $count } зображень
   *[other] { $count } зображення
}
library-a11y-location-offline = { $location }, Офлайн, { $count ->
    [one] { $count } зображення
    [few] { $count } зображення
    [many] { $count } зображень
   *[other] { $count } зображення
}

## Indexing progress
library-walking = Знайдено зображень: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Знайдено зображень: { $count }
library-reading = Індексування… { $done } з { $total }
library-held = Індексування призупиняється, поки ви переглядаєте
library-up-to-date = Актуально
# machine-drafted, needs native review
library-stopped = Індексування зупинено
library-stopped-pending = Індексування зупинено · залишилося індексувати: { $count }
library-resume = Продовжити
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Індексування зупинено: { $reason }
library-reason-stalled = нічого не рухалося протягом { $minutes ->
    [one] однієї хвилини
    [few] { $minutes } хвилин
    [many] { $minutes } хвилин
   *[other] { $minutes } хвилини
}. Файл або диск, можливо, не відповідає.
library-retry = Спробувати ще раз
library-retry-description = Знову запускає індексування. Проіндексоване залишається доступним для пошуку.
library-save-diagnostics = Зберегти діагностичний пакет…
library-stop-description = Зупиняє індексування. У вже проіндексованому можна й далі шукати; «Продовжити» або «Оновити» (F5) продовжує.
library-resume-description = Знову переглядає всі розташування й індексує те, що залишилося.
library-opening = Відкриття бібліотеки…
library-searching = Пошук…

## Empty and error states
library-empty = Розташувань ще немає. Додайте теку: її зображення та зображення з підтек з’являться тут в одній сітці.
library-empty-offline = Ці розташування офлайн. Їхні зображення з’являться знову, коли вони повернуться.
library-indexing = Пошук зображень…
# machine-drafted, needs native review
library-remove-confirm = Вилучити «{ $location }» з бібліотеки?
library-remove-confirm-detail = Ваші фото залишаються на диску разом з оцінками, мітками й ключовими словами. Бібліотека забуває те, що там проіндексувала; якщо додати теку знову, її буде проіндексовано знову.
library-remove-confirm-ok = Вилучити
library-indexing-in = Пошук зображень у { $location }… поки що знайдено: { $count }
# machine-drafted, needs native review
library-empty-stopped = Індексування зупинили, перш ніж ці зображення було прочитано. «Продовжити» продовжить звідти.
library-unavailable = Не вдалося відкрити бібліотеку: { $reason }
library-read-only = Цю бібліотеку записав новіший Specular: у ній можна шукати, але не оновлювати.

## Screen readers
library-a11y-list = Бібліотека, { $count ->
    [one] { $count } зображення
    [few] { $count } зображення
    [many] { $count } зображень
   *[other] { $count } зображення
}
library-a11y-added = { $location } додано до бібліотеки
library-a11y-removed = { $location } вилучено з бібліотеки

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Додати все з «{ $folder }»?
library-home-confirm-detail = Там файли програм, завантаження та кеші, а не лише фото. Індексування триває довше, а бібліотека заповнюється піктограмами й іншими зображеннями.
library-home-confirm-pictures = У вашій теці «Зображення» лише ваші зображення.
library-home-confirm-use-pictures = Додати «Зображення»
library-home-confirm-keep = Додати домашню теку

## Folder watches
# machine-drafted, needs native review
library-unwatched = За деякими теками не стежать
library-unwatched-detail = { $minutes ->
    [one] Зміни з’являються протягом хвилини
    [few] Зміни з’являються протягом { $minutes } min
    [many] Зміни з’являються протягом { $minutes } min
   *[other] Зміни з’являються протягом { $minutes } min
}
library-unwatched-description = У Specular закінчилися стеження за теками. Відкриває довідку про те, як підвищити ліміт.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Бібліотека
settings-library-skip-folders = Пропускати теки з назвою
settings-library-skip-folders-help = Бібліотека не шукає в теках із цими назвами, розділеними комами. Приховані теки завжди пропускаються.
settings-library-skip-marked = Пропускати теки, що містять
settings-library-skip-marked-help = Не шукає й у теках, що містять щось із цього: .git для сховищ коду, pyvenv.cfg для середовищ Python, CACHEDIR.TAG для кешів. Власна тека розташування ніколи не пропускається.
settings-library-show-timeline = Шкала часу
settings-library-show-timeline-help = Показує над сіткою бібліотеки, скільки фото має кожен рік, місяць чи день.
