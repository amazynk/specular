# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Convertir…

## Settings → Convert
settings-section-convert = Convertir
settings-convert-format = Formato
settings-convert-format-help = El formato que escribe Convertir…; PNG y TIFF son sin pérdida, y WebP y JPEG XL pueden ser de ambos tipos.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = Calidad JPEG
settings-convert-jpeg-quality-help = Un valor más alto conserva más detalle en archivos más grandes. Con 90 cuesta distinguirlo del original.
settings-convert-jpeg-full-chroma = Color a resolución completa en JPEG
settings-convert-jpeg-full-chroma-help = Mantiene el color a resolución completa (4:4:4) en lugar de a la mitad (4:2:0): bordes de color más nítidos, archivos más grandes.
settings-convert-png-effort = Compresión PNG
settings-convert-png-effort-help = Un valor más alto crea archivos más pequeños, más despacio. 1 es el más rápido.
settings-convert-jxl-effort = Esfuerzo de JPEG XL
settings-convert-jxl-effort-help = Un valor más alto crea archivos más pequeños, más despacio.
settings-convert-tiff-compression = Compresión TIFF
settings-convert-tiff-compression-help = Deflate es sin pérdida y lo lee cualquier lector de TIFF.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Ninguna
settings-convert-depth = Profundidad de bits
settings-convert-depth-help = Automática escribe 16 bits por muestra cuando la imagen tiene más de 8 y el formato los admite (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automática
settings-convert-depth-8 = 8 bits
settings-convert-depth-16 = 16 bits
settings-convert-resize = Cambiar tamaño
settings-convert-resize-help = Las imágenes se reducen en luz lineal con un filtro de alta calidad.
settings-convert-resize-none = Tamaño original
settings-convert-resize-fit = Ajustar dentro de
settings-convert-resize-long-edge = Lado largo
settings-convert-resize-percent = Porcentaje
settings-convert-width = Ancho
settings-convert-width-help = El ancho máximo que puede tener una imagen con Ajustar dentro de.
settings-convert-height = Alto
settings-convert-height-help = El alto máximo que puede tener una imagen con Ajustar dentro de.
settings-convert-long-edge = Lado largo
settings-convert-long-edge-help = La longitud del lado más largo de la imagen con Lado largo.
settings-convert-percent = Porcentaje
settings-convert-percent-help = El tamaño respecto al original con Porcentaje.
settings-convert-upscale = Ampliar imágenes pequeñas
settings-convert-upscale-help = Si está desactivado, las imágenes ya más pequeñas que el tamaño pedido conservan su tamaño.
settings-convert-colour = Color
settings-convert-colour-help = Conservar guarda el perfil de color de la imagen junto con ella. Convertir a sRGB es adecuado para la web y para dispositivos sin gestión del color.
settings-convert-colour-keep = Conservar el perfil
settings-convert-colour-srgb = Convertir a sRGB
settings-convert-hdr = Imágenes HDR
settings-convert-hdr-help = PNG y JPEG XL conservan el HDR tal cual; los demás formatos siempre se pasan a SDR con mapeo de tonos.
settings-convert-hdr-keep = Conservar si es posible
settings-convert-hdr-tone-map = Mapeo de tonos a SDR
settings-convert-metadata = Metadatos
settings-convert-metadata-help = Exif y XMP, como la cámara, la fecha de captura y la ubicación. El perfil de color se conserva siempre.
settings-convert-metadata-all = Conservar todo
settings-convert-metadata-no-location = Todo salvo la ubicación
settings-convert-metadata-strip = Quitar
settings-convert-folder = Carpeta de destino
settings-convert-folder-help = Dónde van los archivos convertidos. Si no se define, van a una subcarpeta junto a cada imagen.
settings-convert-folder-unset = Subcarpeta junto a las imágenes
settings-convert-subfolder = Nombre de la subcarpeta
settings-convert-subfolder-help = La subcarpeta que se usa cuando no hay carpeta de destino. Vacío: la propia carpeta de las imágenes.
settings-convert-pattern = Nombres de archivo
settings-convert-pattern-help = {"{"}name{"}"} es el nombre original, {"{"}n:03{"}"} el número en el lote, {"{"}date:%Y-%m-%d{"}"} la fecha de captura, {"{"}w{"}"} y {"{"}h{"}"} el nuevo tamaño, {"{"}ext{"}"} la extensión del formato.
settings-convert-conflicts = Si un nombre ya existe
settings-convert-conflicts-help = Los archivos existentes nunca se reemplazan.
settings-convert-conflicts-suffix = Añadir un número
settings-convert-conflicts-skip = Omitir la imagen

## The dialog
convert-title = Convertir
convert-images = Imágenes
convert-images-current = Esta imagen
convert-images-selection = Selección ({ $count })
convert-images-folder = Carpeta ({ $count })
convert-images-picked = Elegidas ({ $count })
convert-quality = Calidad
convert-chroma = Detalle del color
convert-chroma-half = 4:2:0 (más pequeño)
convert-chroma-full = 4:4:4 (más nítido)
convert-effort = Esfuerzo
convert-on = Sí
convert-off = No
convert-pixels = { $value } px
convert-effort-fastest = { $value } (el más rápido)
convert-folder-subfolder = Subcarpeta «{ $name }»
convert-folder-same = La misma carpeta que la imagen
convert-choose-folder = Elegir carpeta…
convert-name = Nombre
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Hay una {"{"} sin cerrar.
convert-name-error-stray = Una {"}"} no tiene su {"{"}. Escriba {"}}"} para poner una llave.
convert-name-error-unknown = { $token } no es una parte del nombre. Use name, n, date, w, h o ext.
convert-name-error-format = { $token } tiene un formato incorrecto.
convert-name-error-empty = Escriba un patrón de nombre.
convert-start = Convertir
convert-stop = Detener
convert-hide = Ocultar
convert-close = Cerrar
convert-progress = Convirtiendo { $done } de { $total }
convert-progress-eta = Convirtiendo { $done } de { $total } · quedan unos { $time }
convert-eta-seconds = { $count ->
    [one] { $count } segundo
   *[other] { $count } segundos
}
convert-eta-minutes = { $count ->
    [one] { $count } minuto
   *[other] { $count } minutos
}
convert-done = { $count ->
    [one] { $count } imagen convertida
   *[other] { $count } imágenes convertidas
}
convert-done-skipped = { $count } omitidas: el nombre ya existía
convert-done-failed = { $count } con error
convert-stopped = Detenido tras { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = no se puede abrir ({ $error })
convert-failure-decode = no se puede leer ({ $error })
convert-failure-encode = no se puede escribir en este formato ({ $error })
convert-failure-write = no se puede guardar ({ $error })
convert-failure-same = el archivo nuevo reemplazaría al original
convert-report-more = y { $count } más
convert-running = Ya hay una conversión en curso.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = WebP sin pérdida
settings-convert-webp-lossless-help = Conserva cada píxel exactamente. Desactivado, los archivos WebP son mucho más pequeños con la calidad WebP.
settings-convert-webp-quality = Calidad WebP
settings-convert-webp-quality-help = Un valor más alto conserva más detalle en archivos más grandes. Se usa cuando WebP no está en modo sin pérdida.
settings-convert-webp-effort = Esfuerzo de WebP
settings-convert-webp-effort-help = Un valor más alto crea archivos más pequeños, más despacio. Se usa cuando WebP no está en modo sin pérdida.
settings-convert-jxl-lossless = JPEG XL sin pérdida
settings-convert-jxl-lossless-help = Conserva cada píxel exactamente. Desactivado, los archivos JPEG XL son mucho más pequeños con la calidad JPEG XL.
settings-convert-jxl-quality = Calidad JPEG XL
settings-convert-jxl-quality-help = Un valor más alto conserva más detalle en archivos más grandes. Con 90 cuesta distinguirlo del original. Se usa cuando JPEG XL no está en modo sin pérdida.
settings-convert-avif-quality = Calidad AVIF
settings-convert-avif-quality-help = Un valor más alto conserva más detalle en archivos más grandes.
settings-convert-avif-effort = Esfuerzo de AVIF
settings-convert-avif-effort-help = Un valor más alto crea archivos más pequeños, más despacio. 1 es el más rápido.
convert-lossless = Sin pérdida
