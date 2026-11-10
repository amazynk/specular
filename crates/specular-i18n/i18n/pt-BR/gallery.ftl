# machine-drafted, needs native review
# Specular — a galeria (modo de miniaturas, DESIGN §10) e Configurações → Galeria. Português do Brasil.

## Paleta e menus

palette-gallery = Galeria
gallery-palette-view = Exibição da galeria…
gallery-palette-layout = Layout
gallery-palette-sort = Ordenar por
gallery-palette-group = Agrupar por
gallery-palette-filter = Filtrar…
gallery-palette-quick-filters = Filtros rápidos
gallery-palette-details = Exibição de detalhes
gallery-palette-info-overlay = Informações nas miniaturas
gallery-palette-stacks = Empilhar RAW+JPEG e sequências
gallery-palette-subfolders = Incluir subpastas
gallery-palette-compare = Comparar seleção
gallery-palette-rate = Classificar…
gallery-palette-settings = Configurações da galeria…
gallery-view-menu = Exibir
gallery-more-menu = Mais
gallery-open = Abrir
gallery-new-window = Abrir em nova janela
gallery-reveal = Mostrar na pasta
gallery-properties = Propriedades
gallery-copy-paths = Copiar caminhos
gallery-select-all = Selecionar tudo
gallery-select-none = Selecionar nada
gallery-select-invert = Inverter seleção
gallery-next-unrated = Próxima sem classificação
gallery-stack-toggle = Expandir ou recolher pilha
gallery-run-on = { $name } ({ $count } arquivos)
gallery-depth-off = Somente esta pasta
gallery-depth-all = Todas as subpastas
gallery-depth-levels = { $depth ->
    [one] { $depth } nível de subpastas
   *[other] { $depth } níveis de subpastas
}

## Status e estados vazios

gallery-items = { $count ->
    [one] { $count } item
   *[other] { $count } itens
}
gallery-selected = { $count } de { $total } selecionados
gallery-shown-of = { $count } de { $total } exibidos
gallery-listing = Lendo pasta… { $count }
gallery-empty = Esta pasta não tem imagens.
gallery-empty-filter = Nenhuma imagem corresponde ao filtro.
gallery-list-error = Não é possível ler esta pasta: { $reason }
gallery-no-folder = Nenhuma pasta aberta
gallery-kind-folder = Pasta
gallery-drag-count = { $count ->
    [one] { $count } arquivo
   *[other] { $count } arquivos
}
gallery-badge-pages = { $count } páginas

## Barra de filtro

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Nome, ou rating>=3 camera:X-T5 …
gallery-filter-clear = Limpar filtro
gallery-filter-rating = { $stars ->
    [one] ★ { $stars } ou mais
   *[other] ★ { $stars } ou mais
}
gallery-filter-label = Rótulo: { $label }
gallery-filter-picks = Escolhidas
gallery-filter-hide-rejects = Ocultar rejeitadas
gallery-filter-unrated = Sem classificação
gallery-filter-raw = RAW
gallery-filter-animated = Animadas
gallery-filter-hdr = HDR
gallery-filter-landscape = Paisagem
gallery-filter-portrait = Retrato

## Query errors and facets

gallery-query-unknown-field = Campo desconhecido “{ $field }”
gallery-query-unknown-field-suggest = Campo desconhecido “{ $field }”: você quis dizer “{ $suggestion }”?
gallery-query-missing-value = “{ $field }” precisa de um valor
gallery-query-bad-number = “{ $value }” não é um número ({ $field })
gallery-query-bad-date = “{ $value }” não é uma data ({ $field }): use 2024, 2024-05 ou 2024-05-12
gallery-query-bad-word = “{ $value }” não serve para { $field }: use { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = “{ $value }” não é um nome de tag Exif
gallery-query-no-compare = { $field } não pode ser comparado com < ou > aqui
gallery-query-unterminated = Falta fechar as aspas
gallery-query-unavailable = “{ $field }” precisa da Biblioteca (indisponível aqui)
gallery-facets = Facetas
gallery-facets-counting = Contando…
gallery-facets-empty = Nada para contar
gallery-facets-search = Buscar nas facetas
gallery-facets-no-match = Nenhum valor corresponde
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Tela inicial
gallery-facet-format = Formato
gallery-facet-camera = Câmera
gallery-facet-lens = Lente
gallery-facet-rating = Classificação
gallery-facet-year = Ano
gallery-facet-orientation = Orientação
gallery-facet-label = Rótulo
gallery-facet-properties = Propriedades
gallery-facet-square = Quadrada
gallery-facet-alpha = Transparência
gallery-facet-gps = Localização
gallery-facet-multipage = Várias páginas
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } arquivo
   *[other] { $count } arquivos
}

## Grupos

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Esta pasta
gallery-group-unknown = Desconhecido
gallery-group-unrated = Sem classificação

## Seleção de fotos

gallery-stars = { $stars ->
    [one] { $stars } estrela
   *[other] { $stars } estrelas
}
gallery-rate-none = Sem classificação
gallery-pick = Escolher
gallery-reject = Rejeitar
gallery-unflag = Sem marcação
gallery-label-red = Vermelho
gallery-label-yellow = Amarelo
gallery-label-green = Verde
gallery-label-blue = Azul
gallery-label-none = Sem rótulo
gallery-compare-hint = Selecione de 2 a 4 imagens para compará-las.

## Arquivos

gallery-trash-count = { $count ->
    [one] Mover { $count } arquivo para a Lixeira
   *[other] Mover { $count } arquivos para a Lixeira
}
gallery-moved-to = { $count ->
    [one] { $count } arquivo movido para { $folder }
   *[other] { $count } arquivos movidos para { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } arquivo copiado para { $folder }
   *[other] { $count } arquivos copiados para { $folder }
}
gallery-move-failed = Não foi possível mover { $file }: { $reason }
gallery-drop-move = Mover para { $folder }
gallery-drop-copy = Copiar para { $folder }
gallery-move-here = Mover seleção para cá
gallery-copy-here = Copiar seleção para cá
gallery-not-an-image = { $file } não é uma imagem que o { -app-name } consiga abrir.
gallery-cache-cleared = Cache de miniaturas limpo
# machine-drafted, needs native review
gallery-thumb-failed = Sem miniatura: { $reason }
gallery-cache-clear-failed = Não foi possível limpar o cache de miniaturas: { $reason }

## Propriedades

gallery-prop-name = Nome
gallery-prop-folder = Pasta
gallery-prop-size = Tamanho do arquivo
gallery-prop-dimensions = Dimensões
gallery-prop-format = Formato
gallery-prop-date = Data
gallery-prop-camera = Câmera
gallery-prop-lens = Lente
gallery-prop-rating = Classificação
gallery-prop-label = Rótulo
gallery-prop-items = Itens

## Leitores de tela

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } item
   *[other] { $count } itens
}
gallery-a11y-dims = { $width } por { $height }
gallery-a11y-folder = pasta
gallery-a11y-up = Subiu para { $folder }
gallery-a11y-archive = arquivo compactado
gallery-a11y-stack = pilha de { $count }
gallery-a11y-filter-bar = Filtro
gallery-a11y-filter-field = Filtrar por nome ou campo
gallery-a11y-chip = Critério de filtro { $term }
gallery-a11y-chip-remove = Pressione para remover
gallery-a11y-results = { $count ->
    [0] Nenhum item corresponde
    [one] { $count } item corresponde
   *[other] { $count } itens correspondem
}

## Configurações → Galeria

settings-section-gallery = Galeria
settings-gallery-layout = Layout
settings-gallery-layout-help = Quadrados cortam as miniaturas em uma grade; linhas justificadas mantêm o formato de cada imagem; sem corte encaixa imagens inteiras em células iguais.
settings-gallery-layout-squares = Quadrados
settings-gallery-layout-justified = Linhas justificadas
settings-gallery-layout-uncropped = Sem corte
settings-gallery-layout-details = Detalhes
settings-gallery-thumbnail-size = Tamanho das miniaturas
settings-gallery-thumbnail-size-help = Também com Ctrl+roda do mouse, gesto de pinça ou Ctrl+mais e menos na galeria.
settings-gallery-captions = Mostrar nomes de arquivo
settings-gallery-captions-help = Uma legenda embaixo de cada miniatura.
settings-gallery-badges = Mostrar selos
settings-gallery-badges-help = Pequenas marcas para RAW, animação, HDR, várias páginas, classificação e rótulo.
settings-gallery-sort-order = Ordenar por
settings-gallery-sort-order-help = A ordem da galeria. As pastas sempre vêm primeiro.
settings-gallery-sort-order-name = Nome
settings-gallery-sort-order-date-taken = Data da foto
settings-gallery-sort-order-modified = Data de modificação
settings-gallery-sort-order-size = Tamanho do arquivo
settings-gallery-sort-order-type = Tipo de arquivo
settings-gallery-sort-order-dimensions = Dimensões
settings-gallery-sort-descending = Ordem inversa
settings-gallery-sort-descending-help = Ordena do último para o primeiro.
settings-gallery-group-by = Agrupar por
settings-gallery-group-by-help = Cabeçalhos de seção que ficam no topo durante a rolagem.
settings-gallery-group-by-none = Nada
settings-gallery-group-by-day = Dia
settings-gallery-group-by-month = Mês
settings-gallery-group-by-year = Ano
settings-gallery-group-by-event = Evento
settings-gallery-group-by-folder = Pasta
settings-gallery-group-by-camera = Câmera
settings-gallery-group-by-format = Formato
settings-gallery-group-by-rating = Classificação
settings-gallery-group-by-target = Alvo
settings-gallery-group-by-filter = Filtro
settings-gallery-per-folder-view = Lembrar a exibição de cada pasta
settings-gallery-per-folder-view-help = Layout, tamanho, ordem e agrupamento são guardados por pasta; caso contrário, uma única exibição é usada em todo lugar.
settings-gallery-open-folders-in-gallery = Abrir pastas na galeria
settings-gallery-open-folders-in-gallery-help = Ao abrir uma pasta, começar pelas miniaturas em vez da primeira imagem.
settings-gallery-escape-opens-gallery = Esc abre a galeria
settings-gallery-escape-opens-gallery-help = Quando não há mais nada para fechar, Esc no visualizador vai para a galeria em vez de não fazer nada.
settings-gallery-wrap-around = As setas dão a volta
settings-gallery-wrap-around-help = No fim da lista, as setas voltam ao início, e do início vão para o fim. Mantendo uma tecla pressionada, a navegação ainda para no fim.
settings-gallery-show-hidden = Mostrar arquivos ocultos
settings-gallery-show-hidden-help = Inclui arquivos e pastas cujos nomes começam com um ponto.
settings-gallery-show-other-files = Mostrar outros arquivos
settings-gallery-show-other-files-help = Inclui arquivos que o { -app-name } não consegue abrir, mostrados com um ícone genérico.
settings-gallery-show-archives = Mostrar arquivos compactados
settings-gallery-show-archives-help = Incluir quadrinhos e outros arquivos compactados (zip, cbz, rar, 7z…) como blocos. Abrir um arquivo compactado diretamente continua funcionando.
settings-gallery-subfolder-depth = Níveis de subpastas
settings-gallery-subfolder-depth-help = Mostra também imagens de subpastas, até esta quantidade de níveis (0: somente esta pasta).
settings-gallery-stacks = Empilhar arquivos relacionados
settings-gallery-stacks-help = Mostra pares RAW+JPEG e sequências como um só bloco. Pressione S para expandir uma pilha.
settings-gallery-info-overlay = Informações nas miniaturas
settings-gallery-info-overlay-help = Dimensões e exposição sobre cada miniatura.
settings-gallery-prefer-embedded = Preferir prévias incorporadas
settings-gallery-prefer-embedded-help = Usa a prévia armazenada no arquivo quando ela é grande o bastante. Muito mais rápido; um pouco menos exato.
settings-gallery-sharper-raw-thumbnails = Miniaturas RAW mais nítidas
settings-gallery-sharper-raw-thumbnails-help = Fotos RAW editadas são reveladas a 1024 px para as miniaturas. Mais nítidas, um pouco mais lentas. Desligado: 512 px.
settings-gallery-rating-sidecars = Gravar classificações em arquivos XMP auxiliares
settings-gallery-rating-sidecars-help = Classificações, rótulos e marcações também vão para um arquivo .xmp ao lado da imagem, que outros aplicativos de fotos leem.
settings-gallery-cache-size = Tamanho do cache de miniaturas
settings-gallery-cache-size-help = As miniaturas mais antigas são removidas quando o cache fica cheio.
settings-gallery-cache-location = Local do cache de miniaturas
settings-gallery-cache-location-help = Uma pasta em uma unidade rápida funciona melhor.
settings-gallery-clear-cache = Limpar cache de miniaturas
settings-gallery-clear-cache-help = Exclui as miniaturas salvas. Elas são criadas de novo conforme você navega.

## Compare
# machine-drafted, needs native review

compare-side = Lado a lado
compare-overlay = Sobreposição
compare-difference = Diferença
compare-flicker = Alternância
compare-blend = Mesclagem
compare-kind-absolute = Erro absoluto
compare-kind-squared = Erro quadrático
compare-kind-relative = Erro relativo
compare-scale-down = Menos sensível
compare-scale-up = Mais sensível
compare-scale = Escala total { $value }
compare-space = Luz linear, como exibida
compare-space-help = Erros e métricas são medidos em luz linear como a imagem é exibida, após o mapeamento de tons, por canal, incluindo alfa.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = amostrado em { $width } × { $height }
compare-aligned = Alinhado no canto superior esquerdo, { $width } × { $height } comparados
compare-vs = { $b } em relação a { $a }
compare-overlay-label = { $b } sobre { $a } a { PERCENT($percent) }
compare-pending = Medindo…
# machine-drafted, needs native review
compare-diff-failed = Não foi possível mostrar a diferença: { $reason }
compare-no-overlap = Nada para comparar na visualização
compare-zoom-fit = Ajustar
compare-zoom = { PERCENT($percent) }
compare-showing = Mostrando { $name }
compare-a11y-group = Comparar
compare-a11y-blend = Mesclar entre as imagens
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Comparar lado a lado
gallery-palette-compare-overlay = Comparar como sobreposição
gallery-palette-compare-difference = Comparar diferença: { $kind }
gallery-palette-compare-flicker = Alternar entre as imagens comparadas
gallery-palette-compare-actual = Comparar a { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Espaço ou Ctrl+Enter seleciona vários

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Perfil de cor
gallery-facet-date = Data da foto
gallery-facet-picked = Escolhidas
gallery-facet-rejected = Rejeitadas
gallery-facet-flagged = Escolhidas ou rejeitadas
gallery-facet-rated = Classificadas
gallery-facet-labelled = Com rótulo

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Profundidade de bits
gallery-facet-pages = Páginas
gallery-facet-width = Largura
gallery-facet-height = Altura
gallery-facet-megapixels = Megapixels
gallery-picker-bits = { $bits } bits
gallery-picker-pages = { $pages ->
    [one] { $pages } página
   *[other] { $pages } páginas
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = Qualquer um em um grupo
gallery-picker-group-keep = Sugestão: manter
gallery-picker-group-discard = Sugestão: descartar
gallery-picker-ops = Comparar
gallery-picker-op-eq = Igual a
gallery-picker-op-ge = No mínimo
gallery-picker-op-le = No máximo
gallery-picker-ge = no mínimo { $value }
gallery-picker-gt = mais de { $value }
gallery-picker-le = no máximo { $value }
gallery-picker-lt = menos de { $value }

# Map view.
gallery-facet-place = Local
gallery-palette-map = Mapa
map-title = Mapa
map-summary = { $count ->
    [one] { $count } foto com local
   *[other] { $count } fotos com local
}
map-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } fotos
}
map-zoom-in = Aproximar
map-zoom-out = Afastar
map-show-all = Mostrar tudo
map-done = Concluído
map-credit = Dados do mapa: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Mapa: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Blocos de mapa online
map-basemap-consent-server = Os blocos do mapa vêm de { $host }.
map-basemap-consent-sees = Esse servidor vê seu endereço de internet e as áreas que você visualiza.
map-basemap-consent-private = Suas fotos, os nomes e os locais delas não são enviados.
map-basemap-consent-cache = Os blocos ficam neste computador, até { $size }.
map-basemap-allow = Usar mapa online
map-basemap-decline = Manter mapa offline
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Mapa base não exibido: { $reason }
map-basemap-bad-address = o endereço do servidor de blocos precisa de {"{"}z{"}"}, {"{"}x{"}"} e {"{"}y{"}"}.
map-basemap-unknown-tiles = os blocos do arquivo de mapa são de um tipo desconhecido.
map-loading = Lendo os locais das fotos…
map-empty = Nenhuma destas fotos tem local
map-hidden = Os locais das fotos estão ocultos (Configurações › Metadados e privacidade)
map-a11y-group = Mapa dos locais das fotos
map-a11y-status = { $count ->
    [one] { $count } local mostrado, nível de zoom { $zoom }
   *[other] { $count } locais mostrados, nível de zoom { $zoom }
}
