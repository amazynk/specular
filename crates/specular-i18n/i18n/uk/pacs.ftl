# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Тип
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Хост
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Порт
dicomweb-hint-port = 104 або 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE title сервера
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Наш AE title
# machine-drafted, needs native review
dicomweb-field-tls = Захищене з’єднання (TLS)
dicomweb-tls-on = Захищене з’єднання ввімкнено
dicomweb-tls-off = Захищене з’єднання вимкнено
dicomweb-problem-host = Введіть ім’я хоста або адресу сервера.
dicomweb-problem-port = Порт — це число від 1 до 65535.
dicomweb-problem-ae = AE title має від 1 до 16 символів, без зворотної скісної риски.
# machine-drafted, needs native review
dicomweb-dimse-note-open = На серверах DICOM (DIMSE) можна шукати, відкривати серії та надсилати на них.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Отримувати через
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Приймати на адресі
# machine-drafted, needs native review
pacs-hint-receive-address = Адреса цього комп’ютера, наприклад 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Приймати на порту
# machine-drafted, needs native review
pacs-hint-receive-port = Наприклад 11113
# machine-drafted, needs native review
pacs-move-note = Для C-MOVE сервер має знати наш AE title { $ae } з цією адресою і портом.
    Specular слухає там лише під час надходження серії.
# machine-drafted, needs native review
pacs-problem-receive-address = Введіть адресу цього комп’ютера (наприклад 192.168.1.20), на якій приймати.
# machine-drafted, needs native review
pacs-problem-receive-port = Введіть порт для приймання. Він потрібен для C-MOVE.
dicomweb-verify = Перевірити
dicomweb-verifying = Перевірка сервера…
dicomweb-verify-ok = Сервер { $server } відповів.

## The Studies browser and the palette.

studies-send = Надіслати в PACS…
studies-send-description = Надсилає цю серію, її дослідження або все на диску на сервер.
palette-send-to-pacs = Надіслати в PACS…
palette-send-to-pacs-help = Надіслати поточну серію або дослідження на сервер PACS.

## The Send page.

pacs-title = Надіслати в PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } дослідження
    [few] { $studies } дослідження
    [many] { $studies } досліджень
   *[other] { $studies } дослідження
} · { $files ->
    [one] { $files } файл
    [few] { $files } файли
    [many] { $files } файлів
   *[other] { $files } файлу
}
pacs-what = Надіслати
pacs-scope-series = Ця серія ({ $files ->
    [one] { $files } файл
    [few] { $files } файли
    [many] { $files } файлів
   *[other] { $files } файлу
})
pacs-scope-study = Це дослідження ({ $files ->
    [one] { $files } файл
    [few] { $files } файли
    [many] { $files } файлів
   *[other] { $files } файлу
})
pacs-scope-all = Усе прочитане ({ $studies ->
    [one] { $studies } дослідження
    [few] { $studies } дослідження
    [many] { $studies } досліджень
   *[other] { $studies } дослідження
}, { $files ->
    [one] { $files } файл
    [few] { $files } файли
    [many] { $files } файлів
   *[other] { $files } файлу
})
pacs-anonymise = Анонімізувати
pacs-anonymise-note = Дані пацієнта вилучаються з надісланих копій. Ваші файли не змінюються.
pacs-identified-note = Файли надсилаються як є, з даними пацієнта.
# The list of servers to choose from.
pacs-to = Куди
pacs-no-servers = Серверів ще немає. Додайте сервер у розділі «Сервери DICOMweb».
pacs-send = Надіслати…
pacs-go-back = Назад
pacs-confirm-send = Надіслати
pacs-confirm = Надіслати { $studies ->
    [one] { $studies } дослідження
    [few] { $studies } дослідження
    [many] { $studies } досліджень
   *[other] { $studies } дослідження
} ({ $files ->
    [one] { $files } файл
    [few] { $files } файли
    [many] { $files } файлів
   *[other] { $files } файлу
}) на сервер { $server }?
pacs-busy = Надсилання ще триває.
pacs-started = Надсилання.
pacs-stopping = Зупинка…
pacs-sending = Надсилання { $done } з { $total } на сервер { $server }…
pacs-sent-all = { $total ->
    [one] { $total } файл надіслано на сервер { $server }.
    [few] Усі { $total } файли надіслано на сервер { $server }.
    [many] Усі { $total } файлів надіслано на сервер { $server }.
   *[other] Усі { $total } файлу надіслано на сервер { $server }.
}
pacs-sent-some = На сервер { $server } надіслано { $sent } з { $total } файлів.
pacs-stopped = Зупинено після { $sent } з { $total ->
        [one] { $total } файлу
        [few] { $total } файлів
        [many] { $total } файлів
       *[other] { $total } файлу
    }.
pacs-stopped-because = Зупинено після { $sent } з { $total ->
        [one] { $total } файлу
        [few] { $total } файлів
        [many] { $total } файлів
       *[other] { $total } файлу
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } файл збережено з попередженням.
    [few] { $count } файли збережено з попередженням.
    [many] { $count } файлів збережено з попередженням.
   *[other] { $count } файлу збережено з попередженням.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = І ще { $count }.

## Why one file was not sent.

pacs-reason-unreadable = Не вдалося прочитати: { $reason }
pacs-reason-not-dicom = Не файл DICOM
pacs-reason-burned-in = Містить дані пацієнта, вбудовані в зображення, тому не надсилається анонімізованим
pacs-reason-document = Документ, який неможливо анонімізувати
pacs-reason-anonymise = Не вдалося анонімізувати: { $reason }
pacs-reason-not-accepted = Сервер не приймає { $class }
pacs-reason-convert = Не вдалося перетворити для сервера: { $reason }
pacs-reason-resources = Серверу бракує місця або ресурсів
pacs-reason-mismatch = За словами сервера, дані не відповідають своєму типу
pacs-reason-understand = Сервер не зміг його прочитати
pacs-reason-class = Сервер не підтримує цей тип об’єкта
pacs-reason-not-authorised = Сервер цього не дозволяє
pacs-reason-status = Сервер відхилив його (статус { $code })
pacs-reason-refused = Сервер відхилив його
pacs-reason-too-large = Завеликий для сервера
pacs-reason-http = Сервер відхилив його (помилка { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Мережу вимкнено.
pacs-fatal-unknown-host = { $host } не є відомим ім’ям хоста.
pacs-fatal-connect = За адресою { $host }, порт { $port }, ніщо не відповідає.
pacs-fatal-unreachable = { $host } недоступний.
pacs-fatal-timeout = { $host } не відповів вчасно.
pacs-fatal-cancelled = Зупинено.
pacs-fatal-called-ae = { $host } не відповідає на AE title { $ae }. Перевірте AE title сервера.
pacs-fatal-calling-ae = { $host } не знає нашого AE title { $ae }. Додайте його на сервері.
pacs-fatal-busy = { $host } зайнятий. Спробуйте пізніше.
pacs-fatal-rejected = { $host } відхилив з’єднання.
pacs-fatal-aborted = { $host } розірвав з’єднання.
pacs-fatal-nothing-accepted = { $host } не приймає жодного з цих типів об’єктів.
pacs-fatal-protocol = { $host } відповів у спосіб, якого Specular не розуміє.
pacs-fatal-auth = { $host } не прийняв вхід.
pacs-fatal-http = { $host } відповів помилкою { $status }.

## Teaching quiz.

palette-teaching-quiz = Навчальна вікторина
palette-teaching-quiz-help = Переглядати ключові зображення або навчальний випадок, з прихованими знахідками, доки ви їх не відкриєте.
# "Case 2 of 5".
quiz-case = Випадок { $n } з { $total }
quiz-hint-hidden = Пробіл відкриває знахідки · → наступний випадок · Esc завершує
quiz-hint-revealed = Пробіл знову ховає їх · → наступний випадок · Esc завершує
quiz-hidden = Знахідки приховано.
quiz-no-findings = Знахідок не записано.
quiz-notes = Нотатки до випадку: { $notes }
quiz-ended = Вікторину завершено.
quiz-empty = Немає випадків для показу. Позначте ключові зображення клавішею K або виберіть теку навчального випадку.
quiz-not-a-case = Ця тека не є навчальним випадком, експортованим Specular.
quiz-pick-title = Виберіть теку навчального випадку

## Teaching case folder (Export key images).

figure-keys-case = Тека навчального випадку
figure-case-notes = Нотатки до випадку
figure-case-findings = Знахідки
figure-case-none = Нічого не записано
figure-case-readme = Навчальний випадок, експортований Specular. Дані пацієнта вилучено.
    images: ключові зображення, вбудований текст зачорнено.
    DICOM: анонімізовані копії та маніфест Key Object Selection «For Teaching».
    case.txt: нотатки до випадку та знахідки для кожного зображення. case.json: те саме для скриптів.
    Для Radiopaedia: завантажте теку DICOM або зображення та скопіюйте нотатки з case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } не відповідає на пошукові запити (C-FIND).
pacs-find-status = { $host } відхилив пошук (статус { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } не надсилає серії через C-GET.
    Виберіть C-MOVE у налаштуваннях сервера.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } не приймає запити C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } не знає нашого AE title { $ae }.
    Додайте його на сервері з адресою цього комп’ютера і портом { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = У { $host } бракує ресурсів. Спробуйте пізніше.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } відмовився надіслати серію (статус { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } не має зображень для цієї серії.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } відмовився надсилати зображення через це з’єднання.
    Виберіть C-MOVE у налаштуваннях сервера.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } не зміг надіслати зображення на цей комп’ютер.
    Перевірте адресу, порт і брандмауер, а також чи знає сервер наш AE title { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } не зміг надіслати { $count ->
    [one] зображення
    [few] жодного з { $count } зображень
    [many] жодного з { $count } зображень
   *[other] жодного з { $count } зображення
} у формі, яку читає Specular.
# machine-drafted, needs native review
pacs-retrieve-listen = Не вдається приймати на { $address }, порт { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Сертифікат { $host } не є довіреним.
    Його видавець має бути серед довірених сертифікатів системи.
pacs-fatal-tls = Немає захищеного з’єднання з { $host }.
    Перевірте, чи порт { $port } використовує DICOM через TLS.
