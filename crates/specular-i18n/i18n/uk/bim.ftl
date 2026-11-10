# machine-drafted, needs native review
## Перегляд 3D-моделей: ходьба й політ, площини перерізу й рамка перерізу, збережені вигляди,
## теми BCF (режим 3D). Названі клавіші — типові.

cheatsheet-review = Перегляд

palette-mesh-walk = Ходьба
palette-mesh-fly = Політ
palette-mesh-views = Збережені вигляди
palette-mesh-save-view = Зберегти вигляд
palette-mesh-section-plane = Додати площину перерізу
palette-mesh-section-box = Рамка перерізу
palette-mesh-section-flip = Перевернути площину перерізу
palette-mesh-section-clear = Прибрати всі перерізи
palette-mesh-measure-cut = Виміряти на перерізі
palette-mesh-bcf-topics = Теми BCF
# machine-drafted, needs native review
palette-mesh-model-checks = Перевірки моделі
# machine-drafted, needs native review
palette-mesh-models = Моделі у сцені
# machine-drafted, needs native review
palette-mesh-add-model = Додати модель…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Додати моделі з теки…

# Ходьба й політ: сповіщення та рядки показників, за $what. $eye: висота ока над підлогою,
# з одиницею (лише walk-line).
mesh-nav = { $what ->
    [walk-toast] Ходьба: W A S D — рух, тягніть, щоб роззирнутися, Esc — зупинити
    [fly-toast] Політ: W A S D — рух, Q E — вниз і вгору, Esc — зупинити
    [orbit] Орбіта
    [walk-line] Ходьба · око { $eye } над підлогою
    [walk-keys] W A S D рух · тягнення — огляд · Shift швидше · Esc зупиняє
    [preparing] Стіни ще не готові: ви проходите крізь них
    [fly-keys] W A S D рух · Q E вниз, вгору · тягнення — огляд · Shift швидше · Esc зупиняє
   *[fly-line] Політ
}

# Короткі тексти перерізів, вимірювання та збережених виглядів, за $what.
bim-note = { $what ->
    [plane-added] Площину перерізу додано: тягніть її маркер, щоб посунути
    [box-on] Рамку перерізу ввімкнено: тягніть маркер грані, щоб посунути її
    [box-off] Рамку перерізу вимкнено
    [cleared] Перерізи прибрано
    [measure-on] Клацніть дві точки на перерізі
    [measure-needs-cut] Спершу додайте площину перерізу або рамку
    [measure-off-cut] Клацніть на перерізі
    [measure-hint] Вимірювання: клацніть дві точки на перерізі
    [ask-view-name] Назва вигляду
    [ask-rename] Нова назва
    [views-empty] Немає збережених виглядів. Натисніть N, щоб зберегти цей.
    [views-hints] Enter перейти · N зберегти · F2 перейменувати · Delete прибрати · P зберегти як PNG
    [delete-detail] Прибирається лише збережений вигляд. Модель не змінюється.
    [delete-ok] Прибрати
    [not-over-model] Виберіть інший файл: цей замінив би модель.
   *[view-name] Вигляд
}

# Короткі тексти BCF (BIM Collaboration Format), за $what.
bcf-note = { $what ->
    [title-none] Теми BCF
    [new-file] новий
    [empty] Немає тем. Натисніть O, щоб відкрити файл BCF, або N, щоб додати тему.
    [hints] Enter перейти · ← → точки огляду · N тема · C коментар · S статус · E назва · O відкрити · Ctrl+S експорт
    [ask-title] Назва теми
    [ask-comment] Коментар
    [untitled] Без назви
    [topic-added] Тему додано з цим виглядом
   *[comment-added] Коментар додано з цим виглядом
}

# $planes: скільки площин перерізу.
bim-sections-line = { $planes ->
    [0] Немає площин перерізу
    [one] { $planes } площина перерізу
    [few] { $planes } площини перерізу
    [many] { $planes } площин перерізу
   *[other] { $planes } площини перерізу
}
# $size: розмір рамки перерізу з одиницею.
bim-box-line = рамка { $size }
# $count: найбільша можлива кількість площин.
bim-planes-full = Щонайбільше { $count } площин перерізу
# Маркер для перетягування, для читачів екрана. $n: його номер.
bim-handle = { $kind ->
    [box] Грань рамки перерізу { $n }
   *[plane] Площина перерізу { $n }
}
# $length: довжина з одиницею.
bim-measure-line = Довжина на перерізі: { $length }
bim-views-title = { $count ->
    [one] { $count } збережений вигляд
    [few] { $count } збережені вигляди
    [many] { $count } збережених виглядів
   *[other] { $count } збереженого вигляду
}
# $name: назва вигляду.
bim-view-saved = Збережено «{ $name }»
bim-view-delete-title = Прибрати вигляд «{ $name }»?
# $file: записаний файл.
bim-saved-file = Збережено: { $file }
# $reason: чому не вдалося.
bim-failed = { $what ->
    [open] Не вдалося відкрити файл BCF: { $reason }
    [views] Вигляди збережено лише для цього сеансу: { $reason }
   *[save] Не вдалося зберегти: { $reason }
}
# $count: теми; $file: назва файлу архіву.
bcf-title = { $count ->
    [one] { $count } тема
    [few] { $count } теми
    [many] { $count } тем
   *[other] { $count } теми
} · { $file }
bcf-comments = { $count ->
    [one] { $count } коментар
    [few] { $count } коментарі
    [many] { $count } коментарів
   *[other] { $count } коментаря
}
# $at: показана точка огляду; $count: точки огляду теми.
bcf-viewpoint-of = Точка огляду { $at } з { $count }
bcf-opened = { $count ->
    [one] Відкрито { $count } тему
    [few] Відкрито { $count } теми
    [many] Відкрито { $count } тем
   *[other] Відкрито { $count } теми
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Моделі: 1
    [few] Моделі: { $count }
    [many] Моделі: { $count }
   *[other] Моделі: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Цей файл не є моделлю IFC.
    [not-ifc-open] Спершу відкрийте модель IFC. Поєднувати можна лише моделі IFC.
    [this-model] Ця модель
    [hints] A додати модель · Shift+A додати теку · Space показати чи сховати · C колір за моделлю · Enter перейти · Delete прибрати
    [origin-ok] Розмістити в початку координат
    [first-stays] Відкрита модель лишається. Щоб змінити її, відкрийте інший файл.
    [remove-detail] Вона зникне зі сцени. Файл на диску лишиться.
   *[remove-ok] Прибрати
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] { $elements } елемент
    [few] { $elements } елементи
    [many] { $elements } елементів
   *[other] { $elements } елемента
} · { $shown ->
    [hidden] приховано
   *[shown] показано
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] відкрита модель
    [map-crs] за координатами мапи ({ $crs })
    [map] за координатами мапи
    [site] за широтою й довготою ділянки (приблизно)
    [project] за спільними координатами проєкту
   *[origin] у початку координат (не вирівняно)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Моделі використовують різні системи координат: { $first } і { $this }.
    [only-first] Відкрита модель має геоприв’язку, ця — ні.
    [only-this] Ця модель має геоприв’язку, відкрита — ні.
   *[broken] Перетворення мапи цієї моделі не можна використати (її масштаб нульовий).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Зсунуто на { $distance }, повернуто на { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Читання { $count } моделі…
    [few] Читання { $count } моделей…
    [many] Читання { $count } моделей…
   *[other] Читання { $count } моделі…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } не вдається вирівняти. Розмістити в початку координат?
# machine-drafted, needs native review
fed-no-files = У { $folder } немає файлів IFC.
# machine-drafted, needs native review
fed-not-added = { $file } не додано: { $reason }
# machine-drafted, needs native review
fed-added = { $file } додано. Моделей у сцені: { $count }.
# machine-drafted, needs native review
fed-remove-title = Прибрати { $file } зі сцени?
# machine-drafted, needs native review
fed-removed = { $file } прибрано зі сцени.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] приховано
   *[shown] показано
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Колір за моделлю увімкнено
   *[off] Колір за моделлю вимкнено
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI та збережені вигляди
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] { $count } анотація
    [few] { $count } анотації
    [many] { $count } анотацій
   *[other] { $count } анотації
}
# machine-drafted, needs native review
pmi-view = Збережений вигляд
# machine-drafted, needs native review
pmi-view-all = показує всі анотації
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] показує { $count } анотацію
    [few] показує { $count } анотації
    [many] показує { $count } анотацій
   *[other] показує { $count } анотації
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Показано вигляд «{ $name }». A показує все.
# machine-drafted, needs native review
pmi-has-faces = грані підсвічено
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] У цій моделі немає PMI.
    [hints] ↑ ↓ вибрати · Enter перейти · A усі · D як намальовано · Shift+A сховати
    [hidden] Анотації приховано
    [shown] Анотації показано
    [as-drawn] Анотації так, як їх малює файл
    [as-labels] Анотації як підписи
   *[all-shown] Усі анотації показано
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Розмір
    [tolerance] Геометричний допуск
    [datum] База
    [target] Часткова база
   *[graphic] Намальована анотація
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Відстань
    [angular] Кут
    [diameter] Діаметр
    [radius] Радіус
    [length] Довжина
    [thickness] Товщина
   *[other] Розмір
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Прямолінійність
    [flatness] Площинність
    [circularity] Круглість
    [cylindricity] Циліндричність
    [line-profile] Форма лінії
    [surface-profile] Форма поверхні
    [angularity] Нахил
    [perpendicularity] Перпендикулярність
    [parallelism] Паралельність
    [position] Позиція
    [concentricity] Концентричність
    [coaxiality] Співвісність
    [symmetry] Симетричність
    [circular-runout] Биття
   *[total-runout] Повне биття
}
