# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = Додаткові декодери
settings-decoders-look-again = Шукати знову
settings-decoders-look-again-help = Шукає декодери, встановлені відтоді.
    { -app-name } також шукає щоразу, коли його вікно виходить на передній план.
settings-decoders-braw = Використовувати Blackmagic RAW SDK
settings-decoders-braw-help = Кліпи Blackmagic RAW відкриваються з ним, коли його встановлено.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Залиште порожнім, щоб шукати там, куди його ставить інсталятор.
settings-decoders-dwg = Використовувати ODA File Converter
settings-decoders-dwg-help = Креслення DWG відкриваються з ним, коли його встановлено.
    Копію креслення перетворюють в окремій приватній теці, яку потім видаляють.
settings-decoders-dwg-path = Програма ODA File Converter
settings-decoders-dwg-path-help = Залиште порожнім, щоб шукати там, куди її ставить інсталятор.
settings-decoders-occt = Використовувати OpenCASCADE
settings-decoders-occt-help = Моделі STEP і IGES відкриваються з ним, коли його встановлено.
settings-decoders-occt-path = Бібліотеки OpenCASCADE
settings-decoders-occt-path-help = Залиште порожнім, щоб шукати їх там, де система зберігає бібліотеки.
# machine-drafted, needs native review
settings-decoders-czi = Використовувати libCZI
settings-decoders-czi-help = Файли Zeiss CZI відкриваються за його допомогою, якщо його встановлено.
    Він працює в ізольованому допоміжному процесі { -app-name }, без доступу до інших ваших файлів.
settings-decoders-czi-path = Тека бібліотеки libCZI
settings-decoders-czi-path-help = Залиште порожнім, щоб шукати там, де система зберігає бібліотеки.

## The panel: one block per decoder.

decoders-intro = Деякі формати відкриваються безкоштовними програмами від їхніх розробників.
    Установіть один раз: { -app-name } знайде й використає. Сам він їх ніколи не завантажує.
decoders-keywords = додаткові декодери розробник програма кодек плагін потрібен декодер optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = Знайдено
decoders-state-off = Вимкнено
decoders-state-not-found = Не встановлено
decoders-state-broken = Не можна використати
decoders-state-refused = Неможливо
decoders-state-planned = Ще недоступно
decoders-state-checking = Пошук…
# The row's lines.
decoders-uses = Відкривається за допомогою: { $decoder } від { $vendor }.
decoders-not-found = Це безкоштовно. Установіть, а тоді натисніть «Шукати знову».
decoders-off = Вимкнено: ці файли не відкриваються.
decoders-version = Версія { $version }.
decoders-r3d-why = Ліцензія RED дозволяє читати файли R3D лише програмам, створеним з її SDK на власних умовах RED.
    { -app-name } не може прийняти ці умови, тож файли R3D не відкриваються.
decoders-ecw-why = Ліцензія Hexagon дозволяє використовувати ECW SDK лише програмам, які його постачають, а не копії, яку ви встановили самі.
    Тому файли ECW не відкриваються. Файли JPEG 2000 з ECW SDK відкриваються.
decoders-mrsid-why = Ліцензія MrSID SDK дозволяє використовувати його лише програмам, які його постачають, а не копії, яку ви встановили самі.
    Тому файли MrSID не відкриваються.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = Ліцензія SDK від DJI дозволяє використовувати його лише програмам, які його постачають, а не копії, яку ви встановили самі. Фото та його метадані все одно відкриваються.
decoders-occt-why = Тіла STEP і IGES потребують OpenCASCADE, а ця збірка { -app-name } не може його використовувати.
# Rows with one button each, under a decoder.
decoders-get-row = Сторінка завантаження від розробника
decoders-get = Отримати
decoders-locate-row = Встановлено деінде?
decoders-locate = Вказати…
decoders-forget-row = Ви вибрали його вручну.
decoders-forget = Забути…
# Asked before Forget… does anything.
decoders-forget-ask = Забути вибраний вами декодер { $decoder }?
decoders-forget-detail = { -app-name } знову шукатиме його сам. Нічого на диску не видаляється.
decoders-forget-ok = Забути
# The title of the Locate… file dialog.
decoders-locate-title = Вкажіть декодер { $decoder }
# Toasts.
decoders-locate-wrong = Це не { $decoder }.
decoders-located = Використовується вибраний вами декодер { $decoder }.
decoders-forgotten = { -app-name } знову сам шукає декодер { $decoder }.
decoders-still-missing = Декодер { $decoder } досі не знайдено.
decoders-now-found = Знайдено декодер { $decoder }.
decoders-looked = Пошук декодерів повторено.

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = Для { $format } потрібен безкоштовний декодер від { $vendor }
decoders-card-what = { $file } відкривається за допомогою: { $decoder }. Це безкоштовно.
decoders-card-how = Установіть його й поверніться: { -app-name } сам його знайде.
decoders-card-get = Отримати
decoders-card-installed = Уже встановлено
decoders-card-locate = Вказати…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = Потрібен декодер
gallery-needs-decoder = Відкривається безкоштовним декодером { $decoder } від { $vendor }, який не встановлено.

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = Для файлів { $format } потрібен безкоштовний декодер { $decoder } від { $vendor }, який не встановлено.
