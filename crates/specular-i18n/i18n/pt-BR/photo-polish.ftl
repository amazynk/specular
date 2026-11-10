# machine-drafted, needs native review
# Photographer polish pass: Develop regions, Crop card rows, import choices, keyword rename and move, auto-apply rules.

# Develop: F6 between the picture, the panel and the filmstrip.
palette-develop-next-region = Próxima região
palette-develop-next-region-help = Leva o teclado adiante: imagem, painel, tira de filme.
palette-develop-prev-region = Região anterior
palette-develop-prev-region-help = Leva o teclado de volta: tira de filme, painel, imagem.
develop-region-picture = Imagem
develop-region-panel = Painel

# Develop: the Crop card's rows (the crop tool's keys as rows).
develop-crop-aspect-row = Proporção
develop-crop-aspect-option-free = Livre
develop-crop-aspect-option-original = Original
develop-crop-aspect-option-custom = Personalizada
develop-crop-type-ratio = Digitar uma proporção…
develop-crop-swap = Trocar retrato e paisagem
develop-crop-turn-left = Girar para a esquerda
develop-crop-turn-right = Girar para a direita
develop-crop-flip-horizontal = Espelhar da esquerda para a direita
develop-crop-flip-vertical = Espelhar de cima para baixo
develop-crop-ratio-title = Proporção do corte
develop-crop-ratio-field = Largura:altura
develop-crop-ratio-ok = Definir
develop-crop-ratio-bad = Digite uma proporção como 5:4, ou um número como 1.85.

# Import: what each copied photo gets.
import-template = Modelo de metadados
import-style = Estilo de revelação
import-choice-none = Nenhum
# A choice row stepped, read out: "Metadata template: Copyright".
import-choice-said = { $label }: { $value }
import-keywords = Palavras-chave
import-keywords-hint = Por exemplo Família; Lugares > Paris
import-style-missing = O estilo { $name } não existe mais. As fotos foram importadas sem ele.

# Keywords: rename, move and merge.
keywords-rename = Renomear…
keywords-move = Mover para…
keywords-rename-of = Novo nome para { $keyword }
keywords-move-of = Colocar { $keyword } abaixo de (vazio: o topo)
keywords-move-into-itself = { $keyword } não pode ficar abaixo de si mesma.
keywords-renaming = Renomeando { $from } para { $to }…
keywords-merging = Mesclando { $from } em { $to }…
keywords-renamed = { $from } agora é { $to } em { $count ->
    [one] { $count } foto
   *[other] { $count } fotos
}
keywords-rename-row = Palavra-chave { $from } para { $to }
keywords-rename-finding = Procurando fotos…
keywords-rename-row-detail = { $done } de { $total } fotos

# Develop: the before/after divider dragged ("Divider at 30 %").
develop-split-moved = Divisória em { PERCENT($percent) }

# Develop: auto-apply rules, changed row by row.
develop-rules-placeholder-edit = Escolha uma regra para alterá-la ou excluí-la
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = Fabricante: { $value }
develop-rule-model = Modelo: { $value }
develop-rule-lens = Lente: { $value }
develop-rule-iso = ISO: { $value }
develop-rule-type = Tipo de arquivo: { $value }
develop-rule-style = Estilo: { $value }
develop-rule-any = qualquer
develop-rule-type-any = qualquer arquivo
develop-rule-delete = Excluir regra…
develop-rule-field-title = Alterar “{ $rule }”
develop-rule-field-maker = Fabricante (parte do nome basta; vazio: qualquer)
develop-rule-field-model = Modelo (parte do nome basta; vazio: qualquer)
develop-rule-field-lens = Lente (parte do nome basta; vazio: qualquer)
develop-rule-field-iso = ISO, como 100-800, 1600- ou -400 (vazio: qualquer)
develop-rule-field-ok = Definir
develop-rule-iso-bad = Digite um intervalo de ISO como 100-800.
develop-rule-style-placeholder = Escolha o estilo da regra
