# machine-drafted, needs native review
# Specular — Modo de triagem (DESIGN.md §6.3).

palette-cull = Entrar ou sair do modo de triagem
cheatsheet-cull = Modo de triagem
settings-section-cull = Modo de triagem
settings-cull-auto-enter = Triar pastas RAW
settings-cull-auto-enter-help = Abrir uma pasta com maioria de arquivos RAW direto no modo de triagem.
settings-cull-auto-advance = Próxima imagem após marcar
settings-cull-auto-advance-help = Avançar após escolher, rejeitar ou dar estrelas. Rótulos e desmarcar ficam na imagem.
settings-cull-show = Mostrar em pares RAW+JPEG
settings-cull-show-help = Um RAW e um JPEG com o mesmo nome são triados como uma só imagem, e as marcas valem para os dois. O JPEG abre mais rápido.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Empilhar sequências
settings-cull-bursts-help = Fotos tiradas em sequência rápida por uma câmera aparecem como uma pilha. S abre ou fecha uma pilha, B mantém a imagem na tela e rejeita o resto da pilha.
settings-cull-burst-gap = Intervalo da sequência
settings-cull-burst-gap-help = A maior pausa entre duas fotos de uma sequência.
settings-cull-watch = Mostrar arquivos novos
settings-cull-watch-help = Arquivos que aparecem na pasta durante a triagem (câmera conectada, importação) são adicionados ao final quando terminam de ser gravados.
settings-cull-follow-newest = Seguir o arquivo mais novo
settings-cull-follow-newest-help = Ir para cada arquivo novo assim que chegar. T alterna isso durante a triagem.
settings-cull-slot-1 = Pasta 1
settings-cull-slot-1-help = Alt+1 copia a imagem (com seu RAW ou JPEG e arquivos auxiliares) para esta pasta, Alt+Shift+1 a move para lá.
settings-cull-slot-1-unset = Não definida
settings-cull-slot-2 = Pasta 2
settings-cull-slot-2-help = Alt+2 copia a imagem (com seu RAW ou JPEG e arquivos auxiliares) para esta pasta, Alt+Shift+2 a move para lá.
settings-cull-slot-2-unset = Não definida
settings-cull-slot-3 = Pasta 3
settings-cull-slot-3-help = Alt+3 copia a imagem (com seu RAW ou JPEG e arquivos auxiliares) para esta pasta, Alt+Shift+3 a move para lá.
settings-cull-slot-3-unset = Não definida
settings-cull-collisions = Quando um nome já existe
settings-cull-collisions-help = Arquivos nas pastas nunca são substituídos.
settings-cull-collisions-suffix = Adicionar um número
settings-cull-collisions-skip = Pular a imagem
# machine-drafted, needs native review
# Modo de seleção escolhido sem pasta nem imagens da galeria abertas.
cull-nothing = Nada para triar — abra primeiro uma pasta de fotos
cull-slot-unset = Nenhuma pasta { $slot } definida — escolha uma em Configurações → Modo de triagem
cull-slot-missing = A pasta { $slot } não está disponível: { $reason }
cull-copied = Copiado para { $folder }
cull-moved = Movido para { $folder }
cull-slot-renamed = Salvo como { $name } em { $folder }
cull-slot-skipped = { $folder } já tem { $name } — pulado
cull-slot-same = A imagem já está em { $folder }
cull-slot-failed = { $op ->
    [move] Não foi possível mover { $file }
   *[copy] Não foi possível copiar { $file }
}: { $reason }
cull-copy-kept = { $count ->
    [one] Mantida uma cópia alterada desde então
   *[other] Mantidas { $count } cópias alteradas desde então
}
cull-undo-failed = Não foi possível desfazer para { $file }: { $reason }
cull-burst-open = Sequência { $index } de { $count }
cull-burst-closed = Sequência de { $count }
cull-burst-toggle = Abrir ou fechar a sequência
cull-burst-best = 1 mantida, { $count } rejeitadas
cull-burst-none = Não faz parte de uma sequência
cull-follow-on = Seguindo arquivos novos
cull-follow-off = Sem seguir arquivos novos
cull-new-files = { $count ->
    [one] 1 arquivo novo
   *[other] { $count } arquivos novos
}
cull-key-copy = Copiar para a pasta 1–3
cull-key-move = Mover para a pasta 1–3
cull-key-burst = Abrir ou fechar uma sequência
cull-key-best = Manter esta, rejeitar o resto da sequência
cull-key-follow = Seguir arquivos novos
cull-key-subframes = Mostrar ou ocultar a tabela de subexposições
cull-key-sort = Ordenar pela próxima coluna
cull-key-sort-reverse = Inverter a ordem
cull-key-metric = Mostrar a próxima métrica no gráfico
cull-key-sigma-down = Rejeição automática: diminuir k
cull-key-sigma-up = Rejeição automática: aumentar k
cull-key-auto-reject = Rejeitar os quadros sugeridos
cull-key-move-rejects = Mover os quadros rejeitados para uma subpasta
cull-key-same-stretch = Mesmo stretch para todos os quadros

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Abrir a sequência
cull-tip-burst-close = Fechar a sequência
cull-tip-exit = Sair do modo de triagem
cull-tip-label = Rótulo: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Subexposições
cull-astro-title = { $count ->
    [one] { $count } subexposição
   *[other] { $count } subexposições
}
cull-astro-pending = medindo { $count }
cull-astro-col-name = Quadro
cull-astro-col-filter = Filtro
cull-astro-col-exposure = Exp.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Exc.
cull-astro-col-stars = Estrelas
cull-astro-col-bg = Fundo
cull-astro-col-snr = SNR
cull-astro-col-noise = Ruído
cull-astro-same-stretch = Mesmo stretch
cull-astro-own-stretch = Stretch próprio
cull-astro-suggest-high = { $metric } acima de { $value } (mediana + { $k }σ): { $count ->
    [one] { $count } quadro
   *[other] { $count } quadros
}
cull-astro-suggest-low = { $metric } abaixo de { $value } (mediana − { $k }σ): { $count ->
    [one] { $count } quadro
   *[other] { $count } quadros
}
cull-astro-suggest-none = Nenhum quadro além de { $k }σ
cull-astro-suggest-few = A rejeição automática precisa de 3 quadros medidos
cull-astro-suggested = sugerido para rejeição
cull-astro-k = k { $k }
cull-astro-reject = Rejeitar { $count }
cull-astro-move = Mover rejeitados…
cull-astro-chart = { $metric } em { $count ->
        [one] { $count } quadro
       *[other] { $count } quadros
    }, mediana { $median }.
cull-astro-sort-by = Ordenar por { $column }
cull-astro-ascending = crescente
cull-astro-descending = decrescente
cull-astro-sorted = Ordenado por { $column }
cull-astro-reversed = Ordem invertida
cull-astro-metric = Gráfico: { $metric }
cull-astro-k-set = Rejeição automática em { $k }σ
cull-astro-none = Nenhuma subexposição aqui
cull-astro-same-on = Mesmo stretch para todos os quadros
cull-astro-same-off = Cada quadro com stretch próprio
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = excentricidade { $value }
cull-astro-line-stars = { $count ->
    [one] { $count } estrela
   *[other] { $count } estrelas
}
cull-astro-measuring = medindo estrelas…
cull-astro-reject-confirm = { $count ->
    [one] Rejeitar o quadro sugerido?
   *[other] Rejeitar os { $count } quadros sugeridos?
}
cull-astro-reject-detail = { $rule }. Desfazer traz as marcações de volta.
cull-astro-reject-ok = Rejeitar
cull-astro-nothing-to-reject = Nenhum quadro para rejeitar
cull-astro-rejected = { $count ->
    [one] { $count } quadro rejeitado
   *[other] { $count } quadros rejeitados
}
cull-move-rejects-confirm = { $count ->
    [one] Mover o quadro rejeitado para a pasta “{ $folder }”?
   *[other] Mover os { $count } quadros rejeitados para a pasta “{ $folder }”?
}
cull-move-rejects-detail = Os arquivos são movidos, não excluídos. Desfazer os coloca de volta.
cull-move-rejects-ok = Mover
cull-move-rejects-none = Nenhum quadro rejeitado para mover
cull-moved-rejects = { $count ->
    [one] { $count } quadro movido para “{ $folder }”
   *[other] { $count } quadros movidos para “{ $folder }”
}
