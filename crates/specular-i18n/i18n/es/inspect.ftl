# machine-drafted, needs native review
# Herramientas de inspección: recorte, focus peaking, histograma, inspector de píxeles.

## Paleta de comandos
palette-clipping = Mostrar u ocultar avisos de recorte
palette-focus-peaking = Mostrar u ocultar el focus peaking
palette-histogram = Mostrar u ocultar el histograma
palette-pixel-inspector = Mostrar u ocultar el inspector de píxeles
# machine-drafted, needs native review
palette-crosshair = Cruz de teclado
palette-announce-cursor = Leer el valor bajo el puntero

## Histograma
histogram-title = Histograma
histogram-clipped = Recorte: altas luces { $high }, sombras { $low }
histogram-empty = No hay imagen visible
histogram-a11y = Histograma de la imagen visible: brillo medio { $mean }, altas luces recortadas { $high }, sombras recortadas { $low }

## Inspector de píxeles
inspector-title = Inspector de píxeles
inspector-position = Posición
inspector-source-rgb = Origen RGB
inspector-source-rgba = Origen RGBA
inspector-source-ycc = Origen Y′CbCr
inspector-source-gray = Origen gris
inspector-source-gray16 = Origen gris (16 bits)
inspector-source-gray-alpha16 = Origen gris + alfa (16 bits)
inspector-source-rgb16 = Origen RGB (16 bits)
inspector-source-rgba16 = Origen RGBA (16 bits)
inspector-source-pending = Origen
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Lineal
inspector-approximate = De una vista previa reducida; amplíe para obtener valores exactos
inspector-outside = Señale la imagen para inspeccionar un píxel
inspector-a11y = Píxel en { $x }, { $y }: sRGB { $value }

## Configuración
settings-section-inspection = Inspección
settings-inspection-clip-highlight = Nivel de recorte de altas luces
settings-inspection-clip-highlight-help = Los píxeles con algún canal igual o superior a este valor de 8 bits se marcan como altas luces recortadas.
settings-inspection-clip-shadow = Nivel de recorte de sombras
settings-inspection-clip-shadow-help = Los píxeles con algún canal igual o inferior a este valor de 8 bits se marcan como sombras recortadas.
settings-inspection-highlight-color = Color de aviso de altas luces
settings-inspection-highlight-color-help = Color que marca las altas luces recortadas.
settings-inspection-shadow-color = Color de aviso de sombras
settings-inspection-shadow-color-help = Color que marca las sombras recortadas.
settings-inspection-peaking-sensitivity = Sensibilidad del focus peaking
settings-inspection-peaking-sensitivity-help = Los valores más altos marcan también bordes más suaves como enfocados.
settings-inspection-peaking-color = Color del focus peaking
settings-inspection-peaking-color-help = Color que marca los bordes nítidos.
settings-inspection-histogram-channels = Canales del histograma
settings-inspection-histogram-channels-help = Qué curvas muestra el histograma.
settings-inspection-histogram-channels-colour = Rojo, verde y azul
settings-inspection-histogram-channels-luminance = Luminancia
settings-inspection-histogram-channels-both = Color y luminancia
settings-inspection-histogram-log = Histograma logarítmico
settings-inspection-histogram-log-help = Escalar el histograma de forma logarítmica para que los valores pequeños sigan visibles.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Mostrar el canal rojo
palette-channel-green = Mostrar el canal verde
palette-channel-blue = Mostrar el canal azul
palette-channel-alpha = Mostrar el canal alfa (otra vez: sobre damero, luego sobre un color)
palette-channel-luma = Mostrar la luminancia
channel-normal = Todos los canales
channel-red = Canal rojo
channel-green = Canal verde
channel-blue = Canal azul
channel-alpha = Canal alfa
channel-alpha-checker = Transparencia sobre damero
channel-alpha-matte = Transparencia sobre color
channel-luma = Luminancia
disabled-no-alpha = Esta imagen no tiene transparencia
settings-inspection-alpha-matte-color = Color de fondo del alfa
settings-inspection-alpha-matte-color-help = Color sólido detrás de la imagen en la vista «Transparencia sobre color».

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Mostrar u ocultar el falso color
palette-next-colour-map = Siguiente mapa de color
palette-channel-colours = Siguientes colores de canal (rojo–verde, magenta–verde, magenta–amarillo–cian)
palette-hilo = Mostrar u ocultar HiLo (píxeles en el máximo y el mínimo de la profundidad de bits)
palette-colour-vision = Siguiente simulación de visión del color
palette-colour-vision-off = Dejar de simular la visión del color
palette-colour-vision-protanopia = Simular protanopía (sin conos rojos)
palette-colour-vision-deuteranopia = Simular deuteranopía (sin conos verdes)
palette-colour-vision-tritanopia = Simular tritanopía (sin conos azules)
palette-colour-vision-achromatopsia = Simular acromatopsia (sin color)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (no apto para daltonismo)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = valores en bruto
# Screen reader summary of the colour bar.
colour-bar-a11y = Barra de color { $map }: de { $low } a { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Falso color: { $map }
badge-hilo = HiLo: marcando píxeles en el máximo y el mínimo
badge-colour-vision-protanopia = Simulando protanopía
badge-colour-vision-deuteranopia = Simulando deuteranopía
badge-colour-vision-tritanopia = Simulando tritanopía
badge-colour-vision-achromatopsia = Simulando acromatopsia
channel-colours-red-green = Canales: rojo, verde, azul
channel-colours-magenta-green = Canales: magenta, verde
channel-colours-magenta-yellow-cyan = Canales: magenta, amarillo, cian
settings-inspection-false-colour-map = Mapa de falso color
settings-inspection-false-colour-map-help = Mapa de color para el falso color y su barra de color. Todos salvo jet y phase son legibles con cualquier visión del color.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergente)
settings-inspection-false-colour-map-vik = Vik (divergente)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (divergente)
settings-inspection-false-colour-map-curl = Curl (divergente)
settings-inspection-false-colour-map-phase = Phase (cíclica)
settings-inspection-false-colour-map-jet = Jet (no apto para daltonismo)
settings-inspection-colour-bar = Barra de color
settings-inspection-colour-bar-help = Mostrar una barra de color con valores en las unidades de la imagen mientras el falso color está activo.
settings-inspection-channel-colours = Colores de canal
settings-inspection-channel-colours-help = Cómo se muestran los canales de color. Magenta–verde y magenta–amarillo–cian siguen distinguiéndose con daltonismo rojo–verde.
settings-inspection-channel-colours-red-green = Rojo, verde, azul
settings-inspection-channel-colours-magenta-green = Magenta, verde
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, amarillo, cian
settings-inspection-colour-vision = Simulación de visión del color
settings-inspection-colour-vision-help = Mostrar toda la ventana como se ve con una deficiencia de visión del color (Machado et al. 2009). Las exportaciones nunca se ven afectadas.
settings-inspection-colour-vision-off = Desactivada
settings-inspection-colour-vision-protanopia = Protanopía
settings-inspection-colour-vision-deuteranopia = Deuteranopía
settings-inspection-colour-vision-tritanopia = Tritanopía
settings-inspection-colour-vision-achromatopsia = Acromatopsia
settings-inspection-auto-range = Autorrango de grises de alta profundidad
settings-inspection-auto-range-help = Estirar las imágenes en escala de grises de 16 bits (TIFF, PNG, FITS) de su valor más oscuro al más claro. Las imágenes DICOM conservan su propia ventana.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Cruz activada. Las flechas la mueven, Mayús para pasos mayores. Intro hace clic. Esc termina.
crosshair-off = Cruz desactivada
announce-cursor-nothing = Nada bajo el puntero
