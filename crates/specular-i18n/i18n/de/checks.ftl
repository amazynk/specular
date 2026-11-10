# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Kollisionen
    [changes] Änderungen
   *[quantities] Mengen
}
# $tab: the tab's name.
checks-title = Modellprüfung · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Die Modellprüfung braucht ein IFC-Modell.
    [reading] Modellinformationen werden gelesen…
    [ids-empty] Keine IDS-Datei. O öffnet eine.
    [ids-checking] Wird geprüft…
    [ids-no-failures] Keine Fehler.
    [ids-hints] Enter Fehler, aufrufen · Backspace zurück · O öffnen · P Eigenschaften · Ctrl+S BCF · Ctrl+E CSV · Tab weiter
    [unnamed] Unbenannte Spezifikation
    [clash-running] Kollisionen werden gesucht…
    [clash-empty] R sucht nach Kollisionen.
    [clash-none] Keine Kollisionen.
    [clashes-hints] R starten · A B Gruppen · M hart oder Abstand · + − Distanz · Enter aufrufen · S Status · Ctrl+S BCF · Ctrl+E CSV · Tab weiter
    [changes-reading] Andere Version wird gelesen…
    [changes-empty] O wählt eine andere Version dieses Modells.
    [changes-none] Keine Änderungen.
    [changes-hints] O andere Version · F Filter · Enter aufrufen · P Eigenschaften · Ctrl+S BCF · Ctrl+E CSV · Tab weiter
    [quantities-hints] G gruppieren · Enter aufrufen · P Eigenschaften · Ctrl+E CSV · Ctrl+S XLSX · Tab weiter
   *[not-drawn] Dieses Bauteil hat keine Geometrie zum Anzeigen.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Klasse
    [attribute] Attribut { $name }
    [classification] Klassifikation
    [material] Material
   *[partof] Teil von { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Klasse ist { $actual }
    [predefined] Vordefinierter Typ ist { $actual }
    [missing] { $what } fehlt
    [empty] { $what } ist leer
    [value] { $what } ist { $actual }: nicht zulässig
    [datatype] { $what } hat den Datentyp { $actual }
    [system] { $what }: System ist { $actual }
    [whole] { $what }: Teil von { $actual }
    [prohibited] { $what } darf nicht vorhanden sein
   *[unsupported] { $what } kann nicht geprüft werden
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Geschoss: { $value }
    [type] Typ: { $value }
    [material] Material: { $value }
    [container] Container: { $value }
   *[classification] Klassifikation: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $ok } von { $count } erfüllt

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } erfüllt
    [fail] ✗ { $failed } von { $applicable } nicht erfüllt
    [none] ✗ gilt für nichts
    [none-ok] ✓ gilt für nichts
    [prohibited] ✗ { $applicable }, die nicht vorhanden sein dürfen
   *[prohibited-ok] ✓ keine, die nicht vorhanden sein dürfen
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] alle Bauteile
    [class] { $name }
   *[storey] Geschoss { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] hart, tiefer als { $distance }
   *[clearance] näher als { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] 1 Kollision
   *[other] { $count } Kollisionen
}, { $new } neu, { $resolved } gelöst · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] hart { $depth }
   *[clearance] Abstand { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Hart
   *[clearance] Abstand
}
checks-status = { $status ->
    [new] neu
    [active] aktiv
   *[resolved] gelöst
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Kollision: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Keine Kollisionen
    [one] 1 Kollision
   *[other] { $count } Kollisionen
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } hinzugefügt · { $removed } entfernt · { $changed } geändert{ $filter ->
    [added] {" "}· hinzugefügte werden gezeigt
    [removed] {" "}· entfernte werden gezeigt
    [changed] {" "}· geänderte werden gezeigt
   *[all] {""}
}
checks-change = { $kind ->
    [added] Hinzugefügt
    [removed] Entfernt
   *[geometry] Geometrie
}
checks-change-fields = { $count ->
    [one] 1 Feld
   *[other] { $count } Felder
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Verschoben um { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } Bauteil
       *[other] { $count } Bauteile
    } · { $area } · { $volume } · { $by ->
    [element] nach Bauteil
    [class] nach Klasse
    [storey] nach Geschoss
   *[type] nach Typ
}
# Where quantities came from.
checks-source = { $source ->
    [file] aus der Datei
    [computed] berechnet
   *[mixed] teils berechnet
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Bauteile
    [class] Nach Klasse
    [storey] Nach Geschoss
   *[type] Nach Typ
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Spezifikation
    [guid] GlobalId
    [class] Klasse
    [name] Name
    [reason] Grund
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Art
    [depth] Tiefe oder Abstand (m)
    [status] Status
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Änderung
    [field] Feld
    [old] Alt
    [new] Neu
    [type] Typ
    [container] Container
    [material] Material
    [classification] Klassifikation
    [storey] Geschoss
    [count] Anzahl
    [length] Länge (m)
    [area] Fläche (m²)
    [volume] Volumen (m³)
   *[source] Quelle
}

# $reason: why, in plain words.
checks-error = Nicht möglich: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Geprüft: Alle Spezifikationen sind erfüllt
    [one] Geprüft: 1 Spezifikation ist nicht erfüllt
   *[other] Geprüft: { $failed } Spezifikationen sind nicht erfüllt
}
# $file: the file written.
checks-saved = { $file } gespeichert

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = alle Modelle
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Modell: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M Modell · Enter Fehler, aufrufen · Backspace zurück · O öffnen · P Eigenschaften · Ctrl+S BCF · Ctrl+E CSV · Tab weiter
    [clashes] R starten · A B Gruppen · G H ihre Modelle · M hart oder Abstand · + − Distanz · Enter aufrufen · S Status · Ctrl+S BCF · Ctrl+E CSV · Tab weiter
    [changes] M Modell · O andere Version · F Filter · Enter aufrufen · P Eigenschaften · Ctrl+S BCF · Ctrl+E CSV · Tab weiter
   *[quantities] M Modell · G gruppieren · Enter aufrufen · P Eigenschaften · Ctrl+E CSV · Ctrl+S XLSX · Tab weiter
}
