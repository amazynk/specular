# machine-drafted, needs native review
# Ferramentas de inspeção: avisos de recorte, realce de foco, histograma, inspetor de pixels. Português do Brasil.

## Paleta de comandos
palette-clipping = Mostrar ou ocultar avisos de recorte
palette-focus-peaking = Mostrar ou ocultar realce de foco
palette-histogram = Mostrar ou ocultar histograma
palette-pixel-inspector = Mostrar ou ocultar inspetor de pixels
# machine-drafted, needs native review
palette-crosshair = Mira do teclado
palette-announce-cursor = Ler o valor sob o cursor

## Painel do histograma
histogram-title = Histograma
# Parcela de pixels visíveis no nível de recorte ou além; os valores são porcentagens.
histogram-clipped = Recortado: realces { $high }, sombras { $low }
histogram-empty = Nenhuma imagem visível
# Resumo para leitores de tela. $mean é o brilho médio (0–255), $high/$low são porcentagens.
histogram-a11y = Histograma da imagem visível: brilho médio { $mean }, realces recortados { $high }, sombras recortadas { $low }

## Painel do inspetor de pixels
inspector-title = Inspetor de pixels
inspector-position = Posição
inspector-source-rgb = RGB de origem
inspector-source-rgba = RGBA de origem
inspector-source-ycc = Y′CbCr de origem
inspector-source-gray = Cinza de origem
inspector-source-gray16 = Cinza de origem (16 bits)
inspector-source-gray-alpha16 = Cinza de origem + alfa (16 bits)
inspector-source-rgb16 = RGB de origem (16 bits)
inspector-source-rgba16 = RGBA de origem (16 bits)
inspector-source-pending = Origem
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Linear
# Mostrado quando os valores vêm de uma prévia reduzida, não dos pixels em resolução total.
inspector-approximate = De uma prévia reduzida; amplie para valores exatos
inspector-outside = Aponte para a imagem para inspecionar um pixel
# Resumo para leitores de tela: posição do pixel e seu valor sRGB.
inspector-a11y = Pixel em { $x }, { $y }: sRGB { $value }

## Configurações
settings-section-inspection = Inspeção
settings-inspection-clip-highlight = Nível de recorte dos realces
settings-inspection-clip-highlight-help = Pixels com algum canal neste valor de 8 bits ou acima são marcados como realces recortados.
settings-inspection-clip-shadow = Nível de recorte das sombras
settings-inspection-clip-shadow-help = Pixels com algum canal neste valor de 8 bits ou abaixo são marcados como sombras recortadas.
settings-inspection-highlight-color = Cor do aviso de realces
settings-inspection-highlight-color-help = Cor que marca os realces recortados.
settings-inspection-shadow-color = Cor do aviso de sombras
settings-inspection-shadow-color-help = Cor que marca as sombras recortadas.
settings-inspection-peaking-sensitivity = Sensibilidade do realce de foco
settings-inspection-peaking-sensitivity-help = Valores mais altos marcam bordas mais suaves como em foco.
settings-inspection-peaking-color = Cor do realce de foco
settings-inspection-peaking-color-help = Cor que marca as bordas nítidas.
settings-inspection-histogram-channels = Canais do histograma
settings-inspection-histogram-channels-help = Quais curvas o histograma mostra.
settings-inspection-histogram-channels-colour = Vermelho, verde e azul
settings-inspection-histogram-channels-luminance = Luminância
settings-inspection-histogram-channels-both = Cor e luminância
settings-inspection-histogram-log = Histograma logarítmico
settings-inspection-histogram-log-help = Usa escala logarítmica no histograma para que contagens pequenas continuem visíveis.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Mostrar o canal vermelho
palette-channel-green = Mostrar o canal verde
palette-channel-blue = Mostrar o canal azul
palette-channel-alpha = Mostrar o canal alfa (de novo: sobre xadrez, depois sobre uma cor)
palette-channel-luma = Mostrar a luminância
channel-normal = Todos os canais
channel-red = Canal vermelho
channel-green = Canal verde
channel-blue = Canal azul
channel-alpha = Canal alfa
channel-alpha-checker = Transparência sobre xadrez
channel-alpha-matte = Transparência sobre cor
channel-luma = Luminância
disabled-no-alpha = Esta imagem não tem transparência
settings-inspection-alpha-matte-color = Cor de fundo do alfa
settings-inspection-alpha-matte-color-help = Cor sólida atrás da imagem na visualização “Transparência sobre cor”.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Mostrar ou ocultar falsa cor
palette-next-colour-map = Próximo mapa de cores
palette-channel-colours = Próximas cores de canal (vermelho–verde, magenta–verde, magenta–amarelo–ciano)
palette-hilo = Mostrar ou ocultar HiLo (pixels no máximo e no mínimo da profundidade de bits)
palette-colour-vision = Próxima simulação de visão de cores
palette-colour-vision-off = Parar de simular a visão de cores
palette-colour-vision-protanopia = Simular protanopia (sem cones vermelhos)
palette-colour-vision-deuteranopia = Simular deuteranopia (sem cones verdes)
palette-colour-vision-tritanopia = Simular tritanopia (sem cones azuis)
palette-colour-vision-achromatopsia = Simular acromatopsia (sem cores)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (não seguro para daltonismo)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = valores brutos
# Screen reader summary of the colour bar.
colour-bar-a11y = Barra de cores { $map }: { $low } a { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Falsa cor: { $map }
badge-hilo = HiLo: marcando pixels no máximo e no mínimo
badge-colour-vision-protanopia = Simulando protanopia
badge-colour-vision-deuteranopia = Simulando deuteranopia
badge-colour-vision-tritanopia = Simulando tritanopia
badge-colour-vision-achromatopsia = Simulando acromatopsia
channel-colours-red-green = Canais: vermelho, verde, azul
channel-colours-magenta-green = Canais: magenta, verde
channel-colours-magenta-yellow-cyan = Canais: magenta, amarelo, ciano
settings-inspection-false-colour-map = Mapa de falsa cor
settings-inspection-false-colour-map-help = Mapa de cores da falsa cor e da sua barra de cores. Todos, exceto jet e phase, continuam legíveis com qualquer visão de cores.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergente)
settings-inspection-false-colour-map-vik = Vik (divergente)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (divergente)
settings-inspection-false-colour-map-curl = Curl (divergente)
settings-inspection-false-colour-map-phase = Phase (cíclico)
settings-inspection-false-colour-map-jet = Jet (não seguro para daltonismo)
settings-inspection-colour-bar = Barra de cores
settings-inspection-colour-bar-help = Mostrar uma barra de cores com valores nas unidades da própria imagem enquanto a falsa cor estiver ativa.
settings-inspection-channel-colours = Cores dos canais
settings-inspection-channel-colours-help = Como os canais de cor são mostrados. Magenta–verde e magenta–amarelo–ciano continuam distintos com daltonismo vermelho–verde.
settings-inspection-channel-colours-red-green = Vermelho, verde, azul
settings-inspection-channel-colours-magenta-green = Magenta, verde
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, amarelo, ciano
settings-inspection-colour-vision = Simulação de visão de cores
settings-inspection-colour-vision-help = Mostrar a janela inteira como vista com uma deficiência de visão de cores (Machado et al. 2009). As exportações nunca são afetadas.
settings-inspection-colour-vision-off = Desligada
settings-inspection-colour-vision-protanopia = Protanopia
settings-inspection-colour-vision-deuteranopia = Deuteranopia
settings-inspection-colour-vision-tritanopia = Tritanopia
settings-inspection-colour-vision-achromatopsia = Acromatopsia
settings-inspection-auto-range = Faixa automática para tons de cinza de alta profundidade
settings-inspection-auto-range-help = Esticar imagens em tons de cinza de 16 bits (TIFF, PNG, FITS) do valor mais escuro ao mais claro. Imagens DICOM mantêm a própria janela.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Mira ativada. As setas a movem, Shift para passos maiores. Enter clica. Esc encerra.
crosshair-off = Mira desativada
announce-cursor-nothing = Nada sob o cursor
