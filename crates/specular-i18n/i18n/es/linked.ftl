# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Comparar series (vistas enlazadas)
# machine-drafted, needs native review
palette-compare-layout = Siguiente disposición de visores
# machine-drafted, needs native review
palette-next-viewport = Siguiente visor
# machine-drafted, needs native review
palette-prev-viewport = Visor anterior
# machine-drafted, needs native review
palette-reference-lines = Mostrar u ocultar las líneas de referencia
# machine-drafted, needs native review
palette-sync-window = Siguiente sinc. de ventana
# machine-drafted, needs native review
palette-sync-zoom = Zoom y desplazamiento conjuntos por tamaño
# machine-drafted, needs native review
palette-link-series = Siguiente enlace de serie
# machine-drafted, needs native review
palette-align-series = Alinear aquí las demás series
# machine-drafted, needs native review
palette-compare-series-help = Mostrar varias series lado a lado, con desplazamiento conjunto.
# machine-drafted, needs native review
palette-compare-layout-help = Mostrar más o menos visores.
# machine-drafted, needs native review
palette-next-viewport-help = Hacer activo el siguiente visor.
# machine-drafted, needs native review
palette-prev-viewport-help = Hacer activo el visor anterior.
# machine-drafted, needs native review
palette-reference-lines-help = Dibujar por dónde corta el corte activo a los demás visores.
# machine-drafted, needs native review
palette-sync-window-help = Elegir qué visores comparten un cambio de ventana/nivel: desactivada, misma modalidad o todas.
# machine-drafted, needs native review
palette-sync-zoom-help = Mostrar todos los visores al mismo tamaño y desplazarlos juntos.
# machine-drafted, needs native review
palette-link-series-help = Elegir cómo sigue esta serie a las demás.
# machine-drafted, needs native review
palette-align-series-help = Alinear una serie previa con esta en los cortes mostrados.
# machine-drafted, needs native review
disabled-not-comparing = Compare series primero

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Disposición: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Líneas de referencia
# machine-drafted, needs native review
linked-zoom = Zoom sincronizado
# machine-drafted, needs native review
linked-window-sync = Ventana sincronizada: { $mode ->
    [off] no
    [modality] misma modalidad
   *[all] todas
}
# machine-drafted, needs native review
linked-link = Enlace: { $mode ->
    [frame] marco de referencia
    [position] posición del paciente
   *[off] no
}
# machine-drafted, needs native review
linked-align = Alinear aquí

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Visor { $n } de { $count }, activo
   *[no] Visor { $n } de { $count }
}
# machine-drafted, needs native review
linked-empty = Arrastre una serie aquí
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Corte { $n } de { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = En un hueco: el corte más cercano está a { $distance } mm
# machine-drafted, needs native review
linked-outside = Fuera de esta serie ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Sin enlazar
# machine-drafted, needs native review
linked-aligned = Alineada a mano ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = Cursor 3D en { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } imagen
       *[other] { $count } imágenes
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Solo hay una serie: arrastre otra desde el explorador de estudios
   *[other] Comparando { $count } series
}
# machine-drafted, needs native review
linked-no-geometry = Esta serie no tiene posiciones de corte: no se puede enlazar
# machine-drafted, needs native review
linked-active = Visor { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Nada que alinear: las demás series comparten este marco de referencia
    [one] 1 serie alineada con la activa, durante esta sesión
   *[other] { $count } series alineadas con la activa, durante esta sesión
}
