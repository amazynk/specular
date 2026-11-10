# machine-drafted, needs native review
# Specular — Tareas en segundo plano: el indicador que se muestra mientras algo se ejecuta en segundo plano y el panel que abre. Español.

## El panel

work-title = Tareas en segundo plano
work-show = Mostrar tareas en segundo plano
work-empty = No hay nada ejecutándose en segundo plano.

## Tareas

work-kind-library = Biblioteca de fotos
work-kind-ai-search = Búsqueda con IA
work-kind-faces = Personas
work-kind-text = Texto en fotos
work-kind-culling = Puntuaciones de cribado
work-kind-download = Descarga del modelo de IA
work-kind-similar = Fotos parecidas

## Pasos

work-step-finding = Buscando fotos
work-step-reading = Leyendo los detalles de las fotos
work-step-signatures = Comparando fotos
work-step-grouping = Agrupando fotos parecidas
work-step-text-index = Creando el índice de búsqueda
work-step-embedding = Indexando fotos para la búsqueda con IA
work-step-switching = Reindexando para el nuevo tamaño de modelo
work-step-faces = Buscando caras
work-step-ocr = Leyendo el texto de las fotos
work-step-scoring = Puntuando nitidez y exposición
work-step-downloading = Descargando
work-step-focus = Comprobando la nitidez
work-step-item = { $step } · { $item }

## Progreso

work-count-photos = { $done } de { $total } { $count ->
    [one] foto
   *[other] fotos
}
work-count = { $done } de { $total }
work-count-left = { $count } · faltan { $left }
work-so-far-photos = { $done } { $count ->
    [one] foto
   *[other] fotos
} hasta ahora
work-so-far = { $done } hasta ahora

## Tiempo restante

work-eta-estimating = Calculando…
work-eta-seconds = { $n ->
    [one] Queda aproximadamente { $n } segundo
   *[other] Quedan aproximadamente { $n } segundos
}
work-eta-minutes = { $n ->
    [one] Queda aproximadamente { $n } minuto
   *[other] Quedan aproximadamente { $n } minutos
}
work-eta-hours = { $h ->
    [one] Queda aproximadamente { $h } hora
   *[other] Quedan aproximadamente { $h } horas
}
work-eta-hours-minutes = Quedan aproximadamente { $h ->
    [one] { $h } hora
   *[other] { $h } horas
} y { $m ->
    [one] { $m } minuto
   *[other] { $m } minutos
}

## Por qué espera una tarea

work-paused-browsing = En pausa mientras navegas por las fotos
work-paused-battery = En pausa con batería
work-paused-low-power = En pausa en modo de ahorro de energía
work-paused-heat = En pausa mientras el ordenador está caliente
work-paused-memory = En pausa mientras haya poca memoria
work-paused-user = En pausa

## Lo que sigue

work-then = Después: { $steps }

## Botones

work-pause = Pausar
work-resume = Reanudar
work-settings = Configuración…

## El indicador

work-chip = { $job } · { $done } de { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Tareas en segundo plano: { $status }

## Avisos

work-started = { $job }: iniciado en segundo plano
work-finished = { $job }: terminado
