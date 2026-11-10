# machine-drafted, needs native review
# Specular — ventana de Configuración. Español (internacional neutro).

## Elementos de la ventana

settings-title = Configuración
settings-search-placeholder = Buscar en la configuración
settings-search-no-results = Ningún ajuste coincide con { $query }
settings-reset = Restablecer
settings-reset-section = Restablecer sección
settings-modified = Modificado
settings-default-value = Predeterminado: { $value }
settings-restart-required = Se aplicará tras reiniciar
settings-restart-now = Reiniciar ahora
settings-undo = Deshacer
settings-redo = Rehacer
settings-value-on = Activado
settings-value-off = Desactivado
settings-value-auto = Automático
settings-value-system = Según el sistema
settings-value-system-language = Predeterminado del sistema
settings-value-follow-language = Según el idioma de la interfaz
settings-value-default-location = Ubicación predeterminada
settings-choose = Elegir…
settings-clear = Borrar
settings-file-reloaded = settings.toml se ha modificado fuera de { -app-name } y se ha vuelto a cargar.
settings-warning-unknown-key = Ajuste desconocido { $key } en settings.toml. Se conserva en el archivo, pero se ignora.
settings-warning-invalid-value = Valor no válido para { $key }: { $reason } Se usa el valor predeterminado en su lugar.
settings-warning-newer-version = settings.toml lo escribió una versión más reciente de { -app-name }. Se conservan los ajustes que esta versión no reconoce.
settings-warning-syntax = No se puede leer settings.toml (línea { $line }): { $reason } Se usan los valores predeterminados hasta que se corrija.

## Errores de validación (también los muestra `specular --set`)

settings-error-unknown-key = No existe ningún ajuste llamado { $key }.
settings-error-unknown-key-suggestion = No existe ningún ajuste llamado { $key }. ¿Quiso decir { $suggestion }?
settings-error-not-bool = Se esperaba on u off (true o false).
settings-error-not-integer = Se esperaba un número entero.
settings-error-not-number = Se esperaba un número.
settings-error-out-of-range = Debe estar entre { $min } y { $max }.
settings-error-out-of-range-auto = Debe ser 0 (automático) o estar entre { $min } y { $max }.
settings-error-unknown-option = Se esperaba uno de estos valores: { $options }.
settings-error-bad-colour = Se esperaba un color como #1E1E1E o #1E1E1E80.
settings-error-bad-language = Se esperaba una etiqueta de idioma como de, pt-BR o zh-Hans, o bien “system”.
settings-error-bad-path = Se esperaba la ruta de un archivo o una carpeta.
settings-error-not-settable = Esta entrada es un comando y no tiene valor.
settings-error-syntax = Se esperaba clave=valor, por ejemplo viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = No puede contener tabulaciones, saltos de línea ni otros caracteres de control.
settings-error-invalid = { $reason }
settings-error-tile-url = Necesita una dirección http o https con {"{"}z{"}"}, {"{"}x{"}"} e {"{"}y{"}"}.

## Unidades

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } imagen
        [many] { $value } de imágenes
       *[other] { $value } imágenes
    }
unit-threads =
    { $value ->
        [one] { $value } hilo
        [many] { $value } de hilos
       *[other] { $value } hilos
    }
unit-bytes =
    { $value ->
        [one] { $value } byte
        [many] { $value } de bytes
       *[other] { $value } bytes
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Secciones

settings-section-general = General
settings-section-viewing = Visualización
settings-section-browsing = Exploración
settings-section-colour = Color
settings-section-metadata = Metadatos y privacidad
# machine-drafted, needs native review
settings-section-privacy = Privacidad
settings-section-performance = Rendimiento
settings-section-accessibility = Accesibilidad
settings-section-keys = Teclas
settings-section-advanced = Avanzado

## General

settings-general-language = Idioma de la interfaz
settings-general-language-help = Los cambios se aplican de inmediato. «Predeterminado del sistema» usa el idioma del sistema operativo.
settings-general-region-formats = Formatos regionales
settings-general-region-formats-help = Cómo se escriben las fechas, horas, números y tamaños. De forma predeterminada siguen el idioma de la interfaz.
settings-general-theme = Tema
settings-general-theme-help = Aspecto claro u oscuro para ventanas y controles. Las imágenes nunca se modifican.
settings-general-theme-system = Según el sistema
settings-general-theme-light = Claro
settings-general-theme-dark = Oscuro
# machine-drafted, needs native review
settings-general-on-start = Cuando { -app-name } se abre sin un archivo
settings-general-on-start-help = Qué mostrar cuando { -app-name } se abre por sí solo. Un archivo abierto desde el gestor de archivos o la línea de comandos siempre se muestra en el visor.
settings-general-on-start-start-screen = La pantalla de inicio
settings-general-on-start-gallery = Galería de la última carpeta
settings-general-on-start-library = Biblioteca
settings-general-start-tips = Mostrar «Para probar» en la pantalla de inicio
settings-general-start-tips-help = Una fila de tarjetas con funciones útiles y sus teclas. «Mostrar bienvenida» en la paleta de comandos la recupera.
settings-general-tips = Mostrar sugerencias
settings-general-tips-help = Una sugerencia breve, una sola vez, la primera vez que una función puede ayudar, como la tecla que muestra una carpeta como galería. Nunca durante una presentación.
settings-general-tips-shown = Sugerencias ya mostradas
settings-general-tips-shown-help = Las sugerencias únicas ya dadas, para no repetirlas.
# machine-drafted, needs native review
settings-general-your-name = Su nombre
settings-general-your-name-help = Aparece en los sellos y marcas que añada. Solo se guarda en los archivos de marcas que cree.
settings-general-single-instance = Abrir archivos en la ventana existente
settings-general-single-instance-help = Si { -app-name } ya está en ejecución, al abrir otra imagen se muestra en esa ventana en lugar de iniciar una nueva.
settings-general-file-associations = Asociaciones de archivos…
settings-general-file-associations-help = Hacer de { -app-name } el visor predeterminado para los formatos de imagen que admite.

## Visualización

settings-viewing-initial-zoom = Zoom inicial
settings-viewing-initial-zoom-help = Con qué tamaño se muestra una imagen recién abierta.
settings-viewing-initial-zoom-fit = Ajustar a la ventana
settings-viewing-initial-zoom-fit-if-larger = Ajustar las imágenes grandes y mostrar las pequeñas al { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Tamaño real ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Rellenar la ventana
settings-viewing-wheel-behaviour = Rueda del ratón
settings-viewing-wheel-behaviour-help = Qué hace la rueda al girarla. Mantenga pulsada Ctrl para la otra acción.
settings-viewing-wheel-behaviour-help-macos = Qué hace la rueda al girarla. Mantenga pulsada ⌘ para la otra acción.
settings-viewing-wheel-behaviour-zoom = Zoom
settings-viewing-wheel-behaviour-navigate = Imagen siguiente y anterior
settings-viewing-keep-zoom = Mantener el zoom entre imágenes
settings-viewing-keep-zoom-help = Con el zoom aplicado, la imagen siguiente y la anterior se abren con el mismo zoom y muestran la misma parte del encuadre: útil para revisar el enfoque de una ráfaga.
settings-viewing-zoom-animation = Animar el zoom
settings-viewing-zoom-animation-help = Hacer zoom de forma suave en lugar de a saltos. Se desactiva automáticamente cuando «Reducir movimiento» está activado.
settings-viewing-zoom-animation-speed = Velocidad de la animación del zoom
settings-viewing-zoom-animation-speed-help = Con qué rapidez se completan los zooms animados.
settings-viewing-zoom-animation-speed-relaxed = Pausada
settings-viewing-zoom-animation-speed-normal = Normal
settings-viewing-zoom-animation-speed-fast = Rápida
settings-viewing-background = Fondo
settings-viewing-background-help = El color detrás de la imagen en una ventana.
settings-viewing-background-theme = Según el tema
settings-viewing-background-black = Negro
settings-viewing-background-dark-grey = Gris oscuro
settings-viewing-background-light-grey = Gris claro
settings-viewing-background-white = Blanco
settings-viewing-background-custom = Color personalizado
settings-viewing-background-color = Color de fondo personalizado
settings-viewing-background-color-help = Se usa cuando Fondo está establecido en «Color personalizado».
settings-viewing-fullscreen-background = Fondo en pantalla completa
settings-viewing-fullscreen-background-help = El color detrás de la imagen en pantalla completa.
settings-viewing-transparency = Zonas transparentes
settings-viewing-transparency-help = Cómo se dibujan las partes transparentes de una imagen.
settings-viewing-transparency-checkerboard = Tablero de ajedrez
settings-viewing-transparency-background = Color de fondo
settings-viewing-transparency-black = Negro
settings-viewing-transparency-white = Blanco
settings-viewing-pixel-grid-threshold = Mostrar cuadrícula de píxeles a partir de
settings-viewing-pixel-grid-threshold-help = Dibujar una cuadrícula entre los píxeles cuando el zoom alcance al menos este valor.
settings-viewing-smoothing-high-zoom = Píxeles ampliados
settings-viewing-smoothing-high-zoom-help = Cómo se ven los píxeles cuando la imagen se muestra a más del { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Suavizados y nítidos a partir del { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Siempre suavizados
settings-viewing-smoothing-high-zoom-pixelated = Siempre cuadrados nítidos
settings-viewing-double-click = Doble clic en la imagen
settings-viewing-double-click-help = Hacer doble clic en el fondo vacío siempre activa o desactiva la pantalla completa.
settings-viewing-double-click-toggle-zoom = Alternar entre ajustado y { PERCENT(100) }
settings-viewing-double-click-fullscreen = Activar o desactivar pantalla completa
settings-viewing-double-click-nothing = No hacer nada
settings-viewing-fullscreen-on-open = Abrir imágenes en pantalla completa
settings-viewing-fullscreen-on-open-help = Iniciar en pantalla completa cuando se abre una imagen desde fuera de { -app-name }.
settings-viewing-fullscreen-mode = Estilo de pantalla completa
settings-viewing-fullscreen-mode-help = La pantalla completa nativa usa su propio espacio; la instantánea cubre el espacio actual sin animación.
settings-viewing-fullscreen-mode-native = Nativa (espacio propio)
settings-viewing-fullscreen-mode-borderless = Instantánea
settings-viewing-toolbar-autohide = Ocultar la barra de herramientas automáticamente
settings-viewing-toolbar-autohide-help = La barra de herramientas y las flechas laterales se desvanecen cuando el puntero está quieto y reaparecen cuando se mueve.
settings-viewing-toolbar-autohide-delay = Ocultar la barra de herramientas tras
settings-viewing-toolbar-autohide-delay-help = Cuánto tiempo debe estar quieto el puntero antes de que se desvanezca la barra de herramientas.
settings-viewing-toolbar-items = Botones de la barra de herramientas
settings-viewing-toolbar-items-help = Qué botones muestra la barra de herramientas y en qué orden.
settings-viewing-toolbar-drag = Reorganizar la barra de herramientas arrastrando
settings-viewing-toolbar-drag-help = Arrastre un botón a lo largo de la barra de herramientas para moverlo. Un clic breve sigue pulsándolo; Esc devuelve a su sitio un botón arrastrado.
settings-viewing-toolbar-items-open = Abrir
settings-viewing-toolbar-items-prev = Imagen anterior
settings-viewing-toolbar-items-next = Imagen siguiente
settings-viewing-toolbar-items-zoom-out = Alejar
settings-viewing-toolbar-items-zoom-level = Nivel de zoom
settings-viewing-toolbar-items-zoom-in = Acercar
settings-viewing-toolbar-items-fit = Ajustar a la ventana
settings-viewing-toolbar-items-rotate-left = Girar a la izquierda
settings-viewing-toolbar-items-rotate-right = Girar a la derecha
settings-viewing-toolbar-items-info = Información
settings-viewing-toolbar-items-copy = Copiar imagen
settings-viewing-toolbar-items-delete = Mover a la papelera
settings-viewing-toolbar-items-palette = Paleta de comandos
settings-viewing-toolbar-items-settings = Configuración
settings-viewing-toolbar-items-fullscreen = Pantalla completa
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Espacio flexible
settings-viewing-toolbar-items-separator = Separador
settings-toolbar-preview = Vista previa de la barra
settings-toolbar-hidden = Botones ocultos
settings-toolbar-add-spacer = Añadir espacio flexible
settings-toolbar-add-separator = Añadir separador
settings-toolbar-reset = Restablecer valores predeterminados
settings-toolbar-remove = Quitar { $item }
settings-toolbar-hint = Arrastre una fila para moverla, o selecciónela y pulse Alt+↑ o Alt+↓. Tab pasa entre el campo de búsqueda y la lista.
settings-toolbar-move-up = Subir
settings-toolbar-move-down = Bajar
settings-toolbar-move-start = Mover al principio
settings-toolbar-move-end = Mover al final
settings-toolbar-menu-hide = Ocultar
settings-toolbar-menu-show = Mostrar
settings-toolbar-menu-remove = Quitar
settings-toolbar-row-menu = Más acciones para { $item }
settings-toolbar-position = Posición { $index } de { $count }
settings-toolbar-hidden-state = Oculto

## Exploración

settings-browsing-sort-order = Ordenar imágenes por
settings-browsing-sort-order-help = El orden que se sigue al recorrer una carpeta. El orden por nombre coloca «img2» antes de «img10» y sigue el alfabeto de su idioma.
settings-browsing-sort-order-natural = Nombre
settings-browsing-sort-order-date-taken = Fecha de captura
settings-browsing-sort-order-modified = Fecha de modificación
settings-browsing-sort-order-size = Tamaño del archivo
settings-browsing-sort-order-type = Tipo de archivo
settings-browsing-sort-descending = Orden inverso
settings-browsing-sort-descending-help = Ordenar del último al primero.
settings-browsing-loop-folder = Volver al principio al final de la carpeta
settings-browsing-loop-folder-help = Después de la última imagen, continuar con la primera, y viceversa.
settings-browsing-include-subfolders = Incluir subcarpetas
settings-browsing-include-subfolders-help = Recorrer también las imágenes de las subcarpetas.
settings-browsing-hold-acceleration = Aceleración al mantener pulsado siguiente o anterior
settings-browsing-hold-acceleration-help = Con qué rapidez se acelera el paso de imágenes mientras se mantiene pulsada una tecla o un botón.
settings-browsing-hold-acceleration-off = Sin aceleración
settings-browsing-hold-acceleration-gentle = Suave
settings-browsing-hold-acceleration-normal = Normal
settings-browsing-hold-acceleration-fast = Rápida
settings-browsing-confirm-trash = Preguntar antes de mover a la papelera
settings-browsing-confirm-trash-help = Mover a la papelera siempre se puede deshacer, por lo que { -app-name } no pregunta de forma predeterminada.
settings-browsing-arrows-follow-reading-direction = Las teclas de flecha siguen la dirección de lectura
settings-browsing-arrows-follow-reading-direction-help = En idiomas de derecha a izquierda, la tecla de flecha izquierda pasa a la imagen siguiente.

## Color

settings-colour-management = Gestión del color
settings-colour-management-help = Mostrar los colores tal como deben verse, usando el perfil de color de la imagen y el de la pantalla.
settings-colour-display-profile = Perfil de la pantalla
settings-colour-display-profile-help = El perfil de color de la pantalla. «Automático» usa el que indica el sistema operativo.
settings-colour-rendering-intent = Intento de reproducción
settings-colour-rendering-intent-help = Cómo se llevan a su gama los colores que la pantalla no puede mostrar.
settings-colour-rendering-intent-perceptual = Perceptual
settings-colour-rendering-intent-relative-colorimetric = Colorimétrico relativo
settings-colour-rendering-intent-saturation = Saturación
settings-colour-rendering-intent-absolute-colorimetric = Colorimétrico absoluto
settings-colour-black-point-compensation = Compensación de punto negro
settings-colour-black-point-compensation-help = Conservar el detalle en las sombras cuando el negro de la pantalla es más claro que el de la imagen.
settings-colour-untagged-images = Imágenes sin perfil de color
settings-colour-untagged-images-help = Cómo mostrar las imágenes que no indican qué colores usan.
settings-colour-untagged-images-srgb = Tratar como sRGB
settings-colour-untagged-images-display = Mostrar sin cambios
settings-colour-dithering = Difuminado
settings-colour-dithering-help = Añadir un ruido azul fino al reducir la profundidad de color, lo que evita las bandas en cielos y degradados.
settings-colour-hdr-output = Salida HDR
settings-colour-hdr-output-help = Mostrar las imágenes HDR con todo su brillo en las pantallas que lo admiten. Automático usa HDR cuando la pantalla indica brillo adicional; si no, las zonas claras se comprimen suavemente.
settings-colour-hdr-output-auto = Automático
settings-colour-hdr-output-on = Activada
settings-colour-hdr-output-off = Desactivada
# machine-drafted, needs native review
settings-colour-ocio-config = Configuración OCIO
settings-colour-ocio-config-help = Una configuración OpenColorIO (.ocio) para imágenes EXR, Radiance HDR y PFM. Si está vacía, se usa la que indica $OCIO.
settings-colour-ocio-display = Pantalla OCIO
settings-colour-ocio-display-help = La pantalla de la configuración OCIO que corresponde a tu monitor.
settings-colour-ocio-display-default = La primera de la configuración
settings-colour-ocio-view = Vista OCIO
settings-colour-ocio-view-help = Cómo se muestran las imágenes en escena lineal (scene-linear) en esa pantalla, por ejemplo con una vista con mapeo de tonos.
settings-colour-ocio-view-default = La primera de la pantalla
settings-colour-ocio-input = Espacio de color de entrada OCIO
settings-colour-ocio-input-help = El espacio de color de los valores de las imágenes.
settings-colour-ocio-input-default = El rol scene-linear de la configuración
# machine-drafted, needs native review
settings-colour-proof-profile = Perfil de prueba en pantalla
settings-colour-proof-profile-help = El perfil de impresora que la prueba en pantalla de Revelar usó por última vez.
settings-colour-proof-intent = Intención de la prueba en pantalla
settings-colour-proof-intent-help = Cómo se ajustan los colores que la impresora no puede imprimir.
settings-colour-proof-intent-perceptual = Perceptual
settings-colour-proof-intent-relative = Colorimétrica relativa
settings-colour-proof-bpc = Compensación de punto negro en la prueba
settings-colour-proof-bpc-help = Lleva el negro al negro de la impresora.
settings-colour-proof-paper = Blanco del papel en la prueba
settings-colour-proof-paper-help = Muestra el color propio del papel.
settings-colour-proof-ink = Negro de la tinta en la prueba
settings-colour-proof-ink-help = Muestra lo oscuro que es el negro de la tinta.

## Metadatos y privacidad

settings-metadata-show-info-on-open = Mostrar el panel de información al abrir
settings-metadata-show-info-on-open-help = Abrir el panel con los datos de la cámara, la fecha y el lugar cada vez que se abre una imagen.
settings-metadata-info-detail = El panel de información muestra
settings-metadata-info-detail-help = El resumen explica en lenguaje sencillo los datos más útiles; «Todos los campos» enumera todo lo que hay almacenado en el archivo.
settings-metadata-info-detail-summary = Resumen
settings-metadata-info-detail-all = Todos los campos
settings-metadata-reverse-geocoding = Mostrar nombres de lugares para las posiciones GPS
settings-metadata-reverse-geocoding-help = Buscar la localidad más cercana en los datos integrados. Funciona sin conexión: las posiciones nunca salen del equipo.
settings-metadata-hide-gps = Ocultar las posiciones GPS en pantalla
settings-metadata-hide-gps-help = No mostrar coordenadas ni lugares, por ejemplo al compartir la pantalla. Los archivos no se modifican.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Guardar los ajustes de Revelar junto a la foto
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Escribe IMG_1234.CR2.specular.xmp junto al archivo para que los ajustes viajen con él. Desactivado: los ajustes quedan solo en la biblioteca.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importar ajustes de Lightroom en Revelar
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Una foto con ajustes de Lightroom y sin edición se abre con ellos, de forma aproximada. Desactivado: Revelar ofrece un botón.
settings-metadata-units = Unidades
settings-metadata-units-help = Unidades de altitud, distancia y velocidad.
settings-metadata-units-auto = Según los formatos regionales
settings-metadata-units-metric = Métrico
settings-metadata-units-imperial = Imperial
# machine-drafted, needs native review
settings-metadata-basemap = Mapa base
settings-metadata-basemap-help = El mapa bajo las ubicaciones de las fotos. Los mapas en línea descargan mosaicos de la zona que se ve.
settings-metadata-basemap-off = Solo mapa integrado (sin conexión)
settings-metadata-basemap-openstreetmap = OpenStreetMap (en línea)
settings-metadata-basemap-custom = Otro servidor de mosaicos (en línea)
settings-metadata-basemap-file = Mi propio archivo de mapa
settings-metadata-basemap-url = Dirección del servidor de mosaicos
settings-metadata-basemap-url-help = Una dirección con {"{"}z{"}"}, {"{"}x{"}"} y {"{"}y{"}"}, por ejemplo https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Archivo de mapa
settings-metadata-basemap-path-help = Un archivo MBTiles o PMTiles. Funciona sin conexión.
settings-metadata-basemap-cache = Caché de mosaicos de mapa
settings-metadata-basemap-cache-help = Espacio en disco para los mosaicos descargados. Los más antiguos se eliminan primero.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Servidores de mosaicos permitidos
settings-metadata-basemap-allowed-help = Servidores de mapas de los que aceptó descargar mosaicos.

## Rendimiento

settings-performance-ram-cache = Caché en memoria
settings-performance-ram-cache-help = Memoria usada para mantener listas las imágenes decodificadas. «Automático» elige según la cantidad de memoria del equipo.
settings-performance-vram-cache = Caché en memoria de GPU
settings-performance-vram-cache-help = Memoria de la GPU usada para imágenes y mosaicos. «Automático» elige según la GPU.
settings-performance-prefetch-depth = Imágenes que se preparan por adelantado
settings-performance-prefetch-depth-help = Cuántas imágenes se decodifican por adelantado en la dirección en la que se avanza.
settings-performance-decoder-threads = Hilos de decodificación
settings-performance-decoder-threads-help = Hilos de la CPU usados para decodificar imágenes. «Automático» usa todos los núcleos de rendimiento y se adapta al archivo.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Qué GPU usar cuando el equipo tiene más de una.
settings-performance-gpu-adapter-high-performance = Máximo rendimiento
settings-performance-gpu-adapter-low-power = Mínimo consumo
settings-performance-processing = Dónde se procesan las imágenes
settings-performance-processing-help = Algunas tareas pueden hacerse en la CPU, en la GPU o en un decodificador por hardware. Automático mide este equipo y elige el camino más rápido para la imagen que está viendo y el más económico para el trabajo en segundo plano. Lo que se eligió en este equipo, y por qué, se muestra más abajo.
settings-performance-processing-automatic = Automático
settings-performance-processing-prefer-gpu = Preferir la GPU
settings-performance-processing-cpu-only = Solo la CPU
settings-performance-drag-preview = Vista previa al arrastrar
settings-performance-drag-preview-help = Cómo muestra Revelar la foto mientras arrastra un control deslizante. Automático usa una imagen más pequeña solo cuando el equipo no da abasto.
settings-performance-drag-preview-automatic = Automático
settings-performance-drag-preview-sharp = Siempre nítida
settings-performance-drag-preview-fast = Siempre rápida

## Settings → Performance → How { -app-name } runs on this computer (DESIGN.md §12.3.1)
# machine-drafted, needs native review
settings-paths-title = Cómo funciona { -app-name } en este equipo
settings-paths-intro = { -app-name } cronometra cada forma de hacer estas tareas en este equipo y usa la mejor. Los tiempos son para una foto de { $mpx } megapíxeles.
settings-paths-intro-measuring = Aún no ha terminado de medir: { -app-name } mide mientras el equipo está inactivo.
settings-paths-intro-battery = Con batería: las elecciones favorecen un menor consumo de energía.
settings-paths-keywords = GPU CPU NPU tarjeta gráfica hardware aceleración decodificador benchmark velocidad
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, todos los núcleos
paths-engine-gpu = GPU
paths-engine-hw-decoder = Decodificador por hardware
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (su ajuste)
settings-paths-measured-faster = Medido en este equipo: { $ratio } veces más rápido que con { $other }.
settings-paths-measured-similar = Medido en este equipo: más o menos igual de rápido que con { $other }.
settings-paths-measured-power = Con batería, es lo que menos energía consume (medido en este equipo).
settings-paths-not-measured = Aún sin medir: la elección habitual para este tipo de equipo.
settings-paths-set-by = Elegido por su ajuste «{ $setting }: { $option }».
settings-paths-blocked-check = { $engine }: se desactivó el { $date } porque su resultado no coincidía con el de la CPU.
settings-paths-blocked-error = { $engine }: se desactivó el { $date } tras un error.
settings-paths-blocked-lost = { $engine }: se desactivó el { $date } porque dejó de responder.
settings-paths-blocked-undated = { $engine }: se desactivó tras un error.
settings-paths-checking = { $engine }: se compara con la CPU antes de usarse para la imagen que está viendo.
settings-paths-exploring = Probando ahora en segundo plano: { $engine }.
settings-paths-unavailable = Este equipo no tiene { $engine } para esto.
settings-paths-background-cheaper = En segundo plano: { $engine }, que según lo medido usa un { $percent } menos de tiempo de CPU.
settings-paths-background-power = En segundo plano: { $engine }, para ahorrar energía.
settings-paths-background = En segundo plano: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms de CPU, { $runs ->
    [one] { $runs } medición
   *[other] { $runs } mediciones
} ({ $confidence })
settings-paths-detail-guess = { $engine }: unos { $time } ms (estimación)
settings-paths-confidence-low = confianza baja
settings-paths-confidence-medium = confianza media
settings-paths-confidence-high = confianza alta
settings-paths-machine = Medido para { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Reintentar
settings-paths-measure-again = Volver a medir
settings-paths-measure-again-question = ¿Olvidar lo medido en este equipo, incluidas las vías desactivadas, y volver a medir cuando el equipo esté inactivo?
settings-paths-measure-again-confirm = Olvidar mediciones

settings-performance-display-cache = Mantener una caché de visualización en el disco
settings-performance-display-cache-help = Guardar copias de las imágenes al tamaño de la pantalla para que las carpetas ya vistas se abran al instante.
settings-performance-display-cache-size = Tamaño de la caché de visualización
settings-performance-display-cache-size-help = Las entradas más antiguas se eliminan cuando la caché está llena.
settings-performance-display-cache-location = Ubicación de la caché de visualización
settings-performance-display-cache-location-help = Lo ideal es una carpeta en una unidad rápida.
settings-performance-fast-simd = Usar instrucciones SIMD
settings-performance-fast-simd-help = Usar las instrucciones vectoriales de la CPU para decodificar. Desactívelo solo para diagnosticar un problema.
settings-performance-fast-dct-scaling = Decodificar al tamaño de la pantalla
settings-performance-fast-dct-scaling-help = Decodificar los JPEG grandes directamente al tamaño con el que se muestran. Desactívelo solo para diagnosticar un problema.
settings-performance-fast-parallel-decode = Decodificar en varios núcleos
settings-performance-fast-parallel-decode-help = Repartir las imágenes grandes entre los núcleos de la CPU. Desactívelo solo para diagnosticar un problema.
settings-performance-fast-restart-index = Indexar los JPEG grandes para el zoom
settings-performance-fast-restart-index-help = Recordar dónde empieza cada parte de un JPEG para que las zonas ampliadas se decodifiquen rápidamente. Desactívelo solo para diagnosticar un problema.
settings-performance-fast-yuv-upload = Convertir los colores en la GPU
settings-performance-fast-yuv-upload-help = Convertir los colores JPEG en la GPU o en la CPU. Automático elige lo que sea más rápido en este equipo. Desactívelo solo para diagnosticar un problema.
settings-performance-fast-yuv-upload-auto = Automático
settings-performance-fast-yuv-upload-on = Activado
settings-performance-fast-yuv-upload-off = Desactivado
settings-performance-fast-async-io = Lectura asíncrona de archivos
settings-performance-fast-async-io-help = Leer por adelantado con la interfaz de archivos más rápida del sistema operativo. Desactívelo solo para diagnosticar un problema.
settings-performance-fast-mmap = Archivos asignados en memoria
settings-performance-fast-mmap-help = Leer los archivos sin copiarlos. Desactívelo solo para diagnosticar un problema.
settings-performance-hardware-decode = Decodificación por hardware
settings-performance-hardware-decode-help = Abrir las fotos JPEG con el decodificador integrado de la GPU. Automático lo usa donde ayuda, normalmente para imágenes preparadas en segundo plano. Primero se compara con el decodificador normal, y lo que no admita se abre de la forma habitual.
settings-performance-hardware-decode-auto = Automático
settings-performance-hardware-decode-on = Activado
settings-performance-hardware-decode-off = Desactivado
settings-performance-frame-time-hud = HUD de tiempo de fotograma
settings-performance-frame-time-hud-help = Mostrar los tiempos de decodificación y de fotograma, y qué rutas rápidas están en uso.

## Accesibilidad

settings-accessibility-reduce-motion = Reducir movimiento
settings-accessibility-reduce-motion-help = Sustituir las animaciones de zoom, deslizamiento y fundido por cambios instantáneos.
settings-accessibility-reduce-motion-system = Según el sistema
settings-accessibility-reduce-motion-on = Activado
settings-accessibility-reduce-motion-off = Desactivado
settings-accessibility-increase-contrast = Aumentar contraste
settings-accessibility-increase-contrast-help = Bordes y texto más marcados, y menos transparencia.
settings-accessibility-increase-contrast-system = Según el sistema
settings-accessibility-increase-contrast-on = Activado
settings-accessibility-increase-contrast-off = Desactivado
settings-accessibility-text-scale = Tamaño del texto
settings-accessibility-text-scale-help = Tamaño del texto con respecto al tamaño estándar.
settings-accessibility-ui-scale = Escala de la interfaz
settings-accessibility-ui-scale-help = Tamaño de toda la interfaz, además del escalado de pantalla establecido en el sistema operativo.
settings-accessibility-focus-ring-always = Mostrar siempre el indicador de foco
settings-accessibility-focus-ring-always-help = Mostrar qué control tiene el foco del teclado incluso al usar el ratón.
settings-accessibility-screen-reader-verbosity = Nivel de detalle del lector de pantalla
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Cuánta información anuncia el lector de pantalla al pasar de una imagen a otra y cuánto de la lectura bajo el puntero lee.
settings-accessibility-screen-reader-verbosity-concise = Breve
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Detallado
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Leer el valor bajo el puntero
settings-accessibility-announce-cursor-help = El lector de pantalla lee el valor del píxel, las HU, la temperatura o la posición bajo el puntero o la cruz de teclado cuando deja de moverse.

## Privacidad

# machine-drafted, needs native review
settings-privacy-no-network = Sin red
# machine-drafted, needs native review
settings-privacy-no-network-help = Desactivado: solo se conectan estos, cada uno al activarlo o abrir algo con él: la búsqueda de actualizaciones, la descarga de modelos de IA local, los mapas en línea, las imágenes desde una dirección web o almacenamiento en la nube y los servidores que añada (DICOMweb, OMERO, mapas). Activado: nada se conecta.

## Avanzado

settings-advanced-portable-mode = Modo portátil
settings-advanced-portable-mode-help = Guardar la configuración y las cachés en la carpeta del programa en lugar de en el perfil de usuario.
settings-advanced-log-level = Nivel de detalle del registro
settings-advanced-log-level-help = Cuánta información escribe { -app-name } en su archivo de registro.
settings-advanced-log-level-error = Solo errores
settings-advanced-log-level-warn = Advertencias
settings-advanced-log-level-info = Información
settings-advanced-log-level-debug = Depuración
settings-advanced-log-level-trace = Todo
settings-advanced-developer-mode = Modo de desarrollador
settings-advanced-developer-mode-help = Muestra herramientas para desarrollar y probar { -app-name }, como los pseudoidiomas de la lista de idiomas.
# machine-drafted, needs native review
settings-advanced-audit-log = Registro de auditoría local
# machine-drafted, needs native review
settings-advanced-audit-log-help = Registra las imágenes médicas abiertas, las exportaciones, las anonimizaciones, las impresiones y los envíos a un PACS en audit.jsonl, en la carpeta de datos. Se queda en este equipo.
settings-advanced-hw-report = Informe de hardware…
settings-advanced-hw-report-help = Mostrar qué funciones de la CPU y de la GPU ha detectado { -app-name } y qué rutas rápidas usa.
# machine-drafted, needs native review
settings-advanced-renderer-info = Información del renderizador…
settings-advanced-renderer-info-help = Muestra el adaptador gráfico, el controlador, los formatos de pantalla y los límites de memoria que usa { -app-name }, listos para copiar en un informe de errores.
settings-advanced-edit-settings-file = Editar settings.toml
settings-advanced-edit-settings-file-help = Edite el archivo de ajustes en el editor propio de { -app-name }. Comprueba el archivo mientras escribe y solo lo guarda cuando no tiene errores.
settings-advanced-import = Importar configuración…
settings-advanced-import-help = Cargar la configuración desde un archivo exportado de { -app-name }.
settings-advanced-export = Exportar configuración…
settings-advanced-export-help = Guardar toda la configuración en un único archivo, como copia de seguridad o para otro equipo.
settings-advanced-reset-all = Restablecer toda la configuración…
settings-advanced-reset-all-help = Devolver todos los ajustes a sus valores predeterminados. Se puede deshacer.

# Stage 1 merge (machine-drafted, needs native review)
settings-performance-stall-warning = Informar de cargas bloqueadas tras
settings-performance-stall-warning-help = Si una imagen sigue cargándose pasado este tiempo, se escriben en el registro su estado de carga y las tareas pendientes (salida de error estándar o SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Consejos de rendimiento
settings-performance-insights-help = Cuando las imágenes siguen abriéndose despacio por el mismo motivo, mostrar «¿Por qué va lento?» en la esquina con lo que ayudaría. Lo que aprende se queda en este equipo.
settings-performance-insights-storage = Consejos sobre almacenamiento lento
settings-performance-insights-storage-help = Mostrar consejos cuando la lectura de archivos (desde una carpeta compartida en red, una unidad lenta o archivos comprimidos) sigue retrasando la apertura de imágenes.
settings-performance-insights-decoder = Consejos sobre la decodificación
settings-performance-insights-decoder-help = Mostrar consejos cuando las imágenes se siguen decodificando en la CPU aunque un decodificador por hardware podría ayudar.
# machine-drafted, needs native review
settings-performance-insights-restore = Volver a mostrar los consejos ocultos
settings-performance-insights-restore-help = Recupera los consejos que ocultaste con × en «¿Por qué va lento?», tanto al abrir imágenes como en las miniaturas. No disponible mientras no haya ninguno oculto.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Guardar
config-editor-revert = Revertir
config-editor-close = Cerrar
config-editor-modified = Modificado
config-editor-text = Archivo de ajustes
config-editor-position = Línea { $line }, columna { $column }
config-editor-saved = settings.toml guardado y aplicado.
config-editor-cant-save =
    { $count ->
        [one] No se puede guardar: 1 problema. Corríjalo primero.
       *[other] No se puede guardar: { $count } problemas. Corríjalos primero.
    }
config-editor-save-failed = No se puede guardar settings.toml: { $reason }
config-editor-conflict = settings.toml ha cambiado en otro lugar.
config-editor-reload = Recargar
config-editor-keep-mine = Conservar los míos
config-editor-discard-question = ¿Descartar los cambios sin guardar en settings.toml?
config-editor-discard = Descartar
config-editor-keep-editing = Seguir editando
config-editor-find-placeholder = Buscar en el archivo
config-editor-find-count = { $current } de { $total }
config-editor-find-none = Sin coincidencias
config-editor-find-previous = Coincidencia anterior
config-editor-find-next = Coincidencia siguiente
config-editor-find-close = Cerrar búsqueda
config-editor-problems = Problemas
config-editor-problems-count =
    { $count ->
        [0] Sin problemas
        [one] 1 problema
       *[other] { $count } problemas
    }
config-editor-no-problems = Sin problemas. El archivo se puede guardar.
config-editor-problem-at = Línea { $line }: { $message }
config-editor-severity-error = Error
config-editor-severity-warning = Advertencia
config-editor-severity-info = Nota
config-editor-problem-syntax = TOML no válido: { $reason }
config-editor-problem-duplicate = { $key } está definido dos veces en la misma tabla. Elimine uno de ellos.
config-editor-problem-unknown-section = No existe ninguna sección de ajustes [{ $key }].
config-editor-problem-unknown-section-suggestion = No existe ninguna sección de ajustes [{ $key }]. ¿Quería decir [{ $suggestion }]?
config-editor-problem-unknown-key = No existe ningún ajuste { $key }.
config-editor-problem-unknown-key-suggestion = No existe ningún ajuste { $key }. ¿Quería decir { $suggestion }?
config-editor-problem-invalid = Valor no válido para { $key }: { $reason }
config-editor-problem-migrated = { $key } está escrito como lo escribía una versión anterior de { -app-name }. Se convierte al leerlo.
config-editor-problem-newer-version = Este archivo lo escribió una versión más reciente de { -app-name } (formato { $found }; esta versión lee { $supported }). Los ajustes que no conoce se conservan.
config-editor-problem-command = Comando { $index } en [[commands]]: { $reason }
config-editor-problem-hook = Gancho { $index } en [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Archivo de atajos de teclado
config-editor-keymap-saved = keymap.toml guardado y aplicado.
config-editor-keymap-save-failed = No se puede guardar keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml cambió en otro lugar.
config-editor-keymap-discard-question = ¿Descartar los cambios sin guardar en keymap.toml?
config-editor-new-file = archivo nuevo, se crea al guardar
config-editor-no-folder = no hay una carpeta de ajustes donde guardarlo.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = ¿Restablecer todos los ajustes a sus valores predeterminados?
settings-reset-all-confirm-detail = Afecta a todas las secciones de Configuración. Se puede deshacer después.
settings-reset-all-confirm-ok = Restablecer todo
settings-reset-all-done = Todos los ajustes se restablecieron a sus valores predeterminados.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Codificador por hardware

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = { $engine }: se compara con la CPU antes de usarse.
settings-paths-unit-thumbnail = Los tiempos son para una miniatura de { $size }
settings-paths-unit-frame = Los tiempos son por fotograma de vídeo de { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Carpeta de fotogramas de calibración
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flats y darks para Revelar. Sus nombres de archivo o carpeta contienen «flat» o «dark».

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Blanco
settings-colour-light-grey = Gris claro
settings-colour-grey = Gris
settings-colour-dark-grey = Gris oscuro
settings-colour-black = Negro
settings-colour-yellow = Amarillo
settings-colour-orange = Naranja
settings-colour-red = Rojo
settings-colour-magenta = Magenta
settings-colour-blue = Azul
settings-colour-cyan = Cian
settings-colour-green = Verde
