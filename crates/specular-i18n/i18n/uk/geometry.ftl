# machine-drafted, needs native review
# Vet, orthopaedic and cephalometric geometry in the measuring tool. Every point is placed
# by hand; nothing is detected or graded.

measure-tool-geometry = Геометрія
measure-tool-hint-geometry = Виберіть шаблон, потім розставте всі точки, які він запитує.
measure-tool-template = Шаблон: { $name }

geometry-prompt = { $template }, точка { $n } з { $total }: { $point }
geometry-readout = { $name }: { $value }
geometry-value-angle = { $value }°
geometry-value-vertebrae = { $value } хр.
geometry-extrapolated = { $value } (за T9, оцінка)
geometry-degenerate = Точки надто близько або на одній прямій. Перемістіть точку.
geometry-moved = Точку переміщено. { $value }
# machine-drafted, needs native review
geometry-picked = Точку { $point } піднято. Стрілки переміщують її, Shift — більшими кроками. Пробіл кладе її, Esc повертає на місце.
geometry-put-back = Точку повернуто на місце.

# Templates.
geometry-vhs = Вертебральний серцевий індекс (VHS)
geometry-vlas = Вертебральний розмір лівого передсердя (VLAS)
geometry-norberg = Кут Норберга
geometry-tpa = Кут плато великогомілкової кістки (TPA)
geometry-distraction-index = Індекс дистракції
geometry-femoral-head = Коло головки стегнової кістки
geometry-femoral-offset = Стегновий офсет
geometry-leg-length = Довжина кінцівки
geometry-acetabular-angle = Кут кульшової западини (Sharp)
geometry-steiner = Аналіз за Стейнером
geometry-downs = Аналіз за Даунсом

# Points, in the order they are asked for.
geometry-pt-long-axis-carina = довга вісь, вентральний край карини
geometry-pt-long-axis-apex = довга вісь, верхівка серця
geometry-pt-short-axis = коротка вісь, кінець { $n }
geometry-pt-t4-cranial = краніальний край T4
geometry-pt-vertebra-caudal = каудальний край T{ $n }
geometry-pt-vlas-carina = вентральний край карини
geometry-pt-vlas-atrium = ліве передсердя біля каудальної порожнистої вени
geometry-pt-left-head-rim = ліва головка стегнової кістки, точка краю { $n } з 3
geometry-pt-right-head-rim = права головка стегнової кістки, точка краю { $n } з 3
geometry-pt-left-acetabular-rim = лівий краніолатеральний край кульшової западини
geometry-pt-right-acetabular-rim = правий краніолатеральний край кульшової западини
geometry-pt-eminence = центр міжвиросткових підвищень
geometry-pt-talus = центр таранної кістки
geometry-pt-plateau-cranial = краніальний край медіального плато великогомілкової кістки
geometry-pt-plateau-caudal = каудальний край медіального плато великогомілкової кістки
geometry-pt-head-rim = головка стегнової кістки, точка краю { $n } з 3
geometry-pt-acetabulum-rim = кульшова западина, точка краю { $n } з 3
geometry-pt-shaft-proximal = вісь діафіза стегнової кістки, верхня точка
geometry-pt-shaft-distal = вісь діафіза стегнової кістки, нижня точка
geometry-pt-reference = опорна лінія таза, точка { $n }
geometry-pt-left-landmark = орієнтир лівої ноги
geometry-pt-right-landmark = орієнтир правої ноги
geometry-pt-teardrop = фігура сльози, цей бік
geometry-pt-teardrop-other = фігура сльози, інший бік
geometry-pt-acetabular-edge = латеральний край кульшової западини
geometry-pt-s = села (S)
geometry-pt-n = назіон (N)
geometry-pt-a = точка A
geometry-pt-b = точка B
geometry-pt-pog = погоніон (Pog)
geometry-pt-go = гоніон (Go)
geometry-pt-gn = гнатіон (Gn)
geometry-pt-me = ментон (Me)
geometry-pt-po = поріон (Po)
geometry-pt-or = орбітале (Or)
geometry-pt-u1-tip = ріжучий край верхнього різця
geometry-pt-u1-apex = верхівка кореня верхнього різця
geometry-pt-l1-tip = ріжучий край нижнього різця
geometry-pt-l1-apex = верхівка кореня нижнього різця
geometry-pt-occlusal-front = оклюзійна площина, передня точка
geometry-pt-occlusal-back = оклюзійна площина, задня точка

# Results.
geometry-out-vhs = VHS
geometry-out-long-axis = Довга вісь
geometry-out-short-axis = Коротка вісь
geometry-out-long-axis-length = Довжина довгої осі
geometry-out-short-axis-length = Довжина короткої осі
geometry-out-vlas = VLAS
geometry-out-vlas-length = Довжина VLAS
geometry-out-norberg-left = Норберг ліворуч
geometry-out-norberg-right = Норберг праворуч
geometry-out-tpa = TPA
geometry-out-distraction-index = Індекс дистракції
geometry-out-centre-distance = Відстань між центрами
geometry-out-head-radius = Радіус головки
geometry-out-head-diameter = Діаметр головки
geometry-out-offset = Офсет
geometry-out-leg-difference = Різниця (ліва − права)
geometry-out-leg-left = Ліва
geometry-out-leg-right = Права
geometry-out-acetabular-angle = Кут кульшової западини
geometry-out-sna = SNA
geometry-out-snb = SNB
geometry-out-anb = ANB
geometry-out-sn-gogn = SN–GoGn
geometry-out-u1-na-angle = Кут U1–NA
geometry-out-u1-na-mm = Відстань U1–NA
geometry-out-l1-nb-angle = Кут L1–NB
geometry-out-l1-nb-mm = Відстань L1–NB
geometry-out-pog-nb-mm = Відстань Pog–NB
geometry-out-interincisal = Міжрізцевий кут
geometry-out-occlusal-sn = Оклюзійна площина–SN
geometry-out-facial-angle = Лицевий кут
geometry-out-convexity = Кут опуклості
geometry-out-ab-plane = Кут площини A–B
geometry-out-mandibular-plane = Кут нижньощелепної площини
geometry-out-y-axis = Вісь Y
geometry-out-occlusal-cant = Нахил оклюзійної площини
geometry-out-l1-occlusal = L1 до оклюзійної площини
geometry-out-l1-mandibular = L1 до нижньощелепної площини
geometry-out-u1-apog-mm = U1 до A–Pog
