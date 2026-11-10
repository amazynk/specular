# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Kolize
    [changes] Změny
   *[quantities] Výkaz množství
}
# $tab: the tab's name.
checks-title = Kontroly modelu · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Kontroly modelu vyžadují model IFC.
    [reading] Načítají se informace o modelu…
    [ids-empty] Žádný soubor IDS. Klávesou O jej otevřete.
    [ids-checking] Kontroluje se…
    [ids-no-failures] Nic neselhalo.
    [ids-hints] Enter chyby, přejít · Backspace zpět · O otevřít · P vlastnosti · Ctrl+S BCF · Ctrl+E CSV · Tab další
    [unnamed] Nepojmenovaná specifikace
    [clash-running] Hledají se kolize…
    [clash-empty] Klávesou R vyhledáte kolize.
    [clash-none] Žádné kolize.
    [clashes-hints] R spustit · A B sady · M tvrdá nebo odstup · + − vzdálenost · Enter přejít · S stav · Ctrl+S BCF · Ctrl+E CSV · Tab další
    [changes-reading] Načítá se druhá verze…
    [changes-empty] Klávesou O vyberete jinou verzi tohoto modelu.
    [changes-none] Žádné změny.
    [changes-hints] O jiná verze · F filtr · Enter přejít · P vlastnosti · Ctrl+S BCF · Ctrl+E CSV · Tab další
    [quantities-hints] G seskupit · Enter přejít · P vlastnosti · Ctrl+E CSV · Ctrl+S XLSX · Tab další
   *[not-drawn] Tento prvek nemá geometrii k zobrazení.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Třída
    [attribute] Atribut { $name }
    [classification] Klasifikace
    [material] Materiál
   *[partof] Součást { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Třída je { $actual }
    [predefined] Předdefinovaný typ je { $actual }
    [missing] { $what }: chybí
    [empty] { $what }: prázdné
    [value] { $what } je { $actual }: nepovoleno
    [datatype] { $what }: datový typ { $actual }
    [system] { $what }: systém je { $actual }
    [whole] { $what }: součást { $actual }
    [prohibited] { $what }: nesmí být přítomno
   *[unsupported] { $what }: nelze zkontrolovat
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Podlaží: { $value }
    [type] Typ: { $value }
    [material] Materiál: { $value }
    [container] Kontejner: { $value }
   *[classification] Klasifikace: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: splněno { $ok } z { $count }

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ vyhovuje: { $applicable }
    [fail] ✗ nevyhovuje: { $failed } z { $applicable }
    [none] ✗ neplatí pro nic
    [none-ok] ✓ neplatí pro nic
    [prohibited] ✗ nepovolených: { $applicable }
   *[prohibited-ok] ✓ žádné nepovolené
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] všechny prvky
    [class] { $name }
   *[storey] podlaží { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] tvrdá, hlubší než { $distance }
   *[clearance] blíž než { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] 1 kolize
    [few] { $count } kolize
    [many] { $count } kolize
   *[other] { $count } kolizí
}, nové: { $new }, vyřešené: { $resolved } · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] tvrdá { $depth }
   *[clearance] odstup { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Tvrdá
   *[clearance] Odstup
}
checks-status = { $status ->
    [new] nová
    [active] aktivní
   *[resolved] vyřešená
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Kolize: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Žádné kolize
    [one] 1 kolize
    [few] { $count } kolize
    [many] { $count } kolize
   *[other] { $count } kolizí
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: přidáno { $added } · odebráno { $removed } · změněno { $changed }{ $filter ->
    [added] {" "}· zobrazeny přidané
    [removed] {" "}· zobrazeny odebrané
    [changed] {" "}· zobrazeny změněné
   *[all] {""}
}
checks-change = { $kind ->
    [added] Přidáno
    [removed] Odebráno
   *[geometry] geometrie
}
checks-change-fields = { $count ->
    [one] 1 pole
    [few] { $count } pole
    [many] { $count } pole
   *[other] { $count } polí
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Posunuto o { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = Prvky: { $count } · { $area } · { $volume } · { $by ->
    [element] podle prvku
    [class] podle třídy
    [storey] podle podlaží
   *[type] podle typu
}
# Where quantities came from.
checks-source = { $source ->
    [file] ze souboru
    [computed] vypočteno
   *[mixed] částečně vypočteno
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Prvky
    [class] Podle třídy
    [storey] Podle podlaží
   *[type] Podle typu
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Specifikace
    [guid] GlobalId
    [class] Třída
    [name] Název
    [reason] Důvod
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Druh
    [depth] Hloubka nebo vzdálenost (m)
    [status] Stav
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Změna
    [field] Pole
    [old] Původní
    [new] Nová
    [type] Typ
    [container] Kontejner
    [material] Materiál
    [classification] Klasifikace
    [storey] Podlaží
    [count] Počet
    [length] Délka (m)
    [area] Plocha (m²)
    [volume] Objem (m³)
   *[source] Zdroj
}

# $reason: why, in plain words.
checks-error = Nelze provést: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Zkontrolováno: všechny specifikace jsou splněny
    [one] Zkontrolováno: 1 specifikace není splněna
    [few] Zkontrolováno: { $failed } specifikace nejsou splněny
    [many] Zkontrolováno: { $failed } specifikace není splněno
   *[other] Zkontrolováno: { $failed } specifikací není splněno
}
# $file: the file written.
checks-saved = Uloženo: { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = všechny modely
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Model: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M model · Enter chyby, přejít · Backspace zpět · O otevřít · P vlastnosti · Ctrl+S BCF · Ctrl+E CSV · Tab další
    [clashes] R spustit · A B sady · G H jejich modely · M tvrdá nebo odstup · + − vzdálenost · Enter přejít · S stav · Ctrl+S BCF · Ctrl+E CSV · Tab další
    [changes] M model · O jiná verze · F filtr · Enter přejít · P vlastnosti · Ctrl+S BCF · Ctrl+E CSV · Tab další
   *[quantities] M model · G seskupit · Enter přejít · P vlastnosti · Ctrl+E CSV · Ctrl+S XLSX · Tab další
}
