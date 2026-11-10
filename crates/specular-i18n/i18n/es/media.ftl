# Reproducción de animaciones y modo lectura (archivos comprimidos, cómics).

disabled-not-animated = Esta imagen no está animada
disabled-in-archive = Las páginas de un archivo comprimido no se pueden mover a la papelera

## Paleta de comandos y hoja de atajos

palette-play-pause = Reproducir o pausar la animación
palette-next-frame = Fotograma siguiente
palette-prev-frame = Fotograma anterior
palette-faster = Reproducir más rápido
palette-slower = Reproducir más despacio
palette-normal-speed = Velocidad de reproducción normal
palette-loop-forever = Repetir la animación sin fin
palette-right-to-left = Páginas de derecha a izquierda
# machine-drafted, needs native review
palette-contents = Índice
palette-spreads = Páginas dobles

## Controles de animación

transport-region = Controles de animación
transport-play = Reproducir
transport-pause = Pausar
transport-prev-frame = Fotograma anterior
transport-next-frame = Fotograma siguiente
transport-slower = Más despacio
transport-faster = Más rápido
transport-frame = Fotograma { $index } / { $count }
transport-frame-unknown = Fotograma { $index }
transport-speed = { $speed }×
transport-speed-name = Velocidad de reproducción { $speed }×
# machine-drafted, needs native review
motion-badge = Movimiento
motion-badge-live = Live
motion-play = Reproducir el clip en movimiento
motion-play-live = Reproducir la Live Photo
motion-stop = Volver a la foto fija
toast-motion-failed = No se puede reproducir el clip en movimiento: { $reason }

## Avisos

toast-speed = Velocidad de reproducción { $speed }×
toast-loop-forever-on = Repetición sin fin
toast-loop-forever-off = Repetición según el archivo
toast-right-to-left-on = Páginas de derecha a izquierda
toast-right-to-left-off = Páginas de izquierda a derecha
toast-spreads-on = Páginas dobles
toast-spreads-off = Una página cada vez

## Archivos comprimidos

title-archive-page = { $page } — { $archive }
readout-position-spread = { $first }–{ $last } / { $count }
archive-needs-sandbox = Los archivos { $format } solo puede abrirlos el decodificador aislado, que esta versión aún no incluye.
archive-empty = Este archivo no contiene imágenes.
archive-failed = No se pudo abrir el archivo: { $error }
archive-encrypted = Este archivo está cifrado. Ábralo de nuevo para introducir la contraseña.
archive-wrong-password = La contraseña de este archivo es incorrecta.
password-title = Se necesita una contraseña
password-archive = «{ $archive }» está cifrado.
password-label = Contraseña
password-show = Mostrar contraseña
password-wrong = Contraseña incorrecta. Inténtelo de nuevo.
password-open = Abrir
password-cancel = Cancelar

## Ajustes

settings-viewing-animation-autoplay = Reproducir animaciones automáticamente
settings-viewing-animation-autoplay-help = Iniciar las imágenes animadas (GIF, APNG, WebP…) al abrirlas.
settings-viewing-animation-loop = Repetición de animaciones
settings-viewing-animation-loop-help = Cuántas veces se repiten las animaciones.
settings-viewing-animation-loop-file = Según el archivo
settings-viewing-animation-loop-forever = Sin fin
settings-viewing-animation-loop-once = Una vez
settings-viewing-animation-short-delays = Retardos de fotograma muy cortos
settings-viewing-animation-short-delays-help = Fotogramas de 10 ms o menos: ralentizarlos a 100 ms como los navegadores web o reproducirlos tal como están guardados.
settings-viewing-animation-short-delays-browser = Como los navegadores (100 ms)
settings-viewing-animation-short-delays-as-stored = Tal como están guardados
settings-browsing-reading-order = Orden de páginas en archivos comprimidos
settings-browsing-reading-order-help = Los cómics se leen de derecha a izquierda cuando su ComicInfo.xml lo indica (manga), salvo que se elija otra cosa aquí.
settings-browsing-reading-order-auto = Según el archivo
settings-browsing-reading-order-left-to-right = De izquierda a derecha
settings-browsing-reading-order-right-to-left = De derecha a izquierda
settings-browsing-page-layout = Páginas en archivos comprimidos
settings-browsing-page-layout-help = Mostrar una página o dos juntas; la portada siempre se muestra sola.
settings-browsing-page-layout-auto = Según el archivo
settings-browsing-page-layout-single = Una página
settings-browsing-page-layout-spreads = Páginas dobles

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Página siguiente
palette-prev-page = Página anterior
palette-first-page = Primera página
palette-last-page = Última página
disabled-one-page = La imagen solo tiene una página
page-indicator = Página { $index } de { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Capa { $index }
page-label-face = Cara { $face }
page-label-sphere = Mapa esférico
page-label-slice = Corte { $index }
page-label-join = { $a } · { $b }
page-label-slide = Portaobjetos
page-label-slide-label = Etiqueta
page-label-slide-macro = Vista general
page-label-slide-thumbnail = Miniatura
# machine-drafted, needs native review
page-label-thermal = Imagen térmica
page-label-visible = Foto visible
# machine-drafted, needs native review
page-label-left-view = Vista izquierda
# machine-drafted, needs native review
page-label-right-view = Vista derecha
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } de { $count }
page-label-t = T { $index } de { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Bandas predeterminadas
page-label-product-bands = Todas las bandas
page-label-product-true-colour = Color verdadero
page-label-product-false-colour = Falso color (infrarrojo cercano)
page-label-product-swir = Infrarrojo de onda corta
page-label-product-agriculture = Agricultura
page-label-product-scene-class = Clasificación de escena
page-label-product-cloud = Probabilidad de nubes
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Página
info-page-position = Posición
info-page-contents = Contenido

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = No se usa OCIO: { $reason }
ocio-builtin-config = $OCIO indica una configuración integrada (ocio://); guárdala como archivo y elige ese archivo.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } bits

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Imagen combinada
page-label-from-layers = Compuesta a partir de las capas
page-label-layer-hidden = { $name } (oculta)
page-label-layer-unnamed = Capa sin nombre { $index }
