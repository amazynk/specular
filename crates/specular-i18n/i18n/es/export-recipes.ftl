# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Exportar con recetas…
palette-export-recipes-help = Exporta la selección con una o varias recetas guardadas.
palette-develop-export-recipes = Exportar con recetas…
palette-develop-export-recipes-help = Exporta la foto editada, o varias, con recetas guardadas.

export-recipes-title = Exportar con recetas
export-recipes-new = Nueva
export-recipes-edit = Editar…
export-recipes-duplicate = Duplicar
export-recipes-delete = Eliminar…
export-recipes-start = Exportar
export-recipes-save = Guardar
export-recipes-open-folder = Abrir carpeta
export-recipes-choose-image = Elegir imagen…
export-recipes-no-images = No hay imágenes que exportar.
export-recipes-none-checked = Marca al menos una receta.
export-recipes-checked = { $name }: activada
export-recipes-unchecked = { $name }: desactivada

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Impresión, tamaño completo
export-recipes-starter-email = Correo, pequeño
export-recipes-unnamed = Receta sin nombre
export-recipes-new-name = Nueva receta
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (copia)

## The list's one-line summary.
export-recipes-summary-mark = marca de agua
export-recipes-size-full = Tamaño completo
export-recipes-size-long = { $px } px lado largo
export-recipes-size-short = { $px } px lado corto
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Junto al original
export-recipes-folder-sub = Carpeta «{ $name }»
export-recipes-folder-ask = Preguntar cada vez

## The editor.
export-recipes-new-title = Nueva receta
export-recipes-edit-title = Editar receta
export-recipes-name = Nombre de la receta
export-recipes-text = Texto propio
export-recipes-pattern = Nombre de archivo
export-recipes-mark-text-field = Texto de la marca de agua
export-recipes-subfolder = Nombre de la carpeta
export-recipes-format = Formato
export-recipes-quality = Calidad
export-recipes-lossless = Sin pérdida
export-recipes-bits = Profundidad de bits
export-recipes-bits-8 = 8 bits
export-recipes-bits-16 = 16 bits
export-recipes-space = Espacio de color
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Perfil
export-recipes-profile-embed = Incrustar
export-recipes-profile-none = Omitir
export-recipes-folder = Carpeta
export-recipes-folder-subfolder = Subcarpeta
export-recipes-collision = Nombre ya existe
export-recipes-collision-unique = Añadir un número
export-recipes-collision-overwrite = Reemplazar (pregunta)
export-recipes-collision-skip = Omitir
export-recipes-after = Al terminar
export-recipes-after-nothing = Nada
export-recipes-after-open = Abrir la carpeta
export-recipes-resize = Tamaño
export-recipes-resize-long = Lado largo
export-recipes-resize-short = Lado corto
export-recipes-resize-mp = Megapíxeles
export-recipes-resize-exact = Tamaño exacto
export-recipes-resize-percent = Porcentaje
export-recipes-edge = Lado
export-recipes-width = Ancho
export-recipes-height = Alto
export-recipes-megapixels = Megapíxeles
export-recipes-percent = Porcentaje
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Ampliar
export-recipes-sharpen = Enfocar para
export-recipes-sharpen-off = No
export-recipes-sharpen-screen = Pantalla
export-recipes-sharpen-matte = Papel mate
export-recipes-sharpen-glossy = Papel brillante
export-recipes-amount = Cantidad
export-recipes-amount-low = Baja
export-recipes-amount-standard = Normal
export-recipes-amount-high = Alta
export-recipes-metadata = Metadatos
export-recipes-meta-all = Todos
export-recipes-meta-copyright = Solo copyright
export-recipes-meta-contact = Copyright y contacto
export-recipes-meta-none = Ninguno
export-recipes-location = Ubicación
export-recipes-people = Personas que aparecen
export-recipes-keep = Conservar
export-recipes-remove = Quitar
export-recipes-mark = Marca de agua
export-recipes-mark-none = Ninguna
export-recipes-mark-text = Texto
export-recipes-mark-image = Imagen (elige una)
export-recipes-position = Posición
export-recipes-pos-top-left = Arriba a la izquierda
export-recipes-pos-top = Arriba
export-recipes-pos-top-right = Arriba a la derecha
export-recipes-pos-left = Izquierda
export-recipes-pos-centre = Centro
export-recipes-pos-right = Derecha
export-recipes-pos-bottom-left = Abajo a la izquierda
export-recipes-pos-bottom = Abajo
export-recipes-pos-bottom-right = Abajo a la derecha
export-recipes-opacity = Opacidad
export-recipes-scale = Ancho
export-recipes-yes = Sí
export-recipes-no = No
export-recipes-error-name = Pon un nombre a la receta.
export-recipes-error-mark-text = Escribe el texto de la marca de agua.
export-recipes-error-mark-image = Elige la imagen de la marca de agua.
export-recipes-error-subfolder = Escribe el nombre de la carpeta.
export-recipes-saved = Receta «{ $name }» guardada.

## Delete.
export-recipes-delete-title = ¿Eliminar la receta «{ $name }»?
export-recipes-delete-detail = Los archivos que exportó se quedan en el disco.
export-recipes-delete-confirm = Eliminar
export-recipes-delete-keep = Conservar
export-recipes-deleted = Receta «{ $name }» eliminada.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Carpeta para «{ $name }»
export-recipes-asking = Elige la carpeta de cada receta que la pide.
export-recipes-no-folder = No se eligió ninguna carpeta. No se exportó nada.
export-recipes-planning = Nombrando los archivos…
export-recipes-plan-pattern = Receta «{ $name }»: { $reason }
export-recipes-replace-title = { $count ->
    [one] ¿Reemplazar 1 archivo?
   *[other] ¿Reemplazar { $count } archivos?
}
export-recipes-replace-detail = Los originales nunca se reemplazan.
export-recipes-replace-confirm = Reemplazar
export-recipes-replace-kept = No se exportó nada.
export-recipes-running = Exportando…
export-recipes-progress = { $done } de { $total } archivos
export-recipes-done = { $count ->
    [one] 1 archivo exportado.
   *[other] { $count } archivos exportados.
}
export-recipes-stopped = { $count ->
    [one] Detenido. 1 archivo exportado.
   *[other] Detenido. { $count } archivos exportados.
}
export-recipes-done-skipped = { $count ->
    [one] 1 omitido: el nombre ya existía.
   *[other] { $count } omitidos: los nombres ya existían.
}
export-recipes-done-failed = { $count ->
    [one] 1 con error:
   *[other] { $count } con error:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
