# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Botsingen
    [changes] Wijzigingen
   *[quantities] Hoeveelheden
}
# $tab: the tab's name.
checks-title = Modelcontroles · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Modelcontroles hebben een IFC-model nodig.
    [reading] Modelinformatie lezen…
    [ids-empty] Geen IDS-bestand. Druk op O om er een te openen.
    [ids-checking] Controleren…
    [ids-no-failures] Niets faalt.
    [ids-hints] Enter fouten, ga naar · Backspace terug · O openen · P eigenschappen · Ctrl+S BCF · Ctrl+E CSV · Tab volgende
    [unnamed] Naamloze specificatie
    [clash-running] Botsingen zoeken…
    [clash-empty] Druk op R om botsingen te zoeken.
    [clash-none] Geen botsingen.
    [clashes-hints] R starten · A B sets · M hard of vrije ruimte · + − afstand · Enter ga naar · S status · Ctrl+S BCF · Ctrl+E CSV · Tab volgende
    [changes-reading] De andere versie lezen…
    [changes-empty] Druk op O om een andere versie van dit model te kiezen.
    [changes-none] Geen wijzigingen.
    [changes-hints] O andere versie · F filter · Enter ga naar · P eigenschappen · Ctrl+S BCF · Ctrl+E CSV · Tab volgende
    [quantities-hints] G groeperen · Enter ga naar · P eigenschappen · Ctrl+E CSV · Ctrl+S XLSX · Tab volgende
   *[not-drawn] Dit element heeft geen geometrie om te tonen.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Klasse
    [attribute] Attribuut { $name }
    [classification] Classificatie
    [material] Materiaal
   *[partof] Deel van { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Klasse is { $actual }
    [predefined] Voorgedefinieerd type is { $actual }
    [missing] { $what } ontbreekt
    [empty] { $what } is leeg
    [value] { $what } is { $actual }: niet toegestaan
    [datatype] { $what } heeft gegevenstype { $actual }
    [system] { $what }: systeem is { $actual }
    [whole] { $what }: deel van { $actual }
    [prohibited] { $what } mag er niet zijn
   *[unsupported] { $what } kan niet worden gecontroleerd
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Bouwlaag: { $value }
    [type] Type: { $value }
    [material] Materiaal: { $value }
    [container] Container: { $value }
   *[classification] Classificatie: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $ok } van { $count } voldoen

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } voldoen
    [fail] ✗ { $failed } van { $applicable } falen
    [none] ✗ geldt voor niets
    [none-ok] ✓ geldt voor niets
    [prohibited] ✗ { $applicable } die er niet mogen zijn
   *[prohibited-ok] ✓ geen die er niet mogen zijn
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] alle elementen
    [class] { $name }
   *[storey] bouwlaag { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] hard, dieper dan { $distance }
   *[clearance] dichterbij dan { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] 1 botsing
   *[other] { $count } botsingen
}, { $new } nieuw, { $resolved } opgelost · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] hard { $depth }
   *[clearance] vrije ruimte { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Hard
   *[clearance] Vrije ruimte
}
checks-status = { $status ->
    [new] nieuw
    [active] actief
   *[resolved] opgelost
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Botsing: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Geen botsingen
    [one] 1 botsing
   *[other] { $count } botsingen
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } toegevoegd · { $removed } verwijderd · { $changed } gewijzigd{ $filter ->
    [added] {" "}· toegevoegde getoond
    [removed] {" "}· verwijderde getoond
    [changed] {" "}· gewijzigde getoond
   *[all] {""}
}
checks-change = { $kind ->
    [added] Toegevoegd
    [removed] Verwijderd
   *[geometry] geometrie
}
checks-change-fields = { $count ->
    [one] 1 veld
   *[other] { $count } velden
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = { $distance } verplaatst

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } element
       *[other] { $count } elementen
    } · { $area } · { $volume } · { $by ->
    [element] per element
    [class] per klasse
    [storey] per bouwlaag
   *[type] per type
}
# Where quantities came from.
checks-source = { $source ->
    [file] uit het bestand
    [computed] berekend
   *[mixed] deels berekend
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Elementen
    [class] Per klasse
    [storey] Per bouwlaag
   *[type] Per type
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Specificatie
    [guid] GlobalId
    [class] Klasse
    [name] Naam
    [reason] Reden
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Soort
    [depth] Diepte of afstand (m)
    [status] Status
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Wijziging
    [field] Veld
    [old] Oud
    [new] Nieuw
    [type] Type
    [container] Container
    [material] Materiaal
    [classification] Classificatie
    [storey] Bouwlaag
    [count] Aantal
    [length] Lengte (m)
    [area] Oppervlakte (m²)
    [volume] Volume (m³)
   *[source] Bron
}

# $reason: why, in plain words.
checks-error = Dat kan niet: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Gecontroleerd: elke specificatie voldoet
    [one] Gecontroleerd: 1 specificatie voldoet niet
   *[other] Gecontroleerd: { $failed } specificaties voldoen niet
}
# $file: the file written.
checks-saved = { $file } opgeslagen

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = alle modellen
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Model: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M model · Enter fouten, ga naar · Backspace terug · O openen · P eigenschappen · Ctrl+S BCF · Ctrl+E CSV · Tab volgende
    [clashes] R starten · A B sets · G H hun modellen · M hard of vrije ruimte · + − afstand · Enter ga naar · S status · Ctrl+S BCF · Ctrl+E CSV · Tab volgende
    [changes] M model · O andere versie · F filter · Enter ga naar · P eigenschappen · Ctrl+S BCF · Ctrl+E CSV · Tab volgende
   *[quantities] M model · G groeperen · Enter ga naar · P eigenschappen · Ctrl+E CSV · Ctrl+S XLSX · Tab volgende
}
