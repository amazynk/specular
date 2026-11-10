# machine-drafted, needs native review
# Specular — Duplicados e fotografias semelhantes (categorias da galeria). Português europeu.

similar-duplicates = Duplicados
similar-similar = Fotografias semelhantes

## Menu Ver e entradas da paleta
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } grupo
   *[other] { $groups } grupos
}, { $size } a libertar
similar-menu-scanning = { $name } — a analisar ({ PERCENT($percent) })
similar-menu-none = { $name } — nenhum encontrado

## Cabeçalho
similar-summary = { $groups ->
    [one] { $groups } grupo para rever
   *[other] { $groups } grupos para rever
}, { $files ->
    [one] { $files } ficheiro
   *[other] { $files } ficheiros
} a descartar, { $size } a libertar
similar-progress = { $resolved } de { $total } resolvidos
similar-skipped = { $count } ignorados
similar-scanning = A procurar duplicados e fotografias semelhantes… { $done } de { $total }
# machine-drafted, needs native review
similar-checking-sharpness = A verificar a nitidez… { $done } de { $total }
# machine-drafted, needs native review
# A lista enquanto decorre a primeira análise (o progresso está no cabeçalho acima).
similar-waiting = Os grupos aparecem aqui quando a análise terminar.
# machine-drafted, needs native review
similar-stopped = Análise interrompida: nada foi agrupado
similar-stopped-sharpness = Verificação da nitidez interrompida: melhores escolhas a partir das miniaturas
similar-stopped-empty = A análise foi interrompida. O que mediu é mantido: Analisar novamente prossegue a partir daí.
similar-scan-again = Analisar novamente
similar-empty-duplicates = Não há duplicados nesta pasta.
similar-empty-similar = Não há fotografias semelhantes nesta pasta.
similar-all-done = Todos os grupos foram revistos.

## Fotografias semelhantes: quão parecidas têm de ser para formar um grupo
similar-sensitivity = Sensibilidade
similar-sensitivity-strict = Rigorosa
similar-sensitivity-loose = Flexível

## Rodapé da lista de grupos
similar-accept-all = Aceitar todas as sugestões
similar-show-skipped = Mostrar grupos ignorados
similar-discard-with = Descartar: { $action }
similar-action-trash = Mover para o Lixo
similar-action-move = Mover para uma pasta…
similar-action-move-to = Mover para «{ $folder }»
similar-action-reject = Apenas marcar como rejeitada
similar-choose-folder = Escolher pasta para os ficheiros descartados…

## Confirmação em massa
similar-confirm = Aceitar as sugestões de { $groups ->
    [one] { $groups } grupo
   *[other] { $groups } grupos
}? { $files ->
    [one] { $files } ficheiro
   *[other] { $files } ficheiros
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Aceitar e descartar { $files } ficheiro
   *[other] Aceitar e descartar { $files } ficheiros
}
similar-cancel = Cancelar

## Um grupo
similar-group-title = Grupo { $index } de { $total }
similar-group-photos = { $count ->
    [one] { $count } fotografia
   *[other] { $count } fotografias
}
similar-to-free = { $size } a libertar

## Ações e estados de revisão
similar-keep = Manter
similar-discard = Descartar
similar-undecided = Por decidir
similar-accept-next = Aceitar e seguinte
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
    [one] { $count } grupo aceite
   *[other] { $count } grupos aceites
}
similar-toast-rejected = { $count ->
    [one] { $count } ficheiro marcado como rejeitado
   *[other] { $count } ficheiros marcados como rejeitados
}
similar-toast-protected = As fotografias classificadas, marcadas, etiquetadas, com palavras-chave ou editadas são sempre mantidas.
# machine-drafted, needs native review
similar-zoom-already = Esta fotografia já é apresentada a { PERCENT(100) } ou maior

## Acessibilidade
similar-a11y-member = { $name }: { $state }, { $reason }

## Dicas de teclas
similar-hint-list = ↑↓ escolher · Enter rever · A aceitar tudo · Tab alternar · Esc fechar

## Porque é mantida uma fotografia
similar-reason-rated = Classificada
similar-reason-flagged = Marcada como escolhida
similar-reason-labelled = Tem etiqueta de cor
similar-reason-tagged = Tem palavras-chave
similar-reason-edited = Tem edições
similar-reason-original = O original
similar-reason-raw-original = Original RAW
similar-reason-highest-resolution = Maior resolução
similar-reason-deeper-colour = Maior profundidade de cor
similar-reason-least-compressed = Menos comprimida
similar-reason-has-metadata = Mantém os metadados
similar-reason-oldest = Cópia mais antiga
similar-reason-sharpest = Mais nítida
similar-reason-steadiest = Menos arrastamento
similar-reason-best-exposure = Melhor exposição
similar-reason-least-noise = Menos ruído
similar-reason-also-good = Também é boa

## Porque se sugere descartar uma fotografia
similar-reason-same-file = Ficheiro idêntico
similar-reason-derivative = Cópia derivada
similar-reason-rendering-of-raw = Revelação do RAW
similar-reason-smaller-copy = Cópia mais pequena
similar-reason-shallower-colour = Menor profundidade de cor
similar-reason-more-compressed = Mais comprimida
similar-reason-no-metadata = Metadados removidos
similar-reason-newer-copy = Cópia mais recente
similar-reason-blurrier = Mais desfocada
similar-reason-motion-blur = Arrastamento
similar-reason-overexposed = Sobreexposta
similar-reason-underexposed = Subexposta
similar-reason-noisier = Mais ruído
similar-reason-lower-resolution = Resolução inferior
