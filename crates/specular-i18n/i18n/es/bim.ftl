# machine-drafted, needs native review
## Revisión de modelos 3D: caminar y volar, planos de sección y caja de sección, vistas guardadas,
## temas BCF (modo 3D). Las teclas citadas son las predeterminadas.

cheatsheet-review = Revisión

palette-mesh-walk = Caminar
palette-mesh-fly = Volar
palette-mesh-views = Vistas guardadas
palette-mesh-save-view = Guardar la vista
palette-mesh-section-plane = Añadir un plano de sección
palette-mesh-section-box = Caja de sección
palette-mesh-section-flip = Invertir el plano de sección
palette-mesh-section-clear = Quitar todos los cortes
palette-mesh-measure-cut = Medir en el corte
palette-mesh-bcf-topics = Temas BCF
# machine-drafted, needs native review
palette-mesh-model-checks = Comprobaciones del modelo
# machine-drafted, needs native review
palette-mesh-models = Modelos en la escena
# machine-drafted, needs native review
palette-mesh-add-model = Añadir modelo…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Añadir modelos de una carpeta…

# Caminar y volar: avisos y líneas de lectura, según $what. $eye: altura del ojo sobre el
# suelo, con su unidad (solo walk-line).
mesh-nav = { $what ->
    [walk-toast] Caminando: W A S D para moverse, arrastrar para mirar, Esc para parar
    [fly-toast] Volando: W A S D para moverse, Q E para bajar y subir, Esc para parar
    [orbit] Órbita
    [walk-line] Caminando · ojo a { $eye } del suelo
    [walk-keys] W A S D mover · arrastrar mira · Shift más rápido · Esc para
    [preparing] Paredes aún no listas: las atraviesa
    [fly-keys] W A S D mover · Q E bajar, subir · arrastrar mira · Shift más rápido · Esc para
   *[fly-line] Volando
}

# Textos cortos de cortes, medición y vistas guardadas, según $what.
bim-note = { $what ->
    [plane-added] Plano de sección añadido: arrastre su tirador para moverlo
    [box-on] Caja de sección activada: arrastre el tirador de una cara para moverla
    [box-off] Caja de sección desactivada
    [cleared] Cortes quitados
    [measure-on] Haga clic en dos puntos del corte
    [measure-needs-cut] Añada primero un plano de sección o la caja
    [measure-off-cut] Haga clic en el corte
    [measure-hint] Midiendo: haga clic en dos puntos del corte
    [ask-view-name] Nombre de la vista
    [ask-rename] Nombre nuevo
    [views-empty] No hay vistas guardadas. Pulse N para guardar esta.
    [views-hints] Enter ir · N guardar · F2 renombrar · Delete quitar · P guardar como PNG
    [delete-detail] Solo se quita la vista guardada. El modelo no cambia.
    [delete-ok] Quitar
    [not-over-model] Elija otro archivo: este reemplazaría el modelo.
   *[view-name] Vista
}

# Textos cortos de BCF (BIM Collaboration Format), según $what.
bcf-note = { $what ->
    [title-none] Temas BCF
    [new-file] nuevo
    [empty] No hay temas. Pulse O para abrir un archivo BCF o N para añadir un tema.
    [hints] Enter ir · ← → puntos de vista · N tema · C comentario · S estado · E título · O abrir · Ctrl+S exportar
    [ask-title] Título del tema
    [ask-comment] Comentario
    [untitled] Sin título
    [topic-added] Tema añadido con esta vista
   *[comment-added] Comentario añadido con esta vista
}

# $planes: cuántos planos de sección.
bim-sections-line = { $planes ->
    [0] Sin planos de sección
    [one] 1 plano de sección
   *[other] { $planes } planos de sección
}
# $size: el tamaño de la caja de sección, con su unidad.
bim-box-line = caja { $size }
# $count: el máximo de planos posible.
bim-planes-full = Como máximo { $count } planos de sección
# Un tirador de arrastre, para lectores de pantalla. $n: su número.
bim-handle = { $kind ->
    [box] Cara { $n } de la caja de sección
   *[plane] Plano de sección { $n }
}
# $length: la longitud con su unidad.
bim-measure-line = Longitud en el corte: { $length }
bim-views-title = { $count ->
    [one] 1 vista guardada
   *[other] { $count } vistas guardadas
}
# $name: el nombre de una vista.
bim-view-saved = «{ $name }» guardada
bim-view-delete-title = ¿Quitar la vista «{ $name }»?
# $file: un archivo escrito.
bim-saved-file = { $file } guardado
# $reason: por qué falló.
bim-failed = { $what ->
    [open] No se pudo abrir el archivo BCF: { $reason }
    [views] Vistas conservadas solo en esta sesión: { $reason }
   *[save] No se pudo guardar: { $reason }
}
# $count: temas; $file: el nombre del archivo.
bcf-title = { $count ->
    [one] 1 tema
   *[other] { $count } temas
} · { $file }
bcf-comments = { $count ->
    [one] 1 comentario
   *[other] { $count } comentarios
}
# $at: el punto de vista mostrado; $count: los puntos de vista del tema.
bcf-viewpoint-of = Punto de vista { $at } de { $count }
bcf-opened = { $count ->
    [one] 1 tema abierto
   *[other] { $count } temas abiertos
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modelos: 1
   *[other] Modelos: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Este archivo no es un modelo IFC.
    [not-ifc-open] Abra primero un modelo IFC. Solo se pueden combinar modelos IFC.
    [this-model] Este modelo
    [hints] A añadir modelo · Shift+A añadir carpeta · Space mostrar u ocultar · C colorear por modelo · Enter ir · Delete quitar
    [origin-ok] Situar en el origen
    [first-stays] El modelo abierto se mantiene. Abra otro archivo para cambiarlo.
    [remove-detail] Sale de la escena. El archivo en el disco se conserva.
   *[remove-ok] Quitar
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 elemento
   *[other] { $elements } elementos
} · { $shown ->
    [hidden] oculto
   *[shown] visible
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] modelo abierto
    [map-crs] por coordenadas de mapa ({ $crs })
    [map] por coordenadas de mapa
    [site] por latitud y longitud del emplazamiento (aproximado)
    [project] por coordenadas de proyecto compartidas
   *[origin] en el origen (sin alinear)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Los modelos usan sistemas de coordenadas distintos: { $first } y { $this }.
    [only-first] El modelo abierto está georreferenciado; este no.
    [only-this] Este modelo está georreferenciado; el abierto no.
   *[broken] La conversión de mapa de este modelo no se puede usar (su escala es cero).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Desplazado { $distance }, girado { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Leyendo 1 modelo…
   *[other] Leyendo { $count } modelos…
}
# machine-drafted, needs native review
fed-ask-origin = No se puede alinear { $file }. ¿Situarlo en el origen?
# machine-drafted, needs native review
fed-no-files = No hay archivos IFC en { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } no se añadió: { $reason }
# machine-drafted, needs native review
fed-added = { $file } añadido. Modelos en la escena: { $count }.
# machine-drafted, needs native review
fed-remove-title = ¿Quitar { $file } de la escena?
# machine-drafted, needs native review
fed-removed = { $file } quitado de la escena.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] oculto
   *[shown] visible
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Color por modelo activado
   *[off] Color por modelo desactivado
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI y vistas guardadas
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 anotación
   *[other] { $count } anotaciones
}
# machine-drafted, needs native review
pmi-view = Vista guardada
# machine-drafted, needs native review
pmi-view-all = muestra todas las anotaciones
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] muestra 1 anotación
   *[other] muestra { $count } anotaciones
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Mostrando la vista «{ $name }». A muestra todo.
# machine-drafted, needs native review
pmi-has-faces = caras resaltadas
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] No hay PMI en este modelo.
    [hints] ↑ ↓ elegir · Enter ir · A todo · D como está dibujado · Shift+A ocultar
    [hidden] Anotaciones ocultas
    [shown] Anotaciones visibles
    [as-drawn] Anotaciones como las dibuja el archivo
    [as-labels] Anotaciones como etiquetas
   *[all-shown] Todas las anotaciones visibles
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Cota
    [tolerance] Tolerancia geométrica
    [datum] Referencia
    [target] Referencia parcial
   *[graphic] Anotación dibujada
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Distancia
    [angular] Ángulo
    [diameter] Diámetro
    [radius] Radio
    [length] Longitud
    [thickness] Espesor
   *[other] Cota
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Rectitud
    [flatness] Planitud
    [circularity] Redondez
    [cylindricity] Cilindricidad
    [line-profile] Forma de una línea
    [surface-profile] Forma de una superficie
    [angularity] Inclinación
    [perpendicularity] Perpendicularidad
    [parallelism] Paralelismo
    [position] Posición
    [concentricity] Concentricidad
    [coaxiality] Coaxialidad
    [symmetry] Simetría
    [circular-runout] Oscilación circular
   *[total-runout] Oscilación total
}
