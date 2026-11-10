# machine-drafted, needs native review
# Photographer polish pass: Develop regions, Crop card rows, import choices, keyword rename and move, auto-apply rules.

# Develop: F6 between the picture, the panel and the filmstrip.
palette-develop-next-region = Região seguinte
palette-develop-next-region-help = Leva o teclado para a frente: imagem, painel, tira de filme.
palette-develop-prev-region = Região anterior
palette-develop-prev-region-help = Leva o teclado para trás: tira de filme, painel, imagem.
develop-region-picture = Imagem
develop-region-panel = Painel

# Develop: the Crop card's rows (the crop tool's keys as rows).
develop-crop-aspect-row = Proporção
develop-crop-aspect-option-free = Livre
develop-crop-aspect-option-original = Original
develop-crop-aspect-option-custom = Personalizada
develop-crop-type-ratio = Escrever uma proporção…
develop-crop-swap = Trocar vertical e horizontal
develop-crop-turn-left = Rodar para a esquerda
develop-crop-turn-right = Rodar para a direita
develop-crop-flip-horizontal = Inverter da esquerda para a direita
develop-crop-flip-vertical = Inverter de cima para baixo
develop-crop-ratio-title = Proporção do recorte
develop-crop-ratio-field = Largura:altura
develop-crop-ratio-ok = Definir
develop-crop-ratio-bad = Escreva uma proporção como 5:4, ou um número como 1.85.

# Import: what each copied photo gets.
import-template = Modelo de metadados
import-style = Estilo de revelação
import-choice-none = Nenhum
# A choice row stepped, read out: "Metadata template: Copyright".
import-choice-said = { $label }: { $value }
import-keywords = Palavras-chave
import-keywords-hint = Por exemplo Família; Locais > Paris
import-style-missing = O estilo { $name } já não existe. As fotografias foram importadas sem ele.

# Keywords: rename, move and merge.
keywords-rename = Mudar o nome…
keywords-move = Mover para…
keywords-rename-of = Novo nome para { $keyword }
keywords-move-of = Colocar { $keyword } abaixo de (vazio: o topo)
keywords-move-into-itself = { $keyword } não pode ficar abaixo de si própria.
keywords-renaming = A mudar o nome de { $from } para { $to }…
keywords-merging = A fundir { $from } em { $to }…
keywords-renamed = { $from } é agora { $to } em { $count ->
    [one] { $count } fotografia
   *[other] { $count } fotografias
}
keywords-rename-row = Palavra-chave { $from } para { $to }
keywords-rename-finding = A procurar fotografias…
keywords-rename-row-detail = { $done } de { $total } fotografias

# Develop: the before/after divider dragged ("Divider at 30 %").
develop-split-moved = Divisória em { PERCENT($percent) }

# Develop: auto-apply rules, changed row by row.
develop-rules-placeholder-edit = Escolha uma regra para a alterar ou eliminar
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = Fabricante: { $value }
develop-rule-model = Modelo: { $value }
develop-rule-lens = Objetiva: { $value }
develop-rule-iso = ISO: { $value }
develop-rule-type = Tipo de ficheiro: { $value }
develop-rule-style = Estilo: { $value }
develop-rule-any = qualquer
develop-rule-type-any = qualquer ficheiro
develop-rule-delete = Eliminar regra…
develop-rule-field-title = Alterar “{ $rule }”
develop-rule-field-maker = Fabricante (basta parte do nome; vazio: qualquer)
develop-rule-field-model = Modelo (basta parte do nome; vazio: qualquer)
develop-rule-field-lens = Objetiva (basta parte do nome; vazio: qualquer)
develop-rule-field-iso = ISO, como 100-800, 1600- ou -400 (vazio: qualquer)
develop-rule-field-ok = Definir
develop-rule-iso-bad = Escreva um intervalo de ISO como 100-800.
develop-rule-style-placeholder = Escolha o estilo da regra
