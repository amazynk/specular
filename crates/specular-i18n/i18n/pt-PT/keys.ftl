# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Aguardar a tecla seguinte
settings-keys-sequence-timeout-help = Quanto tempo uma sequência como G G espera pela tecla seguinte. Uma tecla que também inicia uma sequência mais longa é executada após esta pausa.
settings-keys-bindings = Atalhos de teclado
settings-keys-bindings-help = Selecione uma ação e prima as teclas a atribuir. Cada modo tem as suas teclas; o modo de triagem e as imagens médicas recorrem às do visualizador. Para atribuir Esc, toque-lhe; mantê-la premida cancela. Os atalhos são guardados em keymap.toml.
palette-customise-keys = Personalizar teclas…
keys-mode-viewer = Visualizador
keys-mode-cull = Modo de triagem
keys-mode-medical = Imagens médicas
keys-modes = Modo
keys-presets = Predefinição
keys-preset-default = Predefinido
keys-preset-vim = Vim
keys-preset-culling = Triagem ao estilo Lightroom
keys-preset-photo-mechanic = Ao estilo Photo Mechanic
action-pan-left = Deslocar para a esquerda
action-pan-right = Deslocar para a direita
action-pan-up = Deslocar para cima
action-pan-down = Deslocar para baixo
action-pan-page-left = Deslocar um ecrã para a esquerda
action-pan-page-right = Deslocar um ecrã para a direita
action-pan-page-up = Deslocar um ecrã para cima
action-pan-page-down = Deslocar um ecrã para baixo
action-reload = Recarregar a imagem
keys-action-cull-pick = Escolher
keys-action-cull-reject = Rejeitar
keys-action-cull-unflag = Limpar a marcação
keys-action-cull-rate = { $stars ->
    [0] Limpar a classificação
    [one] Classificar com 1 estrela
   *[other] Classificar com { $stars } estrelas
}
keys-action-cull-label = Etiqueta de cor: { $label }
keys-action-cull-copy-to = Copiar para a pasta { $slot }
keys-action-cull-move-to = Mover para a pasta { $slot }
keys-import = Importar…
keys-export = Exportar…
keys-export-mode = Exportar { $mode }…
keys-reset-mode = Repor { $mode }
keys-reset-all = Repor todas as teclas
keys-add = Adicionar uma tecla para { $action }
keys-change = Alterar { $keys }
keys-remove = Remover { $keys }
keys-reset-action = Repor { $action }
keys-none = Sem teclas
keys-changed = Alterado
keys-press = Prima as teclas…
keys-recording-hint = Mantenha Esc premida para cancelar
keys-cancel-recording = Cancelar
keys-no-match = Nenhuma ação ou tecla corresponde a «{ $query }».
keys-list = Ações e as suas teclas
keys-confirm-preset = Usar a predefinição { $preset }? Substitui todos os seus atalhos.
keys-confirm-reset-mode = Repor todas as teclas de { $mode } na predefinição?
keys-confirm-reset-all = Repor todas as teclas de todos os modos na predefinição?
keys-confirm-ok = Substituir
keys-confirm-reset = Repor
keys-confirm-cancel = Cancelar
keys-conflict-same = { $keys } já executa «{ $other }».
keys-conflict-prefix = { $keys } e { $sequence } («{ $other }») começam da mesma forma: a mais curta só é executada após uma pausa.
keys-conflict-shadow = { $keys } executa «{ $other }» em { $mode }.
keys-conflict-reserved = { $keys } pode nunca chegar ao Specular: { $reason }
keys-choice-replace = Substituir
keys-choice-swap = Trocar
keys-choice-keep = Manter ambas
keys-choice-anyway = Atribuir mesmo assim
keys-choice-cancel = Cancelar
keys-reserved-switch = alterna janelas.
keys-reserved-close = fecha a janela.
keys-reserved-secure = o sistema usa-a.
keys-reserved-desktop = os atalhos com Super pertencem ao ambiente de trabalho.
keys-reserved-workspaces = alterna terminais ou áreas de trabalho.
keys-reserved-input = alterna o método de introdução ou a disposição do teclado.
keys-reserved-macos = o macOS usa-a.
keys-announce-recording = A gravar teclas para { $action }. Prima as teclas ou mantenha Esc premida para cancelar.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } executa agora { $action }.
keys-announce-already = { $keys } já executa { $action }.
keys-announce-cancelled = Gravação cancelada.
keys-announce-removed = { $keys } removida de { $action }.
keys-announce-reset-action = { $action } tem novamente as teclas da predefinição.
keys-announce-reset-mode = As teclas de { $mode } foram repostas.
keys-announce-reset-all = Todas as teclas foram repostas.
keys-announce-preset = A usar agora a predefinição { $preset }.
keys-toast-imported = Atalhos importados
keys-toast-exported = Atalhos exportados
keys-toast-reloaded = keymap.toml alterado: atalhos recarregados
keys-toast-error = Não foi possível ler os atalhos: { $error }
keys-toast-write-error = Não foi possível guardar keymap.toml: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = Não foi possível ler keymap.toml: { $error }
keys-problem-key = A tecla desconhecida «{ $key }» foi ignorada.
keys-problem-action = «{ $key }» indica uma ação desconhecida «{ $action }»; foi ignorada.
keys-problem-mode = A tabela [{ $mode }] não é um modo; foi ignorada.
keys-problem-duplicate = { $keys } aparece duas vezes em { $mode }; vale a segunda.
keys-problem-prefix = { $keys } inicia { $longer } em { $mode }, por isso só é executada após uma pausa.
keys-problem-preset = Predefinição desconhecida «{ $preset }»; aplicam-se as teclas predefinidas.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = Editar keymap.toml
keys-problem-action-suggestion = «{ $key }» indica uma ação desconhecida «{ $action }»; foi ignorada. «{ $suggestion }»ueria dizer «{ $suggestion }»?
keys-problem-mode-suggestion = A tabela [{ $mode }] não é um modo; foi ignorada. [{ $suggestion }]ueria dizer [{ $suggestion }]?
keys-problem-preset-suggestion = Predefinição desconhecida «{ $preset }»; aplicam-se as teclas predefinidas. «{ $suggestion }»ueria dizer «{ $suggestion }»?
