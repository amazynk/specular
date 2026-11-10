# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Kollisioner
    [changes] Ändringar
   *[quantities] Mängder
}
# $tab: the tab's name.
checks-title = Modellkontroller · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Modellkontroller kräver en IFC-modell.
    [reading] Läser modellens information…
    [ids-empty] Ingen IDS-fil. Tryck O för att öppna en.
    [ids-checking] Kontrollerar…
    [ids-no-failures] Inget underkänns.
    [ids-hints] Enter fel, gå till · Backspace tillbaka · O öppna · P egenskaper · Ctrl+S BCF · Ctrl+E CSV · Tab nästa
    [unnamed] Namnlös specifikation
    [clash-running] Söker kollisioner…
    [clash-empty] Tryck R för att söka kollisioner.
    [clash-none] Inga kollisioner.
    [clashes-hints] R kör · A B grupper · M hård eller frigång · + − avstånd · Enter gå till · S status · Ctrl+S BCF · Ctrl+E CSV · Tab nästa
    [changes-reading] Läser den andra versionen…
    [changes-empty] Tryck O för att välja en annan version av modellen.
    [changes-none] Inga ändringar.
    [changes-hints] O annan version · F filter · Enter gå till · P egenskaper · Ctrl+S BCF · Ctrl+E CSV · Tab nästa
    [quantities-hints] G gruppera · Enter gå till · P egenskaper · Ctrl+E CSV · Ctrl+S XLSX · Tab nästa
   *[not-drawn] Objektet har ingen geometri att visa.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Klass
    [attribute] Attribut { $name }
    [classification] Klassifikation
    [material] Material
   *[partof] Del av { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Klassen är { $actual }
    [predefined] Fördefinierad typ är { $actual }
    [missing] { $what } saknas
    [empty] { $what } är tomt
    [value] { $what } är { $actual }: inte tillåtet
    [datatype] { $what } har datatypen { $actual }
    [system] { $what }: systemet är { $actual }
    [whole] { $what }: del av { $actual }
    [prohibited] { $what } får inte finnas
   *[unsupported] { $what } kan inte kontrolleras
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Våningsplan: { $value }
    [type] Typ: { $value }
    [material] Material: { $value }
    [container] Behållare: { $value }
   *[classification] Klassifikation: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $ok } av { $count } uppfylls

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } godkända
    [fail] ✗ { $failed } av { $applicable } underkända
    [none] ✗ gäller inget
    [none-ok] ✓ gäller inget
    [prohibited] ✗ { $applicable } som inte får finnas
   *[prohibited-ok] ✓ inga som inte får finnas
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] alla objekt
    [class] { $name }
   *[storey] våningsplan { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] hård, djupare än { $distance }
   *[clearance] närmare än { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] 1 kollision
   *[other] { $count } kollisioner
}, { $new } nya, { $resolved } lösta · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] hård { $depth }
   *[clearance] frigång { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Hård
   *[clearance] Frigång
}
checks-status = { $status ->
    [new] ny
    [active] aktiv
   *[resolved] löst
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Kollision: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Inga kollisioner
    [one] 1 kollision
   *[other] { $count } kollisioner
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } tillagda · { $removed } borttagna · { $changed } ändrade{ $filter ->
    [added] {" "}· visar tillagda
    [removed] {" "}· visar borttagna
    [changed] {" "}· visar ändrade
   *[all] {""}
}
checks-change = { $kind ->
    [added] Tillagt
    [removed] Borttaget
   *[geometry] geometri
}
checks-change-fields = { $count ->
    [one] 1 fält
   *[other] { $count } fält
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Flyttat { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } objekt
       *[other] { $count } objekt
    } · { $area } · { $volume } · { $by ->
    [element] per objekt
    [class] per klass
    [storey] per våningsplan
   *[type] per typ
}
# Where quantities came from.
checks-source = { $source ->
    [file] från filen
    [computed] beräknade
   *[mixed] delvis beräknade
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Objekt
    [class] Per klass
    [storey] Per våningsplan
   *[type] Per typ
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Specifikation
    [guid] GlobalId
    [class] Klass
    [name] Namn
    [reason] Orsak
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Sort
    [depth] Djup eller avstånd (m)
    [status] Status
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Ändring
    [field] Fält
    [old] Gammalt
    [new] Nytt
    [type] Typ
    [container] Behållare
    [material] Material
    [classification] Klassifikation
    [storey] Våningsplan
    [count] Antal
    [length] Längd (m)
    [area] Area (m²)
    [volume] Volym (m³)
   *[source] Källa
}

# $reason: why, in plain words.
checks-error = Det går inte: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Kontrollerat: alla specifikationer uppfylls
    [one] Kontrollerat: 1 specifikation uppfylls inte
   *[other] Kontrollerat: { $failed } specifikationer uppfylls inte
}
# $file: the file written.
checks-saved = Sparade { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = alla modeller
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Modell: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M modell · Enter fel, gå till · Backspace tillbaka · O öppna · P egenskaper · Ctrl+S BCF · Ctrl+E CSV · Tab nästa
    [clashes] R kör · A B grupper · G H deras modeller · M hård eller frigång · + − avstånd · Enter gå till · S status · Ctrl+S BCF · Ctrl+E CSV · Tab nästa
    [changes] M modell · O annan version · F filter · Enter gå till · P egenskaper · Ctrl+S BCF · Ctrl+E CSV · Tab nästa
   *[quantities] M modell · G gruppera · Enter gå till · P egenskaper · Ctrl+E CSV · Ctrl+S XLSX · Tab nästa
}
