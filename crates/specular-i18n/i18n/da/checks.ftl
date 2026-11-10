# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Kollisioner
    [changes] Ændringer
   *[quantities] Mængder
}
# $tab: the tab's name.
checks-title = Modelkontrol · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Modelkontrol kræver en IFC-model.
    [reading] Læser modellens oplysninger…
    [ids-empty] Ingen IDS-fil. Tryk O for at åbne en.
    [ids-checking] Kontrollerer…
    [ids-no-failures] Intet fejler.
    [ids-hints] Enter fejl, gå til · Backspace tilbage · O åbn · P egenskaber · Ctrl+S BCF · Ctrl+E CSV · Tab næste
    [unnamed] Unavngiven specifikation
    [clash-running] Søger efter kollisioner…
    [clash-empty] Tryk R for at søge efter kollisioner.
    [clash-none] Ingen kollisioner.
    [clashes-hints] R kør · A B grupper · M hård eller frigang · + − afstand · Enter gå til · S status · Ctrl+S BCF · Ctrl+E CSV · Tab næste
    [changes-reading] Læser den anden version…
    [changes-empty] Tryk O for at vælge en anden version af modellen.
    [changes-none] Ingen ændringer.
    [changes-hints] O anden version · F filter · Enter gå til · P egenskaber · Ctrl+S BCF · Ctrl+E CSV · Tab næste
    [quantities-hints] G gruppér · Enter gå til · P egenskaber · Ctrl+E CSV · Ctrl+S XLSX · Tab næste
   *[not-drawn] Elementet har ingen geometri at vise.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Klasse
    [attribute] Attribut { $name }
    [classification] Klassifikation
    [material] Materiale
   *[partof] Del af { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Klassen er { $actual }
    [predefined] Foruddefineret type er { $actual }
    [missing] { $what } mangler
    [empty] { $what } er tom
    [value] { $what } er { $actual }: ikke tilladt
    [datatype] { $what } har datatypen { $actual }
    [system] { $what }: systemet er { $actual }
    [whole] { $what }: del af { $actual }
    [prohibited] { $what } må ikke findes
   *[unsupported] { $what } kan ikke kontrolleres
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Etage: { $value }
    [type] Type: { $value }
    [material] Materiale: { $value }
    [container] Beholder: { $value }
   *[classification] Klassifikation: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $ok } af { $count } opfyldt

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } bestået
    [fail] ✗ { $failed } af { $applicable } fejler
    [none] ✗ gælder intet
    [none-ok] ✓ gælder intet
    [prohibited] ✗ { $applicable }, der ikke må findes
   *[prohibited-ok] ✓ ingen, der ikke må findes
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] alle elementer
    [class] { $name }
   *[storey] etage { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] hård, dybere end { $distance }
   *[clearance] tættere end { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] 1 kollision
   *[other] { $count } kollisioner
}, { $new } nye, { $resolved } løst · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] hård { $depth }
   *[clearance] frigang { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Hård
   *[clearance] Frigang
}
checks-status = { $status ->
    [new] ny
    [active] aktiv
   *[resolved] løst
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Kollision: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Ingen kollisioner
    [one] 1 kollision
   *[other] { $count } kollisioner
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } tilføjet · { $removed } fjernet · { $changed } ændret{ $filter ->
    [added] {" "}· viser tilføjede
    [removed] {" "}· viser fjernede
    [changed] {" "}· viser ændrede
   *[all] {""}
}
checks-change = { $kind ->
    [added] Tilføjet
    [removed] Fjernet
   *[geometry] geometri
}
checks-change-fields = { $count ->
    [one] 1 felt
   *[other] { $count } felter
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Flyttet { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } element
       *[other] { $count } elementer
    } · { $area } · { $volume } · { $by ->
    [element] pr. element
    [class] pr. klasse
    [storey] pr. etage
   *[type] pr. type
}
# Where quantities came from.
checks-source = { $source ->
    [file] fra filen
    [computed] beregnet
   *[mixed] delvis beregnet
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Elementer
    [class] Pr. klasse
    [storey] Pr. etage
   *[type] Pr. type
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Specifikation
    [guid] GlobalId
    [class] Klasse
    [name] Navn
    [reason] Årsag
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Slags
    [depth] Dybde eller afstand (m)
    [status] Status
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Ændring
    [field] Felt
    [old] Gammel
    [new] Ny
    [type] Type
    [container] Beholder
    [material] Materiale
    [classification] Klassifikation
    [storey] Etage
    [count] Antal
    [length] Længde (m)
    [area] Areal (m²)
    [volume] Volumen (m³)
   *[source] Kilde
}

# $reason: why, in plain words.
checks-error = Det kan ikke lade sig gøre: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Kontrolleret: alle specifikationer er opfyldt
    [one] Kontrolleret: 1 specifikation er ikke opfyldt
   *[other] Kontrolleret: { $failed } specifikationer er ikke opfyldt
}
# $file: the file written.
checks-saved = Gemte { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = alle modeller
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Model: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M model · Enter fejl, gå til · Backspace tilbage · O åbn · P egenskaber · Ctrl+S BCF · Ctrl+E CSV · Tab næste
    [clashes] R kør · A B grupper · G H deres modeller · M hård eller frigang · + − afstand · Enter gå til · S status · Ctrl+S BCF · Ctrl+E CSV · Tab næste
    [changes] M model · O anden version · F filter · Enter gå til · P egenskaber · Ctrl+S BCF · Ctrl+E CSV · Tab næste
   *[quantities] M model · G gruppér · Enter gå til · P egenskaber · Ctrl+E CSV · Ctrl+S XLSX · Tab næste
}
