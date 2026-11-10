# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Об’єкт зйомки
develop-mask-kind-sky = Небо
develop-mask-kind-object = Об’єкт за клацанням
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Пошук об’єкта зйомки…
        [sky] Пошук неба…
       *[object] Пошук об’єкта…
    }
develop-ai-ready = Готово
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Точок: { $n }
develop-ai-click-help =
    Клацніть об’єкт.
    Alt+клацніть те, що до нього не належить.
    Перетягніть для рамки.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Потрібен додаток { $model }.
    Доти нічого не маскує.
develop-ai-failed = Не вдалося створити маску: { $error }
develop-ai-failed-worker = Процес масок ШІ зупинився.
develop-ai-open-settings = Відкрити локальний ШІ…
develop-ai-clear = Очистити точки
develop-ai-too-many = Щонайбільше { $n } точок (рамка рахується як дві).
develop-ai-box-set = Рамку задано
develop-ai-point-added = Точку додано
develop-ai-point-removed = Точку додано поза об’єктом
develop-ai-cleared = Точки очищено
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Точка { $n }, на об’єкті
develop-ai-point-off = Точка { $n }, поза об’єктом

## Settings › Local AI: the Masks feature
settings-ai-masks = Маски ШІ
settings-ai-masks-help = Маски об’єкта зйомки, неба та об’єктів у Проявленні. Модель об’єктів входить до функції; об’єкт зйомки й небо — додатки.
settings-ai-masks-size = Модель
settings-ai-masks-size-help = EfficientSAM Ti швидший. EfficientSAM S точніше йде за краями.
settings-ai-masks-size-recommended = Типово
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Великий
