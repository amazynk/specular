# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Медичні зображення
palette-window-level = Інструмент вікна/рівня (перетягування лівою)
palette-window-default = Вікно: типове з файлу
palette-window-lung = Вікно: легені (W 1500, L −600)
palette-window-mediastinum = Вікно: середостіння (W 350, L 50)
palette-window-bone = Вікно: кістки (W 1800, L 400)
palette-window-brain = Вікно: мозок (W 80, L 40)
palette-window-stroke = Вікно: інсульт (W 40, L 40)
palette-window-soft-tissue = Вікно: м’які тканини (W 400, L 50)
palette-window-liver = Вікно: печінка (W 150, L 30)
palette-window-subdural = Вікно: субдуральне (W 200, L 75)
palette-window-full-range = Вікно: увесь діапазон
palette-invert-grey = Інвертувати відтінки сірого
palette-next-slice = Наступний кадр
palette-prev-slice = Попередній кадр
disabled-not-medical = Лише для медичних зображень у відтінках сірого
disabled-one-frame = Зображення має один кадр
settings-section-medical = Медичні зображення
settings-medical-keep-window = Зберігати вікно/рівень для наступної серії
settings-medical-keep-window-help = Зберігати встановлене вікно під час переходу до іншої серії замість її типового значення. Кадри одного файлу та зрізи однієї серії завжди спільно його використовують.
medical-window = C { $center }  W { $width }
medical-inverted = інвертовано
medical-frame = Кадр { $frame } з { $count }
medical-tool = Інструмент вікна/рівня: перетягніть
medical-tool-on = Інструмент вікна/рівня увімкнено: перетягніть для налаштування
medical-tool-off = Інструмент вікна/рівня вимкнено
medical-preset-not-ct = Набори для КТ потребують значень в одиницях Гаунсфілда
medical-file-window-none = У цьому файлі немає вікна { $index }
medical-file-window-unnamed = Вікно файлу { $index } з { $count }
medical-file-window = Вікно файлу { $index } з { $count }: { $name }
measure-templates-need-medical = Геометричні шаблони доступні в профілі «Медицина»
medical-off = Немає медичних інструментів для цього зображення: { $reason }
medical-readout = Значення і вікно

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Відтворити кадри (кіно)
palette-measure-length = Виміряти довжину
palette-measure-angle = Виміряти кут
palette-measure-ellipse = Виміряти еліптичну ділянку
palette-measure-rectangle = Виміряти прямокутну ділянку
palette-clear-measurements = Очистити вимірювання
palette-measure-templates = Геометричні шаблони…
palette-overlays = Показати або сховати накладання
medical-cine = Відтворення: { $rate } кадр./с
medical-overlays-shown = Накладання показано
medical-overlays-hidden = Накладання сховано
measure-hint-length = Довжина: перетягніть або клацніть обидва кінці
measure-hint-angle = Кут: клацніть три точки, вершину другою
measure-hint-ellipse = Еліпс: перетягніть через ділянку
measure-hint-rectangle = Прямокутник: перетягніть через ділянку
measure-off = Інструмент вимірювання вимкнено
measure-cleared = Вимірювання очищено
measure-length-mm = { $value } мм
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Площа { $value } мм²
measure-area-px = Площа { $value } px²
measure-angle = { $value }°
measure-mean = Середнє { $mean } ± { $sd } { $unit }
measure-range = Мін { $min }  Макс { $max }  ({ $count } px)
measure-caveat-detector = На детекторі: без корекції збільшення
measure-caveat-magnification = Оцінена корекція збільшення
measure-caveat-uncalibrated = Немає відстані між пікселями: у пікселях
measure-caveat-outside-region = Поза каліброваною ділянкою УЗД: у пікселях

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Показувати мітки орієнтації
settings-medical-orientation-labels-help = Літери біля країв зображення, що позначають напрямки відносно пацієнта (R/L праворуч/ліворуч, A/P передній/задній, S/I голова/ноги), беруться з файлу й враховують поворот і віддзеркалення. Клавіша L вмикає чи вимикає їх на час сеансу.
settings-medical-wheel-steps-frames = Коліщатко миші гортає кадри
settings-medical-wheel-steps-frames-help = У файлі чи серії з кількома кадрами коліщатко переходить між кадрами замість масштабування чи зміни зображення. Ctrl+коліщатко й надалі змінює масштаб.
# machine-drafted, needs native review
settings-medical-display-function = Відображення сірого
settings-medical-display-function-help = Як показуються відтінки сірого медичних зображень. DICOM GSDF (стандартна функція відображення відтінків сірого) розподіляє їх кроками однакової помітності між яскравістю чорного й білого дисплея (DICOM PS3.14), як на каліброваному діагностичному моніторі. Колірний профіль дисплея й надалі застосовується.
settings-medical-display-function-srgb = Стандартне (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Яскравість чорного дисплея
settings-medical-display-black-help = Яскравість чорного дисплея з урахуванням зовнішнього освітлення, виміряна або взята з технічних даних дисплея. Використовується DICOM GSDF.
settings-medical-display-white = Яскравість білого дисплея
settings-medical-display-white-help = Яскравість білого дисплея, виміряна або взята з технічних даних дисплея. Використовується DICOM GSDF.
palette-orientation-labels = Показати або сховати мітки орієнтації
palette-presentation-state = Наступний стан представлення
disabled-no-presentation-state = Жоден стан представлення не посилається на це зображення
medical-labels-shown = Мітки орієнтації показано
medical-labels-hidden = Мітки орієнтації сховано
medical-presentation = Стан представлення: { $label } ({ $index } з { $count })
medical-presentation-none = Стан представлення вимкнено
medical-presentation-readout = Представлення: { $label }
medical-preset-presentation = Рівні сірого задає таблиця відповідності стану представлення

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Орієнтація: ліворуч { $left }, праворуч { $right }, угорі { $top }, унизу { $bottom }
