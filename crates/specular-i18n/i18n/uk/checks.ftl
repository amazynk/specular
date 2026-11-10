# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Колізії
    [changes] Зміни
   *[quantities] Обсяги
}
# $tab: the tab's name.
checks-title = Перевірки моделі · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Для перевірок потрібна модель IFC.
    [reading] Читання відомостей моделі…
    [ids-empty] Немає файлу IDS. Натисніть O, щоб відкрити його.
    [ids-checking] Перевірка…
    [ids-no-failures] Порушень немає.
    [ids-hints] Enter порушення, перейти · Backspace назад · O відкрити · P властивості · Ctrl+S BCF · Ctrl+E CSV · Tab далі
    [unnamed] Специфікація без назви
    [clash-running] Пошук колізій…
    [clash-empty] Натисніть R, щоб шукати колізії.
    [clash-none] Колізій немає.
    [clashes-hints] R запустити · A B набори · M жорстка або зазор · + − відстань · Enter перейти · S статус · Ctrl+S BCF · Ctrl+E CSV · Tab далі
    [changes-reading] Читання іншої версії…
    [changes-empty] Натисніть O, щоб вибрати іншу версію цієї моделі.
    [changes-none] Змін немає.
    [changes-hints] O інша версія · F фільтр · Enter перейти · P властивості · Ctrl+S BCF · Ctrl+E CSV · Tab далі
    [quantities-hints] G групувати · Enter перейти · P властивості · Ctrl+E CSV · Ctrl+S XLSX · Tab далі
   *[not-drawn] Цей елемент не має геометрії для показу.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Клас
    [attribute] Атрибут { $name }
    [classification] Класифікація
    [material] Матеріал
   *[partof] Частина { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Клас: { $actual }
    [predefined] Попередньо визначений тип: { $actual }
    [missing] { $what }: відсутнє
    [empty] { $what }: порожнє
    [value] { $what } = { $actual }: не дозволено
    [datatype] { $what }: тип даних { $actual }
    [system] { $what }: система { $actual }
    [whole] { $what }: частина { $actual }
    [prohibited] { $what }: не повинно бути
   *[unsupported] { $what }: неможливо перевірити
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Поверх: { $value }
    [type] Тип: { $value }
    [material] Матеріал: { $value }
    [container] Контейнер: { $value }
   *[classification] Класифікація: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: виконано { $ok } з { $count }

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ відповідають: { $applicable }
    [fail] ✗ не відповідають: { $failed } з { $applicable }
    [none] ✗ ні до чого не застосовується
    [none-ok] ✓ ні до чого не застосовується
    [prohibited] ✗ заборонених: { $applicable }
   *[prohibited-ok] ✓ заборонених немає
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] усі елементи
    [class] { $name }
   *[storey] поверх { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] жорстка, глибше ніж { $distance }
   *[clearance] ближче ніж { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] { $count } колізія
    [few] { $count } колізії
    [many] { $count } колізій
   *[other] { $count } колізії
}, нові: { $new }, розв’язані: { $resolved } · { $seconds } с
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] жорстка { $depth }
   *[clearance] зазор { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Жорстка
   *[clearance] Зазор
}
checks-status = { $status ->
    [new] нова
    [active] активна
   *[resolved] розв’язана
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Колізія: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Колізій немає
    [one] { $count } колізія
    [few] { $count } колізії
    [many] { $count } колізій
   *[other] { $count } колізії
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: додано { $added } · вилучено { $removed } · змінено { $changed }{ $filter ->
    [added] {" "}· показано додані
    [removed] {" "}· показано вилучені
    [changed] {" "}· показано змінені
   *[all] {""}
}
checks-change = { $kind ->
    [added] Додано
    [removed] Вилучено
   *[geometry] геометрія
}
checks-change-fields = { $count ->
    [one] { $count } поле
    [few] { $count } поля
    [many] { $count } полів
   *[other] { $count } поля
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Зміщено на { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = Елементів: { $count } · { $area } · { $volume } · { $by ->
    [element] за елементами
    [class] за класами
    [storey] за поверхами
   *[type] за типами
}
# Where quantities came from.
checks-source = { $source ->
    [file] з файлу
    [computed] обчислено
   *[mixed] частково обчислено
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Елементи
    [class] За класами
    [storey] За поверхами
   *[type] За типами
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Специфікація
    [guid] GlobalId
    [class] Клас
    [name] Назва
    [reason] Причина
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Вид
    [depth] Глибина або відстань (m)
    [status] Статус
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Зміна
    [field] Поле
    [old] Старе
    [new] Нове
    [type] Тип
    [container] Контейнер
    [material] Матеріал
    [classification] Класифікація
    [storey] Поверх
    [count] Кількість
    [length] Довжина (m)
    [area] Площа (m²)
    [volume] Об’єм (m³)
   *[source] Джерело
}

# $reason: why, in plain words.
checks-error = Неможливо виконати: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Перевірено: усі специфікації виконано
    [one] Перевірено: не виконано { $failed } специфікацію
    [few] Перевірено: не виконано { $failed } специфікації
    [many] Перевірено: не виконано { $failed } специфікацій
   *[other] Перевірено: не виконано { $failed } специфікації
}
# $file: the file written.
checks-saved = Збережено { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = усі моделі
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Модель: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M модель · Enter порушення, перейти · Backspace назад · O відкрити · P властивості · Ctrl+S BCF · Ctrl+E CSV · Tab далі
    [clashes] R запустити · A B набори · G H їхні моделі · M жорстка або зазор · + − відстань · Enter перейти · S статус · Ctrl+S BCF · Ctrl+E CSV · Tab далі
    [changes] M модель · O інша версія · F фільтр · Enter перейти · P властивості · Ctrl+S BCF · Ctrl+E CSV · Tab далі
   *[quantities] M модель · G групувати · Enter перейти · P властивості · Ctrl+E CSV · Ctrl+S XLSX · Tab далі
}
