# machine-drafted, needs native review
# Specular — Імпорт із фотоапаратів, телефонів і карток пам’яті. Українська.
# Картку лише читають; файли видаляються з неї лише на запит, після перевіреної копії.

palette-import = Імпортувати фото…
disabled-no-import-source = Не під’єднано жодної картки пам’яті, фотоапарата чи телефона

## Налаштування → Імпорт
settings-section-import = Імпорт
settings-import-on-card = Коли вставлено картку
settings-import-on-card-help = Що відбувається, коли під’єднано картку пам’яті, фотоапарат або телефон із фото.
settings-import-on-card-ask = Питати
settings-import-on-card-open = Відкрити Імпорт
settings-import-on-card-nothing = Нічого не робити
settings-import-destination = Імпортувати до
settings-import-destination-help = Тека для імпортованих фото й відео. Якщо не задано, вони потрапляють до теки Зображення.
settings-import-destination-unset = Тека Зображення
settings-import-folder-pattern = Теки
settings-import-folder-pattern-help = Теки, що створюються в ній: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} і {"{"}event{"}"}, розділені /.
settings-import-rename-pattern = Перейменувати
settings-import-rename-pattern-help = Нові назви файлів, як у Конвертувати: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Порожнє зберігає назви з фотоапарата.
settings-import-keep-pairs = Тримати RAW і JPEG разом
settings-import-keep-pairs-help = Файл RAW і знятий із ним JPEG потрапляють до однієї теки під однією назвою, навіть після перейменування.
settings-import-low-res-videos = Імпортувати копії відео низької роздільності
settings-import-low-res-videos-help = Деякі фотоапарати записують малу копію попереднього перегляду кожного відео (.LRV). Вимкнено: вони лишаються на картці.
settings-import-eject = Вийняти після завершення
settings-import-eject-help = Виймає картку, щойно все скопійовано, щоб її можна було витягти.

## Пропозиція, коли вставлено картку
import-offer = Імпортувати з { $name }?
import-offer-button = Імпортувати

## Вигляд імпорту
import-title = Імпорт
import-from = з { $name }
import-source-next = Наступне джерело
import-close = Закрити
import-scanning = Пошук нових фото… { $done } з { $total }
import-scanning-start = Пошук нових фото…
import-none-new = Усе з { $name } уже імпортовано раніше.
import-empty = На { $name } немає фото чи відео.
import-no-source = Не під’єднано жодної картки пам’яті, фотоапарата чи телефона.
import-summary = { $count ->
    [one] { $count } новий елемент
    [few] { $count } нові елементи
    [many] { $count } нових елементів
   *[other] { $count } нового елемента
} · { $size }
import-already = Імпортовано раніше: { $count }
import-unreadable = { $count ->
    [one] Не вдалося прочитати { $count } файл
    [few] Не вдалося прочитати { $count } файли
    [many] Не вдалося прочитати { $count } файлів
   *[other] Не вдалося прочитати { $count } файлу
}
import-selected = Вибрано { $count } з { $total }
import-select-all = Вибрати все
import-select-none = Зняти вибір
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Відео

import-destination = Імпортувати до
import-choose = Вибрати…
import-folder-pattern = Теки
import-event = Подія
import-event-hint = Необов’язково, для {"{"}event{"}"}
import-rename = Перейменувати
import-rename-hint = Порожнє зберігає назви з фотоапарата
import-preview = Перший файл: { $path }
import-pattern-error = { $error }
import-keep-pairs = Тримати RAW і JPEG разом
import-eject = Вийняти після завершення
import-delete = Видалити з картки після імпорту
import-free = Скопіювати { $size } · вільне місце { $free }
import-free-unknown = Скопіювати { $size }
import-no-space = Недостатньо вільного місця в { $folder }: цей імпорт потребує { $need }, а вільно лише { $free }. Звільніть місце або виберіть іншу теку.
import-start = { $count ->
    [one] Імпортувати { $count } елемент
    [few] Імпортувати { $count } елементи
    [many] Імпортувати { $count } елементів
   *[other] Імпортувати { $count } елемента
}
import-nothing-selected = Виберіть фото для імпорту

import-confirm-delete = { $count ->
    [one] Після копіювання й перевірки { $count } файл буде видалено з { $name }. Це не можна скасувати.
    [few] Після копіювання й перевірки { $count } файли буде видалено з { $name }. Це не можна скасувати.
    [many] Після копіювання й перевірки { $count } файлів буде видалено з { $name }. Це не можна скасувати.
   *[other] Після копіювання й перевірки { $count } файлу буде видалено з { $name }. Це не можна скасувати.
}
import-confirm-yes = Імпортувати й видалити
import-confirm-no = Назад

import-progress = Імпорт { $done } з { $total } · { $size }
import-progress-eta = Імпорт { $done } з { $total } · { $size } · { $time }
import-paused = Призупинено · { $done } з { $total }
import-pause = Призупинити
import-resume = Продовжити
import-cancel = Скасувати
import-hide = Сховати
import-stopping = Зупинка…
import-running = Імпорт уже триває.

import-done = { $count ->
    [one] Імпортовано { $count } файл
    [few] Імпортовано { $count } файли
    [many] Імпортовано { $count } файлів
   *[other] Імпортовано { $count } файлу
} до { $folder }
import-done-existed = Уже були: { $count }
import-done-deleted = Видалено з картки: { $count }
import-done-failed = { $count ->
    [one] Не вдалося імпортувати { $count } файл
    [few] Не вдалося імпортувати { $count } файли
    [many] Не вдалося імпортувати { $count } файлів
   *[other] Не вдалося імпортувати { $count } файлу
}
import-done-cancelled = Імпорт скасовано після { $count ->
        [one] { $count } файлу
        [few] { $count } файлів
        [many] { $count } файлів
       *[other] { $count } файлу
    }. Повторний імпорт продовжить з місця зупинки.
import-done-nothing = Нічого нового не імпортовано.
import-ejected = { $name } можна витягти.
import-eject-failed = Не вдалося вийняти { $name }: { $error }
import-failed-destination = Теку { $folder } не можна використати: { $error }
import-ledger-failed = Не вдалося прочитати список імпортованих файлів ({ $error }); раніше імпортовані файли можуть бути запропоновані знову.

## Читач екрана
import-a11y-group = Імпорт
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, вибрано
