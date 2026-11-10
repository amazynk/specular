# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Colisiones
    [changes] Cambios
   *[quantities] Cantidades
}
# $tab: the tab's name.
checks-title = Comprobaciones del modelo · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Las comprobaciones del modelo necesitan un modelo IFC.
    [reading] Leyendo la información del modelo…
    [ids-empty] No hay archivo IDS. Pulse O para abrir uno.
    [ids-checking] Comprobando…
    [ids-no-failures] Nada falla.
    [ids-hints] Enter fallos, ir · Backspace atrás · O abrir · P propiedades · Ctrl+S BCF · Ctrl+E CSV · Tab siguiente
    [unnamed] Especificación sin nombre
    [clash-running] Buscando colisiones…
    [clash-empty] Pulse R para buscar colisiones.
    [clash-none] No hay colisiones.
    [clashes-hints] R ejecutar · A B conjuntos · M dura u holgura · + − distancia · Enter ir · S estado · Ctrl+S BCF · Ctrl+E CSV · Tab siguiente
    [changes-reading] Leyendo la otra versión…
    [changes-empty] Pulse O para elegir otra versión de este modelo.
    [changes-none] No hay cambios.
    [changes-hints] O otra versión · F filtro · Enter ir · P propiedades · Ctrl+S BCF · Ctrl+E CSV · Tab siguiente
    [quantities-hints] G agrupar · Enter ir · P propiedades · Ctrl+E CSV · Ctrl+S XLSX · Tab siguiente
   *[not-drawn] Este elemento no tiene geometría que mostrar.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Clase
    [attribute] Atributo { $name }
    [classification] Clasificación
    [material] Material
   *[partof] Parte de { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] La clase es { $actual }
    [predefined] El tipo predefinido es { $actual }
    [missing] Falta { $what }
    [empty] { $what }: vacío
    [value] { $what } es { $actual }: no permitido
    [datatype] { $what } tiene el tipo de dato { $actual }
    [system] { $what }: el sistema es { $actual }
    [whole] { $what }: parte de { $actual }
    [prohibited] { $what } no debe estar
   *[unsupported] { $what } no se puede comprobar
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Planta: { $value }
    [type] Tipo: { $value }
    [material] Material: { $value }
    [container] Contenedor: { $value }
   *[classification] Clasificación: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: se cumplen { $ok } de { $count }

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } cumplen
    [fail] ✗ { $failed } de { $applicable } fallan
    [none] ✗ no se aplica a nada
    [none-ok] ✓ no se aplica a nada
    [prohibited] ✗ { $applicable } que no deben existir
   *[prohibited-ok] ✓ ninguno que no deba existir
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] todos los elementos
    [class] { $name }
   *[storey] planta { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] dura, más profunda que { $distance }
   *[clearance] más cerca que { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] { $count } colisión
    [many] { $count } colisiones
   *[other] { $count } colisiones
}, { $new } nuevas, { $resolved } resueltas · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] dura { $depth }
   *[clearance] holgura { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Dura
   *[clearance] Holgura
}
checks-status = { $status ->
    [new] nueva
    [active] activa
   *[resolved] resuelta
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Colisión: { $a } × { $b }
checks-clash-done = { $count ->
    [0] No hay colisiones
    [one] { $count } colisión
    [many] { $count } colisiones
   *[other] { $count } colisiones
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } añadidos · { $removed } quitados · { $changed } cambiados{ $filter ->
    [added] {" "}· se muestran los añadidos
    [removed] {" "}· se muestran los quitados
    [changed] {" "}· se muestran los cambiados
   *[all] {""}
}
checks-change = { $kind ->
    [added] Añadido
    [removed] Quitado
   *[geometry] geometría
}
checks-change-fields = { $count ->
    [one] { $count } campo
    [many] { $count } campos
   *[other] { $count } campos
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Movido { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } elemento
       *[other] { $count } elementos
    } · { $area } · { $volume } · { $by ->
    [element] por elemento
    [class] por clase
    [storey] por planta
   *[type] por tipo
}
# Where quantities came from.
checks-source = { $source ->
    [file] del archivo
    [computed] calculadas
   *[mixed] algunas calculadas
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Elementos
    [class] Por clase
    [storey] Por planta
   *[type] Por tipo
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Especificación
    [guid] GlobalId
    [class] Clase
    [name] Nombre
    [reason] Motivo
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Tipo de colisión
    [depth] Profundidad o distancia (m)
    [status] Estado
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Cambio
    [field] Campo
    [old] Anterior
    [new] Nuevo
    [type] Tipo
    [container] Contenedor
    [material] Material
    [classification] Clasificación
    [storey] Planta
    [count] Cantidad
    [length] Longitud (m)
    [area] Área (m²)
    [volume] Volumen (m³)
   *[source] Origen
}

# $reason: why, in plain words.
checks-error = No se puede hacer: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Comprobado: se cumplen todas las especificaciones
    [one] Comprobado: { $failed } especificación no se cumple
    [many] Comprobado: { $failed } especificaciones no se cumplen
   *[other] Comprobado: { $failed } especificaciones no se cumplen
}
# $file: the file written.
checks-saved = Guardado { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = todos los modelos
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Modelo: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M modelo · Enter fallos, ir · Backspace atrás · O abrir · P propiedades · Ctrl+S BCF · Ctrl+E CSV · Tab siguiente
    [clashes] R ejecutar · A B conjuntos · G H sus modelos · M dura u holgura · + − distancia · Enter ir · S estado · Ctrl+S BCF · Ctrl+E CSV · Tab siguiente
    [changes] M modelo · O otra versión · F filtro · Enter ir · P propiedades · Ctrl+S BCF · Ctrl+E CSV · Tab siguiente
   *[quantities] M modelo · G agrupar · Enter ir · P propiedades · Ctrl+E CSV · Ctrl+S XLSX · Tab siguiente
}
