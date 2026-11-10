# machine-drafted, needs native review
# Specular — Duplicados y fotos parecidas (categorías de la galería). Español.

similar-duplicates = Duplicados
similar-similar = Fotos parecidas
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } grupo
   *[other] { $groups } grupos
}, { $size } por liberar
similar-menu-scanning = { $name } — buscando ({ PERCENT($percent) })
similar-menu-none = { $name } — ninguno
similar-summary = { $groups ->
    [one] { $groups } grupo por revisar
   *[other] { $groups } grupos por revisar
}, { $files ->
    [one] { $files } archivo
   *[other] { $files } archivos
} por descartar, { $size } por liberar
similar-progress = { $resolved } de { $total } resueltos
similar-skipped = { $count } omitidos
similar-scanning = Buscando duplicados y fotos parecidas… { $done } de { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Comprobando la nitidez… { $done } de { $total }
# machine-drafted, needs native review
# La lista durante el primer análisis (su progreso está en el encabezado de arriba).
similar-waiting = Los grupos aparecerán aquí cuando termine el análisis.
# machine-drafted, needs native review
similar-stopped = Análisis detenido: nada agrupado
similar-stopped-sharpness = Comprobación de nitidez detenida: mejores opciones según las miniaturas
similar-stopped-empty = El análisis se detuvo. Lo que midió se conserva: Volver a analizar continúa desde ahí.
similar-scan-again = Volver a analizar
similar-empty-duplicates = No hay duplicados en esta carpeta.
similar-empty-similar = No hay fotos parecidas en esta carpeta.
similar-all-done = Todos los grupos revisados.
similar-sensitivity = Sensibilidad
similar-sensitivity-strict = Estricta
similar-sensitivity-loose = Amplia
similar-accept-all = Aceptar todas las sugerencias
similar-show-skipped = Mostrar grupos omitidos
similar-discard-with = Descartar: { $action }
similar-action-trash = Mover a la papelera
similar-action-move = Mover a una carpeta…
similar-action-move-to = Mover a «{ $folder }»
similar-action-reject = Solo marcar como rechazada
similar-choose-folder = Elegir carpeta para los descartes…
similar-confirm = ¿Aceptar las sugerencias de { $groups ->
    [one] { $groups } grupo
   *[other] { $groups } grupos
}? { $files ->
    [one] { $files } archivo
   *[other] { $files } archivos
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Aceptar y descartar { $files } archivo
   *[other] Aceptar y descartar { $files } archivos
}
similar-cancel = Cancelar
similar-group-title = Grupo { $index } de { $total }
similar-group-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } fotos
}
similar-to-free = { $size } por liberar
similar-keep = Conservar
similar-discard = Descartar
similar-undecided = Sin decidir
similar-accept-next = Aceptar y siguiente
similar-not-similar = No es parecida
similar-skip = Omitir
similar-compare = Comparar
similar-zoom = { PERCENT(100) }
similar-back = Atrás
similar-done = Listo
similar-suggested-keep = Sugerencia: conservar
similar-suggested-discard = Sugerencia: descartar
similar-toast-accepted = { $count ->
    [one] { $count } grupo aceptado
   *[other] { $count } grupos aceptados
}
similar-toast-rejected = { $count ->
    [one] { $count } archivo marcado como rechazado
   *[other] { $count } archivos marcados como rechazados
}
similar-toast-protected = Las fotos valoradas, marcadas, etiquetadas, con palabras clave o editadas siempre se conservan.
# machine-drafted, needs native review
similar-zoom-already = Esta foto ya se muestra al { PERCENT(100) } o más grande
similar-a11y-member = { $name }: { $state }, { $reason }
similar-hint-list = ↑↓ elegir · Intro revisar · A aceptar todo · Tab cambiar · Esc cerrar
similar-reason-rated = Valorada
similar-reason-flagged = Marcada como elegida
similar-reason-labelled = Tiene etiqueta de color
similar-reason-tagged = Tiene palabras clave
similar-reason-edited = Tiene ediciones
similar-reason-original = El original
similar-reason-raw-original = Original RAW
similar-reason-highest-resolution = Mayor resolución
similar-reason-deeper-colour = Mayor profundidad de color
similar-reason-least-compressed = Menos comprimida
similar-reason-has-metadata = Conserva sus metadatos
similar-reason-oldest = Copia más antigua
similar-reason-sharpest = La más nítida
similar-reason-steadiest = Menos trepidación
similar-reason-best-exposure = Mejor expuesta
similar-reason-least-noise = Menos ruido
similar-reason-also-good = También buena
similar-reason-same-file = Archivo idéntico
similar-reason-derivative = Copia derivada
similar-reason-rendering-of-raw = Revelado del RAW
similar-reason-smaller-copy = Copia más pequeña
similar-reason-shallower-colour = Menor profundidad de color
similar-reason-more-compressed = Más comprimida
similar-reason-no-metadata = Sin metadatos
similar-reason-newer-copy = Copia más reciente
similar-reason-blurrier = Más borrosa
similar-reason-motion-blur = Trepidación
similar-reason-overexposed = Sobreexpuesta
similar-reason-underexposed = Subexpuesta
similar-reason-noisier = Más ruido
similar-reason-lower-resolution = Menor resolución
