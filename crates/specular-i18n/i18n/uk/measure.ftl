# Specular — Інструмент вимірювання (довжини, кути, ділянки та їхня статистика на будь-якому зображенні). Машинний переклад, потребує перевірки носієм мови.

palette-measure = Виміряти
palette-measure-copy-csv = Копіювати вимірювання як CSV
palette-measure-export = Експортувати вимірювання…
toast-copied-measurements = Вимірювання скопійовано як CSV

measure-tool-title = Вимірювання
measure-tool-list = Вимірювання
measure-tool-count = { $count ->
    [0] Немає вимірювань
    [one] { $count } вимірювання
    [few] { $count } вимірювання
    [many] { $count } вимірювань
   *[other] { $count } вимірювання
}
measure-tool-empty = Виберіть інструмент і малюйте на зображенні.

measure-tool-point = Точка
measure-tool-line = Лінія
measure-tool-polyline = Ламана
measure-tool-angle = Кут
measure-tool-rect = Прямокутник
measure-tool-ellipse = Еліпс
measure-tool-polygon = Многокутник
measure-tool-freehand = Довільна форма
measure-tool-calibrate = Калібрувати
# machine-drafted, needs native review
measure-tool-spot = Точка

measure-tool-undo = Скасувати
measure-tool-delete = Видалити
measure-tool-copy = Копіювати CSV
measure-tool-export = Експортувати…
measure-tool-import = Імпортувати…
measure-tool-done = Готово
measure-tool-unit = Одиниця: { $unit }
measure-tool-apply = Застосувати

measure-tool-hint-line = Перетягніть або клацніть дві точки. Стрілки рухають перехрестя, Пробіл ставить точку.
measure-tool-hint-angle = Клацніть три точки; друга — вершина.
measure-tool-hint-rect = Перетягніть або клацніть два кути.
measure-tool-hint-ellipse = Перетягніть або клацніть два кути її рамки.
measure-tool-hint-polygon = Клацайте кути; Enter або клацання першого кута замикає фігуру.
measure-tool-hint-calibrate = Проведіть лінію вздовж чогось відомої довжини.
# machine-drafted, needs native review
measure-tool-hint-spot = Клацніть точку, щоб прочитати її температуру.
measure-tool-known-length = Відома довжина: { $value } { $unit } (введіть число, U змінює одиницю, Enter застосовує)
measure-tool-calibrated = Відкалібровано: один піксель — { $size } { $unit }
measure-tool-bad-length = Введіть довжину, більшу за нуль.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Площа { $value }
measure-tool-mean = Середнє { $mean } ± { $sd }
measure-tool-means = Середнє { $values }
measure-tool-range = Діапазон { $min } – { $max } по { $count } px
measure-tool-pixels = По { $count } px
measure-tool-stats-pending = Читання пікселів…
measure-tool-stats-none = Тут немає значень пікселів
measure-tool-approximate = Зі зменшеної копії (приблизно)

measure-tool-deleted = Вимірювання видалено
measure-tool-undone = Скасовано
measure-tool-nothing = На цьому зображенні немає вимірювань
measure-tool-exported = Вимірювання збережено в { $name }
measure-tool-export-failed = Не вдалося зберегти вимірювання: { $reason }
measure-tool-imported = { $count ->
    [one] Імпортовано { $count } вимірювання.
    [few] Імпортовано { $count } вимірювання.
    [many] Імпортовано { $count } вимірювань.
   *[other] Імпортовано { $count } вимірювання.
}
measure-tool-skipped = { $count ->
    [one] Не вдалося прочитати { $count } фігуру.
    [few] Не вдалося прочитати { $count } фігури.
    [many] Не вдалося прочитати { $count } фігур.
   *[other] Не вдалося прочитати { $count } фігури.
}
measure-tool-import-failed = Не вдалося прочитати вимірювання: { $reason }
measure-tool-too-large = Файл завеликий.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR потребує зображення DICOM. Збережіть натомість як CSV або GeoJSON.

measure-source-uncalibrated = Пікселі (не відкалібровано)
measure-source-dicom-pixel-spacing = Відстань між пікселями DICOM
measure-source-dicom-calibrated = Відкалібрована відстань DICOM
measure-source-dicom-imager-pixel-spacing = Відстань DICOM на детекторі
measure-source-dicom-magnification = Відстань DICOM з корекцією збільшення
measure-source-dicom-ultrasound-region = Ультразвукова ділянка DICOM
measure-source-tiff-resolution = Роздільність TIFF
measure-source-exif-resolution = Роздільність EXIF
measure-source-ome-physical-size = Фізичний розмір OME
measure-source-fits-wcs = Світові координати FITS
measure-source-vendor-tag = Метадані виробника
measure-source-drawing-units = Одиниці креслення
# machine-drafted, needs native review
measure-source-drawing-scale = За масштабом креслення
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = З креслення (масштаб { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = З креслення (масштаб { $scale }, видовий екран { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Масштаб мапи в центрі (приблизно; Мапи вимірюють на Землі)
measure-source-user = Ваше калібрування
