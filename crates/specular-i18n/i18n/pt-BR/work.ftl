# machine-drafted, needs native review
# Specular — Tarefas em segundo plano: o indicador mostrado enquanto algo é executado em segundo plano e o painel que ele abre. Português (Brasil).

## O painel

work-title = Tarefas em segundo plano
work-show = Mostrar tarefas em segundo plano
work-empty = Nada está sendo executado em segundo plano.

## Tarefas

work-kind-library = Biblioteca de fotos
work-kind-ai-search = Pesquisa com IA
work-kind-faces = Pessoas
work-kind-text = Texto em fotos
work-kind-culling = Notas de triagem
work-kind-download = Download do modelo de IA
work-kind-similar = Fotos semelhantes

## Etapas

work-step-finding = Procurando fotos
work-step-reading = Lendo os detalhes das fotos
work-step-signatures = Comparando fotos
work-step-grouping = Agrupando fotos semelhantes
work-step-text-index = Criando o índice de pesquisa
work-step-embedding = Indexando fotos para a pesquisa com IA
work-step-switching = Reindexando para o novo tamanho de modelo
work-step-faces = Procurando rostos
work-step-ocr = Lendo o texto das fotos
work-step-scoring = Avaliando nitidez e exposição
work-step-downloading = Baixando
work-step-focus = Verificando a nitidez
work-step-item = { $step } · { $item }

## Progresso

work-count-photos = { $done } de { $total } { $count ->
    [one] foto
   *[other] fotos
}
work-count = { $done } de { $total }
work-count-left = { $count } · faltam { $left }
work-so-far-photos = { $done } { $count ->
    [one] foto
   *[other] fotos
} até agora
work-so-far = { $done } até agora

## Tempo restante

work-eta-estimating = Estimando…
work-eta-seconds = { $n ->
    [one] Falta cerca de { $n } segundo
   *[other] Faltam cerca de { $n } segundos
}
work-eta-minutes = { $n ->
    [one] Falta cerca de { $n } minuto
   *[other] Faltam cerca de { $n } minutos
}
work-eta-hours = { $h ->
    [one] Falta cerca de { $h } hora
   *[other] Faltam cerca de { $h } horas
}
work-eta-hours-minutes = Faltam cerca de { $h ->
    [one] { $h } hora
   *[other] { $h } horas
} e { $m ->
    [one] { $m } minuto
   *[other] { $m } minutos
}

## Por que uma tarefa espera

work-paused-browsing = Pausado enquanto você navega pelas fotos
work-paused-battery = Pausado na bateria
work-paused-low-power = Pausado no modo de economia de energia
work-paused-heat = Pausado enquanto o computador está quente
work-paused-memory = Pausado enquanto há pouca memória
work-paused-user = Pausado

## O que vem depois

work-then = Depois: { $steps }

## Botões

work-pause = Pausar
work-resume = Retomar
work-settings = Configurações…

## O indicador

work-chip = { $job } · { $done } de { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Tarefas em segundo plano: { $status }

## Avisos

work-started = { $job }: iniciado em segundo plano
work-finished = { $job }: concluído
