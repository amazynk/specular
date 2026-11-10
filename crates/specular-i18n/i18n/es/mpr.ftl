# Multiplanar reformatting (MPR).

palette-mpr = Reconstrucción multiplanar (MPR)
palette-mpr-slab = Siguiente grosor MPR (fino, MIP, MinIP, promedio)
disabled-no-volume = Esto no es un volumen: abra una serie, un archivo multifotograma, un volumen 3D o una pila z
disabled-mpr-closed = Abra primero las vistas multiplanares
mpr-axial = Axial
mpr-coronal = Coronal
mpr-sagittal = Sagital
mpr-oblique = Oblicuo
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = Vista 3D
mpr-thin = Fino
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Promedio
mpr-slab = Grosor: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Grosor: { $mode }, { $thickness } Å
mpr-slab-unit = Grosor: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Grosor: { $mode }, { $thickness } vóxeles
mpr-thicker = Grosor mayor
mpr-thinner = Grosor menor
mpr-reset = Restablecer vistas
mpr-close = Cerrar MPR
mpr-maximise = Maximizar vista
mpr-export-planes = Exportar planos
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Líneas de corriente de { $files }: grosor { $thickness } mm
mpr-tracts-off = Líneas de corriente de { $files }: ocultas
mpr-tracts-none = No hay líneas de corriente de TrackVis o MRtrix junto a este volumen
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } vóxeles
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } de { $count }
mpr-axis-channel = Canal
mpr-axis-unnamed = Eje { $axis }
mpr-one-channel = MPR muestra un canal cada vez
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } canales en color, cada uno con su rango de visualización
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Se muestra un canal: { $reason }
mpr-isotropic = El archivo no indica el tamaño de vóxel: vóxeles mostrados como cubos
mpr-isotropic-z = El archivo no indica la separación entre cortes: cortes mostrados a un píxel de distancia
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Cargando { $loaded } de { $count ->
        [one] { $count } corte
       *[other] { $count } cortes
    }
mpr-loaded = { $count ->
        [one] { $count } corte
       *[other] { $count } cortes
    }, { $width } × { $height }
mpr-downsampled = Mostrado a 1/{ $factor } de resolución (límites de memoria o gráficos)
mpr-converted = Valores mostrados como flotantes de 32 bits
mpr-opening = Abriendo el volumen…
mpr-gpu-memory = { $size } en la tarjeta gráfica
mpr-refused = No se puede construir un volumen: { $reason }
mpr-failed = El volumen no se cargó: { $reason }
mpr-no-gpu = La tarjeta gráfica no puede alojar este volumen: { $reason }
mpr-planes-saved = { $count ->
        [one] Se guardó { $count } plano en
       *[other] Se guardaron { $count } planos en
    } { $folder }
mpr-planes-failed = No se pudieron guardar los planos: { $reason }
mpr-view-a11y = Vista { $view }, { $position } mm
mpr-crosshair-a11y = Cruz en { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Cruz en { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Cruz en { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Cruz en el vóxel { $x }, { $y }, { $z }
mpr-controls = Controles de MPR

# 3D view palette entries.
palette-mpr-preset = Siguiente ajuste 3D (TC hueso, partes blandas, pulmón, angio; RM)
palette-mpr-render = Siguiente renderizado 3D (sombreado, Cinematic, MIP, MinIP)
palette-mpr-clip = Siguiente recorte 3D (ninguno, caja de las vistas 2D, esquina recortada)
palette-mpr-export-stl = Exportar la superficie 3D como STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Siguiente grosor de líneas de corriente en MPR
palette-mpr-maximise = Maximizar la vista MPR o mostrar todas las vistas
palette-mpr-export-planes = Exportar los planos MPR como imágenes…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] TC hueso
    [soft] TC partes blandas
    [lung] TC pulmón
    [angio] Angio-TC
   *[mr] RM
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Sombreado
}
vr-clip = { $clip ->
    [box] Recorte: caja de las vistas
    [cut] Recorte: esquina recortada
   *[off] Sin recorte
}
vr-smooth = Suavizar la superficie
vr-export = Exportar STL
vr-threshold = Superficie en { $value }
vr-stl-exporting = Construyendo la superficie…
vr-stl-saved = { $count ->
    [0] Nada en este umbral: no se guardó ninguna superficie
   *[other] { $count } triángulos guardados en { $name }
}
vr-stl-failed = No se pudo guardar la superficie: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, superficie en { $threshold }. Mayús+flechas para orbitar, más y menos para hacer zoom.

## Volumen sin procesar: cómo se organiza un archivo .raw / .vol sin cabecera

# machine-drafted, needs native review
raw-volume-title = Abrir volumen sin procesar
raw-volume-file = { $file } no tiene cabecera. Indique cómo están organizadas sus muestras.
raw-volume-guess-label = Organización sugerida
raw-volume-guess = { $index } de { $count }
raw-volume-custom = Personalizada
raw-volume-width = Anchura (vóxeles)
raw-volume-height = Altura (vóxeles)
raw-volume-slices = Cortes
raw-volume-type = Tipo de muestra
raw-volume-type-unsigned = Sin signo de { $bits } bits
raw-volume-type-signed = Con signo de { $bits } bits
raw-volume-type-float = Coma flotante de { $bits } bits
raw-volume-order = Orden de bytes
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Desplazamiento de cabecera
raw-volume-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } bytes
}
raw-volume-pixel = Tamaño de píxel
raw-volume-spacing = Separación entre cortes
raw-volume-mm = { $value } mm
raw-volume-exact = Ocupa el archivo exactamente ({ $size })
raw-volume-fits = Usa { $size } de { $file }; sobran { $left }
raw-volume-too-big = Necesita { $size }, pero el archivo ocupa { $file }
raw-volume-cancel = Cancelar
raw-volume-open = Abrir
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Conectividad de { $seed }; mueva la cruz a otra semilla
# machine-drafted, needs native review
mpr-measure-hint-edit = Arrastra un extremo para moverlo; sobre el otro extremo para eliminarla
# machine-drafted, needs native review
mpr-measure-removed = Medida eliminada
mpr-measure-lengths-only = MPR solo mide longitudes
mpr-more = Más…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = MPR curva a lo largo de una curva marcada con clics
palette-mpr-curve-layout = MPR curva: enderezada o estirada
palette-mpr-curve-save = Guardar la curva…
palette-mpr-curve-load = Cargar una curva…
palette-mpr-export-segments = Exportar cada segmento como STL…
palette-mpr-dental = Disposición dental (panorámica y cortes transversales)
palette-mpr-nerve = Trazar el conducto mandibular
cpr-kind = { $kind ->
    [stretched] Estirada
   *[straightened] Enderezada
}
cpr-button = Curva
cpr-controls = Controles de la MPR curva
cpr-at = En { $position }
cpr-section = Corte transversal
cpr-hint-start = Haga clic en puntos a lo largo de la estructura en la vista { $view }
cpr-hint-more = Haga clic en un punto más
cpr-add-points = Añadir puntos
cpr-undo-point = Deshacer punto
# machine-drafted, needs native review
cpr-mark = Marcar aquí
cpr-mark-set = Marca { $n } colocada.
cpr-mark-removed = Marca eliminada.
cpr-mark-here = Marca { $n }
cpr-diameter = Diámetro { $value }
cpr-diameters = Diámetro de { $min } a { $max }
cpr-mark-span = Marcas { $from } a { $to }: { $length } a lo largo de la línea central
cpr-mark-total = Todas las marcas: { $length }
cpr-clear = Borrar curva
cpr-save = Guardar curva
cpr-load = Cargar curva
cpr-done = Hecho
cpr-a11y = { $name } a lo largo de { $length }, cursor en { $position }. La rueda mueve el cursor.
cpr-none = Aún no hay curva: haga clic primero en puntos
cpr-cleared = Curva borrada
cpr-saved = Curva guardada: { $name }
cpr-save-failed = No se pudo guardar la curva: { $reason }
cpr-loaded = Curva cargada
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 línea central cargada
    [many] { $count } de líneas centrales cargadas
   *[other] { $count } líneas centrales cargadas
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] Se encontró 1 línea central guardada
    [many] Se encontraron { $count } de líneas centrales guardadas
   *[other] Se encontraron { $count } líneas centrales guardadas
}
# A centreline without a name, by its place in the list.
cpr-centreline = Línea central { $number }
cpr-new = Nueva línea central
cpr-rename = Cambiar nombre
cpr-name = Nombre de la línea central
cpr-delete = Eliminar
# $name: the centreline's name.
cpr-delete-title = ¿Eliminar la línea central «{ $name }»?
cpr-delete-detail = Sus puntos se borran. Los archivos guardados en el disco se quedan.
cpr-deleted = Se eliminó «{ $name }»
cpr-load-failed = No se pudo cargar la curva: { $reason }
cpr-load-not-json = el archivo no es JSON
cpr-load-not-curves = el archivo no es una curva de Specular
cpr-load-empty = el archivo no contiene ninguna curva de dos o más puntos
cpr-panoramic = Panorámica
cpr-panoramic-slab = Panorámica, grosor de { $thickness } mm
cpr-panoramic-thin = Panorámica, fina

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Dental
dental-not-for-diagnosis = No destinado al diagnóstico ni a la planificación del tratamiento
dental-nerve = Conducto mandibular
dental-nerve-hint = Haga clic en puntos a lo largo del conducto mandibular en la panorámica o en un corte transversal
dental-nerve-done = Trazado del conducto mandibular desactivado
dental-spacing = Cada { $spacing } mm
dental-section = Corte { $number }
dental-off = Abra primero la disposición dental
dental-suggesting = Buscando la arcada dental…
dental-suggested = Arcada sugerida: arrastre sus puntos para ajustarla
dental-no-arch = No se encontró la arcada: haga clic en puntos a lo largo de ella en la vista axial
dental-vendor = CBCT dental de { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Exportar segmentos
seg-stl-detail = Segmentos: { $count ->
        [one] { $count } triángulo
       *[other] { $count } triángulos
    }
seg-stl-detail-full = Segmentos: detalle completo
seg-stl-none = No hay segmentos que exportar
seg-stl-not-labels = este volumen no es un mapa de etiquetas
seg-stl-exporting = Construyendo las superficies de los segmentos…
seg-stl-saved = { $count ->
    [one] Se guardó { $count } segmento en { $folder }
    [many] Se guardaron { $count } segmentos en { $folder }
   *[other] Se guardaron { $count } segmentos en { $folder }
}
seg-stl-open = Sin cerrar: { $count }.
seg-stl-empty = Los segmentos están vacíos: no se guardó nada
seg-stl-failed = No se pudieron guardar los segmentos: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Tres planos (MPR)
toolbar-mpr-close = Cerrar tres planos (MPR)
settings-viewing-toolbar-items-mpr = Tres planos (MPR)
