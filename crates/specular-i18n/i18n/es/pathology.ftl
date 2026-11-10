# machine-drafted, needs native review
# Specular — Patología: separación de tinciones, exportación e importación de anotaciones, volúmenes de etiquetas sobre volúmenes. Español.

## Paleta
palette-stain-separation = Separar tinciones
palette-stain-1 = Mostrar u ocultar la tinción 1
palette-stain-2 = Mostrar u ocultar la tinción 2
palette-stain-3 = Mostrar u ocultar la tinción 3
palette-stain-mode = Tinciones como luz o densidad óptica
palette-stain-estimate = Estimar tinciones a partir de la región
palette-stain-normalise = Normalizar los colores de tinción
palette-stain-reference = Usar la región como referencia de tinción
palette-stain-reset = Vectores de tinción publicados
palette-export-annotations = Exportar anotaciones…
palette-import-annotations = Importar anotaciones…
palette-label-volume = Superponer volumen de etiquetas…

## Motivos de desactivación
disabled-not-colour-image = Necesita una imagen en color
disabled-stains-off = Active primero la separación de tinciones
disabled-no-region = Dibuje primero una región
disabled-no-slide-annotations = No hay anotaciones, regiones ni puntos que exportar
disabled-no-label-target = Abra un volumen en vistas multiplanares, o una imagen médica

## Nombres de tinciones
stain-hematoxylin = Hematoxilina
stain-eosin = Eosina
stain-dab = DAB
stain-residual = Residuo
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Avisos y leyenda de tinciones
stain-off = Separación de tinciones desactivada
# { $set }: H&E, H-DAB o H&E-DAB.
stain-on = Tinciones: { $set }
stain-shown = { $stain } visible
stain-hidden = { $stain } no visible
stain-mode-light = Tinciones como luz
stain-mode-od = Tinciones como densidad óptica
# Título de la leyenda: «H&E · luz», «H-DAB · densidad óptica».
stain-legend-light = { $set } · luz
stain-legend-od = { $set } · densidad óptica
stain-legend-estimated = Estimadas a partir de la región
stain-legend-normalised = Normalizadas
stain-legend-published = Vectores publicados
# { $pixels }: píxeles teñidos usados.
stain-estimated = Tinciones estimadas a partir de { $pixels } píxeles
stain-estimate-failed = No se pueden estimar las tinciones: { $reason }
stain-estimate-background = la región es sobre todo fondo
stain-estimate-one-colour = la región tiene un solo color
stain-estimate-three = la estimación necesita H&E o H-DAB
stain-estimate-no-pixels = los píxeles de la región aún no se han cargado
stain-reference-set = Región fijada como referencia de tinción
stain-normalise-on = Colores de tinción normalizados
stain-normalise-off = Colores de tinción tal como se escanearon
stain-reset-done = Vectores de tinción publicados

## Diálogo de exportación de anotaciones
annot-export-title = Exportar anotaciones
annot-export-format = Formato
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Máscaras de etiquetas
annot-export-output = Máscaras como
annot-export-output-png = Teselas PNG
annot-export-output-tiff = Teselas TIFF
annot-export-output-ome = Pirámide OME-TIFF
annot-export-region = Región
annot-export-region-slide = Preparación completa
annot-export-region-view = Área visible
annot-export-region-drawn = Última región dibujada
annot-export-level = Nivel
# { $level }: 0 para la resolución completa; { $width } × { $height } su tamaño en píxeles.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Tamaño de tesela
annot-export-empty = Teselas vacías
annot-export-empty-skip = Omitir
annot-export-empty-keep = Escribir
annot-export-what = { $count ->
    [one] 1 forma
    [many] { $count } de formas
   *[other] { $count } formas
}, { $classes ->
    [one] 1 clase
    [many] { $classes } de clases
   *[other] { $classes } clases
}
annot-export-lines = Las líneas no se incluyen en COCO ni en las máscaras.
annot-export-export = Exportar…
annot-export-writing = Escribiendo…
# { $files }: archivos escritos.
annot-export-done = { $files ->
    [one] 1 archivo exportado
    [many] { $files } de archivos exportados
   *[other] { $files } archivos exportados
}
annot-export-failed = Error al exportar: { $reason }
annot-import-done = { $count ->
    [one] 1 anotación importada
    [many] { $count } de anotaciones importadas
   *[other] { $count } anotaciones importadas
}
annot-import-failed = No se puede importar: { $reason }
annot-import-skipped = { $count ->
    [one] 1 forma no tenía un contorno utilizable
    [many] { $count } de formas no tenían un contorno utilizable
   *[other] { $count } formas no tenían un contorno utilizable
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Volúmenes de etiquetas
label-volume-file = Volumen de etiquetas
label-volume-table = Tabla de colores
# { $file }: nombre del archivo del volumen de etiquetas; { $count }: sus etiquetas.
label-volume-loaded = { $count ->
    [one] { $file }: 1 etiqueta
    [many] { $file }: { $count } de etiquetas
   *[other] { $file }: { $count } etiquetas
}
label-volume-failed = No se puede superponer { $file }: { $reason }
label-volume-outside = { $file } no se solapa con este volumen
label-volume-not-whole = sus valores no son números enteros
label-volume-colour = contiene color, no etiquetas
label-volume-empty = no tiene etiquetas
label-volume-too-many = tiene demasiadas etiquetas
label-volume-too-large = es demasiado grande para remuestrearlo
label-volume-degenerate = su orientación no es válida
# Una etiqueta sin nombre en la tabla de colores.
label-volume-unnamed = Etiqueta { $value }
