# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Exportar com receitas…
palette-export-recipes-help = Exportar a seleção com uma ou mais receitas salvas.
palette-develop-export-recipes = Exportar com receitas…
palette-develop-export-recipes-help = Exportar a foto editada, ou várias, com receitas salvas.

export-recipes-title = Exportar com receitas
export-recipes-new = Nova
export-recipes-edit = Editar…
export-recipes-duplicate = Duplicar
export-recipes-delete = Excluir…
export-recipes-start = Exportar
export-recipes-save = Salvar
export-recipes-open-folder = Abrir pasta
export-recipes-choose-image = Escolher imagem…
export-recipes-no-images = Nenhuma imagem para exportar.
export-recipes-none-checked = Marque pelo menos uma receita.
export-recipes-checked = { $name }: ativada
export-recipes-unchecked = { $name }: desativada

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Impressão, tamanho completo
export-recipes-starter-email = E-mail, pequeno
export-recipes-unnamed = Receita sem nome
export-recipes-new-name = Nova receita
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (cópia)

## The list's one-line summary.
export-recipes-summary-mark = marca d'água
export-recipes-size-full = Tamanho completo
export-recipes-size-long = { $px } px no lado maior
export-recipes-size-short = { $px } px no lado menor
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Junto ao original
export-recipes-folder-sub = Pasta “{ $name }”
export-recipes-folder-ask = Perguntar sempre

## The editor.
export-recipes-new-title = Nova receita
export-recipes-edit-title = Editar receita
export-recipes-name = Nome da receita
export-recipes-text = Texto personalizado
export-recipes-pattern = Nome do arquivo
export-recipes-mark-text-field = Texto da marca d'água
export-recipes-subfolder = Nome da pasta
export-recipes-format = Formato
export-recipes-quality = Qualidade
export-recipes-lossless = Sem perdas
export-recipes-bits = Profundidade de bits
export-recipes-bits-8 = 8 bits
export-recipes-bits-16 = 16 bits
export-recipes-space = Espaço de cor
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Perfil
export-recipes-profile-embed = Incorporar
export-recipes-profile-none = Omitir
export-recipes-folder = Pasta
export-recipes-folder-subfolder = Subpasta
export-recipes-collision = Nome já existe
export-recipes-collision-unique = Adicionar um número
export-recipes-collision-overwrite = Substituir (pergunta)
export-recipes-collision-skip = Pular
export-recipes-after = Ao terminar
export-recipes-after-nothing = Nada
export-recipes-after-open = Abrir a pasta
export-recipes-resize = Tamanho
export-recipes-resize-long = Lado maior
export-recipes-resize-short = Lado menor
export-recipes-resize-mp = Megapixels
export-recipes-resize-exact = Tamanho exato
export-recipes-resize-percent = Porcentagem
export-recipes-edge = Lado
export-recipes-width = Largura
export-recipes-height = Altura
export-recipes-megapixels = Megapixels
export-recipes-percent = Porcentagem
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Ampliar
export-recipes-sharpen = Nitidez para
export-recipes-sharpen-off = Desativada
export-recipes-sharpen-screen = Tela
export-recipes-sharpen-matte = Papel fosco
export-recipes-sharpen-glossy = Papel brilhante
export-recipes-amount = Intensidade
export-recipes-amount-low = Baixa
export-recipes-amount-standard = Padrão
export-recipes-amount-high = Alta
export-recipes-metadata = Metadados
export-recipes-meta-all = Todos
export-recipes-meta-copyright = Só direitos autorais
export-recipes-meta-contact = Direitos autorais e contato
export-recipes-meta-none = Nenhum
export-recipes-location = Localização
export-recipes-people = Pessoas na foto
export-recipes-keep = Manter
export-recipes-remove = Remover
export-recipes-mark = Marca d'água
export-recipes-mark-none = Nenhuma
export-recipes-mark-text = Texto
export-recipes-mark-image = Imagem (escolha uma)
export-recipes-position = Posição
export-recipes-pos-top-left = Superior esquerdo
export-recipes-pos-top = Superior
export-recipes-pos-top-right = Superior direito
export-recipes-pos-left = Esquerda
export-recipes-pos-centre = Centro
export-recipes-pos-right = Direita
export-recipes-pos-bottom-left = Inferior esquerdo
export-recipes-pos-bottom = Inferior
export-recipes-pos-bottom-right = Inferior direito
export-recipes-opacity = Opacidade
export-recipes-scale = Largura
export-recipes-yes = Sim
export-recipes-no = Não
export-recipes-error-name = Dê um nome à receita.
export-recipes-error-mark-text = Digite o texto da marca d'água.
export-recipes-error-mark-image = Escolha a imagem da marca d'água.
export-recipes-error-subfolder = Digite o nome da pasta.
export-recipes-saved = Receita “{ $name }” salva.

## Delete.
export-recipes-delete-title = Excluir a receita “{ $name }”?
export-recipes-delete-detail = Os arquivos que ela exportou continuam no disco.
export-recipes-delete-confirm = Excluir
export-recipes-delete-keep = Manter
export-recipes-deleted = Receita “{ $name }” excluída.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Pasta para “{ $name }”
export-recipes-asking = Escolha a pasta de cada receita que pergunta.
export-recipes-no-folder = Nenhuma pasta escolhida. Nada foi exportado.
export-recipes-planning = Nomeando os arquivos…
export-recipes-plan-pattern = Receita “{ $name }”: { $reason }
export-recipes-replace-title = { $count ->
    [one] Substituir 1 arquivo?
   *[other] Substituir { $count } arquivos?
}
export-recipes-replace-detail = Os originais nunca são substituídos.
export-recipes-replace-confirm = Substituir
export-recipes-replace-kept = Nada foi exportado.
export-recipes-running = Exportando…
export-recipes-progress = { $done } de { $total } arquivos
export-recipes-done = { $count ->
    [one] 1 arquivo exportado.
   *[other] { $count } arquivos exportados.
}
export-recipes-stopped = { $count ->
    [one] Parado. 1 arquivo exportado.
   *[other] Parado. { $count } arquivos exportados.
}
export-recipes-done-skipped = { $count ->
    [one] 1 pulado: o nome já existia.
   *[other] { $count } pulados: os nomes já existiam.
}
export-recipes-done-failed = { $count ->
    [one] 1 falhou:
   *[other] { $count } falharam:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
