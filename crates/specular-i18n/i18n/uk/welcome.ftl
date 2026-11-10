# machine-drafted, needs native review
# Specular — Початковий екран (коли Specular відкривається без файлу і коли закрито останнє
# зображення), його картки «Варто спробувати» та одноразові підказки. Українська.

## Початковий екран

welcome-title = { -app-name }
welcome-tagline = Швидкий і точний переглядач фотографій, а також технічних, наукових і медичних зображень.
welcome-open = Відкрити файл…
welcome-open-about = Зображення, документ або файл даних.
welcome-browse = Переглянути теку…
welcome-browse-about = Виберіть теку й перегляньте її як галерею.
welcome-pictures = Ваші зображення
welcome-pictures-about = Усі фото з бібліотеки за датою та місцем.
welcome-pictures-setup-title = Створити бібліотеку…
welcome-pictures-setup = Починаючи з теки { $folder }.
welcome-pictures-ask = Додати теку { $folder } до бібліотеки?
welcome-pictures-add = Додати теку
welcome-drop = Також можна перетягнути файли чи теки будь-куди в це вікно.
welcome-recent = Недавні теки
welcome-recent-name = { $name }, у { $parent }
welcome-tips = Варто спробувати
welcome-tips-hide = Сховати ці підказки
welcome-tips-hidden = Підказки сховано. «Показати вітання» в палітрі команд повертає їх.

## Варто спробувати: назва й один рядок. Клавіша біля назви — власне призначення користувача.

welcome-tip-gallery = Галерея
welcome-tip-gallery-about = Уся тека одразу.
welcome-tip-compare = Порівняння
welcome-tip-compare-about = Виберіть у галереї від двох до чотирьох зображень, щоб побачити їх поруч.
welcome-tip-cull = Режим відбору
welcome-tip-cull-about = Обирайте, відхиляйте й оцінюйте знімки однією клавішею.
welcome-tip-slideshow = Слайд-шоу
welcome-tip-slideshow-about = Тека на весь екран, зображення за зображенням.
welcome-tip-palette = Палітра команд
welcome-tip-palette-about = Знайдіть будь-яку команду чи налаштування за назвою.
welcome-tip-studies = Дослідження
welcome-tip-studies-about = Пацієнти, дослідження та серії з диска чи теки DICOM.
welcome-tip-dicom-servers = Сервери DICOMweb
welcome-tip-dicom-servers-about = Переглядайте й відкривайте дослідження з PACS.
welcome-tip-dicom-tags = Теги DICOM
welcome-tip-dicom-tags-about = Усі теги зображення, з пошуком.
welcome-tip-window-level = Вікно й рівень
welcome-tip-window-level-about = Перетягніть, щоб налаштувати контраст зображення DICOM.
welcome-tip-omero-servers = Сервери OMERO
welcome-tip-omero-servers-about = Переглядайте мікроскопічні проєкти в OMERO та IDR.
welcome-tip-stretch = Розтягування
welcome-tip-stretch-about = Виявляє слабкі деталі в астрономічних і наукових даних.
welcome-tip-drawing-layers = Шари креслення
welcome-tip-drawing-layers-about = Показуйте й ховайте шари технічного креслення.
welcome-tip-needs-image = Спершу відкрийте зображення, потім натисніть { $key }.
welcome-tip-needs-gallery = У галереї виберіть від двох до чотирьох зображень, потім натисніть { $key }.
welcome-tip-key = Скорочення: { $key }

## Одноразові підказки: кожна один раз, ненав’язливо, не більше однієї за запуск.
## Налаштування › Загальні › Показувати підказки вимикає їх усі.

hint-gallery = Натисніть { $key }, щоб побачити цю теку як галерею.
hint-gallery-first = Виберіть від двох до чотирьох зображень і натисніть { $compare }, щоб порівняти їх, або { $cull }, щоб відібрати теку.
hint-raw = Тека з RAW-фото: натисніть { $key }, щоб відібрати (обрати, відхилити, оцінити).
hint-pages =
    { $count ->
        [one] Цей файл має { $count } сторінку. { $next } і { $prev } гортають.
        [few] Цей файл має { $count } сторінки. { $next } і { $prev } гортають.
        [many] Цей файл має { $count } сторінок. { $next } і { $prev } гортають.
       *[other] Цей файл має { $count } сторінки. { $next } і { $prev } гортають.
    }
hint-turn-off = Вимкнути підказки
hint-turned-off = Підказки вимкнено. Налаштування › Загальні › Показувати підказки вмикає їх знову.

## Палітра команд

palette-show-welcome = Показати вітання
palette-show-welcome-help = Початковий екран: Відкрити, Переглянути, ваші зображення й що спробувати.
palette-close = Закрити
palette-close-help = Закрити зображення чи теку й повернутися на початковий екран. На початковому екрані — вийти.

## Перший запуск (під питанням про профіль)

profile-on-start = Коли { -app-name } відкривається без файлу:
profile-skip = Пропустити
