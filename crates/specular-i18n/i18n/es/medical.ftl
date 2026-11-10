# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Imágenes médicas
palette-window-level = Herramienta de ventana/nivel (arrastrar con el izquierdo)
palette-window-default = Ventana: predeterminada del archivo
palette-window-lung = Ventana: pulmón (W 1500, L −600)
palette-window-mediastinum = Ventana: mediastino (W 350, L 50)
palette-window-bone = Ventana: hueso (W 1800, L 400)
palette-window-brain = Ventana: cerebro (W 80, L 40)
palette-window-stroke = Ventana: ictus (W 40, L 40)
palette-window-soft-tissue = Ventana: tejidos blandos (W 400, L 50)
palette-window-liver = Ventana: hígado (W 150, L 30)
palette-window-subdural = Ventana: subdural (W 200, L 75)
palette-window-full-range = Ventana: rango completo
palette-invert-grey = Invertir grises
palette-next-slice = Fotograma siguiente
palette-prev-slice = Fotograma anterior
disabled-not-medical = Solo para imágenes médicas en escala de grises
disabled-one-frame = La imagen tiene un solo fotograma
settings-section-medical = Imágenes médicas
settings-medical-keep-window = Mantener ventana/nivel para la siguiente serie
settings-medical-keep-window-help = Mantener la ventana ajustada al pasar a otra serie, en lugar de empezar por su valor predeterminado. Los fotogramas de un archivo y los cortes de una serie siempre la comparten.
medical-window = C { $center }  W { $width }
medical-inverted = invertido
medical-frame = Fotograma { $frame } de { $count }
medical-tool = Herramienta de ventana/nivel: arrastrar
medical-tool-on = Herramienta de ventana/nivel activada: arrastra para ajustar
medical-tool-off = Herramienta de ventana/nivel desactivada
medical-preset-not-ct = Los preajustes de TC necesitan valores en unidades Hounsfield
medical-file-window-none = No hay ventana { $index } en este archivo
medical-file-window-unnamed = Ventana del archivo { $index } de { $count }
medical-file-window = Ventana del archivo { $index } de { $count }: { $name }
measure-templates-need-medical = Las plantillas geométricas vienen con el perfil Medicina
medical-off = Sin herramientas médicas para esta imagen: { $reason }
medical-readout = Valor y ventana

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Reproducir fotogramas (cine)
palette-measure-length = Medir longitud
palette-measure-angle = Medir ángulo
palette-measure-ellipse = Medir región elíptica
palette-measure-rectangle = Medir región rectangular
palette-clear-measurements = Borrar mediciones
palette-measure-templates = Plantillas geométricas…
palette-overlays = Mostrar u ocultar superposiciones
medical-cine = Reproduciendo a { $rate } fotogramas/s
medical-overlays-shown = Superposiciones visibles
medical-overlays-hidden = Superposiciones ocultas
measure-hint-length = Longitud: arrastre o haga clic en ambos extremos
measure-hint-angle = Ángulo: haga clic en tres puntos, el vértice en segundo lugar
measure-hint-ellipse = Elipse: arrastre sobre la región
measure-hint-rectangle = Rectángulo: arrastre sobre la región
measure-off = Herramienta de medición desactivada
measure-cleared = Mediciones borradas
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Área { $value } mm²
measure-area-px = Área { $value } px²
measure-angle = { $value }°
measure-mean = Media { $mean } ± { $sd } { $unit }
measure-range = Mín { $min }  Máx { $max }  ({ $count } px)
measure-caveat-detector = En el detector: sin corregir la magnificación
measure-caveat-magnification = Corrección de magnificación estimada
measure-caveat-uncalibrated = Sin espaciado de píxeles: en píxeles
measure-caveat-outside-region = Fuera de una región de ecografía calibrada: en píxeles

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Mostrar etiquetas de orientación
settings-medical-orientation-labels-help = Letras en los bordes de la imagen con las direcciones del paciente (R/L derecha/izquierda, A/P anterior/posterior, S/I cabeza/pies), tomadas del archivo y que siguen los giros y volteos. L las activa o desactiva durante la sesión.
settings-medical-wheel-steps-frames = La rueda del ratón recorre los fotogramas
settings-medical-wheel-steps-frames-help = En un archivo o serie con varios fotogramas, la rueda pasa de un fotograma a otro en lugar de hacer zoom o cambiar de imagen. Ctrl+rueda sigue haciendo zoom.
# machine-drafted, needs native review
settings-medical-display-function = Visualización de grises
settings-medical-display-function-help = Cómo se muestran los grises de las imágenes médicas. DICOM GSDF (función de visualización estándar de escala de grises) los reparte en pasos de igual visibilidad entre las luminancias del negro y del blanco de la pantalla (DICOM PS3.14), como en un monitor de diagnóstico calibrado. El perfil de color de la pantalla se sigue aplicando.
settings-medical-display-function-srgb = Estándar (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Luminancia del negro de la pantalla
settings-medical-display-black-help = La luminancia del negro de la pantalla, incluida la luz ambiental, medida o tomada de la ficha técnica de la pantalla. La usa DICOM GSDF.
settings-medical-display-white = Luminancia del blanco de la pantalla
settings-medical-display-white-help = La luminancia del blanco de la pantalla, medida o tomada de la ficha técnica de la pantalla. La usa DICOM GSDF.
palette-orientation-labels = Mostrar u ocultar etiquetas de orientación
palette-presentation-state = Estado de presentación siguiente
disabled-no-presentation-state = Ningún estado de presentación hace referencia a esta imagen
medical-labels-shown = Etiquetas de orientación visibles
medical-labels-hidden = Etiquetas de orientación ocultas
medical-presentation = Estado de presentación: { $label } ({ $index } de { $count })
medical-presentation-none = Estado de presentación desactivado
medical-presentation-readout = Presentación: { $label }
medical-preset-presentation = La tabla de consulta del estado de presentación fija los niveles de gris

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientación: izquierda { $left }, derecha { $right }, arriba { $top }, abajo { $bottom }
