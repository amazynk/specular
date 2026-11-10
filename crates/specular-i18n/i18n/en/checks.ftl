## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Clashes
    [changes] Changes
   *[quantities] Quantities
}
# $tab: the tab's name.
checks-title = Model checks · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Model checks need an IFC model.
    [reading] Reading the model's information…
    [ids-empty] No IDS file. Press O to open one.
    [ids-checking] Checking…
    [ids-no-failures] Nothing fails.
    [ids-hints] Enter failures, go · Backspace back · O open · P properties · Ctrl+S BCF · Ctrl+E CSV · Tab next
    [unnamed] Unnamed specification
    [clash-running] Looking for clashes…
    [clash-empty] Press R to look for clashes.
    [clash-none] No clashes.
    [clashes-hints] R run · A B sets · M hard or clearance · + − distance · Enter go · S status · Ctrl+S BCF · Ctrl+E CSV · Tab next
    [changes-reading] Reading the other version…
    [changes-empty] Press O to choose another version of this model.
    [changes-none] No changes.
    [changes-hints] O other version · F filter · Enter go · P properties · Ctrl+S BCF · Ctrl+E CSV · Tab next
    [quantities-hints] G group · Enter go · P properties · Ctrl+E CSV · Ctrl+S XLSX · Tab next
   *[not-drawn] This element has no geometry to show.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Class
    [attribute] Attribute { $name }
    [classification] Classification
    [material] Material
   *[partof] Part of { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Class is { $actual }
    [predefined] Predefined type is { $actual }
    [missing] { $what } is missing
    [empty] { $what } is empty
    [value] { $what } is { $actual }: not allowed
    [datatype] { $what } has data type { $actual }
    [system] { $what }: system is { $actual }
    [whole] { $what }: part of { $actual }
    [prohibited] { $what } must not be there
   *[unsupported] { $what } cannot be checked
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Storey: { $value }
    [type] Type: { $value }
    [material] Material: { $value }
    [container] Container: { $value }
   *[classification] Classification: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $ok } of { $count } hold

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } pass
    [fail] ✗ { $failed } of { $applicable } fail
    [none] ✗ applies to nothing
    [none-ok] ✓ applies to nothing
    [prohibited] ✗ { $applicable } that must not exist
   *[prohibited-ok] ✓ none that must not exist
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] all elements
    [class] { $name }
   *[storey] storey { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] hard, deeper than { $distance }
   *[clearance] closer than { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] 1 clash
   *[other] { $count } clashes
}, { $new } new, { $resolved } resolved · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] hard { $depth }
   *[clearance] clearance { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Hard
   *[clearance] Clearance
}
checks-status = { $status ->
    [new] new
    [active] active
   *[resolved] resolved
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Clash: { $a } × { $b }
checks-clash-done = { $count ->
    [0] No clashes
    [one] 1 clash
   *[other] { $count } clashes
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } added · { $removed } removed · { $changed } changed{ $filter ->
    [added] {" "}· showing added
    [removed] {" "}· showing removed
    [changed] {" "}· showing changed
   *[all] {""}
}
checks-change = { $kind ->
    [added] Added
    [removed] Removed
   *[geometry] geometry
}
checks-change-fields = { $count ->
    [one] 1 field
   *[other] { $count } fields
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Moved { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } element
       *[other] { $count } elements
    } · { $area } · { $volume } · { $by ->
    [element] by element
    [class] by class
    [storey] by storey
   *[type] by type
}
# Where quantities came from.
checks-source = { $source ->
    [file] from file
    [computed] computed
   *[mixed] some computed
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Elements
    [class] By class
    [storey] By storey
   *[type] By type
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Specification
    [guid] GlobalId
    [class] Class
    [name] Name
    [reason] Reason
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Kind
    [depth] Depth or distance (m)
    [status] Status
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Change
    [field] Field
    [old] Old
    [new] New
    [type] Type
    [container] Container
    [material] Material
    [classification] Classification
    [storey] Storey
    [count] Count
    [length] Length (m)
    [area] Area (m²)
    [volume] Volume (m³)
   *[source] Source
}

# $reason: why, in plain words.
checks-error = Cannot do that: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Checked: every specification holds
    [one] Checked: 1 specification does not hold
   *[other] Checked: { $failed } specifications do not hold
}
# $file: the file written.
checks-saved = Saved { $file }

# Several models in one scene. An element or clash set and the model file it is from.
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
checks-every-model = every model
# The model IDS, changes and quantities are about. $model: its file name.
checks-model = Model: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
checks-hints-models = { $tab ->
    [ids] M model · Enter failures, go · Backspace back · O open · P properties · Ctrl+S BCF · Ctrl+E CSV · Tab next
    [clashes] R run · A B sets · G H their models · M hard or clearance · + − distance · Enter go · S status · Ctrl+S BCF · Ctrl+E CSV · Tab next
    [changes] M model · O other version · F filter · Enter go · P properties · Ctrl+S BCF · Ctrl+E CSV · Tab next
   *[quantities] M model · G group · Enter go · P properties · Ctrl+E CSV · Ctrl+S XLSX · Tab next
}
