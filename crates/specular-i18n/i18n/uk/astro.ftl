# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Накласти області DS9…
palette-science-load-regions-help = Намалювати на зображенні фігури з файлу областей DS9 (.reg).
palette-science-export-regions = Зберегти області як DS9…
palette-science-export-regions-help = Зберегти вимірювання й накладені області як файл областей DS9.
palette-science-edit-regions = Редагувати області як вимірювання
palette-science-edit-regions-help = Перетворити накладені області на вимірювання, які можна переміщувати й змінювати.
palette-science-clear-regions = Прибрати накладені області
palette-science-clear-regions-help = Прибрати області DS9 із зображення.
palette-science-plate-solve = Астрометричне розв’язання
palette-science-plate-solve-help = Знайти небесні координати зображення за допомогою ASTAP або astrometry.net. Запустіть ще раз, щоб зупинити.
palette-science-save-plate-solution = Зберегти астрометричне розв’язання
palette-science-save-plate-solution-help = Зберегти знайдені небесні координати поруч із зображенням як файл .wcs.
palette-science-sky-survey = Огляд неба як тло
palette-science-sky-survey-help = Показати навколо зображення наступний огляд неба або жодного.
palette-science-samp-send-image = Надіслати зображення застосункам SAMP
palette-science-samp-send-image-help = Відкрити це зображення в DS9, Aladin та інших застосунках на хабі SAMP.
palette-science-samp-send-position = Надіслати позицію застосункам SAMP
palette-science-samp-send-position-help = Навести інші застосунки на позицію на небі під вказівником.

disabled-samp-off = Спершу під’єднайтеся до хабу SAMP (Налаштування › Інтеграція).

settings-integration-samp = Під’єднатися до хабу SAMP
settings-integration-samp-help = Обмінюватися зображеннями, таблицями й позиціями на небі з DS9, TOPCAT, Aladin та іншими астрономічними застосунками. Використовується лише хаб на цьому комп’ютері.
settings-integration-astap-path = Програма ASTAP
settings-integration-astap-path-help = Астрометричний розв’язувач ASTAP, якщо його не знайдено автоматично.
settings-integration-solve-field-path = Програма astrometry.net
settings-integration-solve-field-path-help = Програма solve-field з astrometry.net, якщо її не знайдено автоматично.

astro-regions-filter = Області DS9
astro-regions-loaded = { $name }: кількість областей: { $count }.
astro-regions-refused = Непрочитані рядки: { $count }. Перший: рядок { $line }, { $reason }.
astro-regions-approximate = Фігури panda, намальовані як внутрішній і зовнішній краї: { $count }.
astro-regions-none-here = { $name }: жодна область не потрапляє на це зображення.
astro-regions-failed = Не вдалося прочитати { $name }: { $reason }
astro-regions-cleared = Області прибрано.
astro-regions-nothing = Немає областей чи вимірювань для збереження.
astro-regions-saved = Збережено { $name }.
astro-regions-skipped = Пропущені фігури поза небесними координатами: { $count }.
astro-regions-save-failed = Не вдалося зберегти області: { $reason }
astro-regions-edited = Області, перетворені на вимірювання: { $count }.
astro-regions-no-wcs = Для областей на небі потрібне зображення з небесними координатами.

astro-solve-no-solver = Астрометричний розв’язувач не знайдено. Установіть ASTAP або astrometry.net чи виберіть його в Налаштування › Інтеграція.
astro-solve-running = Розв’язання за допомогою { $solver }… { $seconds } с
astro-solve-stop = Зупинити
astro-solve-stopped = Астрометричне розв’язання зупинено.
astro-solve-done = Розв’язано: { $ra }, { $dec }, { $scale } на піксель.
astro-solve-failed = Не вдалося виконати астрометричне розв’язання: { $reason }
astro-solve-not-local = Розв’язати можна лише зображення з теки на цьому комп’ютері.
astro-solve-none = Для цього зображення ще немає астрометричного розв’язання.
astro-solve-saved = Збережено { $name }. Саме зображення не змінено.
astro-solve-sidecar = Небесні координати з { $name }.

astro-samp-connected = Під’єднано до хабу SAMP.
astro-samp-no-hub = SAMP: { $reason }. Нова спроба, коли запуститься хаб.
astro-samp-lost = Хаб SAMP зупинився. Нова спроба, коли запуститься інший.
astro-samp-sent = Надіслано застосункам: { $count }.
astro-samp-nobody = Жоден застосунок на хабі не приймає цього.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } не на цьому комп’ютері, тож його не відкрито.
astro-samp-pointed = Наведено на { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } не на цьому зображенні.
astro-samp-no-table = SAMP: цю таблицю тут не накладено.
astro-samp-rows = Виділені джерела з каталогу: { $count }.

astro-survey-off = Огляд неба вимкнено.
astro-survey-on = Огляд неба: { $name }. { $credit }
astro-survey-failed = Не вдалося показати { $name }: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Плитки огляду неба з { $host }
