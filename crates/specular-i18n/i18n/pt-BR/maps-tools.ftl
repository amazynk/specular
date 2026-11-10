# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Relevo
cheatsheet-map-time = Tempo e clima
cheatsheet-map-online = Serviços online
cheatsheet-map-photos = Fotos e GPS
cheatsheet-map-output = Saída

## Palette entries and their one-line descriptions.
palette-map-slope = Cores de declividade
palette-map-slope-help = Colore um modelo de elevação pela inclinação do terreno.
palette-map-aspect = Cores de orientação
palette-map-aspect-help = Colore um modelo de elevação pela direção para a qual as encostas estão voltadas.
palette-map-contours = Mostrar ou ocultar as curvas de nível
palette-map-contours-help = Curvas de nível sobre um modelo de elevação, rotuladas a cada poucas curvas.
palette-map-terrain-3d = Relevo em 3D
palette-map-terrain-3d-help = Mostra o modelo de elevação como uma superfície 3D com as cores por cima.
palette-map-time = Mostrar ou ocultar o controle de tempo
palette-map-time-help = Percorre os tempos de uma grade ou os horários de uma trilha.
palette-map-play = Reproduzir ou pausar o tempo
palette-map-play-help = Faz o controle de tempo avançar sozinho, passo a passo.
palette-map-wind = Mostrar ou ocultar as setas de vento
palette-map-wind-help = Setas de velocidade e direção do vento a partir dos componentes u e v da grade.
palette-map-weather-colours = Próximo mapa de cores de clima
palette-map-weather-colours-help = Mapas de cores feitos para temperatura, chuva, vento e pressão.
palette-map-services = Serviços de mapas online…
palette-map-services-help = Adiciona camadas de servidores WMS, WMTS, WFS, OGC API ou STAC.
palette-map-photos-on-track = Mostrar ou ocultar as fotos na trilha
palette-map-photos-on-track-help = Posiciona as fotos da pasta ao longo da trilha GPS pela hora em que foram tiradas.
palette-map-geotag = Georreferenciar fotos pela trilha…
palette-map-geotag-help = Grava as posições da trilha em arquivos XMP auxiliares das fotos da pasta.
palette-map-export = Exportar mapa…
palette-map-export-help = Salva o mapa como PNG ou PDF em escala, com legenda, barra de escala e seta do norte.
palette-map-print = Imprimir mapa em escala…
palette-map-print-help = Imprime o mapa numa escala escolhida, como 1:{ NUMBER(25000) }.
palette-map-copy-extent = Copiar extensão
palette-map-copy-extent-help = Copia a área na tela como retângulo envolvente, ou como WKT.

## Why a map command is unavailable.
disabled-no-time = O mapa não tem tempos
disabled-no-wind = O mapa não tem componentes de vento (u e v)
disabled-no-track = Nenhuma trilha GPS com horários
disabled-not-grid = O mapa não é uma grade de valores

## Settings › Maps and Metadata.
settings-maps-services = Serviços de mapas online
settings-maps-services-help = Adiciona camadas de servidores WMS, WMTS, WFS, OGC API e STAC. Desligado: nada é baixado.
settings-maps-services-allowed = Servidores de mapas permitidos
settings-maps-services-allowed-help = Servidores que você aceitou depois da explicação do que eles veem.
settings-metadata-basemap-service = Uma camada de serviço de mapas
settings-metadata-basemap-service-help = A camada escolhida no painel de serviços online do mapa.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NE
    [e] L
    [se] SE
    [s] S
    [sw] SO
    [w] O
    [nw] NO
   *[n] N
}
map-slope-aspect = Declividade { $slope }° · voltada para { $direction } ({ $aspect }°)
map-slope-flat = Declividade { $slope }° · plano
map-slope-on = Cores de declividade
map-aspect-on = Cores de orientação
map-relief-off = Cores de altura
map-contours-on = Curvas de nível ativadas
map-contours-off = Curvas de nível desativadas
map-legend-slope = Declividade
map-legend-aspect = Orientação: para onde as encostas estão voltadas
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° ou mais
map-terrain-3d-on = Relevo em 3D. Esc volta ao mapa.
map-terrain-3d-on-exaggerated = Relevo em 3D, alturas × { $times }. Esc volta ao mapa.
map-terrain-exaggeration = Alturas exageradas { $times } vezes.
map-terrain-drape = Cores do mapa

## Time, weather and wind.
map-time-grid = { $when } · { $step } de { $steps }
map-time-track = { $when } · após { $elapsed }
map-time-speed = × { $times }
map-time-play = Reproduzir
map-time-pause = Pausar
map-weather-map = Cores: { $name }
map-weather-file = Cores do arquivo
map-wind-on = Setas de vento ativadas
map-wind-off = Setas de vento desativadas
map-wind-at = Vento { $speed } { $unit } de { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Retângulo envolvente copiado. Copie de novo para WKT.
map-extent-copied-wkt = Extensão copiada como WKT

## Online map services.
map-services-title = Serviços de mapas online
map-service-off = Os serviços de mapas online estão desligados.
map-service-off-why = Ligue-os para adicionar camadas de servidores na internet.
map-service-turn-on = Ligar
map-service-close = Fechar
map-service-back = Voltar
map-service-cancel = Cancelar
map-service-connect = Conectar
map-service-allow = Permitir
map-service-remove = Remover
map-service-download = Baixar
map-service-list-hint = Enter abre um servidor. Delete o remove.
map-service-add = Adicionar servidor…
map-service-add-sub = WMS, WMTS, WFS, OGC API ou STAC
map-service-add-hint = O endereço do servidor. Enter conecta.
map-service-bad-address = Não é um endereço web (http ou https, sem nome de usuário).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Serviço de mapas online
map-service-consent-server = O Specular vai contatar { $host }.
map-service-consent-sees = Esse servidor vê seu endereço de internet e a área do mapa que você pede.
map-service-consent-private = Seus arquivos e os nomes deles não são enviados.
map-service-asking = Consultando { $host }…
map-service-downloading = Baixando de { $host }…
map-service-layers-hint = Camadas { $kind }. Enter usa uma.
map-service-no-layers = O servidor não lista nenhuma camada.
map-service-offer-tiles = Blocos: vira o mapa base do mapa de fotos
map-service-offer-features = Feições: adicionadas a este mapa para a área na tela
map-service-offer-search = Imagens: buscar na área na tela
map-service-refused-mercator = Não oferecido em Web Mercator (EPSG:3857), que o mapa base exige.
map-service-refused-format = Não oferecido como imagens PNG ou JPEG.
map-service-refused-geojson = Não oferecido como GeoJSON, o formato lido aqui.
map-service-items-hint = { $count ->
        [one] { $count } imagem encontrada. Enter a baixa.
       *[other] { $count } imagens encontradas. Enter baixa uma.
    }
map-service-no-items = Nenhuma imagem encontrada aqui.
map-service-item-cloud = { $when } · { PERCENT($cloud) } de nuvens
map-service-item-no-cog = Sem Cloud-Optimized GeoTIFF
map-service-download-ask = Baixar { $size } de { $host }?
map-service-download-ask-unknown = Baixar esta imagem de { $host }? O tamanho não é conhecido.
map-service-download-cache = Ela fica no cache de mapas e é aberta.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = A imagem tem { $size }, grande demais para baixar.
map-service-opened = Abrindo a imagem baixada
map-service-basemap = { $layer } agora é o mapa base do mapa de fotos. O mapa de fotos pergunta antes do primeiro download.
map-service-status = O servidor respondeu { $status }.
map-service-unknown = Não é um serviço de mapas que esta versão lê.
map-service-exception = O servidor disse: { $message }
map-service-malformed = A resposta do servidor está corrompida: { $reason }
map-service-remove-ask = Remover “{ $server }” dos servidores salvos?
map-service-remove-detail = Nada no disco é perdido. As camadas já no mapa ficam até ele ser fechado.
map-service-removed = { $server } removido
map-service-saved = Servidores de mapas salvos
map-service-saved-none = Nenhum ainda. Adicione um no painel de serviços online do mapa.
map-service-keywords = WMS WMTS WFS OGC API STAC servidores serviços mapas

## Photos and geotagging.
map-geotag-title = Georreferenciar fotos pela trilha
map-photos-on = Fotos na trilha
map-photos-off = Fotos na trilha ocultas
map-geotag-offset = Relógio da câmera: { $offset } em relação à hora do GPS. ← → mudam um minuto, com Shift uma hora, com Alt um segundo.
map-geotag-reading = Lendo as fotos…
map-geotag-summary = { $count } de { $total } fotos recebem uma posição.
map-geotag-sidecars = As posições vão para arquivos XMP auxiliares ao lado das fotos. Os arquivos das fotos não são alterados.
map-geotag-writing = Gravando…
map-geotag-write = Gravar arquivos auxiliares…
map-geotag-guess = Estimar diferença
map-geotag-will-write = Recebe { $position }
map-geotag-has-position = Já tem posição: fica como está
map-geotag-no-time = Sem hora de captura: fica como está
map-geotag-outside = Tirada { $offset } fora da trilha: fica como está
map-geotag-gap = Num intervalo da trilha (a { $seconds } s de um ponto): fica como está
map-geotag-nothing = Nenhuma foto recebe posição com essa diferença.
map-geotag-confirm = Gravar posições em { $count ->
    [one] { $count } arquivo XMP auxiliar
   *[other] { $count } arquivos XMP auxiliares
}? As fotos em si não são alteradas.
map-geotag-confirm-ok = Gravar arquivos auxiliares
map-geotag-done = { $count ->
    [one] { $count } arquivo auxiliar gravado
   *[other] { $count } arquivos auxiliares gravados
}
map-geotag-done-failed = { $count ->
        [one] { $count } arquivo auxiliar gravado
       *[other] { $count } arquivos auxiliares gravados
    }; { $failed } falharam: { $reason }
map-geotag-not-sidecar = Não é um nome de arquivo auxiliar.

## Export and printing.
map-export-title = Exportar mapa
map-print-title = Imprimir mapa em escala
map-export-keys = ↑ ↓ escolhem, ← → mudam, Enter executa.
map-export-format = Formato
map-export-scale = Escala
map-export-scale-value = 1:{ $n }
map-export-resolution = Resolução
map-export-dpi = { $dpi } dpi
map-export-paper = Papel
map-export-size = { $width } × { $height } px · { $across } × { $down } km no terreno
map-export-save = Salvar…
map-export-print = Imprimir…
map-export-too-large = Grande demais nesta escala: { $width } × { $height } px. Escolha uma escala ou resolução menor.
map-export-no-scale = Este mapa não tem escala no terreno.
map-export-no-gpu = A exportação precisa do processador gráfico.
map-export-subtitle = Escala { $scale } · { $crs }
map-export-data = Dados: { $sources }
map-export-saved = { $name } salvo
map-export-printing = Imprimindo…
map-export-printed = Enviado para a impressora
map-export-print-failed = Não impresso: { $reason }
