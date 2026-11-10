# machine-drafted, needs native review
# Інтеграція: «Відкрити за допомогою», команди користувача, вставлення (етап 2).
palette-open-with = Відкрити за допомогою…
palette-paste = Вставити
open-with-placeholder = Відкрити за допомогою…
open-with-loading = Пошук програм…
open-with-default = { $name } (типова)
open-with-choose-other = Вибрати іншу програму…
open-with-choose-title = Вибір програми
open-with-failed = Не вдалося відкрити за допомогою { $name }: { $reason }
command-missing = { $program } не знайдено
command-failed = Не вдалося запустити { $name }: { $reason }
command-exited = { $name } завершилася з помилкою: { $status }
paste-nothing = У буфері обміну немає нічого, що можна відкрити
paste-failed = Не вдалося вставити зображення: { $reason }
drag-failed = Не вдалося перетягнути файл: { $reason }
about-storage = Налаштування: { $settings } · Кеш: { $cache }
about-storage-portable = Портативний режим: налаштування й кеш у { $dir }

## Scripting and recipes

palette-copy-view-command = Копіювати вигляд як команду оболонки
palette-copy-view-python = Копіювати вигляд як Python
toast-copied-view-command = Команду скопійовано
script-copy-no-image = Немає зображення для опису
recipe-confirm-title = Запустити рецепт «{ $name }» уперше? Він виконує: { $command }
recipe-confirm-run = Запустити
recipe-no-file = Рецепту потрібен файл зображення
recipe-running = Виконується { $name }…
recipe-done = { $name }: { $shapes ->
    [one] додано { $shapes } фігуру
    [few] додано { $shapes } фігури
    [many] додано { $shapes } фігур
   *[other] додано { $shapes } фігури
}
recipe-failed = { $name }: помилка: { $reason }
recipe-timeout = зупинено через { $seconds } с
recipe-no-output = { $name } не записав { $file }
