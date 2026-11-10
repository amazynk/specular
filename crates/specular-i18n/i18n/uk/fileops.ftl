# machine-drafted, needs native review
# Дії з файлами: перейменування, пакетне перейменування, копіювання й переміщення до теки,
# поворот, збережений у файлі, скасування й повторення, шпалери, надсилання, надсилання
# поштою, експорт та імпорт налаштувань.

## Палітра, шпаргалка й меню

palette-undo = Скасувати
palette-undo-help = Скасувати останню зміну файлів: переміщення в кошик, перейменування, копіювання чи переміщення, поворот.
palette-redo = Повторити
palette-redo-help = Знову виконати те, що скасовано останнім.
palette-rename = Перейменувати…
palette-rename-help = Дати файлу нову назву; супровідні файли й пара RAW+JPEG змінюються разом із ним.
palette-batch-rename = Перейменувати файли…
palette-batch-rename-help = Перейменувати вибрані файли або всю теку за шаблоном, із попереднім переглядом.
palette-copy-to-folder = Копіювати до теки…
palette-copy-to-folder-help = Копіювати файл або вибране до іншої теки.
palette-move-to-folder = Перемістити до теки…
palette-move-to-folder-help = Перемістити файл або вибране до іншої теки.
palette-rotate-file-left = Повернути файл ліворуч
palette-rotate-file-left-help = Повернути сам файл на чверть оберту ліворуч, без втрат, якщо формат дозволяє.
palette-rotate-file-right = Повернути файл праворуч
palette-rotate-file-right-help = Повернути сам файл на чверть оберту праворуч, без втрат, якщо формат дозволяє.
palette-set-wallpaper-help = Зробити зображення тлом робочого столу.
palette-share = Поділитися…
palette-share-help = Надіслати файл в іншу програму через системне меню «Поділитися».
palette-send-by-email = Надіслати поштою…
palette-send-by-email-help = Створити новий лист у поштовій програмі з вкладеним файлом.
# machine-drafted, needs native review
palette-share-with-zynk = Поділитися через Zynk…
palette-share-with-zynk-help = Створити вебпосилання на файл у застосунку Zynk.
palette-send-with-zynk = Надіслати через Zynk…
palette-send-with-zynk-help = Надіслати файл контакту або на один із ваших пристроїв через Zynk.
palette-export-settings = Експортувати налаштування…
palette-export-settings-help = Зберегти всі налаштування й призначення клавіш у файл.
palette-import-settings = Імпортувати налаштування…
palette-import-settings-help = Прочитати налаштування з файлу, спершу показавши, що зміниться.

disabled-in-archive-file = Ця сторінка в архіві й не має власного файлу
disabled-not-rotatable = Файл такого типу не може зберегти поворот
disabled-not-on-this-desktop = Ваше робоче середовище цього не підтримує

## Налаштування

settings-browsing-rotate-trim-edges = Обтинати краї JPEG для повороту без втрат
settings-browsing-rotate-trim-edges-help = JPEG, розмір якого не кратний 8 чи 16 пікселям, можна повернути без втрат лише відтявши останні кілька пікселів. Без цього натомість змінюється тег орієнтації, а обтинання пропонується щоразу.
settings-browsing-rotate-keep-modified = Зберігати дату зміни під час повороту
settings-browsing-rotate-keep-modified-help = Повернутий файл зберігає дату останньої зміни, тож під час сортування за датою лишається на своєму місці.
settings-browsing-recent-folders = Нещодавні теки
settings-browsing-recent-folders-help = Теки, востаннє використані для копіювання й переміщення.

## Діалоги

fileop-cancel = Скасувати
rename-title = Перейменування
rename-field-label = Назва
rename-button = Перейменувати
rename-sidecars-follow = Також перейменовано: { $files }
rename-extension-changed = Зміна розширення не перетворює файл; інші програми можуть не відкрити його.
rename-problem-empty = Введіть назву.
rename-problem-invalid-char = Назва не може містити «{ $char }».
rename-problem-reserved = «{ $name }» — назва, яку Windows резервує для пристроїв.
rename-problem-too-long = Назва задовга; назви можуть мати до 255 байтів.
rename-problem-dot = «.» і «..» не можна використовувати як назви.
rename-problem-trailing = Назва не може закінчуватися пробілом чи крапкою.
rename-problem-exists = Інший файл уже має цю назву.
rename-problem-read-only = Цю теку не можна змінювати.
rename-problem-duplicate = Два файли отримали б цю назву.

batch-rename-title =
    { $count ->
        [one] Перейменувати { $count } файл
        [few] Перейменувати { $count } файли
        [many] Перейменувати { $count } файлів
       *[other] Перейменувати { $count } файлу
    }
batch-rename-field-label = Шаблон
batch-rename-tokens = Використовуйте { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } і { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } файл буде перейменовано
        [few] { $count } файли буде перейменовано
        [many] { $count } файлів буде перейменовано
       *[other] { $count } файлу буде перейменовано
    }{ $problems ->
        [0] .
        [one] ; { $problems } не можна перейменувати, він лишається як є.
        [few] ; { $problems } не можна перейменувати, вони лишаються як є.
        [many] ; { $problems } не можна перейменувати, вони лишаються як є.
       *[other] ; { $problems } не можна перейменувати, вони лишаються як є.
    }
batch-rename-reading = Читання дат і камер…
batch-rename-preview = Попередній перегляд
batch-rename-unchanged = назва не змінюється
batch-rename-more =
    { $count ->
        [one] ще { $count } нижче
        [few] ще { $count } нижче
        [many] ще { $count } нижче
       *[other] ще { $count } нижче
    }

transfer-copy-title-one = Копіювати «{ $file }» до теки
transfer-copy-title =
    { $count ->
        [one] Копіювати { $count } файл до теки
        [few] Копіювати { $count } файли до теки
        [many] Копіювати { $count } файлів до теки
       *[other] Копіювати { $count } файлу до теки
    }
transfer-move-title-one = Перемістити «{ $file }» до теки
transfer-move-title =
    { $count ->
        [one] Перемістити { $count } файл до теки
        [few] Перемістити { $count } файли до теки
        [many] Перемістити { $count } файлів до теки
       *[other] Перемістити { $count } файлу до теки
    }
transfer-recent = Нещодавні теки
transfer-folder = Тека
transfer-choose-folder = Вибрати теку…
transfer-copy-dialog = Копіювати до теки
transfer-move-dialog = Перемістити до теки
transfer-sidecars =
    { $count ->
        [one] { $count } супровідний файл іде разом.
        [few] { $count } супровідні файли йдуть разом.
        [many] { $count } супровідних файлів ідуть разом.
       *[other] { $count } супровідного файлу йде разом.
    }
transfer-taken =
    { $count ->
        [one] { $count } назва вже зайнята в { $folder }.
        [few] { $count } назви вже зайняті в { $folder }.
        [many] { $count } назв уже зайнято в { $folder }.
       *[other] { $count } назви вже зайнято в { $folder }.
    }
transfer-when-taken = Коли назва вже зайнята
transfer-skip = Пропустити
transfer-keep-both = Залишити обидва
transfer-replace-option = Замінити
transfer-copy = Копіювати
transfer-move = Перемістити
transfer-replace-title =
    { $count ->
        [one] Замінити { $count } файл у { $folder }?
        [few] Замінити { $count } файли у { $folder }?
        [many] Замінити { $count } файлів у { $folder }?
       *[other] Замінити { $count } файлу у { $folder }?
    }
transfer-replace-note = Наявні там файли підуть у кошик. «Скасувати» поверне їх.
transfer-replace = Замінити
transfer-back = Назад
transfer-stop = Зупинити

settings-file-import-title = Імпорт налаштувань
settings-file-import-dialog = Імпорт налаштувань
settings-file-export-dialog = Експорт налаштувань
settings-file-import-changes = Зміни
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = без клавіші
settings-file-import-unknown = Такого налаштування в цій версії немає; пропущено.
settings-file-import-keymap = Призначення клавіш
settings-file-import-nothing = Нічого не зміниться: ці налаштування вже діють.
settings-file-import-summary =
    { $count ->
        [one] { $count } зміна. Імпорт можна скасувати в Налаштуваннях.
        [few] { $count } зміни. Імпорт можна скасувати в Налаштуваннях.
        [many] { $count } змін. Імпорт можна скасувати в Налаштуваннях.
       *[other] { $count } зміни. Імпорт можна скасувати в Налаштуваннях.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } запис схожий на пароль чи ключ і ніколи не імпортується.
        [few] { $count } записи схожі на паролі чи ключі й ніколи не імпортуються.
        [many] { $count } записів схожі на паролі чи ключі й ніколи не імпортуються.
       *[other] { $count } запису схожі на паролі чи ключі й ніколи не імпортуються.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } команду користувача не імпортовано: команди запускають програми. Якщо довіряєте їй, скопіюйте її у файл налаштувань самі.
        [few] { $count } команди користувача не імпортовано: команди запускають програми. Якщо довіряєте їм, скопіюйте їх у файл налаштувань самі.
        [many] { $count } команд користувача не імпортовано: команди запускають програми. Якщо довіряєте їм, скопіюйте їх у файл налаштувань самі.
       *[other] { $count } команди користувача не імпортовано: команди запускають програми. Якщо довіряєте їм, скопіюйте їх у файл налаштувань самі.
    }
settings-file-import-button = Імпортувати

## Результати

fileop-renamed = Перейменовано на { $file }
fileop-renamed-count =
    { $count ->
        [one] Перейменовано { $count } файл
        [few] Перейменовано { $count } файли
        [many] Перейменовано { $count } файлів
       *[other] Перейменовано { $count } файлу
    }
fileop-copied =
    { $count ->
        [one] Скопійовано { $count } файл до { $folder }
        [few] Скопійовано { $count } файли до { $folder }
        [many] Скопійовано { $count } файлів до { $folder }
       *[other] Скопійовано { $count } файлу до { $folder }
    }
fileop-moved =
    { $count ->
        [one] Переміщено { $count } файл до { $folder }
        [few] Переміщено { $count } файли до { $folder }
        [many] Переміщено { $count } файлів до { $folder }
       *[other] Переміщено { $count } файлу до { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } пропущено: його назва там уже зайнята.
        [few] { $count } пропущено: їхні назви там уже зайняті.
        [many] { $count } пропущено: їхні назви там уже зайняті.
       *[other] { $count } пропущено: їхні назви там уже зайняті.
    }
fileop-stopped = Зупинено; зроблене лишається зробленим.
fileop-failed-file =
    { $count ->
        [one] Не вдалося скопіювати чи перемістити { $file }: { $reason }
        [few] Не вдалося скопіювати чи перемістити { $count } файли; перший, { $file }: { $reason }
        [many] Не вдалося скопіювати чи перемістити { $count } файлів; перший, { $file }: { $reason }
       *[other] Не вдалося скопіювати чи перемістити { $count } файлу; перший, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Копіювання { $count } файлу… { PERCENT($percent) }
        [few] Копіювання { $count } файлів… { PERCENT($percent) }
        [many] Копіювання { $count } файлів… { PERCENT($percent) }
       *[other] Копіювання { $count } файлу… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Переміщення { $count } файлу… { PERCENT($percent) }
        [few] Переміщення { $count } файлів… { PERCENT($percent) }
        [many] Переміщення { $count } файлів… { PERCENT($percent) }
       *[other] Переміщення { $count } файлу… { PERCENT($percent) }
    }
fileop-done-status = Готово
fileop-busy = Остання зміна файлів ще триває; спробуйте за мить.
fileop-nothing-to-redo = Нічого повторювати
fileop-undone = Скасовано: { $what }
fileop-redone = Повторено: { $what }
fileop-kind-rename = перейменування
fileop-kind-batch-rename = перейменування кількох файлів
fileop-kind-move = переміщення до теки
fileop-kind-copy = копіювання до теки
fileop-kind-rotate = поворот
rename-failed = Не вдалося перейменувати: { $reason }

undo-problem-changed = Не можна скасувати: { $file } відтоді змінено, і скасування втратить цю зміну.
undo-problem-missing = Не можна скасувати: { $file } більше немає.
undo-problem-taken = Не можна скасувати: назву { $file } тепер має інший файл.
undo-problem-io = Не можна скасувати зміну { $file }: { $reason }

rotate-done-lossless = { $file } повернуто без жодних втрат.
rotate-done-lossless-trimmed = { $file } повернуто без жодних втрат; край обтято до { $width } × { $height }.
rotate-done-tag = { $file } повернуто зміною тегу орієнтації.
rotate-done-sidecar = Поворот { $file } збережено в { $sidecar }; сам RAW-файл ніколи не змінюється.
rotate-tag-unreadable = Дані зображення не вдалося прочитати настільки, щоб його повернути.
rotate-tag-multi-picture = Він містить кілька зображень, і поворот першого пошкодив би інші.
rotate-tag-trailer = Після зображення є дані, які поворот зсунув би.
rotate-tag-not-aligned = Поворот пікселів без втрат обтяв би його до { $width } × { $height }.
rotate-trim-offer = Обтяти й повернути
rotate-problem-unsupported = { $file } — файл такого типу, що не може зберегти поворот.
rotate-problem-too-large = { $file } завеликий для повороту.
rotate-problem-damaged = { $file } пошкоджено там, де зберігається орієнтація: { $reason }
rotate-problem-io = Не вдалося повернути { $file }: { $reason }

share-failed = Не вдалося поділитися: { $reason }
email-failed = Не вдалося запустити поштову програму: { $reason }
# machine-drafted, needs native review
zynk-opening = Відкриття Zynk…
zynk-failed = Не вдалося зв’язатися із Zynk: { $reason }
zynk-get-title-send = «Надіслати через Zynk» потребує програми Zynk
zynk-get-title-share = «Поділитися через Zynk» потребує програми Zynk
zynk-get-about = Zynk — безплатна програма для Mac, Windows, Linux, iPhone та Android. Вона надсилає фото, відео й цілі теки напряму на інші ваші пристрої та іншим людям, будь-якого розміру. Передавання наскрізно зашифровані, а якщо з’єднання обірветься, продовжуються з того місця, де зупинилися. Для одержувача, який не має Zynk, вона створює вебпосилання, за яким файли можна завантажити в браузері.
zynk-get-macos = Завантажте її з Mac App Store. Щойно її буде встановлено, { -app-name } передаватиме ваші файли просто їй.
zynk-get-windows = Завантажте її з Microsoft Store. Щойно її буде встановлено, { -app-name } передаватиме ваші файли просто їй.
zynk-get-linux = Zynk для Linux — це AppImage, нічого не треба встановлювати: завантажте його, покладіть у ~/Applications і зробіть виконуваним (у його властивостях або командою chmod +x). { -app-name } знайде його там, а також будь-де, поки Zynk запущено.
zynk-get = Отримати Zynk
zynk-send-old-windows = «Надіслати через Zynk» потребує Windows 10 версії 2004 або новішої. «Поділитися через Zynk» працює в цій версії Windows.
settings-file-export-done = Налаштування збережено в { $file }
settings-file-export-failed = Не вдалося зберегти налаштування: { $reason }
settings-file-import-unreadable = Не вдалося прочитати { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] Імпортовано { $count } зміну
        [few] Імпортовано { $count } зміни
        [many] Імпортовано { $count } змін
       *[other] Імпортовано { $count } зміни
    }
