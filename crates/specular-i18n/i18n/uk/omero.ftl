# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Українська. machine-drafted, needs native review

palette-omero-servers = Сервери OMERO…

omero-title = Сервери OMERO
omero-servers-intro = Проєкти, скрини та зображення на сервері показуються лише під час перегляду. Доти нічого не надсилається.
omero-servers = Сервери
omero-servers-empty = Серверів ще немає. Додайте сервер за його адресою OMERO.web.
omero-session-only = Паролі зберігаються лише до закриття Specular.
omero-add = Додати…
omero-edit = Змінити…
omero-remove = Вилучити
omero-close = Закрити
omero-browse = Переглянути
omero-back = Назад
omero-save = Зберегти
omero-stop = Зупинити
omero-open = Відкрити
# $server a server's name.
omero-removed = Вилучено { $server }
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Без шифрування
# A server browsed without signing in.
omero-public = Публічний
# $user the user name a server is signed in with.
omero-signed-in-as = Як { $user }

# The page that adds or edits a server.
omero-add-title = Додати сервер
omero-edit-title = Налаштування сервера
omero-field-name = Назва
omero-hint-name = Необов’язково
omero-field-url = Адреса
omero-hint-url = https://omero.example.org
omero-field-user = Ім’я користувача
omero-hint-user = Порожнє для публічних даних
omero-field-password = Пароль
omero-hint-password-kept = Залиште порожнім, щоб зберегти вказаний пароль
omero-password-session = Пароль зберігається лише до закриття Specular, ніколи на диску.
omero-password-needed = Введіть пароль: він зберігається лише до закриття Specular.
omero-http-warning = Ця адреса не зашифрована: пароль було б надіслано відкритим текстом.
omero-url-invalid = Введіть адресу OMERO.web, що починається з https:// або http://.
# $reason why the list could not be saved.
omero-save-failed = Не вдалося зберегти список серверів: { $reason }

# The browse page.
omero-browse-title = Перегляд
omero-top = Проєкти та скрини
omero-datasets = Набори даних
omero-plates = Планшети
omero-wells = Лунки
omero-fields = Поля
omero-images = Зображення
omero-kind-project = Проєкт
omero-kind-dataset = Набір даних
omero-kind-image = Зображення
omero-kind-screen = Скрин
omero-kind-plate = Планшет
omero-kind-well = Лунка
# $well a well's row and column, as B7.
omero-well = Лунка { $well }
omero-well-fields = { $count ->
    [one] { $count } поле
    [few] { $count } поля
    [many] { $count } полів
   *[other] { $count } поля
}
omero-children = { $count ->
    [one] { $count } елемент
    [few] { $count } елементи
    [many] { $count } елементів
   *[other] { $count } елемента
}
omero-load-more = Завантажити ще
omero-loading = Завантаження…
omero-empty = Тут нічого немає
omero-stopped = Зупинено
omero-found = { $count ->
    [one] { $count } елемент
    [few] { $count } елементи
    [many] { $count } елементів
   *[other] { $count } елемента
}
# $reason why the image could not be opened.
omero-open-failed = Не вдалося відкрити зображення: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Принаймні один канал залишається показаним
# $server a server's name.
omero-session-ended = Сеанс на { $server } завершився. Введіть пароль, щоб продовжити.

# Errors.
# $server a server's name.
omero-error-offline = Не вдалося з’єднатися з { $server }.
omero-error-sign-in = Цей сервер вимагає входу.
omero-error-expired = Сеанс завершився.
omero-error-login = Ім’я користувача або пароль відхилено.
omero-error-forbidden = Ваш обліковий запис може не мати доступу до цього.
omero-error-not-found = Не знайдено на сервері (можливо, вилучено).
# $code an HTTP status code.
omero-error-status = Сервер відповів помилкою { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Не вдалося прочитати відповідь сервера: { $reason }
