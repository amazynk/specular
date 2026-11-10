# machine-drafted, needs native review
# Specular — Duplicatas e fotos semelhantes (categorias da galeria). Português do Brasil.

similar-duplicates = Duplicatas
similar-similar = Fotos semelhantes

## Entradas do menu Exibir e da paleta
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } grupo
   *[other] { $groups } grupos
}, { $size } a liberar
similar-menu-scanning = { $name } — analisando ({ PERCENT($percent) })
similar-menu-none = { $name } — nada encontrado

## Cabeçalho
similar-summary = { $groups ->
    [one] { $groups } grupo para revisar
   *[other] { $groups } grupos para revisar
}, { $files ->
    [one] { $files } arquivo
   *[other] { $files } arquivos
} para descartar, { $size } a liberar
similar-progress = { $resolved } de { $total } resolvidos
similar-skipped = { $count } ignorados
similar-scanning = Procurando duplicatas e fotos semelhantes… { $done } de { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Verificando a nitidez… { $done } de { $total }
# machine-drafted, needs native review
# A lista enquanto a primeira varredura é executada (o progresso está no cabeçalho acima).
similar-waiting = Os grupos aparecem aqui quando a varredura terminar.
# machine-drafted, needs native review
similar-stopped = Varredura interrompida: nada foi agrupado
similar-stopped-sharpness = Verificação de nitidez interrompida: melhores escolhas pelas miniaturas
similar-stopped-empty = A varredura foi interrompida. O que ela mediu foi mantido: Varrer novamente continua a partir daí.
similar-scan-again = Varrer novamente
similar-empty-duplicates = Nenhuma duplicata nesta pasta.
similar-empty-similar = Nenhuma foto semelhante nesta pasta.
similar-all-done = Todos os grupos revisados.

## Fotos semelhantes: quão parecidas as fotos precisam ser para agrupar
similar-sensitivity = Sensibilidade
similar-sensitivity-strict = Rigorosa
similar-sensitivity-loose = Flexível

## Rodapé da lista de grupos
similar-accept-all = Aceitar todas as sugestões
similar-show-skipped = Mostrar grupos ignorados
similar-discard-with = Descartar: { $action }
similar-action-trash = Mover para a Lixeira
similar-action-move = Mover para uma pasta…
similar-action-move-to = Mover para “{ $folder }”
similar-action-reject = Apenas marcar como rejeitada
similar-choose-folder = Escolher pasta para os arquivos descartados…

## Confirmação em massa
similar-confirm = Aceitar as sugestões de { $groups ->
    [one] { $groups } grupo
   *[other] { $groups } grupos
}? { $files ->
    [one] { $files } arquivo
   *[other] { $files } arquivos
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Aceitar e descartar { $files } arquivo
   *[other] Aceitar e descartar { $files } arquivos
}
similar-cancel = Cancelar

## Um grupo
similar-group-title = Grupo { $index } de { $total }
similar-group-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } fotos
}
similar-to-free = { $size } a liberar

## Ações e estados da revisão
similar-keep = Manter
similar-discard = Descartar
similar-undecided = Indeciso
similar-accept-next = Aceitar e avançar
similar-not-similar = Não semelhantes
similar-skip = Ignorar
similar-compare = Comparar
similar-zoom = { PERCENT(100) }
similar-back = Voltar
similar-done = Concluído
similar-suggested-keep = Sugestão: manter
similar-suggested-discard = Sugestão: descartar

## Notificações
similar-toast-accepted = { $count ->
    [one] { $count } grupo aceito
   *[other] { $count } grupos aceitos
}
similar-toast-rejected = { $count ->
    [one] { $count } arquivo marcado como rejeitado
   *[other] { $count } arquivos marcados como rejeitados
}
similar-toast-protected = Fotos classificadas, marcadas, com rótulo, com palavras-chave ou editadas são sempre mantidas.
# machine-drafted, needs native review
similar-zoom-already = Esta foto já é exibida em { PERCENT(100) } ou maior

## Acessibilidade
similar-a11y-member = { $name }: { $state }, { $reason }

## Dicas de teclas
similar-hint-list = ↑↓ escolher · Enter revisar · A aceitar tudo · Tab alternar · Esc fechar

## Por que uma foto é mantida
similar-reason-rated = Classificada
similar-reason-flagged = Marcada como escolhida
similar-reason-labelled = Tem rótulo de cor
similar-reason-tagged = Tem palavras-chave
similar-reason-edited = Tem edições
similar-reason-original = O original
similar-reason-raw-original = Original RAW
similar-reason-highest-resolution = Maior resolução
similar-reason-deeper-colour = Maior profundidade de cor
similar-reason-least-compressed = Menos compactada
similar-reason-has-metadata = Mantém os metadados
similar-reason-oldest = Cópia mais antiga
similar-reason-sharpest = Mais nítida
similar-reason-steadiest = Menos desfoque de movimento
similar-reason-best-exposure = Melhor exposição
similar-reason-least-noise = Menos ruído
similar-reason-also-good = Também é boa

## Por que uma foto é sugerida para descarte
similar-reason-same-file = Arquivo idêntico
similar-reason-derivative = Cópia derivada
similar-reason-rendering-of-raw = Renderização do RAW
similar-reason-smaller-copy = Cópia menor
similar-reason-shallower-colour = Menor profundidade de cor
similar-reason-more-compressed = Mais compactada
similar-reason-no-metadata = Metadados removidos
similar-reason-newer-copy = Cópia mais recente
similar-reason-blurrier = Mais desfocada
similar-reason-motion-blur = Desfoque de movimento
similar-reason-overexposed = Superexposta
similar-reason-underexposed = Subexposta
similar-reason-noisier = Mais ruído
similar-reason-lower-resolution = Menor resolução
