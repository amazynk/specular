# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Українська.

palette-dicom-servers = Сервери DICOMweb…

dicomweb-title = Сервери DICOMweb
dicomweb-servers-intro = Дослідження на сервері шукаються й відкриваються лише на ваш запит. Заздалегідь нічого не надсилається.
dicomweb-servers = Сервери
dicomweb-servers-empty = Серверів ще немає. Додайте сервер за його адресою DICOMweb.
dicomweb-session-only = Паролі й токени зберігаються лише до закриття Specular.
dicomweb-add = Додати…
dicomweb-edit = Змінити…
dicomweb-remove = Вилучити
dicomweb-close = Закрити
dicomweb-browse = Переглянути
dicomweb-back = Назад
dicomweb-save = Зберегти
dicomweb-search = Шукати
dicomweb-stop = Зупинити
dicomweb-removed = Вилучено: { $server }
# machine-drafted, needs native review
dicomweb-remove-ask = Вилучити «{ $server }» зі збережених серверів?
dicomweb-remove-detail = Нічого на диску не втрачається. Його пароль або токен для цього сеансу забувається.
# Сервери без шифрування (http://).
dicomweb-not-encrypted = Без шифрування

# Сторінка додавання або зміни сервера.
dicomweb-add-title = Додати сервер
dicomweb-edit-title = Параметри сервера
dicomweb-field-name = Назва
dicomweb-hint-name = Необов’язково
dicomweb-field-url = Адреса
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Вхід
dicomweb-auth-none = Без входу
dicomweb-auth-basic = Ім’я користувача й пароль
dicomweb-auth-bearer = Токен доступу
dicomweb-field-user = Ім’я користувача
dicomweb-field-password = Пароль
dicomweb-field-token = Токен доступу
dicomweb-hint-secret-kept = Залиште порожнім, щоб зберегти вказане значення
dicomweb-secret-session = Пароль або токен зберігається лише до закриття Specular, ніколи на диску.
dicomweb-secret-needed = Введіть пароль або токен знову: він зберігається лише до закриття Specular.
dicomweb-http-warning = Ця адреса не зашифрована: пароль або токен буде надіслано відкритим текстом.
dicomweb-url-empty = Введіть адресу DICOMweb сервера.
dicomweb-url-scheme = Адреса має починатися з https:// або http://.
dicomweb-url-host = В адресі немає імені сервера.
dicomweb-url-userinfo = Вкажіть ім’я користувача й пароль у полях входу, а не в адресі.
dicomweb-save-failed = Не вдалося зберегти список серверів: { $reason }

# Сторінка пошуку.
dicomweb-search-title = Пошук досліджень
dicomweb-field-patient-name = Ім’я пацієнта
dicomweb-field-patient-id = ID пацієнта
dicomweb-field-date-from = З
dicomweb-field-date-to = По
dicomweb-hint-date = РРРРММДД
dicomweb-field-modality = Модальність
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Номер направлення
dicomweb-show-patient = Показати дані пацієнта
dicomweb-switch-on = Увімк.
dicomweb-switch-off = Вимк.
dicomweb-patient-shown = Дані пацієнта показано
dicomweb-patient-hidden = Дані пацієнта приховано
dicomweb-studies = Дослідження
dicomweb-search-hint = Заповніть будь-які поля й запустіть пошук.
dicomweb-no-studies = Немає відповідних досліджень.
dicomweb-searching = Пошук…
dicomweb-stopped = Зупинено
dicomweb-load-more = Завантажити ще
dicomweb-study-untitled = Дослідження без опису
dicomweb-series-untitled = Серія без опису
dicomweb-study-counts = Серії: { $series }, зображення: { $images }
dicomweb-series-images = Зображення: { $count }
dicomweb-patient-id = ID { $id }
dicomweb-accession = Направлення { $number }
dicomweb-found = Знайдено досліджень: { $count }
dicomweb-series-found = Серії: { $count }

# Помилки зв’язку із сервером.
dicomweb-error-offline = Сервер { $server } недоступний. Перевірте мережу й адресу.
dicomweb-error-auth = Сервер відхилив ім’я користувача, пароль або токен.
dicomweb-error-not-found = За цією адресою немає служби DICOMweb.
dicomweb-error-other = Сервер не зміг відповісти: { $reason }

# Серія, що передається в переглядач потоком.
dicomweb-loading = Завантаження: { $series }…
dicomweb-loading-progress = Завантаження: { $series } – { $done } з { $total }
dicomweb-loaded = { $series }: завантажено зображень: { $total }
dicomweb-loaded-failed = Не вдалося завантажити зображень: { $failed } з { $total }
dicomweb-offline-cached = Сервер { $server } недоступний: показано раніше збережені зображення.
dicomweb-cache-failed = Не вдалося відкрити теку завантажень: { $reason }
