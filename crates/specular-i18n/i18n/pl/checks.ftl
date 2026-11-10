# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Kolizje
    [changes] Zmiany
   *[quantities] Ilości
}
# $tab: the tab's name.
checks-title = Kontrole modelu · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Kontrole modelu wymagają modelu IFC.
    [reading] Wczytywanie informacji o modelu…
    [ids-empty] Brak pliku IDS. Naciśnij O, aby go otworzyć.
    [ids-checking] Sprawdzanie…
    [ids-no-failures] Nic nie jest niespełnione.
    [ids-hints] Enter błędy, przejdź · Backspace wstecz · O otwórz · P właściwości · Ctrl+S BCF · Ctrl+E CSV · Tab dalej
    [unnamed] Specyfikacja bez nazwy
    [clash-running] Szukanie kolizji…
    [clash-empty] Naciśnij R, aby wyszukać kolizje.
    [clash-none] Brak kolizji.
    [clashes-hints] R uruchom · A B zbiory · M twarda lub odstęp · + − odległość · Enter przejdź · S status · Ctrl+S BCF · Ctrl+E CSV · Tab dalej
    [changes-reading] Wczytywanie drugiej wersji…
    [changes-empty] Naciśnij O, aby wybrać inną wersję tego modelu.
    [changes-none] Brak zmian.
    [changes-hints] O inna wersja · F filtr · Enter przejdź · P właściwości · Ctrl+S BCF · Ctrl+E CSV · Tab dalej
    [quantities-hints] G grupuj · Enter przejdź · P właściwości · Ctrl+E CSV · Ctrl+S XLSX · Tab dalej
   *[not-drawn] Ten element nie ma geometrii do pokazania.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Klasa
    [attribute] Atrybut { $name }
    [classification] Klasyfikacja
    [material] Materiał
   *[partof] Część { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Klasa to { $actual }
    [predefined] Typ predefiniowany to { $actual }
    [missing] { $what }: brak
    [empty] { $what }: puste
    [value] { $what } = { $actual }: niedozwolone
    [datatype] { $what }: typ danych { $actual }
    [system] { $what }: system { $actual }
    [whole] { $what }: część { $actual }
    [prohibited] { $what }: nie może występować
   *[unsupported] { $what }: nie można sprawdzić
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Kondygnacja: { $value }
    [type] Typ: { $value }
    [material] Materiał: { $value }
    [container] Kontener: { $value }
   *[classification] Klasyfikacja: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: spełnione { $ok } z { $count }

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ spełnia: { $applicable }
    [fail] ✗ nie spełnia: { $failed } z { $applicable }
    [none] ✗ nie dotyczy niczego
    [none-ok] ✓ nie dotyczy niczego
    [prohibited] ✗ niedozwolone: { $applicable }
   *[prohibited-ok] ✓ brak niedozwolonych
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] wszystkie elementy
    [class] { $name }
   *[storey] kondygnacja { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] twarda, głębsza niż { $distance }
   *[clearance] bliżej niż { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] 1 kolizja
    [few] { $count } kolizje
    [many] { $count } kolizji
   *[other] { $count } kolizji
}, nowe: { $new }, rozwiązane: { $resolved } · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] twarda { $depth }
   *[clearance] odstęp { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Twarda
   *[clearance] Odstęp
}
checks-status = { $status ->
    [new] nowa
    [active] aktywna
   *[resolved] rozwiązana
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Kolizja: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Brak kolizji
    [one] 1 kolizja
    [few] { $count } kolizje
    [many] { $count } kolizji
   *[other] { $count } kolizji
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: dodane { $added } · usunięte { $removed } · zmienione { $changed }{ $filter ->
    [added] {" "}· pokazane dodane
    [removed] {" "}· pokazane usunięte
    [changed] {" "}· pokazane zmienione
   *[all] {""}
}
checks-change = { $kind ->
    [added] Dodany
    [removed] Usunięty
   *[geometry] geometria
}
checks-change-fields = { $count ->
    [one] 1 pole
    [few] { $count } pola
    [many] { $count } pól
   *[other] { $count } pola
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Przesunięty o { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = Elementy: { $count } · { $area } · { $volume } · { $by ->
    [element] według elementu
    [class] według klasy
    [storey] według kondygnacji
   *[type] według typu
}
# Where quantities came from.
checks-source = { $source ->
    [file] z pliku
    [computed] obliczone
   *[mixed] częściowo obliczone
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Elementy
    [class] Według klasy
    [storey] Według kondygnacji
   *[type] Według typu
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Specyfikacja
    [guid] GlobalId
    [class] Klasa
    [name] Nazwa
    [reason] Przyczyna
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Rodzaj
    [depth] Głębokość lub odległość (m)
    [status] Status
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Zmiana
    [field] Pole
    [old] Stara
    [new] Nowa
    [type] Typ
    [container] Kontener
    [material] Materiał
    [classification] Klasyfikacja
    [storey] Kondygnacja
    [count] Liczba
    [length] Długość (m)
    [area] Powierzchnia (m²)
    [volume] Objętość (m³)
   *[source] Źródło
}

# $reason: why, in plain words.
checks-error = Nie można tego zrobić: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Sprawdzono: wszystkie specyfikacje są spełnione
    [one] Sprawdzono: 1 specyfikacja nie jest spełniona
    [few] Sprawdzono: { $failed } specyfikacje nie są spełnione
    [many] Sprawdzono: { $failed } specyfikacji nie jest spełnionych
   *[other] Sprawdzono: { $failed } specyfikacji nie jest spełnione
}
# $file: the file written.
checks-saved = Zapisano { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = wszystkie modele
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Model: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M model · Enter błędy, przejdź · Backspace wstecz · O otwórz · P właściwości · Ctrl+S BCF · Ctrl+E CSV · Tab dalej
    [clashes] R uruchom · A B zbiory · G H ich modele · M twarda lub odstęp · + − odległość · Enter przejdź · S status · Ctrl+S BCF · Ctrl+E CSV · Tab dalej
    [changes] M model · O inna wersja · F filtr · Enter przejdź · P właściwości · Ctrl+S BCF · Ctrl+E CSV · Tab dalej
   *[quantities] M model · G grupuj · Enter przejdź · P właściwości · Ctrl+E CSV · Ctrl+S XLSX · Tab dalej
}
