# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Interferenze
    [changes] Modifiche
   *[quantities] Quantità
}
# $tab: the tab's name.
checks-title = Verifiche del modello · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Le verifiche del modello richiedono un modello IFC.
    [reading] Lettura delle informazioni del modello…
    [ids-empty] Nessun file IDS. Premi O per aprirne uno.
    [ids-checking] Verifica in corso…
    [ids-no-failures] Nessun errore.
    [ids-hints] Enter errori, vai · Backspace indietro · O apri · P proprietà · Ctrl+S BCF · Ctrl+E CSV · Tab successivo
    [unnamed] Specifica senza nome
    [clash-running] Ricerca delle interferenze…
    [clash-empty] Premi R per cercare le interferenze.
    [clash-none] Nessuna interferenza.
    [clashes-hints] R avvia · A B insiemi · M dura o distanza minima · + − distanza · Enter vai · S stato · Ctrl+S BCF · Ctrl+E CSV · Tab successivo
    [changes-reading] Lettura dell’altra versione…
    [changes-empty] Premi O per scegliere un’altra versione di questo modello.
    [changes-none] Nessuna modifica.
    [changes-hints] O altra versione · F filtro · Enter vai · P proprietà · Ctrl+S BCF · Ctrl+E CSV · Tab successivo
    [quantities-hints] G raggruppa · Enter vai · P proprietà · Ctrl+E CSV · Ctrl+S XLSX · Tab successivo
   *[not-drawn] Questo elemento non ha geometria da mostrare.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Classe
    [attribute] Attributo { $name }
    [classification] Classificazione
    [material] Materiale
   *[partof] Parte di { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] La classe è { $actual }
    [predefined] Il tipo predefinito è { $actual }
    [missing] Manca { $what }
    [empty] { $what }: vuoto
    [value] { $what } è { $actual }: non ammesso
    [datatype] { $what } ha il tipo di dato { $actual }
    [system] { $what }: il sistema è { $actual }
    [whole] { $what }: parte di { $actual }
    [prohibited] { $what } non deve esserci
   *[unsupported] { $what } non si può verificare
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Piano: { $value }
    [type] Tipo: { $value }
    [material] Materiale: { $value }
    [container] Contenitore: { $value }
   *[classification] Classificazione: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $ok } su { $count } rispettate

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } conformi
    [fail] ✗ { $failed } su { $applicable } non conformi
    [none] ✗ non si applica a nulla
    [none-ok] ✓ non si applica a nulla
    [prohibited] ✗ { $applicable } che non devono esistere
   *[prohibited-ok] ✓ nessuno che non debba esistere
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] tutti gli elementi
    [class] { $name }
   *[storey] piano { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] dura, più profonda di { $distance }
   *[clearance] più vicina di { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] { $count } interferenza
    [many] { $count } interferenze
   *[other] { $count } interferenze
}, { $new } nuove, { $resolved } risolte · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] dura { $depth }
   *[clearance] distanza minima { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Dura
   *[clearance] Distanza minima
}
checks-status = { $status ->
    [new] nuova
    [active] attiva
   *[resolved] risolta
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Interferenza: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Nessuna interferenza
    [one] { $count } interferenza
    [many] { $count } interferenze
   *[other] { $count } interferenze
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } aggiunti · { $removed } rimossi · { $changed } modificati{ $filter ->
    [added] {" "}· mostrati gli aggiunti
    [removed] {" "}· mostrati i rimossi
    [changed] {" "}· mostrati i modificati
   *[all] {""}
}
checks-change = { $kind ->
    [added] Aggiunto
    [removed] Rimosso
   *[geometry] geometria
}
checks-change-fields = { $count ->
    [one] { $count } campo
    [many] { $count } campi
   *[other] { $count } campi
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Spostato di { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } elemento
       *[other] { $count } elementi
    } · { $area } · { $volume } · { $by ->
    [element] per elemento
    [class] per classe
    [storey] per piano
   *[type] per tipo
}
# Where quantities came from.
checks-source = { $source ->
    [file] dal file
    [computed] calcolate
   *[mixed] in parte calcolate
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Elementi
    [class] Per classe
    [storey] Per piano
   *[type] Per tipo
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Specifica
    [guid] GlobalId
    [class] Classe
    [name] Nome
    [reason] Motivo
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Genere
    [depth] Profondità o distanza (m)
    [status] Stato
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Modifica
    [field] Campo
    [old] Vecchio
    [new] Nuovo
    [type] Tipo
    [container] Contenitore
    [material] Materiale
    [classification] Classificazione
    [storey] Piano
    [count] Numero
    [length] Lunghezza (m)
    [area] Area (m²)
    [volume] Volume (m³)
   *[source] Origine
}

# $reason: why, in plain words.
checks-error = Impossibile: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Verificato: tutte le specifiche sono rispettate
    [one] Verificato: { $failed } specifica non è rispettata
    [many] Verificato: { $failed } specifiche non sono rispettate
   *[other] Verificato: { $failed } specifiche non sono rispettate
}
# $file: the file written.
checks-saved = Salvato { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = tutti i modelli
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Modello: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M modello · Enter errori, vai · Backspace indietro · O apri · P proprietà · Ctrl+S BCF · Ctrl+E CSV · Tab successivo
    [clashes] R avvia · A B insiemi · G H i loro modelli · M dura o distanza minima · + − distanza · Enter vai · S stato · Ctrl+S BCF · Ctrl+E CSV · Tab successivo
    [changes] M modello · O altra versione · F filtro · Enter vai · P proprietà · Ctrl+S BCF · Ctrl+E CSV · Tab successivo
   *[quantities] M modello · G raggruppa · Enter vai · P proprietà · Ctrl+E CSV · Ctrl+S XLSX · Tab successivo
}
