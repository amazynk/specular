# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Español.

keys-mode-drawing = Dibujos
cheatsheet-drawing = Dibujos
palette-drawing-layers = Mostrar u ocultar el panel de capas
palette-drawing-all-layers = Mostrar todas las capas o las capas tal como se guardaron
palette-drawing-paper = Alternar papel oscuro y claro
palette-drawing-lineweights = Mostrar u ocultar los grosores de línea
disabled-not-drawing = Solo para dibujos (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Modelo
# machine-drafted, needs native review
cad-pen-name = Plumilla { $n }
cad-page-name = Página { $n }
cad-layer-name = Capa { $n }
cad-map-name = Mapa
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Capas
cad-board-top-name = Superior
cad-board-bottom-name = Inferior
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Marco
cad-trace-name = Traza { $n }
cad-contours-positive-name = Contornos positivos
cad-contours-negative-name = Contornos negativos
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = Modelo 3D
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Capas · { $shown } de { $total } visibles
# Toasts.
drawing-layers-all-toast = Todas las capas visibles
drawing-layers-saved-toast = Capas tal como se guardaron en el dibujo
drawing-paper-dark-toast = Papel oscuro
drawing-paper-light-toast = Papel claro
drawing-lineweights-on = Grosores de línea visibles
drawing-lineweights-off = Grosores de línea desactivados: líneas finas

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Marcar
palette-markup-help = Nubes, texto, flechas, sellos, mediciones y marcadores de foto, guardados junto al archivo.
palette-compare-sheets = Comparar láminas
palette-compare-sheets-help = Superponer otra revisión de esta lámina, alineada, con los cambios resaltados.
markup-title = Marcas
markup-list = Marcas en esta página
markup-count = { $count ->
    [0] No hay marcas en esta página
    [one] 1 marca en esta página
   *[other] { $count } marcas en esta página
}
markup-empty = Elija una herramienta y haga clic en la lámina.
markup-item = { $n }. { $kind }
markup-tool-select = Seleccionar
markup-tool-cloud = Nube
markup-tool-text = Texto
markup-tool-arrow = Flecha
markup-tool-stamp = Sello
markup-tool-length = Longitud
markup-tool-area = Área
markup-tool-pin = Marcador de foto
markup-hint-select = Haga clic en un elemento para seleccionarlo. Arrástrelo para moverlo. Enter edita su texto.
markup-hint-cloud = Arrastre un rectángulo, o haga clic en las esquinas y pulse Enter.
markup-hint-text = Haga clic donde va el texto.
markup-hint-arrow = Arrastre desde la etiqueta hasta el punto, o haga clic en ambos.
markup-hint-stamp = Haga clic donde va el sello.
markup-hint-length = Arrastre, o haga clic en ambos extremos.
markup-hint-area = Haga clic en las esquinas y pulse Enter.
markup-hint-pin = Haga clic en un punto y elija una foto.
markup-kind-cloud = Nube
markup-kind-text = Texto
markup-kind-arrow = Flecha
markup-kind-stamp = Sello
markup-kind-length = Longitud
markup-kind-area = Área
markup-kind-pin = Foto
markup-colour = Color: { $colour }
markup-colour-red = Rojo
markup-colour-blue = Azul
markup-colour-green = Verde
markup-colour-orange = Naranja
markup-colour-magenta = Magenta
markup-colour-black = Negro
markup-width = Línea: { $width }
markup-width-thin = Fina
markup-width-medium = Media
markup-width-thick = Gruesa
markup-stamp = Sello: { $words }
markup-stamp-approved = Aprobado
markup-stamp-rejected = Rechazado
markup-stamp-reviewed = Revisado
markup-stamp-custom = Texto propio
markup-undo = Deshacer
markup-redo = Rehacer
markup-delete = Eliminar
markup-open-photo = Abrir foto
markup-from-measurements = Añadir mediciones
markup-from-measurements-done = { $count ->
    [one] 1 medición añadida como marca
   *[other] { $count } mediciones añadidas como marcas
}
markup-export-pdf = Exportar PDF…
markup-export-png = Exportar PNG…
markup-done = Hecho
markup-ask-text = Texto (Enter para colocar, Esc para cancelar)
markup-ask-label = Etiqueta (opcional; Enter para colocar)
markup-ask-stamp = Texto del sello (Enter para colocar)
markup-ask-edit = Texto nuevo (Enter para guardar)
markup-added = Añadido: { $kind }
markup-moved = Movido
markup-pin-hint = Haga clic de nuevo en el marcador o pulse Enter para abrir su foto.
markup-deleted = Eliminado
markup-undone = Deshecho
markup-redone = Rehecho
markup-delete-ask = ¿Eliminar { $name }?
markup-delete-detail = Se quita del archivo de marcas. El dibujo en sí no cambia.
markup-delete-ok = Eliminar
markup-photo-missing = { $name } ya no está.
markup-load-failed = Marcas no leídas: { $reason }
markup-save-failed = Marcas no guardadas: { $reason }
markup-sidecar-too-large = el archivo de marcas es demasiado grande
markup-pdf-unreadable = no se pudo leer el PDF
markup-exported = Guardado { $name }
markup-export-failed = No exportado: { $reason }
markup-export-same-file = elija otro nombre; el original nunca se sobrescribe
markup-export-encrypted = el PDF está cifrado. Exporte PNG en su lugar.
markup-export-damaged = la estructura del PDF está dañada. Exporte PNG en su lugar.
sheets-title = Comparar láminas
sheets-other = Otra lámina…
sheets-other-named = Otra: { $name }
sheets-page = Página { $page } de { $count }
sheets-auto = Alinear
sheets-pick = Elegir 2 puntos
sheets-overlay = Superposición
sheets-difference = Diferencia
sheets-blend = Fundido
sheets-blink = Parpadeo
sheets-slider = Otra lámina { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Sin cambios
    [one] Marcar 1 cambio con nube
   *[other] Marcar { $count } cambios con nubes
}
sheets-done = Hecho
sheets-choose = Elija la otra lámina.
sheets-reading = Leyendo la lámina…
sheets-aligning = Alineando…
sheets-aligned = Alineada: escala { $scale }, giro { $angle }°, ajuste dentro de { $error } px. { $changes ->
    [0] No se encontraron cambios.
    [one] 1 cambio.
   *[other] { $changes } cambios.
}
sheets-align-failed = No se pudo alinear automáticamente. Elija 2 puntos.
sheets-pick-this = Punto { $n }: haga clic en él en esta lámina.
sheets-pick-other = Punto { $n }: haga clic en el mismo punto en la otra lámina.
sheets-pick-same = Los dos puntos son el mismo. Elija 2 puntos de nuevo.
sheets-read-failed = Lámina no leída: { $reason }
sheets-failed = Ese paso falló. Inténtelo de nuevo o elija otra lámina.
sheets-legend-this = Solo esta lámina
sheets-legend-other = Solo la otra lámina
sheets-legend-both = Ambas
sheets-cloud-text = Modificado
sheets-clouds-added = { $count ->
    [0] No se añadieron nubes
    [one] 1 nube añadida a las marcas
   *[other] { $count } nubes añadidas a las marcas
}
sheets-opened = Comparar láminas
sheets-closed = Comparación cerrada
markup-ask-new-line = Shift+Enter: nueva línea

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Dibujos
settings-drawings-shx-folders = Carpetas de fuentes SHX
settings-drawings-shx-folders-help = Carpetas con fuentes SHX para el texto de DXF y DWG, separadas por ;. Primero se busca en la carpeta del propio dibujo.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = No es una carpeta: { $folder }
