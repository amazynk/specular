# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Español.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Un módulo de esta edición se actualizó a una versión más reciente. El resultado puede verse algo distinto.
    [many] { $count } módulos de esta edición se actualizaron a una versión más reciente. El resultado puede verse algo distinto.
   *[other] { $count } módulos de esta edición se actualizaron a una versión más reciente. El resultado puede verse algo distinto.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Un módulo de esta edición requiere una versión más reciente de Specular. Se conserva, pero no se muestra.
    [many] { $count } módulos de esta edición requieren una versión más reciente de Specular. Se conservan, pero no se muestran.
   *[other] { $count } módulos de esta edición requieren una versión más reciente de Specular. Se conservan, pero no se muestran.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Punto { $index } de { $count }: entrada { $input }, salida { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Las flechas mueven el punto. Re Pág y Av Pág eligen otro. Insert añade un punto, Supr lo quita.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutro
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Tono { $hue }°, fuerza { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Las flechas mueven el color. Supr lo restablece.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Revelar
# machine-drafted, needs native review
develop-undo = Deshacer
# machine-drafted, needs native review
develop-redo = Rehacer
# machine-drafted, needs native review
develop-before-after = Antes y después
# machine-drafted, needs native review
develop-close = Salir de Revelar
# machine-drafted, needs native review
develop-history = Historia
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } activado
develop-card-menu = Opciones de { $module }
develop-card-changed = Modificado
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Tira de película

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Exposición
develop-module-light = Luz
develop-look-profile = Perfil
develop-look-standard = Estándar
develop-look-film = Película
develop-look-natural = Natural
develop-module-grading = Gradación de color
develop-add-tool = Añadir una herramienta
# machine-drafted, needs native review
develop-module-white-balance = Balance de blancos
# machine-drafted, needs native review
develop-module-tone = Tono
# machine-drafted, needs native review
develop-module-contrast = Contraste y rango
# machine-drafted, needs native review
develop-module-colour = Color
# machine-drafted, needs native review
develop-module-curve = Curva
# machine-drafted, needs native review
develop-module-crop = Recortar y enderezar
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Otro módulo
# machine-drafted, needs native review
develop-exposure = Exposición
# machine-drafted, needs native review
develop-black-level = Nivel de negro
# machine-drafted, needs native review
develop-temperature = Temperatura
# machine-drafted, needs native review
develop-tint = Matiz
# machine-drafted, needs native review
develop-highlights = Iluminaciones
# machine-drafted, needs native review
develop-shadows = Sombras
# machine-drafted, needs native review
develop-contrast = Contraste
# machine-drafted, needs native review
develop-whites = Blancos
# machine-drafted, needs native review
develop-blacks = Negros
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = Límite HDR
# machine-drafted, needs native review
develop-hdr-preview = Mostrar
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Intensidad
# machine-drafted, needs native review
develop-saturation = Saturación
# machine-drafted, needs native review
develop-colour-wheels = Ruedas de color
# machine-drafted, needs native review
develop-curve = Curva de tonos
# machine-drafted, needs native review
develop-module-local-contrast = Contraste local
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Exposición automática
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Suavizado
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Textura
# machine-drafted, needs native review
develop-clarity = Claridad
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Punto negro
# machine-drafted, needs native review
develop-levels-mid = Punto medio
# machine-drafted, needs native review
develop-levels-white = Punto blanco
# machine-drafted, needs native review
develop-module-detail = Detalle
# machine-drafted, needs native review
develop-module-denoise = Reducción de ruido
# machine-drafted, needs native review
develop-module-sharpen = Enfoque
# machine-drafted, needs native review
develop-noise-luminance = Ruido de luminancia
# machine-drafted, needs native review
develop-noise-detail = Detalle del ruido
# machine-drafted, needs native review
develop-noise-colour = Ruido de color
# machine-drafted, needs native review
develop-sharpen-amount = Cantidad de enfoque
# machine-drafted, needs native review
develop-sharpen-radius = Radio de enfoque
# machine-drafted, needs native review
develop-sharpen-detail = Detalle de enfoque
# machine-drafted, needs native review
develop-sharpen-masking = Máscara
# machine-drafted, needs native review
develop-capture-amount = Enfoque de captura
# machine-drafted, needs native review
develop-capture-radius = Radio de captura
# machine-drafted, needs native review
develop-straighten = Enderezar
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Recortar
# machine-drafted, needs native review
develop-crop-turn = Girar
# machine-drafted, needs native review
develop-crop-flip = Voltear
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Correcciones de lente
develop-module-optics = Óptica
# machine-drafted, needs native review
develop-lens-distortion = Distorsión
# machine-drafted, needs native review
develop-lens-vignette = Viñeteado
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Franja roja
# machine-drafted, needs native review
develop-lens-tca-blue = Franja azul
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Perfil de lente
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automático
# machine-drafted, needs native review
develop-lensfun-off = Ninguno
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Sin perfil de lente.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = No hay perfil de Lensfun para { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = No hay perfil de Lensfun para esta lente.
# machine-drafted, needs native review
develop-lensfun-no-name = El archivo no indica la lente. Elíjala arriba.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = Coinciden { $count } lentes. Elija una arriba.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Recortar. Arrastra los bordes, o Ctrl+arrastrar a lo largo de una línea que deba quedar horizontal. Intro conserva, Esc vuelve atrás.
# machine-drafted, needs native review
develop-crop-kept = Recorte conservado
# machine-drafted, needs native review
develop-crop-cancelled = Recorte deshecho
# machine-drafted, needs native review
develop-crop-reset = Recorte y enderezado restablecidos
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Enderezado a { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Proporción { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = libre
# machine-drafted, needs native review
develop-crop-aspect-original = original
# machine-drafted, needs native review
develop-crop-turned-right = Girado a la derecha
# machine-drafted, needs native review
develop-crop-turned-left = Girado a la izquierda
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Volteado de izquierda a derecha
# machine-drafted, needs native review
develop-crop-flipped-vertical = Volteado de arriba abajo
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Proporción { $aspect } · Guía: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A proporción · X intercambiar · O guía · [ ] girar · H V voltear · Intro conservar · Esc atrás
# machine-drafted, needs native review
develop-crop-guide-thirds = tercios
# machine-drafted, needs native review
develop-crop-guide-golden = proporción áurea
# machine-drafted, needs native review
develop-crop-guide-diagonal = diagonales
# machine-drafted, needs native review
develop-crop-guide-grid = cuadrícula

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspectiva
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Aplomo
# machine-drafted, needs native review
develop-persp-mode-off = Desactivado
# machine-drafted, needs native review
develop-persp-mode-auto = Automático
# machine-drafted, needs native review
develop-persp-mode-level = Nivel
# machine-drafted, needs native review
develop-persp-mode-vertical = Verticales
# machine-drafted, needs native review
develop-persp-mode-full = Completo
# machine-drafted, needs native review
develop-persp-mode-guided = Guiado
# machine-drafted, needs native review
develop-persp-vertical = Perspectiva vertical
# machine-drafted, needs native review
develop-persp-horizontal = Perspectiva horizontal
# machine-drafted, needs native review
develop-persp-rotate = Girar
# machine-drafted, needs native review
develop-persp-aspect = Aspecto
# machine-drafted, needs native review
develop-persp-scale = Escala
# machine-drafted, needs native review
develop-persp-offset-x = Lateral
# machine-drafted, needs native review
develop-persp-offset-y = Arriba o abajo
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Evitar bordes vacíos
# machine-drafted, needs native review
develop-persp-guides = Guías
# machine-drafted, needs native review
develop-persp-focal = Distancia focal
# machine-drafted, needs native review
develop-persp-draw-guides = Dibujar guías
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Dibujar guías ({ $n } de { $max })
# machine-drafted, needs native review
develop-persp-reset = Restablecer perspectiva
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Aplomo: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Buscando líneas…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = ¿Restablecer la perspectiva? Se quitan tus { $n } guías. Deshacer las recupera.
# machine-drafted, needs native review
develop-persp-reset-ok = Restablecer
# machine-drafted, needs native review
develop-persp-reset-done = Perspectiva restablecida
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = ¿Quitar la guía { $n }? Deshacer la recupera.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Quitar
# machine-drafted, needs native review
develop-persp-guide-removed = Guía quitada
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Perspectiva vertical { $vertical }, horizontal { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Aplomo fallido: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: inclinación { $vertical }°, giro { $horizontal }°, rotación { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } necesita al menos dos líneas rectas que pueda distinguir. Prueba otro modo o dibuja guías.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = { $n } guías como máximo. Arrastra un extremo para mover una.
# machine-drafted, needs native review
develop-persp-guides-on = Guías. Arrastra a lo largo de una línea que deba quedar vertical u horizontal.
# machine-drafted, needs native review
develop-persp-guides-off = Recortar
# machine-drafted, needs native review
develop-persp-guide-moved = Guía movida
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Guía { $n } añadida
# machine-drafted, needs native review
develop-persp-guides-keys = Arrastrar añade · Arrastrar un extremo mueve · Supr quita · Shift+R recortar
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Guías: { $n } de { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Sombras
# machine-drafted, needs native review
develop-wheel-midtones = Medios tonos
# machine-drafted, needs native review
develop-wheel-highlights = Iluminaciones
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminancia de sombras
# machine-drafted, needs native review
develop-luminance-midtones = Luminancia de medios tonos
# machine-drafted, needs native review
develop-luminance-highlights = Luminancia de iluminaciones
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Global
# machine-drafted, needs native review
develop-luminance-global = Luminancia global
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Intensidad del color
# machine-drafted, needs native review
develop-brilliance = Brillo del color

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Mezclador de color
# machine-drafted, needs native review
develop-colour-red = Rojo
# machine-drafted, needs native review
develop-colour-orange = Naranja
# machine-drafted, needs native review
develop-colour-yellow = Amarillo
# machine-drafted, needs native review
develop-colour-green = Verde
# machine-drafted, needs native review
develop-colour-aqua = Aguamarina
# machine-drafted, needs native review
develop-colour-blue = Azul
# machine-drafted, needs native review
develop-colour-purple = Púrpura
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Tono – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Saturación – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminancia – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Color de punto
# machine-drafted, needs native review
develop-point-pick = Elegir un color
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Elegir otro color
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Haz clic en el color que quieres cambiar. Esc cancela.
# machine-drafted, needs native review
develop-point-picked = Color elegido
# machine-drafted, needs native review
develop-point-cancelled = Selección cancelada
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Rango
# machine-drafted, needs native review
develop-point-hue-range = Rango de tono
# machine-drafted, needs native review
develop-point-saturation-range = Rango de saturación
# machine-drafted, needs native review
develop-point-lightness-range = Rango de luminosidad
# machine-drafted, needs native review
develop-point-hue = Desplazamiento de tono
# machine-drafted, needs native review
develop-point-saturation = Desplazamiento de saturación
# machine-drafted, needs native review
develop-point-lightness = Desplazamiento de luminosidad

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Blanco y negro
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminancia – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Elegir un archivo LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Archivo: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = No se puede leer { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Elegir un archivo LUT
# machine-drafted, needs native review
develop-lut-kind = Archivos LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Hecha para
# machine-drafted, needs native review
develop-lut-space-linear = Lineal (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES lineal (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Foto (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Vídeo (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Método
# machine-drafted, needs native review
develop-lut-tetrahedral = Preciso
# machine-drafted, needs native review
develop-lut-trilinear = Simple
# machine-drafted, needs native review
develop-lut-strength = Fuerza

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Original
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } activado
# machine-drafted, needs native review
develop-history-off = { $module } desactivado
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = De vuelta al original
# machine-drafted, needs native review
develop-at-step = De vuelta a: { $step }
# machine-drafted, needs native review
develop-undone = Deshecho: { $step }
# machine-drafted, needs native review
develop-redone = Rehecho: { $step }
# machine-drafted, needs native review
develop-entered = Revelar. Las ediciones nunca cambian el archivo.
# machine-drafted, needs native review
develop-left = Has salido de Revelar
# machine-drafted, needs native review
develop-split-on = Antes y después lado a lado
# machine-drafted, needs native review
develop-split-off = Solo la edición
# machine-drafted, needs native review
develop-showing-before = Se muestra el original
# machine-drafted, needs native review
develop-showing-after = Se muestra la edición
# machine-drafted, needs native review
# Labels over the image.
develop-before = Antes
# machine-drafted, needs native review
develop-after = Después
# machine-drafted, needs native review
develop-working = Revelando…
# machine-drafted, needs native review
develop-preview-failed = No se puede mostrar la edición de este archivo

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Revelar
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Edición rápida: mantener y desplazar
# machine-drafted, needs native review
palette-develop = Revelar
# machine-drafted, needs native review
palette-develop-help = Editar esta foto sin cambiar el archivo. Otra vez para salir.
# machine-drafted, needs native review
palette-develop-before-after = Antes y después
# machine-drafted, needs native review
palette-develop-before-after-help = Mostrar el original, o de nuevo la edición.
# machine-drafted, needs native review
palette-develop-split = Antes y después divididos
# machine-drafted, needs native review
palette-develop-split-help = Mostrar el original y la edición lado a lado.
# machine-drafted, needs native review
palette-develop-panel = Ir al panel Revelar
# machine-drafted, needs native review
palette-develop-panel-help = Llevar el teclado al panel. Esc vuelve.
# machine-drafted, needs native review
palette-develop-speed-exposure = Edición rápida de la exposición
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Mantener y desplazar o pulsar las flechas para cambiar la exposición.
# machine-drafted, needs native review
palette-develop-speed-contrast = Edición rápida del contraste
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Mantener y desplazar o pulsar las flechas para cambiar el contraste.
# machine-drafted, needs native review
palette-develop-speed-highlights = Edición rápida de las iluminaciones
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Mantener y desplazar o pulsar las flechas para cambiar las iluminaciones.
# machine-drafted, needs native review
palette-develop-speed-shadows = Edición rápida de las sombras
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Mantener y desplazar o pulsar las flechas para cambiar las sombras.
# machine-drafted, needs native review
palette-develop-speed-whites = Edición rápida de los blancos
# machine-drafted, needs native review
palette-develop-speed-whites-help = Mantener y desplazar o pulsar las flechas para cambiar los blancos.
# machine-drafted, needs native review
palette-develop-speed-blacks = Edición rápida de los negros
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Mantener y desplazar o pulsar las flechas para cambiar los negros.
# machine-drafted, needs native review
palette-develop-speed-temperature = Edición rápida de la temperatura
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Mantener y desplazar o pulsar las flechas para cambiar la temperatura del balance de blancos.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Edición rápida de la intensidad
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Mantener y desplazar o pulsar las flechas para cambiar la intensidad.
# machine-drafted, needs native review
palette-develop-crop = Recortar y enderezar
# machine-drafted, needs native review
palette-develop-crop-help = Recorta, endereza, gira o voltea la foto. Intro conserva, Esc vuelve atrás.
# machine-drafted, needs native review
disabled-not-photo = Solo para fotos
# machine-drafted, needs native review
disabled-develop-off = Revelar está desactivado en los perfiles Medicina y Ciencia
# machine-drafted, needs native review
develop-module-corrections = Correcciones
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Desactivado
# machine-drafted, needs native review
develop-on = Activado
# machine-drafted, needs native review
develop-highlight-method = Altas luces recortadas
# machine-drafted, needs native review
develop-highlight-opposed = Reconstruir color
# machine-drafted, needs native review
develop-highlight-rebuild = Reconstruir zonas grandes
# machine-drafted, needs native review
develop-highlight-clip = Recortar a blanco
# machine-drafted, needs native review
develop-highlight-threshold = Nivel de recorte
# machine-drafted, needs native review
develop-module-hot-pixels = Píxeles calientes
# machine-drafted, needs native review
develop-hot-threshold = Umbral de píxeles calientes
# machine-drafted, needs native review
develop-module-ca = Aberración cromática
# machine-drafted, needs native review
develop-ca-mode = Franjas
# machine-drafted, needs native review
develop-ca-auto = Automática
# machine-drafted, needs native review
develop-ca-manual = Manual
# machine-drafted, needs native review
develop-ca-red = Desplazamiento rojo
# machine-drafted, needs native review
develop-ca-blue = Desplazamiento azul
# machine-drafted, needs native review
develop-module-defringe = Eliminar franjas
# machine-drafted, needs native review
develop-defringe = Eliminar franjas
# machine-drafted, needs native review
develop-defringe-purple = Tono púrpura
# machine-drafted, needs native review
develop-defringe-green = Tono verde
# machine-drafted, needs native review
develop-defringe-range = Rango de tono
# machine-drafted, needs native review
develop-defringe-edge = Contraste de bordes
# machine-drafted, needs native review
develop-module-flat-field = Fotogramas de calibración
# machine-drafted, needs native review
develop-calibration = Calibración
# machine-drafted, needs native review
develop-calibration-auto = Desde carpeta
# machine-drafted, needs native review
develop-calibration-chosen = Archivos elegidos
# machine-drafted, needs native review
develop-flat-choose = Elegir un flat…
# machine-drafted, needs native review
develop-dark-choose = Elegir un dark…
# machine-drafted, needs native review
develop-flat-file = Fotograma flat: { $name }
# machine-drafted, needs native review
develop-dark-file = Fotograma dark: { $name }
# machine-drafted, needs native review
develop-flat-use = Fotograma flat
# machine-drafted, needs native review
develop-flat-use-full = Viñeteo y polvo
# machine-drafted, needs native review
develop-flat-use-vignetting = Solo viñeteo
# machine-drafted, needs native review
develop-flat-dialog = Elegir un flat
# machine-drafted, needs native review
develop-dark-dialog = Elegir un dark
# machine-drafted, needs native review
develop-raw-kind = Archivos raw
# machine-drafted, needs native review
develop-flat-frame = Fotograma flat
# machine-drafted, needs native review
develop-dark-frame = Fotograma dark

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Añadir máscara
# machine-drafted, needs native review
develop-mask-kind-ellipse = Elipse
# machine-drafted, needs native review
develop-mask-kind-rectangle = Rectángulo
# machine-drafted, needs native review
develop-mask-kind-polygon = Polígono
# machine-drafted, needs native review
develop-mask-kind-linear = Degradado lineal
# machine-drafted, needs native review
develop-mask-kind-radial = Degradado radial
# machine-drafted, needs native review
develop-mask-kind-brush = Pincel
# machine-drafted, needs native review
develop-mask-kind-luminance = Rango de luminancia
# machine-drafted, needs native review
develop-mask-kind-hue = Rango de tono
# machine-drafted, needs native review
develop-mask-kind-chroma = Rango de saturación
# machine-drafted, needs native review
develop-mask-kind-colour = Rango de color
# machine-drafted, needs native review
develop-mask-kind-other = Otra máscara
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Fusión
# machine-drafted, needs native review
develop-blend-normal = Normal
# machine-drafted, needs native review
develop-blend-multiply = Multiplicar
# machine-drafted, needs native review
develop-blend-screen = Trama
# machine-drafted, needs native review
develop-blend-overlay = Superponer
# machine-drafted, needs native review
develop-blend-soft-light = Luz suave
# machine-drafted, needs native review
develop-blend-lightness = Solo luminosidad
# machine-drafted, needs native review
develop-blend-colour = Solo color
# machine-drafted, needs native review
develop-mask-amount = Intensidad de la máscara
# machine-drafted, needs native review
develop-mask-invert = Invertir máscara
# machine-drafted, needs native review
develop-mask-off = Desactivado
# machine-drafted, needs native review
develop-mask-on = Activado
# machine-drafted, needs native review
develop-mask-refine = Refinar bordes
# machine-drafted, needs native review
develop-mask-show = Mostrar máscara
# machine-drafted, needs native review
develop-mask-show-red = Rojo
# machine-drafted, needs native review
develop-mask-show-grey = Gris
# machine-drafted, needs native review
develop-mask-remove = Quitar máscara
# machine-drafted, needs native review
develop-mask-combine-add = Añadir
# machine-drafted, needs native review
develop-mask-combine-subtract = Restar
# machine-drafted, needs native review
develop-mask-combine-intersect = Intersecar
# machine-drafted, needs native review
develop-mask-row-opacity = Opacidad
# machine-drafted, needs native review
develop-mask-row-invert = Invertir
# machine-drafted, needs native review
develop-mask-feather = Difuminado
# machine-drafted, needs native review
develop-mask-roundness = Redondeo
# machine-drafted, needs native review
develop-mask-brush-size = Tamaño del pincel
# machine-drafted, needs native review
develop-mask-brush-feather = Difuminado del pincel
# machine-drafted, needs native review
develop-mask-brush-flow = Flujo
# machine-drafted, needs native review
develop-mask-brush-mode = Pincel
# machine-drafted, needs native review
develop-mask-brush-paint = Pintar
# machine-drafted, needs native review
develop-mask-brush-erase = Borrar
# machine-drafted, needs native review
develop-mask-range-from = Desde
# machine-drafted, needs native review
develop-mask-range-full-from = Pleno desde
# machine-drafted, needs native review
develop-mask-range-full-to = Pleno hasta
# machine-drafted, needs native review
develop-mask-range-to = Hasta
# machine-drafted, needs native review
develop-mask-range-test = Probar
# machine-drafted, needs native review
develop-mask-range-input = Entrada
# machine-drafted, needs native review
develop-mask-range-output = Salida
# machine-drafted, needs native review
develop-mask-colour-pick = Tomar un color
# machine-drafted, needs native review
develop-mask-colour-pick-more = Tomar otro color
# machine-drafted, needs native review
develop-mask-colour-range = Rango
# machine-drafted, needs native review
develop-mask-colour-falloff = Atenuación
# machine-drafted, needs native review
develop-mask-edit = Editar en la imagen
# machine-drafted, needs native review
develop-mask-row-remove = Quitar { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = ¿Quitar la máscara de { $module }? Sus formas se van. La foto en el disco se queda.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = ¿Quitar { $name } de { $module }? Las demás máscaras y la foto en el disco se quedan.
# machine-drafted, needs native review
develop-mask-remove-ok = Quitar
# machine-drafted, needs native review
develop-mask-removed = Máscara quitada.
# machine-drafted, needs native review
develop-mask-added = { $name } añadida.
# machine-drafted, needs native review
develop-mask-full = Esta máscara ya tiene todas las partes que admite.
# machine-drafted, needs native review
develop-mask-editing = Editando { $name }. Arrastre los tiradores, o pulse Tab y las flechas.
# machine-drafted, needs native review
develop-mask-painting = Pintando { $name }. Arrastre sobre la imagen.
# machine-drafted, needs native review
develop-mask-picking = Haga clic en la imagen para tomar un color.
# machine-drafted, needs native review
develop-mask-picked = Color tomado.
# machine-drafted, needs native review
develop-mask-done = Edición de la máscara terminada.
# machine-drafted, needs native review
develop-mask-handles = Tiradores de la máscara
# machine-drafted, needs native review
develop-handle-centre = Centro
# machine-drafted, needs native review
develop-handle-width = Anchura
# machine-drafted, needs native review
develop-handle-height = Altura
# machine-drafted, needs native review
develop-handle-rotate = Girar
# machine-drafted, needs native review
develop-handle-feather = Difuminado
# machine-drafted, needs native review
develop-handle-inner = Borde interior
# machine-drafted, needs native review
develop-handle-start = Inicio
# machine-drafted, needs native review
develop-handle-end = Fin
# machine-drafted, needs native review
develop-handle-point = Punto { $n }
# machine-drafted, needs native review
develop-instance = Capa
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Capa { $n }.
# machine-drafted, needs native review
develop-instance-new = Capa nueva
# machine-drafted, needs native review
develop-instance-duplicate = Duplicar capa
# machine-drafted, needs native review
develop-instance-delete = Eliminar capa
# machine-drafted, needs native review
develop-instance-earlier = Mover antes
# machine-drafted, needs native review
develop-instance-later = Mover después
# machine-drafted, needs native review
develop-instance-delete-confirm = ¿Eliminar la capa { $n } de { $module }? Sus ajustes y máscaras se van. La foto en el disco se queda.
# machine-drafted, needs native review
develop-instance-delete-ok = Eliminar
# machine-drafted, needs native review
develop-instance-deleted = Capa eliminada.
# machine-drafted, needs native review
develop-history-mask = Máscara: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Formas de máscara: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Nuevo: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Eliminado: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Orden: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Otros programas
# machine-drafted, needs native review
develop-darktable-edited = Editado en darktable:
# machine-drafted, needs native review
develop-darktable-open = Abrir en darktable
# machine-drafted, needs native review
develop-darktable-opening = Abriendo en darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable no se inició: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importar de Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] Se importó 1 ajuste de Lightroom. El resultado es aproximado.
    [many] Se importaron { $count } ajustes de Lightroom. El resultado es aproximado.
   *[other] Se importaron { $count } ajustes de Lightroom. El resultado es aproximado.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Nada que importar: estos ajustes de Lightroom no cambian nada aquí.
# machine-drafted, needs native review
develop-history-lightroom = Importado de Lightroom (aproximado)
# machine-drafted, needs native review
develop-conflict-title = La edición de { $file } cambió fuera de Specular
# machine-drafted, needs native review
develop-conflict-file = Archivo adjunto: guardado { $time }
# machine-drafted, needs native review
develop-conflict-library = Biblioteca: guardada { $time }
# machine-drafted, needs native review
develop-conflict-writer = El archivo adjunto lo escribió { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Conserva una. La otra se reemplaza.
# machine-drafted, needs native review
develop-conflict-use-file = Usar la del archivo
# machine-drafted, needs native review
develop-conflict-keep-library = Conservar la de la biblioteca
# machine-drafted, needs native review
develop-conflict-used-file = Se usa la edición del archivo adjunto
# machine-drafted, needs native review
develop-conflict-kept-library = Se conserva la edición de la biblioteca
# machine-drafted, needs native review
toast-develop-edit-newer-process = Esta edición se hizo con un Specular más reciente. Puede verse distinta aquí.
# machine-drafted, needs native review
info-darktable-edited = Editado en darktable
# machine-drafted, needs native review
info-darktable-no-modules = Ningún módulo activo
# machine-drafted, needs native review
info-darktable-group = Archivo
# machine-drafted, needs native review
toast-develop-edit-partial = Esta edición se hizo con un Specular más reciente. Se omiten las partes que esta versión no puede mostrar. Los cambios aquí no se guardan.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = No se puede leer el archivo de edición junto a esta foto. Se conserva tal cual. Los cambios aquí no se guardan.
# machine-drafted, needs native review
develop-demosaic = Detalle raw
develop-demosaic-directional = Estándar
develop-demosaic-dual = Zonas lisas suaves
develop-demosaic-fast = Rápido
develop-dual-threshold = Contraste de zonas lisas
