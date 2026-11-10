# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Українська. machine-drafted, needs native review

palette-tether = Зйомка через кабель…
palette-tether-capture = Зробити знімок (через кабель)

tether-title = Зйомка через кабель
tether-close = Закрити зйомку через кабель
tether-default-folder = Через кабель

## Cameras
tether-none = Камеру не знайдено.
tether-none-hint = Під’єднайте камеру кабелем USB і ввімкніть її. Якщо вона має режим USB, виберіть PTP або з’єднання з ПК.
tether-look-again = Шукати знову
tether-connect = Під’єднати
tether-next-camera = Наступна камера
tether-connecting = Під’єднання…
tether-connected = { $camera } під’єднано
tether-disconnect = Від’єднати
tether-try-again = Спробувати знову
tether-gone = Камеру від’єднано.
tether-not-connected = Камеру не під’єднано. Спершу відкрийте зйомку через кабель.

## Why a camera can't be opened
tether-permission = Цей користувач не може відкрити USB-пристрій камери { $node }.
tether-permission-rule = Це дозволяє правило udev. Як адміністратор додайте цей рядок до { $file }:
tether-permission-reload = Потім виконайте «sudo udevadm control --reload» і під’єднайте камеру знову.
tether-copy-rule = Копіювати правило
tether-busy-other = Камерою користується інша програма.
tether-busy-named = Камерою користується { $programs }.
tether-busy-hint = Закрийте ту програму або відмонтуйте камеру у файловому менеджері, а тоді спробуйте знову.
tether-driver = Системний драйвер цієї камери не дає іншим програмам нею користуватися.

## The camera
tether-battery = Акумулятор { PERCENT($percent) }
tether-card = { $card }: вільно { $free }
tether-card-shots = { $card }: вільно { $free }, знімків: { $shots }
tether-no-card = Немає картки пам’яті.
tether-no-settings = Камера не повідомляє налаштувань експозиції.
tether-no-remote = Цю камеру не можна спустити звідси. Знімки, зроблені її кнопкою спуску, завантажуються.
tether-capture = Зняти
tether-step-down = { $setting }: менше ({ $value })
tether-step-up = { $setting }: більше ({ $value })

## Exposure settings
tether-prop-mode = Режим
tether-prop-shutter = Витримка
tether-prop-aperture = Діафрагма
tether-prop-iso = ISO
tether-prop-bias = Корекція експозиції
tether-prop-white-balance = Баланс білого
tether-auto = Автоматично
tether-bulb = Ручна витримка (B)
tether-camera-value = Налаштування камери { $value }
tether-wb-manual = Вручну
tether-wb-one-push = Одноразовий замір
tether-wb-daylight = Денне світло
tether-wb-fluorescent = Люмінесцентна лампа
tether-wb-tungsten = Лампа розжарювання
tether-wb-flash = Спалах
tether-mode-manual = Ручний
tether-mode-program = Програмний
tether-mode-aperture = Пріоритет діафрагми
tether-mode-shutter = Пріоритет витримки
tether-mode-creative = Творчий
tether-mode-action = Спорт
tether-mode-portrait = Портрет

## Where shots go
tether-folder = Тека
tether-names = Назви
tether-names-camera = Назви з камери
tether-arrival = Стиль Develop
tether-choose-folder = Тека…
tether-choose-style = Стиль…
tether-downloading = Завантаження… { PERCENT($percent) }
tether-saved = Збережено { $name }
tether-saved-count = { $count ->
    [one] Збережено { $count } знімок
    [few] Збережено { $count } знімки
    [many] Збережено { $count } знімків
   *[other] Збережено { $count } знімка
}

## Failures
tether-busy = Камера зайнята. Спробуйте знову.
tether-card-full = Картка пам’яті заповнена.
tether-set-refused = Камера не прийняла це налаштування. Перевірте диск режимів.
tether-disk = Не вдалося зберегти знімок: { $error }
tether-error = Помилка камери: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Тека зйомки через кабель
settings-cull-tether-folder-help = Куди зберігаються знімки з камери, під’єднаної кабелем.
settings-cull-tether-folder-unset = Зображення › Через кабель
settings-cull-tether-rename = Назви файлів зйомки через кабель
settings-cull-tether-rename-help = Нові назви з тими самими позначками, що й Перейменування в Імпорті. Порожнє зберігає назви з камери.
