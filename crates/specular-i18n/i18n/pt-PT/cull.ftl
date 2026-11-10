# machine-drafted, needs native review
# Specular — Modo de triagem (DESIGN.md §6.3).

palette-cull = Entrar ou sair do modo de triagem
cheatsheet-cull = Modo de triagem
settings-section-cull = Modo de triagem
settings-cull-auto-enter = Triar pastas RAW
settings-cull-auto-enter-help = Abrir uma pasta com sobretudo ficheiros RAW diretamente no modo de triagem.
settings-cull-auto-advance = Imagem seguinte após marcar
settings-cull-auto-advance-help = Avançar após escolher, rejeitar ou atribuir estrelas. As etiquetas e o desmarcar ficam na imagem.
settings-cull-show = Mostrar em pares RAW+JPEG
settings-cull-show-help = Um RAW e um JPEG com o mesmo nome são triados como uma só imagem e as marcas aplicam-se a ambos. O JPEG abre mais depressa.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Empilhar sequências
settings-cull-bursts-help = Fotografias tiradas em sequência rápida por uma câmara aparecem como uma pilha. S abre ou fecha uma pilha, B mantém a imagem no ecrã e rejeita o resto da pilha.
settings-cull-burst-gap = Intervalo da sequência
settings-cull-burst-gap-help = A pausa mais longa entre duas fotografias de uma sequência.
settings-cull-watch = Mostrar ficheiros novos
settings-cull-watch-help = Ficheiros que aparecem na pasta durante a triagem (câmara ligada, importação) são adicionados no fim quando acabam de ser escritos.
settings-cull-follow-newest = Seguir o ficheiro mais recente
settings-cull-follow-newest-help = Ir para cada ficheiro novo quando chega. T alterna isto durante a triagem.
settings-cull-slot-1 = Pasta 1
settings-cull-slot-1-help = Alt+1 copia a imagem (com o RAW ou JPEG e os ficheiros auxiliares) para esta pasta, Alt+Shift+1 move-a para lá.
settings-cull-slot-1-unset = Não definida
settings-cull-slot-2 = Pasta 2
settings-cull-slot-2-help = Alt+2 copia a imagem (com o RAW ou JPEG e os ficheiros auxiliares) para esta pasta, Alt+Shift+2 move-a para lá.
settings-cull-slot-2-unset = Não definida
settings-cull-slot-3 = Pasta 3
settings-cull-slot-3-help = Alt+3 copia a imagem (com o RAW ou JPEG e os ficheiros auxiliares) para esta pasta, Alt+Shift+3 move-a para lá.
settings-cull-slot-3-unset = Não definida
settings-cull-collisions = Quando um nome já existe
settings-cull-collisions-help = Os ficheiros nas pastas nunca são substituídos.
settings-cull-collisions-suffix = Acrescentar um número
settings-cull-collisions-skip = Ignorar a imagem
# machine-drafted, needs native review
# Modo de seleção escolhido sem pasta nem imagens da galeria abertas.
cull-nothing = Nada para triar — abra primeiro uma pasta de fotografias
cull-slot-unset = Nenhuma pasta { $slot } definida — escolha uma em Definições → Modo de triagem
cull-slot-missing = A pasta { $slot } não está disponível: { $reason }
cull-copied = Copiado para { $folder }
cull-moved = Movido para { $folder }
cull-slot-renamed = Guardado como { $name } em { $folder }
cull-slot-skipped = { $folder } já tem { $name } — ignorado
cull-slot-same = A imagem já está em { $folder }
cull-slot-failed = { $op ->
    [move] Não foi possível mover { $file }
   *[copy] Não foi possível copiar { $file }
}: { $reason }
cull-copy-kept = { $count ->
    [one] Mantida uma cópia alterada entretanto
   *[other] Mantidas { $count } cópias alteradas entretanto
}
cull-undo-failed = Não foi possível anular para { $file }: { $reason }
cull-burst-open = Sequência { $index } de { $count }
cull-burst-closed = Sequência de { $count }
cull-burst-toggle = Abrir ou fechar a sequência
cull-burst-best = 1 mantida, { $count } rejeitadas
cull-burst-none = Não faz parte de uma sequência
cull-follow-on = A seguir ficheiros novos
cull-follow-off = Sem seguir ficheiros novos
cull-new-files = { $count ->
    [one] 1 ficheiro novo
   *[other] { $count } ficheiros novos
}
cull-key-copy = Copiar para a pasta 1–3
cull-key-move = Mover para a pasta 1–3
cull-key-burst = Abrir ou fechar uma sequência
cull-key-best = Manter esta, rejeitar o resto da sequência
cull-key-follow = Seguir ficheiros novos
cull-key-subframes = Mostrar ou ocultar a tabela de subexposições
cull-key-sort = Ordenar pela coluna seguinte
cull-key-sort-reverse = Inverter a ordem
cull-key-metric = Mostrar a métrica seguinte no gráfico
cull-key-sigma-down = Rejeição automática: diminuir k
cull-key-sigma-up = Rejeição automática: aumentar k
cull-key-auto-reject = Rejeitar os fotogramas sugeridos
cull-key-move-rejects = Mover os fotogramas rejeitados para uma subpasta
cull-key-same-stretch = Mesmo esticamento para todos os fotogramas

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Abrir a sequência
cull-tip-burst-close = Fechar a sequência
cull-tip-exit = Sair do modo de triagem
cull-tip-label = Etiqueta: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Subexposições
cull-astro-title = { $count ->
    [one] 1 subexposição
   *[other] { $count } subexposições
}
cull-astro-pending = a medir { $count }
cull-astro-col-name = Fotograma
cull-astro-col-filter = Filtro
cull-astro-col-exposure = Exp.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Exc.
cull-astro-col-stars = Estrelas
cull-astro-col-bg = Fundo
cull-astro-col-snr = SNR
cull-astro-col-noise = Ruído
cull-astro-same-stretch = Mesmo esticamento
cull-astro-own-stretch = Esticamento próprio
cull-astro-suggest-high = { $metric } acima de { $value } (mediana + { $k }σ): { $count ->
    [one] 1 fotograma
   *[other] { $count } fotogramas
}
cull-astro-suggest-low = { $metric } abaixo de { $value } (mediana − { $k }σ): { $count ->
    [one] 1 fotograma
   *[other] { $count } fotogramas
}
cull-astro-suggest-none = Nenhum fotograma além de { $k }σ
cull-astro-suggest-few = A rejeição automática precisa de 3 fotogramas medidos
cull-astro-suggested = sugerido para rejeição
cull-astro-k = k { $k }
cull-astro-reject = Rejeitar { $count }
cull-astro-move = Mover rejeitados…
cull-astro-chart = { $metric } em { $count ->
        [one] { $count } fotograma
       *[other] { $count } fotogramas
    }, mediana { $median }.
cull-astro-sort-by = Ordenar por { $column }
cull-astro-ascending = crescente
cull-astro-descending = decrescente
cull-astro-sorted = Ordenado por { $column }
cull-astro-reversed = Ordem invertida
cull-astro-metric = Gráfico: { $metric }
cull-astro-k-set = Rejeição automática a { $k }σ
cull-astro-none = Nenhuma subexposição aqui
cull-astro-same-on = Mesmo esticamento para todos os fotogramas
cull-astro-same-off = Cada fotograma esticado à parte
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = excentricidade { $value }
cull-astro-line-stars = { $count ->
    [one] 1 estrela
   *[other] { $count } estrelas
}
cull-astro-measuring = a medir estrelas…
cull-astro-reject-confirm = { $count ->
    [one] Rejeitar o fotograma sugerido?
   *[other] Rejeitar os { $count } fotogramas sugeridos?
}
cull-astro-reject-detail = { $rule }. Anular repõe as marcações.
cull-astro-reject-ok = Rejeitar
cull-astro-nothing-to-reject = Nenhum fotograma para rejeitar
cull-astro-rejected = { $count ->
    [one] 1 fotograma rejeitado
   *[other] { $count } fotogramas rejeitados
}
cull-move-rejects-confirm = { $count ->
    [one] Mover o fotograma rejeitado para a pasta «{ $folder }»?
   *[other] Mover os { $count } fotogramas rejeitados para a pasta «{ $folder }»?
}
cull-move-rejects-detail = Os ficheiros são movidos, não eliminados. Anular volta a colocá-los no lugar.
cull-move-rejects-ok = Mover
cull-move-rejects-none = Nenhum fotograma rejeitado para mover
cull-moved-rejects = { $count ->
    [one] 1 fotograma movido para «{ $folder }»
   *[other] { $count } fotogramas movidos para «{ $folder }»
}
