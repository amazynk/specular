# machine-drafted, needs native review
# Specular — Tarefas em segundo plano: o indicador mostrado enquanto algo é executado em segundo plano e o painel que abre. Português (Portugal).

## O painel

work-title = Tarefas em segundo plano
work-show = Mostrar tarefas em segundo plano
work-empty = Nada está a ser executado em segundo plano.

## Tarefas

work-kind-library = Biblioteca de fotografias
work-kind-ai-search = Pesquisa com IA
work-kind-faces = Pessoas
work-kind-text = Texto em fotografias
work-kind-culling = Classificações de triagem
work-kind-download = Transferência do modelo de IA
work-kind-similar = Fotografias semelhantes

## Passos

work-step-finding = A procurar fotografias
work-step-reading = A ler os detalhes das fotografias
work-step-signatures = A comparar fotografias
work-step-grouping = A agrupar fotografias semelhantes
work-step-text-index = A criar o índice de pesquisa
work-step-embedding = A indexar fotografias para a pesquisa com IA
work-step-switching = A reindexar para o novo tamanho de modelo
work-step-faces = A procurar rostos
work-step-ocr = A ler o texto das fotografias
work-step-scoring = A avaliar nitidez e exposição
work-step-downloading = A transferir
work-step-focus = A verificar a nitidez
work-step-item = { $step } · { $item }

## Progresso

work-count-photos = { $done } de { $total } { $count ->
    [one] fotografia
   *[other] fotografias
}
work-count = { $done } de { $total }
work-count-left = { $count } · faltam { $left }
work-so-far-photos = { $done } { $count ->
    [one] fotografia
   *[other] fotografias
} até agora
work-so-far = { $done } até agora

## Tempo restante

work-eta-estimating = A estimar…
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

## Porque espera uma tarefa

work-paused-browsing = Em pausa enquanto navega pelas fotografias
work-paused-battery = Em pausa com bateria
work-paused-low-power = Em pausa no modo de poupança de energia
work-paused-heat = Em pausa enquanto o computador está quente
work-paused-memory = Em pausa enquanto há pouca memória
work-paused-user = Em pausa

## O que se segue

work-then = Depois: { $steps }

## Botões

work-pause = Pausa
work-resume = Retomar
work-settings = Definições…

## O indicador

work-chip = { $job } · { $done } de { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Tarefas em segundo plano: { $status }

## Avisos

work-started = { $job }: iniciado em segundo plano
work-finished = { $job }: concluído
