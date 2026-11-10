# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Показати теги DICOM
palette-next-series = Наступна серія
palette-prev-series = Попередня серія
disabled-not-dicom = Зображення не є файлом DICOM
disabled-no-other-series = У цій теці немає іншої серії
dicom-tags-title = Теги DICOM
dicom-tags-title-file = Теги DICOM: { $file }
dicom-tags-filter = Фільтрувати теги
dicom-tags-show-patient = Показати дані пацієнта
dicom-tags-col-tag = Тег
dicom-tags-col-keyword = Ключове слово
dicom-tags-col-vr = VR
dicom-tags-col-length = Довжина
dicom-tags-col-value = Значення
dicom-tags-hidden = Приховано
dicom-tags-hidden-count = Приховані значення пацієнта: { $count }
dicom-tags-no-match = Жоден тег не збігається
dicom-tags-reading = Читання тегів…
dicom-tags-error = Не вдалося прочитати теги: { $reason }
dicom-tags-items = Елементи: { $count }
dicom-tags-fragments = Фрагменти: { $count }
dicom-tags-item = Елемент { $index }
dicom-tags-bytes = Байти: { $count }
dicom-tags-empty = (порожньо)
dicom-tags-omitted = Інші непоказані елементи: { $count }
dicom-series-group = Серія
dicom-series-slice = Зріз
dicom-series-slice-value = { $index } з { $count }
dicom-series-frames = Кадри: { $frames }
dicom-series-location = Положення зрізу
dicom-series-mm = { $mm } мм
dicom-series-spacing = Відстань між зрізами
dicom-series-which = Серія
dicom-series-which-value = { $index } з { $count }
dicom-series-order = Порядок
dicom-series-order-position = За положенням
dicom-series-order-instance = За номером екземпляра
dicom-series-order-time = За часом отримання
dicom-series-order-name = За назвою файлу
dicom-series-gaps = Пропущені зрізи
dicom-series-gaps-value = Пропущені зрізи: { $count }
dicom-series-gaps-where = Після зрізу { $slices }
dicom-series-irregular = Нерівномірна відстань
dicom-series-tilt = Нахил гентрі
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Дублікати пропущено
dicom-series-toast = Серія { $index } з { $series } – зрізи: { $count }
dicom-series-capped = Прочитано лише перші файли теки: { $count }

studies-title = Дослідження
studies-title-medium = Дослідження: { $medium }
studies-show-patient = Показати дані пацієнта
studies-patient-hidden = Приховано
studies-patient-hidden-note = Імена та дати народження пацієнтів приховано
studies-none = Досліджень не знайдено
studies-reading = Читання диска…
studies-reading-count = Читання диска: { $done } з { $total ->
        [one] { $total } файлу
        [few] { $total } файлів
        [many] { $total } файлів
       *[other] { $total } файлу
    }
studies-opening = Відкриття серії…
studies-error = Не вдалося прочитати диск: { $reason }
studies-missing = Відсутні файли, зазначені на диску: { $count }
studies-capped = Прочитано лише перші файли: { $count }
# machine-drafted, needs native review
studies-stopped = Читання зупинено після { $done } з { $total ->
        [one] { $total } файлу
        [few] { $total } файлів
        [many] { $total } файлів
       *[other] { $total } файлу
    }: список неповний
studies-stopped-early = Читання зупинено: список неповний
studies-cancel-description = Зупиняє читання диска. Те, що вже прочитано, залишається в списку.
studies-patient-n = Пацієнт { $index }
studies-patient-unknown = Невідомий пацієнт
studies-born = Дата народження: { $date }
studies-no-description = Без опису
studies-series = Серія { $number }: { $description }
studies-series-number = Серія { $number }
studies-images = Зображення: { $count }
studies-images-missing = Відсутні: { $count }
studies-others = Інші об’єкти: { $count }
studies-no-images = Ця серія не має зображень на диску
studies-medium-inserted = Вставлено диск із медичними дослідженнями
studies-medium-show = Показати дослідження
palette-studies = Показати дослідження на цьому диску
