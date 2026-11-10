# machine-drafted, needs native review
## Панель довідника, звіти про збої, діагностичний пакет, перевірка оновлень і «Про програму». Українська.
## Стиль: коротко, ясно, без зайвого; одна думка в реченні.

## Палітра

palette-help-manual = Довідка
palette-help-manual-help = Відкрити довідник на сторінці про те, що на екрані.
palette-save-diagnostics = Зберегти діагностичний пакет…
palette-save-diagnostics-help = Зберегти звіти про збої, журнал і відомості про систему в zip-файл. Нічого не надсилається.
# Пункт палітри, що відкриває розділ довідника; { $title } — сторінка або «Сторінка › Розділ».
palette-help-topic = Довідка: { $title }
palette-help-topic-help = Відкрити цей розділ довідника.
# Під недоступною командою в палітрі.
palette-disabled-help = Зараз недоступно. Натисніть Enter, щоб дізнатися чому.

## Панель довідки

help-title = Довідка
help-search-placeholder = Пошук у довіднику
help-contents = Зміст
help-back = Назад
help-forward = Вперед
help-no-results = За запитом «{ $query }» нічого не знайдено.
help-results = { $count ->
    [one] Знайдено { $count } розділ
    [few] Знайдено { $count } розділи
    [many] Знайдено { $count } розділів
   *[other] Знайдено { $count } розділу
}
help-section-start = Початок роботи
help-section-modes = Перегляд і режими
help-section-tasks = Як…
help-section-reference = Довідкові дані
help-keys-unbound = Не призначено
# Команда з довідника без клавіші; вона запускається з палітри.
help-key-unbound = { $name } (палітра)
help-keys-command = Команда
help-keys-keys = Клавіші
help-commands = Усі команди є в палітрі команд ({ $key }). Введіть частину назви.
help-commands-no-key = Усі команди є в палітрі команд. Введіть частину назви.
help-link = Перейти до: { $title }
help-open = Довідка
# Назва панелі для програм читання з екрана; { $page } — показана сторінка.
help-label = Довідка: { $page }
help-disabled-why = Чому?

## Звіт про збій, запропонований під час наступного запуску

crash-title = { -app-name } неочікувано закрився
crash-body = Звіт збережено на цьому комп’ютері. Нічого не надіслано.
crash-view = Переглянути звіт
crash-hide = Сховати звіт
crash-copy = Копіювати
crash-copied = Звіт скопійовано
crash-save-bundle = Зберегти діагностичний пакет…
crash-dismiss = Закрити

## Діагностичний пакет

diag-title = Зберегти діагностичний пакет
diag-intro = Zip-файл, який можна долучити до звіту про помилку. Він зберігається на цьому комп’ютері. Нічого не надсилається.
diag-contents = Вміст
diag-include-names = Додати назви файлів
diag-include-names-help = Вимкнено: назви замінюються на file-1.jpg, dir-1 тощо.
diag-privacy = Ваша домашня тека показується як ~, а ваше ім’я користувача — як <user>. Паролі, адреси серверів і зображення ніколи не додаються.
diag-where = Повідомити про проблему: { $issues } · { $email }
diag-save = Зберегти…
diag-cancel = Скасувати
diag-saved = Діагностичний пакет збережено: { $path }
diag-failed = Не вдалося зберегти діагностичний пакет: { $reason }
diag-entry-summary = Версія, система, GPU і драйвер
diag-entry-crash = Останній звіт про збій
diag-entry-crashes = Попередні звіти про збої ({ $count })
diag-entry-log = Останні рядки журналу: { $count }
diag-entry-settings = Налаштування, без паролів і адрес
diag-entry-gpu = Відомості про GPU і дисплей
diag-entry-readme = Що це за пакет
diag-size = { $size }
# README.txt у пакеті.
diag-readme =
    Діагностичний пакет { -app-name }.

    Він містить те, що допомагає знайти проблему: версію і відомості про систему, звіти про збої, кінець журналу і налаштування.

    Ваша домашня тека показується як ~, а ваше ім’я користувача — як <user>. Якщо ви не вибрали їх додати, назви файлів і тек замінюються на file-1, dir-1 тощо. Паролі та адреси серверів вилучаються.

    Нічого не надіслано. Ви вирішуєте, чи ділитися цим файлом.

## Оновлення (Налаштування)

settings-section-updates = Оновлення
settings-updates-check = Перевіряти оновлення
settings-updates-check-help = Раз на день запитувати, чи є нова версія. Запит містить лише номер версії: без ідентифікатора, без cookie. Вимкнено: { -app-name } ніколи не з’єднується заради оновлень.
settings-updates-channel = Канал
settings-updates-channel-help = Стабільний: лише випуски. Бета: також тестові версії.
settings-updates-channel-stable = Стабільний
settings-updates-channel-beta = Бета
settings-updates-check-now = Перевірити зараз
settings-updates-check-now-help = Перевірити один раз зараз.
update-available = Доступна версія { $version }
update-notes = Примітки до випуску
update-download = Завантажити
update-none = { -app-name } має найновішу версію.
update-checking = Перевірка оновлень…
update-failed = Не вдалося перевірити оновлення: { $reason }
update-last-checked = Остання перевірка: { $when }

## Про програму

about-build = Збірка { $commit } · { $profile } · { $target }
about-build-no-commit = Збірка { $profile } · { $target }
about-help = Довідка
about-diagnostics = Зберегти діагностичний пакет…
# machine-drafted, needs native review
about-renderer = Відомості про рендерер…
about-licences-count = { $count ->
    [one] { $count } сторонній компонент
    [few] { $count } сторонні компоненти
    [many] { $count } сторонніх компонентів
   *[other] { $count } стороннього компонента
}
about-licence-group = { $licence } ({ $count })
about-contact = Проблеми: { $issues } · Підтримка: { $email }
