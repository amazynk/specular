# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Контроль
ndt-probe = Зонд: { $name }
ndt-probe-profile = Профіль уздовж лінії
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Дуплексний дротяний IQI
ndt-probe-wire-iqi = Дротяний IQI з одним дротом
ndt-probe-crack = Ширина тріщини
ndt-probe-density = Точка густини

ndt-hint-profile = Протягніть лінію, щоб побудувати значення сірого вздовж неї. [ і ] змінюють зонд.
ndt-hint-snr = Протягніть область на рівномірній ділянці. Її буде збільшено щонайменше до 20 × 55 пікселів.
ndt-hint-duplex-iqi = Протягніть лінію через усі пари дротів, уздовж IQI.
ndt-hint-wire-iqi = Протягніть коротку лінію впоперек найтоншого дроту, який видно.
ndt-hint-crack = Клацайте вздовж тріщини, потім натисніть Enter. Ширина зчитується в 10 точках.
ndt-hint-density = Клацніть точку, щоб зчитати її значення сірого. Введіть її відому густину, щоб відкалібрувати.

ndt-refused-needs-scale = Спершу задайте масштаб: розміри дротів потребують міліметрів. Скористайтеся «Калібрувати» на відомій довжині.
ndt-refused-nothing-found = Уздовж цієї лінії ніщо не виділяється з шуму.
ndt-refused-too-small = Зображення менше за область SNR 20 × 55 пікселів.
ndt-refused-no-noise = У цій області немає шуму для вимірювання (усі пікселі однакові).

ndt-reading = Читання…
ndt-no-pixels = Не вдалося прочитати пікселі.

ndt-profile-range = Сірий від { $min } до { $max }
ndt-profile-mean = Середній сірий { $mean }
ndt-profile-density = Густина від { $min } до { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = Для SNRn потрібна SRb: виміряйте дуплексний IQI або введіть значення.
ndt-snr-stats = Середнє { $mean }, σ { $sd }, область { $w } × { $h } px

ndt-duplex-result = Перша нерозділена пара { $pair }: SRb { $srb } µm, нерізкість { $unsharpness }
ndt-duplex-all-resolved = Усі знайдені пари розділено.
ndt-duplex-dips = Провали: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Інтерпольовано: iSRb { $isrb } µm, нерізкість { $unsharpness }

ndt-wire-result = Дріт { $wire }, { $diameter }
ndt-wire-measured = Виміряна ширина { $width } (FWHM), діаметр приблизно { $diameter }

ndt-crack-mean = Середня ширина { $width }
ndt-crack-range = Найвужче місце { $min }, найширше { $max }
ndt-crack-cuts = Точок вимірювання: { $count }

ndt-density-value = Сірий { $grey }, густина { $density }
ndt-density-grey = Сірий { $grey }
ndt-density-uncalibrated = Густину ще не відкалібровано: введіть відомі густини у двох точках.

ndt-ask-density = Відома густина тут: { $value } (Enter застосовує, порожній Enter пропускає)
ndt-ask-srb = Базова просторова роздільність SRb: { $value } µm (Enter застосовує, Esc пропускає)
ndt-bad-density = Введіть густину від 0 до 10.
ndt-bad-srb = Введіть SRb у мікрометрах, наприклад 80.
ndt-srb-set = Для цього зображення задано SRb { $srb } µm.
ndt-density-one-more = Точку густини збережено. Введіть густину ще в одній точці.
ndt-density-calibrated = Густину відкалібровано за двома точками.
ndt-sidecar-failed = Не вдалося зберегти налаштування контролю поруч із файлом: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } м над точкою зльоту
map-export-kml = Зберегти фото як KMZ або KML…
map-export-kmz-full = Зберегти фото як KMZ з повними фото…
map-kml-heading = Курс { $degrees }°
map-kml-height = { $metres } м над точкою зльоту
map-kml-nothing = Немає фото з положенням, які можна зберегти.
map-kml-saved = Фото збережено в { $name }: { $count }.
map-kml-failed = Не вдалося зберегти файл мапи: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Звіт про огляд
inspection-report-default-title = Звіт про огляд
inspection-report-folder = Додати інші фото з цієї теки
inspection-report-docx = Також зберегти документ Word (DOCX)
inspection-report-kmz = Також зберегти KMZ з місцями фото
inspection-report-caption = Фото з вимірюваннями
inspection-report-col-value = Значення
inspection-report-col-scale = Масштаб за
inspection-report-docx-failed = Не вдалося записати документ Word.
inspection-report-unreadable = Не вдалося прочитати жодне з фото.
