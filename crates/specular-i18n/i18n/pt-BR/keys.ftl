# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Aguardar a próxima tecla
settings-keys-sequence-timeout-help = Quanto tempo uma sequência como G G espera pela próxima tecla. Uma tecla que também inicia uma sequência mais longa é executada após essa pausa.
settings-keys-bindings = Atalhos de teclado
settings-keys-bindings-help = Selecione uma ação e pressione as teclas a atribuir. Cada modo tem suas próprias teclas; o modo de triagem e as imagens médicas usam as do visualizador quando não há outra. Para atribuir Esc, toque nela; mantê-la pressionada cancela. Os atalhos são salvos em keymap.toml.
palette-customise-keys = Personalizar teclas…
keys-mode-viewer = Visualizador
keys-mode-cull = Modo de triagem
keys-mode-medical = Imagens médicas
keys-modes = Modo
keys-presets = Predefinição
keys-preset-default = Padrão
keys-preset-vim = Vim
keys-preset-culling = Triagem estilo Lightroom
keys-preset-photo-mechanic = Estilo Photo Mechanic
action-pan-left = Mover para a esquerda
action-pan-right = Mover para a direita
action-pan-up = Mover para cima
action-pan-down = Mover para baixo
action-pan-page-left = Mover uma tela para a esquerda
action-pan-page-right = Mover uma tela para a direita
action-pan-page-up = Mover uma tela para cima
action-pan-page-down = Mover uma tela para baixo
action-reload = Recarregar a imagem
keys-action-cull-pick = Escolher
keys-action-cull-reject = Rejeitar
keys-action-cull-unflag = Limpar a marcação
keys-action-cull-rate = { $stars ->
    [0] Limpar a classificação
    [one] Classificar com 1 estrela
   *[other] Classificar com { $stars } estrelas
}
keys-action-cull-label = Rótulo de cor: { $label }
keys-action-cull-copy-to = Copiar para a pasta { $slot }
keys-action-cull-move-to = Mover para a pasta { $slot }
keys-import = Importar…
keys-export = Exportar…
keys-export-mode = Exportar { $mode }…
keys-reset-mode = Redefinir { $mode }
keys-reset-all = Redefinir todas as teclas
keys-add = Adicionar uma tecla para { $action }
keys-change = Alterar { $keys }
keys-remove = Remover { $keys }
keys-reset-action = Redefinir { $action }
keys-none = Sem teclas
keys-changed = Alterado
keys-press = Pressione as teclas…
keys-recording-hint = Mantenha Esc pressionada para cancelar
keys-cancel-recording = Cancelar
keys-no-match = Nenhuma ação ou tecla corresponde a “{ $query }”.
keys-list = Ações e suas teclas
keys-confirm-preset = Usar a predefinição { $preset }? Ela substitui todos os seus atalhos.
keys-confirm-reset-mode = Redefinir todas as teclas de { $mode } para a predefinição?
keys-confirm-reset-all = Redefinir todas as teclas de todos os modos para a predefinição?
keys-confirm-ok = Substituir
keys-confirm-reset = Redefinir
keys-confirm-cancel = Cancelar
keys-conflict-same = { $keys } já executa “{ $other }”.
keys-conflict-prefix = { $keys } e { $sequence } (“{ $other }”) começam iguais: a mais curta só é executada após uma pausa.
keys-conflict-shadow = { $keys } executa “{ $other }” em { $mode }.
keys-conflict-reserved = { $keys } talvez nunca chegue ao Specular: { $reason }
keys-choice-replace = Substituir
keys-choice-swap = Trocar
keys-choice-keep = Manter ambas
keys-choice-anyway = Atribuir mesmo assim
keys-choice-cancel = Cancelar
keys-reserved-switch = ela alterna janelas.
keys-reserved-close = ela fecha a janela.
keys-reserved-secure = o sistema a usa.
keys-reserved-desktop = atalhos com Super pertencem à área de trabalho.
keys-reserved-workspaces = ela alterna terminais ou espaços de trabalho.
keys-reserved-input = ela alterna o método de entrada ou o layout do teclado.
keys-reserved-macos = o macOS a usa.
keys-announce-recording = Gravando teclas para { $action }. Pressione as teclas ou mantenha Esc pressionada para cancelar.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } agora executa { $action }.
keys-announce-already = { $keys } já executa { $action }.
keys-announce-cancelled = Gravação cancelada.
keys-announce-removed = { $keys } removida de { $action }.
keys-announce-reset-action = { $action } tem as teclas da predefinição novamente.
keys-announce-reset-mode = As teclas de { $mode } foram redefinidas.
keys-announce-reset-all = Todas as teclas foram redefinidas.
keys-announce-preset = Usando agora a predefinição { $preset }.
keys-toast-imported = Atalhos importados
keys-toast-exported = Atalhos exportados
keys-toast-reloaded = keymap.toml mudou: atalhos recarregados
keys-toast-error = Não foi possível ler os atalhos: { $error }
keys-toast-write-error = Não foi possível salvar keymap.toml: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = Não foi possível ler keymap.toml: { $error }
keys-problem-key = A tecla desconhecida “{ $key }” foi ignorada.
keys-problem-action = “{ $key }” indica uma ação desconhecida “{ $action }”; foi ignorada.
keys-problem-mode = A tabela [{ $mode }] não é um modo; foi ignorada.
keys-problem-duplicate = { $keys } aparece duas vezes em { $mode }; vale a segunda.
keys-problem-prefix = { $keys } inicia { $longer } em { $mode }, então só é executada após uma pausa.
keys-problem-preset = Predefinição desconhecida “{ $preset }”; valem as teclas padrão.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = Editar keymap.toml
keys-problem-action-suggestion = “{ $key }” indica uma ação desconhecida “{ $action }”; foi ignorada. Você quis dizer “{ $suggestion }”?
keys-problem-mode-suggestion = A tabela [{ $mode }] não é um modo; foi ignorada. Você quis dizer [{ $suggestion }]?
keys-problem-preset-suggestion = Predefinição desconhecida “{ $preset }”; valem as teclas padrão. Você quis dizer “{ $suggestion }”?
