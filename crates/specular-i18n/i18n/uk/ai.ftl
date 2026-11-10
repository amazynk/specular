# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Ukrainian. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Локальний ШІ
settings-ai-pack = Пакет локального ШІ
# machine-drafted, needs native review
settings-ai-pack-help = Додаткові функції, що працюють лише на цьому комп’ютері. Ніщо з того, що вони знаходять, його не залишає. Виберіть модель для кожної функції; нічого не завантажується, доки ви її не виберете.
# machine-drafted, needs native review
settings-ai-install-from-file = Встановити з файлу…
settings-ai-install-from-file-help = Встановлює файли моделей, скопійовані на цей комп’ютер, для машин без інтернету. Кожен файл перевіряється за списком, що постачається з { -app-name }.
settings-ai-install-from-folder = Встановити з теки…
settings-ai-install-from-folder-help = Те саме для кожного файлу моделі в теці.
settings-ai-search = Пошук за описом
settings-ai-search-help = Знаходьте фото, описуючи їх, наприклад «собака на пляжі», і краще групуйте схожі знімки.
settings-ai-text = Текст на зображеннях
settings-ai-text-help = Розпізнає текст на фото й сканах, щоб пошук text: їх знаходив.
settings-ai-culling = Допомога з відбором
# machine-drafted, needs native review
settings-ai-culling-help = Позначає заплющені очі, розмиття й погану експозицію, щоб допомогти вибрати найкращий знімок серії.
settings-ai-people = Люди
# machine-drafted, needs native review
settings-ai-people-help = Знаходить обличчя й групує їх за людьми для пошуку people:. Групування вимкнене, доки ви не ввімкнете його для бібліотеки. Дані облич можна видалити для однієї людини або для всіх одразу.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] У цій збірці немає доступних моделей.
        [installed] Встановлено
        [indexing] Встановлено · проіндексовано { $done } з { $total } зображень
        [downloading] Завантаження… { $percent }
        [failed] Не вдалося завантажити
       *[available] Не встановлено
    }
ai-removing = Видалення…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Завантажити
        [cancel] Скасувати
        [remove] Вилучити
        [forget] Забути
        [faces] Видалити всі дані облич
       *[pack] Вилучити пакет
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Видалити всі дані облич? Буде видалено всі знайдені обличчя та людей. Ваші фото залишаться.
       *[pack] Вилучити пакет локального ШІ, його моделі та все, що він знайшов? Ваші фото не змінюються.
    }

## Gallery search
gallery-query-needs-ai = { $field }: потрібен пакет локального ШІ (Налаштування › Локальний ШІ)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: пакет локального ШІ ще не може шукати за цим

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Обличчя показано
# machine-drafted, needs native review
faces-hidden = Обличчя приховано
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Заплющені очі
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Заплющені очі
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Заплющені очі: { $closed } з { $faces } облич
# machine-drafted, needs native review
cull-flag-blurred = Розмитий
# machine-drafted, needs native review
cull-flag-dark = Надто темний
# machine-drafted, needs native review
cull-flag-bright = Надто світлий
# machine-drafted, needs native review
cull-flag-clipped = Відсічені світлі ділянки або тіні
# machine-drafted, needs native review
# The People view.
people-title = Люди
# machine-drafted, needs native review
people-open = Люди
# machine-drafted, needs native review
people-done = Готово
# machine-drafted, needs native review
people-back = Назад
# machine-drafted, needs native review
people-show-photos = Показати фото
# machine-drafted, needs native review
people-name = Назвати…
# machine-drafted, needs native review
people-name-prompt = Назвіть цю людину
# machine-drafted, needs native review
people-faces-open = Обличчя
# machine-drafted, needs native review
people-faces-of = Обличчя: { $name }
# machine-drafted, needs native review
people-merge = Об’єднати
# machine-drafted, needs native review
people-merged = Об’єднано людей: { $count }
# machine-drafted, needs native review
people-merge-ask = Об’єднати { $from } з { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Об’єднати людей ({ $count }) з { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Їхні обличчя стануть обличчями { $into }. Фото на диску не зміняться.
# machine-drafted, needs native review
people-not-this = Це не ця людина
# machine-drafted, needs native review
people-face-removed = Обличчя вилучено з цієї людини. Фото залишається.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Людина #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } обличчя
        [few] { $count } обличчя
        [many] { $count } облич
       *[other] { $count } обличчя
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } людина
        [few] { $count } людини
        [many] { $count } людей
       *[other] { $count } людини
    }
# machine-drafted, needs native review
people-face-a11y = Обличчя { $index }
# machine-drafted, needs native review
people-loading = Завантаження…
# machine-drafted, needs native review
people-not-installed = Функцію «Люди» не встановлено. Завантажте її в Налаштування › Локальний ШІ.
# machine-drafted, needs native review
people-grouping-off = Групування облич вимкнене для цієї бібліотеки. Увімкніть «Групувати обличчя за людьми» в Налаштування › Локальний ШІ.
# machine-drafted, needs native review
people-none = Людей ще не знайдено.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Модель
settings-ai-search-size-help = SigLIP 2 B/16 знаходить фото найкраще, багатьма мовами. B/32 індексує приблизно вчетверо швидше, для повільніших відеокарт.
settings-ai-search-size-recommended = Типова
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Модель
settings-ai-text-size-help = PP-OCRv6 small читає точніше; tiny швидша.
settings-ai-text-size-recommended = Типова
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Великий
# machine-drafted, needs native review
settings-ai-culling-size = Модель
settings-ai-culling-size-help = Модель, що знаходить заплющені очі.
settings-ai-culling-size-recommended = Типова
settings-ai-culling-size-small = Малий
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Великий
# machine-drafted, needs native review
settings-ai-people-size = Модель
settings-ai-people-size-help = SFace — мала модель. AuraFace більша й навчена на даних, які її видавець називає ліцензованими для комерційного використання.
settings-ai-people-size-recommended = Типова
settings-ai-people-size-small = Малий
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Малий
        [large] Великий
       *[standard] Стандартний
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Типова ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Типова для вашої { $gpu }: { $size }
ai-recommended = Типова для цього комп’ютера: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Жоден розмір не вміщується в пам’ять цього комп’ютера.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Працює на: GPU ({ $gpu })
        [gpu-unnamed] Працює на: GPU
        [no-gpu] Працює на: CPU (GPU не знайдено)
        [differed] Працює на: CPU — результат GPU відрізнявся від результату CPU
        [failed] Працює на: CPU — модель не спрацювала на GPU
        [setting] Працює на: CPU (обробку налаштовано на «Лише CPU»)
        [checking] Працює на: CPU — триває звірка GPU з ним
       *[cpu] Працює на: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } на диску
       *[download] { $bytes } для завантаження
    } · { $memory } пам’яті{ $gpu ->
        [yes] { " · потребує GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Потрібен GPU, а на цьому комп’ютері немає жодного, який можна використати.
       *[memory] Потрібно { $need } пам’яті — більше, ніж { $have }, які Specular дозволяє для ШІ на цьому комп’ютері.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = Використовується { $current }, а не { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Повернути типову
        [anyway] Усе одно використати
       *[use] Використати цей розмір
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Вибрано, але ще не завантажено: і далі використовується розмір { $current }.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] Використовується
        [downloaded] Завантажено, не використовується
        [missing] Не завантажено
        [previous] Використовується, доки новий розмір не проіндексує бібліотеку
        [target] Завантаження…
        [failed] Не вдалося завантажити
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Переглянути ліцензію
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: перейти на розмір { $size }? Бібліотеку буде проіндексовано знову; доти використовується поточний розмір.
       *[other] { $feature }: перейти на розмір { $size }? Буде завантажено { $download }, а бібліотеку проіндексовано знову; доти використовується поточний розмір.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: усе одно використати розмір { $size }? Він може не працювати на цьому комп’ютері або працювати дуже повільно.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: розмір { $size } тепер краще підходить цьому комп’ютеру. Перейти? { $download ->
        [none] Бібліотеку буде проіндексовано знову.
       *[other] Буде завантажено { $download }, а бібліотеку проіндексовано знову.
    }
ai-suggest-toast = Локальний ШІ: цьому комп’ютеру краще підходить інший розмір моделі (Налаштування › Локальний ШІ).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Перехід на розмір { $size } · проіндексовано { $done } з { $total } зображень

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = З’явиться в пізнішій версії
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Багато мов
        [english] Лише англійська
        [cjk-latin] Китайська, японська й латиниця
        [zh-latin] Китайська й латиниця
        [arabic] Арабська
        [hebrew] Іврит
        [korean] Корейська
        [cyrillic] Кирилиця
        [greek] Грецька
        [thai] Тайська
        [devanagari] Деванагарі
        [tamil] Тамільська
        [telugu] Телугу
        [eslav] Російська, українська та білоруська
        [latin] Латиниця (багато європейських мов)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Швидша, для повільніших відеокарт
        [closed-eyes] Знаходить заплющені очі
        [small-faces] Краща для малих облич
        [no-translate] Не перекладає англійською
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Дані для навчання: { $source }
        [research] Дані для навчання: { $source } (ліцензія лише для досліджень)
       *[unstated] { $named ->
            [no] Дані для навчання: не названо, умови не вказано
           *[yes] Дані для навчання: { $source }, умови не вказано
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Завантажується з { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Групувати обличчя за людьми
ai-grouping-help = Вимкнено, доки ви не ввімкнете його для цієї бібліотеки. Вимкнення зупиняє групування; уже згруповані обличчя лишаються, доки ви їх не видалите.
ai-grouping-state = { $on ->
        [yes] Увімкнено в цій бібліотеці
       *[no] Вимкнено
    }
ai-grouping-button = { $on ->
        [yes] Вимкнути
       *[no] Увімкнути
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Спершу завантажте { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Вилучити { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Файли моделі буде видалено. Ваші фото й уже знайдений текст залишаться.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Розпізнавати текст на фото цієї бібліотеки
# machine-drafted, needs native review
ai-text-indexing-help = Вимкнено, доки ви не ввімкнете його для цієї бібліотеки. Тоді text: у пошуку Бібліотеки знаходить фото за словами на них.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Увімкнено в цій бібліотеці
       *[no] Вимкнено
    }

## Встановлення з файлу

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Скопійовано { $count } файл моделі. Натисніть «Завантажити», щоб установити.
        [few] Скопійовано { $count } файли моделей. Натисніть «Завантажити», щоб установити.
        [many] Скопійовано { $count } файлів моделей. Натисніть «Завантажити», щоб установити.
       *[other] Скопійовано { $count } файлу моделі. Натисніть «Завантажити», щоб установити.
    }
ai-sideload-none = Файлів моделей не знайдено.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } — не модель, відома { -app-name }. Файл не скопійовано.
ai-sideload-failed = Не вдалося скопіювати { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Використати неперевірену модель?
ai-unverified-detail = { $file } немає в списку моделей, що постачаються з { -app-name }, тому його не можна перевірити. Його буде використано як { $model }.
ai-unverified-use = Використати неперевірену модель

# machine-drafted, needs native review
ai-failed-internal = Усередині { -app-name } щось пішло не так. Подробиці — у журналі.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Перейти
       *[keep] Залишити поточний розмір
    }
