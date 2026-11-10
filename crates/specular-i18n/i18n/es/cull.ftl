# machine-drafted, needs native review
# Specular — Modo de criba (DESIGN.md §6.3).

palette-cull = Entrar o salir del modo de criba
cheatsheet-cull = Modo de criba
settings-section-cull = Modo de criba
settings-cull-auto-enter = Cribar carpetas RAW
settings-cull-auto-enter-help = Abrir una carpeta con mayoría de archivos RAW directamente en el modo de criba.
settings-cull-auto-advance = Siguiente imagen tras marcar
settings-cull-auto-advance-help = Avanzar tras elegir, rechazar o valorar con estrellas. Las etiquetas y quitar la marca no cambian de imagen.
settings-cull-show = Mostrar en pares RAW+JPEG
settings-cull-show-help = Un RAW y un JPEG con el mismo nombre se criban como una sola imagen y las marcas se aplican a ambos. El JPEG se abre más rápido.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Apilar ráfagas
settings-cull-bursts-help = Las fotos tomadas en rápida sucesión por una cámara se muestran como una pila. S abre o cierra una pila, B conserva la imagen en pantalla y rechaza el resto de su pila.
settings-cull-burst-gap = Pausa de ráfaga
settings-cull-burst-gap-help = La pausa más larga entre dos fotos de una ráfaga.
settings-cull-watch = Mostrar archivos nuevos
settings-cull-watch-help = Los archivos que aparecen en la carpeta durante la criba (una cámara conectada, una importación) se añaden al final cuando están completamente escritos.
settings-cull-follow-newest = Seguir el archivo más reciente
settings-cull-follow-newest-help = Saltar a cada archivo nuevo al llegar. T activa o desactiva esto durante la criba.
settings-cull-slot-1 = Carpeta 1
settings-cull-slot-1-help = Alt+1 copia la imagen (con su RAW o JPEG y sus archivos auxiliares) a esta carpeta, Alt+Mayús+1 la mueve allí.
settings-cull-slot-1-unset = Sin definir
settings-cull-slot-2 = Carpeta 2
settings-cull-slot-2-help = Alt+2 copia la imagen (con su RAW o JPEG y sus archivos auxiliares) a esta carpeta, Alt+Mayús+2 la mueve allí.
settings-cull-slot-2-unset = Sin definir
settings-cull-slot-3 = Carpeta 3
settings-cull-slot-3-help = Alt+3 copia la imagen (con su RAW o JPEG y sus archivos auxiliares) a esta carpeta, Alt+Mayús+3 la mueve allí.
settings-cull-slot-3-unset = Sin definir
settings-cull-collisions = Si un nombre ya existe
settings-cull-collisions-help = Los archivos de las carpetas nunca se reemplazan.
settings-cull-collisions-suffix = Añadir un número
settings-cull-collisions-skip = Omitir la imagen
# machine-drafted, needs native review
# Modo de criba elegido sin carpeta ni imágenes de galería abiertas.
cull-nothing = Nada que cribar: abre primero una carpeta de fotos
cull-slot-unset = No hay carpeta { $slot } definida: elige una en Configuración → Modo de criba
cull-slot-missing = La carpeta { $slot } no está disponible: { $reason }
cull-copied = Copiado a { $folder }
cull-moved = Movido a { $folder }
cull-slot-renamed = Guardado como { $name } en { $folder }
cull-slot-skipped = { $folder } ya tiene { $name }: omitido
cull-slot-same = La imagen ya está en { $folder }
cull-slot-failed = { $op ->
    [move] No se pudo mover { $file }
   *[copy] No se pudo copiar { $file }
}: { $reason }
cull-copy-kept = { $count ->
    [one] Se conservó una copia modificada después
   *[other] Se conservaron { $count } copias modificadas después
}
cull-undo-failed = No se pudo deshacer para { $file }: { $reason }
cull-burst-open = Ráfaga { $index } de { $count }
cull-burst-closed = Ráfaga de { $count }
cull-burst-toggle = Abrir o cerrar la ráfaga
cull-burst-best = 1 conservada, { $count } rechazadas
cull-burst-none = No forma parte de una ráfaga
cull-follow-on = Siguiendo archivos nuevos
cull-follow-off = Sin seguir archivos nuevos
cull-new-files = { $count ->
    [one] 1 archivo nuevo
   *[other] { $count } archivos nuevos
}
cull-key-copy = Copiar a la carpeta 1–3
cull-key-move = Mover a la carpeta 1–3
cull-key-burst = Abrir o cerrar una ráfaga
cull-key-best = Conservar esta, rechazar el resto de la ráfaga
cull-key-follow = Seguir archivos nuevos
cull-key-subframes = Mostrar u ocultar la tabla de subexposiciones
cull-key-sort = Ordenar por la columna siguiente
cull-key-sort-reverse = Invertir el orden
cull-key-metric = Graficar la métrica siguiente
cull-key-sigma-down = Rechazo automático: bajar k
cull-key-sigma-up = Rechazo automático: subir k
cull-key-auto-reject = Rechazar las tomas sugeridas
cull-key-move-rejects = Mover las tomas rechazadas a una subcarpeta
cull-key-same-stretch = Mismo estirado para todas las tomas

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Abrir la ráfaga
cull-tip-burst-close = Cerrar la ráfaga
cull-tip-exit = Salir del modo de criba
cull-tip-label = Etiqueta: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Subexposiciones
cull-astro-title = { $count ->
    [one] 1 subexposición
   *[other] { $count } subexposiciones
}
cull-astro-pending = midiendo { $count }
cull-astro-col-name = Toma
cull-astro-col-filter = Filtro
cull-astro-col-exposure = Exp.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Exc.
cull-astro-col-stars = Estrellas
cull-astro-col-bg = Fondo
cull-astro-col-snr = SNR
cull-astro-col-noise = Ruido
cull-astro-same-stretch = Mismo estirado
cull-astro-own-stretch = Estirado propio
cull-astro-suggest-high = { $metric } por encima de { $value } (mediana + { $k }σ): { $count ->
    [one] 1 toma
   *[other] { $count } tomas
}
cull-astro-suggest-low = { $metric } por debajo de { $value } (mediana − { $k }σ): { $count ->
    [one] 1 toma
   *[other] { $count } tomas
}
cull-astro-suggest-none = Ninguna toma más allá de { $k }σ
cull-astro-suggest-few = El rechazo automático necesita 3 tomas medidas
cull-astro-suggested = sugerida para rechazo
cull-astro-k = k { $k }
cull-astro-reject = Rechazar { $count }
cull-astro-move = Mover rechazadas…
cull-astro-chart = { $metric } en { $count ->
        [one] { $count } toma
       *[other] { $count } tomas
    }, mediana { $median }.
cull-astro-sort-by = Ordenar por { $column }
cull-astro-ascending = ascendente
cull-astro-descending = descendente
cull-astro-sorted = Ordenado por { $column }
cull-astro-reversed = Orden invertido
cull-astro-metric = Gráfico: { $metric }
cull-astro-k-set = Rechazo automático a { $k }σ
cull-astro-none = No hay subexposiciones aquí
cull-astro-same-on = Mismo estirado para todas las tomas
cull-astro-same-off = Cada toma estirada por separado
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = excentricidad { $value }
cull-astro-line-stars = { $count ->
    [one] 1 estrella
   *[other] { $count } estrellas
}
cull-astro-measuring = midiendo estrellas…
cull-astro-reject-confirm = { $count ->
    [one] ¿Rechazar la toma sugerida?
   *[other] ¿Rechazar las { $count } tomas sugeridas?
}
cull-astro-reject-detail = { $rule }. Deshacer recupera las marcas.
cull-astro-reject-ok = Rechazar
cull-astro-nothing-to-reject = Ninguna toma que rechazar
cull-astro-rejected = { $count ->
    [one] 1 toma rechazada
   *[other] { $count } tomas rechazadas
}
cull-move-rejects-confirm = { $count ->
    [one] ¿Mover la toma rechazada a la carpeta «{ $folder }»?
   *[other] ¿Mover las { $count } tomas rechazadas a la carpeta «{ $folder }»?
}
cull-move-rejects-detail = Los archivos se mueven, no se borran. Deshacer los devuelve.
cull-move-rejects-ok = Mover
cull-move-rejects-none = No hay tomas rechazadas que mover
cull-moved-rejects = { $count ->
    [one] 1 toma movida a «{ $folder }»
   *[other] { $count } tomas movidas a «{ $folder }»
}
