# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Törmäykset
    [changes] Muutokset
   *[quantities] Määrät
}
# $tab: the tab's name.
checks-title = Mallin tarkistukset · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Mallin tarkistukset vaativat IFC-mallin.
    [reading] Luetaan mallin tietoja…
    [ids-empty] Ei IDS-tiedostoa. Avaa tiedosto painamalla O.
    [ids-checking] Tarkistetaan…
    [ids-no-failures] Ei virheitä.
    [ids-hints] Enter virheet, siirry · Backspace takaisin · O avaa · P ominaisuudet · Ctrl+S BCF · Ctrl+E CSV · Tab seuraava
    [unnamed] Nimetön määritys
    [clash-running] Etsitään törmäyksiä…
    [clash-empty] Etsi törmäyksiä painamalla R.
    [clash-none] Ei törmäyksiä.
    [clashes-hints] R suorita · A B joukot · M kova tai vapaa tila · + − etäisyys · Enter siirry · S tila · Ctrl+S BCF · Ctrl+E CSV · Tab seuraava
    [changes-reading] Luetaan toista versiota…
    [changes-empty] Valitse mallin toinen versio painamalla O.
    [changes-none] Ei muutoksia.
    [changes-hints] O toinen versio · F suodatin · Enter siirry · P ominaisuudet · Ctrl+S BCF · Ctrl+E CSV · Tab seuraava
    [quantities-hints] G ryhmittele · Enter siirry · P ominaisuudet · Ctrl+E CSV · Ctrl+S XLSX · Tab seuraava
   *[not-drawn] Tällä rakennusosalla ei ole näytettävää geometriaa.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Luokka
    [attribute] Attribuutti { $name }
    [classification] Luokitus
    [material] Materiaali
   *[partof] Osa kokonaisuutta { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Luokka on { $actual }
    [predefined] Ennalta määritetty tyyppi on { $actual }
    [missing] { $what } puuttuu
    [empty] { $what } on tyhjä
    [value] { $what } on { $actual }: ei sallittu
    [datatype] { $what }: tietotyyppi on { $actual }
    [system] { $what }: järjestelmä on { $actual }
    [whole] { $what }: osa kokonaisuutta { $actual }
    [prohibited] { $what } ei saa olla mukana
   *[unsupported] { $what }: ei voi tarkistaa
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Kerros: { $value }
    [type] Tyyppi: { $value }
    [material] Materiaali: { $value }
    [container] Säiliö: { $value }
   *[classification] Luokitus: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $ok }/{ $count } täyttyy

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ hyväksytty: { $applicable }
    [fail] ✗ hylätty: { $failed }/{ $applicable }
    [none] ✗ ei koske mitään
    [none-ok] ✓ ei koske mitään
    [prohibited] ✗ kiellettyjä: { $applicable }
   *[prohibited-ok] ✓ ei kiellettyjä
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] kaikki rakennusosat
    [class] { $name }
   *[storey] kerros { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] kova, syvempi kuin { $distance }
   *[clearance] lähempänä kuin { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] 1 törmäys
   *[other] { $count } törmäystä
}, uusia { $new }, ratkaistuja { $resolved } · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] kova { $depth }
   *[clearance] vapaa tila { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Kova
   *[clearance] Vapaa tila
}
checks-status = { $status ->
    [new] uusi
    [active] aktiivinen
   *[resolved] ratkaistu
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Törmäys: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Ei törmäyksiä
    [one] 1 törmäys
   *[other] { $count } törmäystä
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: lisätty { $added } · poistettu { $removed } · muutettu { $changed }{ $filter ->
    [added] {" "}· näytetään lisätyt
    [removed] {" "}· näytetään poistetut
    [changed] {" "}· näytetään muutetut
   *[all] {""}
}
checks-change = { $kind ->
    [added] Lisätty
    [removed] Poistettu
   *[geometry] geometria
}
checks-change-fields = { $count ->
    [one] 1 kenttä
   *[other] { $count } kenttää
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Siirretty { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } rakennusosa
       *[other] { $count } rakennusosaa
    } · { $area } · { $volume } · { $by ->
    [element] rakennusosittain
    [class] luokittain
    [storey] kerroksittain
   *[type] tyypeittäin
}
# Where quantities came from.
checks-source = { $source ->
    [file] tiedostosta
    [computed] laskettu
   *[mixed] osin laskettu
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Rakennusosat
    [class] Luokittain
    [storey] Kerroksittain
   *[type] Tyypeittäin
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Määritys
    [guid] GlobalId
    [class] Luokka
    [name] Nimi
    [reason] Syy
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Laji
    [depth] Syvyys tai etäisyys (m)
    [status] Tila
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Muutos
    [field] Kenttä
    [old] Vanha
    [new] Uusi
    [type] Tyyppi
    [container] Säiliö
    [material] Materiaali
    [classification] Luokitus
    [storey] Kerros
    [count] Määrä
    [length] Pituus (m)
    [area] Pinta-ala (m²)
    [volume] Tilavuus (m³)
   *[source] Lähde
}

# $reason: why, in plain words.
checks-error = Ei onnistu: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Tarkistettu: kaikki määritykset täyttyvät
    [one] Tarkistettu: 1 määritys ei täyty
   *[other] Tarkistettu: { $failed } määritystä ei täyty
}
# $file: the file written.
checks-saved = Tallennettu { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = kaikki mallit
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Malli: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M malli · Enter virheet, siirry · Backspace takaisin · O avaa · P ominaisuudet · Ctrl+S BCF · Ctrl+E CSV · Tab seuraava
    [clashes] R suorita · A B joukot · G H niiden mallit · M kova tai vapaa tila · + − etäisyys · Enter siirry · S tila · Ctrl+S BCF · Ctrl+E CSV · Tab seuraava
    [changes] M malli · O toinen versio · F suodatin · Enter siirry · P ominaisuudet · Ctrl+S BCF · Ctrl+E CSV · Tab seuraava
   *[quantities] M malli · G ryhmittele · Enter siirry · P ominaisuudet · Ctrl+E CSV · Ctrl+S XLSX · Tab seuraava
}
