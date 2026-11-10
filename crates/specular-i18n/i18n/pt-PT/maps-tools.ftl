# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Relevo
cheatsheet-map-time = Tempo e meteorologia
cheatsheet-map-online = Serviços online
cheatsheet-map-photos = Fotografias e GPS
cheatsheet-map-output = Saída

## Palette entries and their one-line descriptions.
palette-map-slope = Cores de declive
palette-map-slope-help = Colore um modelo de elevação pela inclinação do terreno.
palette-map-aspect = Cores de exposição
palette-map-aspect-help = Colore um modelo de elevação pela direção para onde as encostas estão viradas.
palette-map-contours = Mostrar ou ocultar as curvas de nível
palette-map-contours-help = Curvas de nível sobre um modelo de elevação, com rótulo de poucas em poucas curvas.
palette-map-terrain-3d = Relevo em 3D
palette-map-terrain-3d-help = Mostra o modelo de elevação como uma superfície 3D com as cores por cima.
palette-map-time = Mostrar ou ocultar o controlo de tempo
palette-map-time-help = Percorre os tempos de uma grelha ou as horas de um trajeto.
palette-map-play = Reproduzir ou pausar o tempo
palette-map-play-help = Faz o controlo de tempo avançar sozinho, passo a passo.
palette-map-wind = Mostrar ou ocultar as setas de vento
palette-map-wind-help = Setas de velocidade e direção do vento a partir das componentes u e v da grelha.
palette-map-weather-colours = Mapa de cores meteorológico seguinte
palette-map-weather-colours-help = Mapas de cores feitos para temperatura, chuva, vento e pressão.
palette-map-services = Serviços de mapas online…
palette-map-services-help = Adiciona camadas de servidores WMS, WMTS, WFS, OGC API ou STAC.
palette-map-photos-on-track = Mostrar ou ocultar as fotografias no trajeto
palette-map-photos-on-track-help = Coloca as fotografias da pasta ao longo do trajeto GPS pela hora em que foram tiradas.
palette-map-geotag = Georreferenciar fotografias pelo trajeto…
palette-map-geotag-help = Escreve as posições do trajeto em ficheiros XMP auxiliares das fotografias da pasta.
palette-map-export = Exportar mapa…
palette-map-export-help = Guarda o mapa como PNG ou PDF à escala, com legenda, barra de escala e seta do norte.
palette-map-print = Imprimir mapa à escala…
palette-map-print-help = Imprime o mapa a uma escala escolhida, como 1:{ NUMBER(25000) }.
palette-map-copy-extent = Copiar extensão
palette-map-copy-extent-help = Copia a área no ecrã como retângulo envolvente, ou como WKT.

## Why a map command is unavailable.
disabled-no-time = O mapa não tem tempos
disabled-no-wind = O mapa não tem componentes de vento (u e v)
disabled-no-track = Nenhum trajeto GPS com horas
disabled-not-grid = O mapa não é uma grelha de valores

## Settings › Maps and Metadata.
settings-maps-services = Serviços de mapas online
settings-maps-services-help = Adiciona camadas de servidores WMS, WMTS, WFS, OGC API e STAC. Desligado: nada é transferido.
settings-maps-services-allowed = Servidores de mapas permitidos
settings-maps-services-allowed-help = Servidores que aceitou depois da explicação do que veem.
settings-metadata-basemap-service = Uma camada de um serviço de mapas
settings-metadata-basemap-service-help = A camada escolhida no painel de serviços online do mapa.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NE
    [e] E
    [se] SE
    [s] S
    [sw] SO
    [w] O
    [nw] NO
   *[n] N
}
map-slope-aspect = Declive { $slope }° · virado a { $direction } ({ $aspect }°)
map-slope-flat = Declive { $slope }° · plano
map-slope-on = Cores de declive
map-aspect-on = Cores de exposição
map-relief-off = Cores de altura
map-contours-on = Curvas de nível ativadas
map-contours-off = Curvas de nível desativadas
map-legend-slope = Declive
map-legend-aspect = Exposição: para onde as encostas estão viradas
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
map-time-pause = Pausa
map-weather-map = Cores: { $name }
map-weather-file = Cores do ficheiro
map-wind-on = Setas de vento ativadas
map-wind-off = Setas de vento desativadas
map-wind-at = Vento { $speed } { $unit } de { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Retângulo envolvente copiado. Copie outra vez para WKT.
map-extent-copied-wkt = Extensão copiada como WKT

## Online map services.
map-services-title = Serviços de mapas online
map-service-off = Os serviços de mapas online estão desligados.
map-service-off-why = Ligue-os para adicionar camadas de servidores na internet.
map-service-turn-on = Ligar
map-service-close = Fechar
map-service-back = Voltar
map-service-cancel = Cancelar
map-service-connect = Ligar ao servidor
map-service-allow = Permitir
map-service-remove = Remover
map-service-download = Transferir
map-service-list-hint = Enter abre um servidor. Delete remove-o.
map-service-add = Adicionar um servidor…
map-service-add-sub = WMS, WMTS, WFS, OGC API ou STAC
map-service-add-hint = O endereço do servidor. Enter liga.
map-service-bad-address = Não é um endereço web (http ou https, sem nome de utilizador).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Serviço de mapas online
map-service-consent-server = O Specular vai contactar { $host }.
map-service-consent-sees = Esse servidor vê o seu endereço de internet e a área do mapa que pede.
map-service-consent-private = Os seus ficheiros e os respetivos nomes não são enviados.
map-service-asking = A consultar { $host }…
map-service-downloading = A transferir de { $host }…
map-service-layers-hint = Camadas { $kind }. Enter usa uma.
map-service-no-layers = O servidor não lista nenhuma camada.
map-service-offer-tiles = Mosaicos: passa a ser o mapa base do mapa de fotografias
map-service-offer-features = Elementos: adicionados a este mapa para a área no ecrã
map-service-offer-search = Imagens: pesquisar na área no ecrã
map-service-refused-mercator = Não disponível em Web Mercator (EPSG:3857), de que o mapa base precisa.
map-service-refused-format = Não disponível como imagens PNG ou JPEG.
map-service-refused-geojson = Não disponível como GeoJSON, o formato lido aqui.
map-service-items-hint = { $count ->
        [one] { $count } imagem encontrada. Enter transfere-a.
       *[other] { $count } imagens encontradas. Enter transfere uma.
    }
map-service-no-items = Nenhuma imagem encontrada aqui.
map-service-item-cloud = { $when } · { PERCENT($cloud) } de nuvens
map-service-item-no-cog = Sem Cloud-Optimized GeoTIFF
map-service-download-ask = Transferir { $size } de { $host }?
map-service-download-ask-unknown = Transferir esta imagem de { $host }? O tamanho não é conhecido.
map-service-download-cache = Fica guardada na cache de mapas e é aberta.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = A imagem tem { $size }, demasiado grande para transferir.
map-service-opened = A abrir a imagem transferida
map-service-basemap = { $layer } é agora o mapa base do mapa de fotografias. O mapa de fotografias pergunta antes da primeira transferência.
map-service-status = O servidor respondeu { $status }.
map-service-unknown = Não é um serviço de mapas que esta versão leia.
map-service-exception = O servidor disse: { $message }
map-service-malformed = A resposta do servidor está danificada: { $reason }
map-service-remove-ask = Remover «{ $server }» dos servidores guardados?
map-service-remove-detail = Nada se perde no disco. As camadas já no mapa ficam até ele ser fechado.
map-service-removed = { $server } removido
map-service-saved = Servidores de mapas guardados
map-service-saved-none = Nenhum ainda. Adicione um no painel de serviços online do mapa.
map-service-keywords = WMS WMTS WFS OGC API STAC servidores serviços mapas

## Photos and geotagging.
map-geotag-title = Georreferenciar fotografias pelo trajeto
map-photos-on = Fotografias no trajeto
map-photos-off = Fotografias no trajeto ocultas
map-geotag-offset = Relógio da câmara: { $offset } em relação à hora GPS. ← → mudam um minuto, com Shift uma hora, com Alt um segundo.
map-geotag-reading = A ler as fotografias…
map-geotag-summary = { $count } de { $total } fotografias recebem uma posição.
map-geotag-sidecars = As posições vão para ficheiros XMP auxiliares junto às fotografias. Os ficheiros das fotografias não são alterados.
map-geotag-writing = A escrever…
map-geotag-write = Escrever ficheiros auxiliares…
map-geotag-guess = Estimar diferença
map-geotag-will-write = Recebe { $position }
map-geotag-has-position = Já tem posição: fica como está
map-geotag-no-time = Sem hora de captura: fica como está
map-geotag-outside = Tirada { $offset } fora do trajeto: fica como está
map-geotag-gap = Numa falha do trajeto (a { $seconds } s de um ponto): fica como está
map-geotag-nothing = Nenhuma fotografia recebe posição com esta diferença.
map-geotag-confirm = Escrever posições em { $count ->
    [one] { $count } ficheiro XMP auxiliar
   *[other] { $count } ficheiros XMP auxiliares
}? As fotografias em si não são alteradas.
map-geotag-confirm-ok = Escrever ficheiros auxiliares
map-geotag-done = { $count ->
    [one] { $count } ficheiro auxiliar escrito
   *[other] { $count } ficheiros auxiliares escritos
}
map-geotag-done-failed = { $count ->
        [one] { $count } ficheiro auxiliar escrito
       *[other] { $count } ficheiros auxiliares escritos
    }; { $failed } falharam: { $reason }
map-geotag-not-sidecar = Não é um nome de ficheiro auxiliar.

## Export and printing.
map-export-title = Exportar mapa
map-print-title = Imprimir mapa à escala
map-export-keys = ↑ ↓ escolhem, ← → mudam, Enter executa.
map-export-format = Formato
map-export-scale = Escala
map-export-scale-value = 1:{ $n }
map-export-resolution = Resolução
map-export-dpi = { $dpi } ppp
map-export-paper = Papel
map-export-size = { $width } × { $height } px · { $across } × { $down } km no terreno
map-export-save = Guardar…
map-export-print = Imprimir…
map-export-too-large = Demasiado grande a esta escala: { $width } × { $height } px. Escolha uma escala ou resolução menor.
map-export-no-scale = Este mapa não tem escala no terreno.
map-export-no-gpu = A exportação precisa do processador gráfico.
map-export-subtitle = Escala { $scale } · { $crs }
map-export-data = Dados: { $sources }
map-export-saved = { $name } guardado
map-export-printing = A imprimir…
map-export-printed = Enviado para a impressora
map-export-print-failed = Não impresso: { $reason }
