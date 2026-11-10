# Specular — Export slideshow as video (DESIGN.md §6.5). Spanish. Machine-drafted, needs native review.

palette-export-slideshow = Exportar presentación como vídeo…

video-export-title = Exportar presentación como vídeo
video-export-resolution = Resolución
# $width and $height in pixels.
video-export-resolution-window = Ventana ({ $width } × { $height })
video-export-fps = Fotogramas por segundo
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Calidad
video-export-quality-high = Alta
video-export-quality-standard = Estándar
video-export-quality-compact = Compacta
video-export-images = { $count ->
    [one] Una imagen, con el orden, la transición y los pies de imagen de la presentación
   *[other] { $count } imágenes, con el orden, la transición y los pies de imagen de la presentación
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Unos { $length } de duración, unos { $size }
video-export-export = Exportar…
video-export-replace = Sustituir
# $file a file name.
video-export-replace-question = { $file } ya existe. ¿Sustituirlo?
# $percent a formatted percentage ("42%").
video-export-progress = Exportando… { $percent }
video-export-cancelling = Cancelando…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Guardado { $file } ({ $size })
video-export-skipped = { $count ->
    [one] No se pudo leer una imagen y se omitió.
   *[other] No se pudieron leer { $count } imágenes y se omitieron.
}
video-export-failed = No se pudo exportar el vídeo: { $reason }
video-export-cancelled = Exportación cancelada
video-export-no-images = No se pudo leer ninguna de las imágenes.
video-export-show-in-folder = Mostrar en la carpeta

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Exportar presentación como vídeo
video-export-compatibility = Compatibilidad
video-export-compatibility-current = Reproductores actuales
video-export-compatibility-all = Todos los reproductores
