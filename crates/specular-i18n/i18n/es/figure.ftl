# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Exportar figura…
# machine-drafted, needs native review
palette-export-figure-help = Guardar la vista como una figura con barra de escala, barra de color y etiquetas.
# machine-drafted, needs native review
palette-toggle-key-image = Marcar imagen clave
# machine-drafted, needs native review
palette-toggle-key-image-help = Marcar esta imagen como imagen clave, o quitar la marca.
# machine-drafted, needs native review
palette-copy-key-images = Copiar imágenes clave
# machine-drafted, needs native review
palette-copy-key-images-help = Copiar las imágenes clave como una sola imagen.
# machine-drafted, needs native review
palette-export-key-images = Exportar imágenes clave…
# machine-drafted, needs native review
palette-export-key-images-help = Guardar las imágenes clave como diapositivas o como una carpeta de PNG.
# machine-drafted, needs native review
palette-export-teaching-file = Exportar caso docente…
# machine-drafted, needs native review
palette-export-teaching-file-help = Guardar las imágenes clave sin datos del paciente, para la docencia.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Exportar figura
# machine-drafted, needs native review
figure-format = Formato
# machine-drafted, needs native review
figure-format-png = PNG, 16 bits
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16 bits
# machine-drafted, needs native review
figure-region = Región
# machine-drafted, needs native review
figure-region-visible = Área visible
# machine-drafted, needs native review
figure-region-whole = Imagen completa
# machine-drafted, needs native review
figure-size = Tamaño
# machine-drafted, needs native review
figure-size-pixels = Píxeles de la imagen (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = { $width } px de ancho
# machine-drafted, needs native review
figure-scale-bar = Barra de escala
# machine-drafted, needs native review
figure-scale-bar-none = Sin calibración
# machine-drafted, needs native review
figure-colour-bar = Barra de color
# machine-drafted, needs native review
figure-colour-bar-none = Sin mapa de color ni ventana
# machine-drafted, needs native review
figure-inset = Detalle ampliado
# machine-drafted, needs native review
figure-inset-rectangle = Último rectángulo
# machine-drafted, needs native review
figure-inset-none = Dibuja primero un rectángulo
# machine-drafted, needs native review
figure-channels = Canales
# machine-drafted, needs native review
figure-channels-merged = Combinados
# machine-drafted, needs native review
figure-channels-split = Separados y combinados
# machine-drafted, needs native review
figure-channels-one = Un canal
# machine-drafted, needs native review
figure-measurements = Mediciones
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Activadas ({ $count })
    [many] Activadas ({ $count })
   *[other] Activadas ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Ninguna
# machine-drafted, needs native review
figure-title-row = Título
# machine-drafted, needs native review
figure-provenance = Procedencia
# machine-drafted, needs native review
figure-provenance-metadata = En el archivo
# machine-drafted, needs native review
figure-provenance-drawn = En el archivo y dibujada
# machine-drafted, needs native review
figure-file = Archivo
# machine-drafted, needs native review
figure-choose = Elegir…
# machine-drafted, needs native review
figure-on = Activado
# machine-drafted, needs native review
figure-off = Desactivado
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Activado · { $source }
# machine-drafted, needs native review
figure-export = Exportar
# machine-drafted, needs native review
figure-working = Exportando…
# machine-drafted, needs native review
figure-not-full-res = Los píxeles a resolución completa no están cargados. Amplía al { PERCENT(100) } o elige un tamaño.
# machine-drafted, needs native review
figure-channel-red = Rojo
# machine-drafted, needs native review
figure-channel-green = Verde
# machine-drafted, needs native review
figure-channel-blue = Azul
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, de { $lo } a { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Ventana de { $lo } a { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Colores de canal { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Canal { $channel }
# machine-drafted, needs native review
figure-display-hilo = Marcas HiLo
# machine-drafted, needs native review
figure-display-as-stored = Tal como se guardó
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Calibración: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Visualización: { $value }
# machine-drafted, needs native review
figure-not-image = Las figuras se crean a partir de una imagen del visor.
# machine-drafted, needs native review
figure-not-ready = La imagen aún se está cargando.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Demasiado grande: { $width } × { $height } px. Elige el área visible o un tamaño.

## Key images
# machine-drafted, needs native review
figure-keys-title = Exportar imágenes clave
# machine-drafted, needs native review
figure-teaching-title = Exportar caso docente
# machine-drafted, needs native review
figure-keys-images = Imágenes
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } imagen clave
    [many] { $count } imágenes clave
   *[other] { $count } imágenes clave
}
# machine-drafted, needs native review
figure-keys-pptx = Diapositivas (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Carpeta de PNG
# machine-drafted, needs native review
figure-keys-preset = Ajuste preestablecido
# machine-drafted, needs native review
figure-keys-standard = Estándar
# machine-drafted, needs native review
figure-keys-teaching = Caso docente
# machine-drafted, needs native review
figure-keys-overlays-on = Dibujadas y en las notas
# machine-drafted, needs native review
figure-keys-masks = Texto incrustado
# machine-drafted, needs native review
figure-keys-masks-auto = Tapado en negro
# machine-drafted, needs native review
figure-keys-notes = Notas del caso
# machine-drafted, needs native review
figure-keys-destination = Guardar en
# machine-drafted, needs native review
figure-keys-empty = Aún no hay imágenes clave. Marca primero una imagen como imagen clave.
# machine-drafted, needs native review
figure-keys-teaching-note = Se omiten los datos del paciente y el texto incrustado se tapa en negro. Revisa cada imagen antes de compartir.
# machine-drafted, needs native review
figure-keys-none-left = No se pudo exportar ninguna imagen sin datos del paciente.
# machine-drafted, needs native review
figure-keys-copied = Imágenes clave copiadas
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Marcada como imagen clave ({ $count })
    [many] Marcada como imagen clave ({ $count })
   *[other] Marcada como imagen clave ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Ya no es imagen clave (quedan { $count })
    [many] Ya no es imagen clave (quedan { $count })
   *[other] Ya no es imagen clave (quedan { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Imagen { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Imagen clave { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } guardado
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } guardado. Se omitió { $count } imagen: no se encontró su texto incrustado.
    [many] { $name } guardado. Se omitieron { $count } imágenes: no se encontró su texto incrustado.
   *[other] { $name } guardado. Se omitieron { $count } imágenes: no se encontró su texto incrustado.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = No se pudo exportar: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } ya está en esa carpeta. Elige una carpeta vacía.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Imagen clave
