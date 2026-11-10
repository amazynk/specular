# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# Português (Brasil).

keys-mode-map = Mapas
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Camadas
cheatsheet-map-coordinates = Coordenadas
cheatsheet-map-measure = Medição

## Palette entries and their one-line descriptions.
palette-map-layers = Mostrar ou ocultar o painel de camadas
palette-map-layers-help = As camadas do mapa: ordem, opacidade, visibilidade e estilo.
palette-map-add-layer = Adicionar camada de um arquivo…
palette-map-add-layer-help = Desenha outro arquivo de mapa sobre este, como uma trilha GPX sobre um shapefile.
palette-map-zoom-to-layer = Zoom na camada
palette-map-zoom-to-layer-help = Mostra a camada escolhida no painel de camadas por inteiro.
palette-map-attribute-table = Mostrar ou ocultar a tabela de atributos
palette-map-attribute-table-help = As feições da camada escolhida em tabela: ordenar, filtrar, pesquisar e selecionar.
palette-map-coordinate-format = Próximo formato de coordenadas
palette-map-coordinate-format-help = Graus decimais, graus minutos segundos, UTM ou MGRS.
palette-map-units = Unidades métricas ou imperiais
palette-map-units-help = Comprimentos e áreas em metros ou em pés e milhas.
palette-map-copy-coordinate = Copiar coordenada
palette-map-copy-coordinate-help = Copia a posição sob o ponteiro ou a do centro do mapa.
palette-map-go-to-coordinate = Ir para coordenada…
palette-map-go-to-coordinate-help = Digite uma posição em qualquer formato para centralizar o mapa nela.
palette-map-measure-distance = Medir uma distância
palette-map-measure-distance-help = Clique em pontos no mapa: o comprimento na superfície da Terra e o rumo.
palette-map-measure-area = Medir uma área
palette-map-measure-area-help = Clique nos cantos de uma área: o tamanho e o perímetro na superfície da Terra.
palette-map-elevation-profile = Perfil de elevação
palette-map-elevation-profile-help = Desenhe uma linha sobre um modelo de elevação para ver o gráfico das alturas.
palette-map-overview = Mostrar ou ocultar o mapa de visão geral
palette-map-overview-help = Um mapa pequeno da extensão inteira, com a parte na tela marcada.

## Why a map command is unavailable.
disabled-not-map = Só para mapas, com as ferramentas de mapas ativadas
disabled-no-lonlat = O mapa não tem sistema de coordenadas conhecido
disabled-no-features = O mapa não tem feições
disabled-no-heights = O mapa não tem alturas

## Settings › Maps.
settings-section-maps = Mapas
settings-maps-coordinate-format = Formato de coordenadas
settings-maps-coordinate-format-help = Como as posições são mostradas e copiadas. Ir para coordenada lê todos os formatos.
settings-maps-coordinate-format-dd = Graus decimais
settings-maps-coordinate-format-dms = Graus, minutos, segundos
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Unidades
settings-maps-units-help = Comprimentos e áreas nos mapas: a barra de escala e as medições. Automático segue Metadados › Unidades.
settings-maps-units-auto = Automático
settings-maps-units-metric = Métrico
settings-maps-units-imperial = Imperial

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Lat, lon { $position }
}
map-height = Altura { $height }
map-height-label = Altura
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Barra de escala: { $length }
map-north-label = Seta do norte: o norte está a { $degrees }° no sentido horário a partir de cima
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] graus, minutos, segundos
    [utm] UTM
    [mgrs] MGRS
   *[dd] graus decimais
}
map-format-toast = Coordenadas em { $format }
map-units-metric = Unidades: metros e quilômetros
map-units-imperial = Unidades: pés e milhas
toast-copied-coordinate = Coordenada copiada

## Go to coordinate.
map-goto-hint = Graus decimais, DMS, UTM ou MGRS. Enter vai até lá.
map-goto-empty = Digite uma posição.
map-goto-unrecognised = Não é uma posição em um formato conhecido.
map-goto-range = Latitude ou longitude fora do intervalo.
map-goto-no-way = Este mapa não consegue situar uma posição.
map-goto-outside = Essa posição está fora do mapa.
map-goto-toast = Centralizado em { $position }

## The layer panel.
map-layers-title = Camadas: { $shown } de { $total } visíveis
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } feição
   *[other] { $features } feições
}, { PERCENT($opacity) } opaca
map-kind-area = Áreas
map-kind-line = Linhas
map-kind-point = Pontos
map-kind-empty = Sem feições
map-layer-visible = Mostrar ou ocultar a camada
map-layer-opacity-down = Menos opaca
map-layer-opacity-up = Mais opaca
map-layer-colour = Próxima cor
map-layer-width = Próxima espessura de linha
map-layer-point = Próximo tamanho de ponto
map-layer-up = Mover para cima
map-layer-down = Mover para baixo
map-layer-remove = Remover
map-layer-keep = Manter
map-layer-remove-ask = Remover { $name } do mapa? O arquivo continua no disco.
map-layer-adding = Adicionando uma camada…
map-removed = { $name } removido do mapa
map-added = { $layers ->
    [one] { $name } adicionado
   *[other] { $name } adicionado: { $layers } camadas
}
map-add-failed = Não é possível adicionar { $name }: { $reason }
map-add-filter = Arquivos de mapa
map-add-not-map = não é um mapa.
map-add-not-vector = não tem feições.
map-add-no-crs = não tem arquivo de projeção (.prj).

## Identify.
map-identify-more = e mais { $count } na tabela de atributos
map-cell = Célula { $x }, { $y }
map-band = Banda { $n }
map-no-data = Sem dados

## The attribute table.
map-table-title = { $layer }: { $shown } de { $total } feições, { $selected } selecionadas
map-table-search = Pesquisar, ou campo = valor
map-table-selected-only = Só selecionadas
map-table-empty = Nenhuma feição corresponde.
map-table-many = muitos
map-table-stats-number = { $field }: { $count ->
        [one] { $count } valor
       *[other] { $count } valores
    }, { $empty } vazios · mín. { $min } · máx. { $max } · média { $mean } · soma { $sum } · desvio padrão { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } valor
       *[other] { $count } valores
    }, { $empty } vazios · { $distinct } distintos
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } valor
       *[other] { $count } valores
    }, { $empty } vazios · { $distinct } distintos · mais comum { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Clique em pontos ao longo do caminho.
map-measure-hint-area = Clique nos cantos da área.
map-measure-hint-profile = Clique em pontos ao longo da linha.
map-measure-keys = Enter conclui. Backspace remove o último ponto. Esc fecha.
map-measure-distance = Distância { $length }
map-measure-bearing = Rumo { $bearing }
map-measure-segment = Último trecho { $length }, rumo { $bearing }
map-measure-area = Área { $area }
map-measure-perimeter = Perímetro { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Comprimento { $length } · mínima { $min } · máxima { $max } · subida { $climb } · descida { $descent }
map-profile-no-heights = Comprimento { $length }. Sem alturas ao longo da linha.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: sem altura
