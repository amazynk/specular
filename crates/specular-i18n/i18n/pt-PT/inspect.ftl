# machine-drafted, needs native review
# Ferramentas de inspeção: avisos de recorte, realce de focagem, histograma, inspetor de píxeis. Português europeu.

## Paleta de comandos
palette-clipping = Mostrar ou ocultar avisos de recorte
palette-focus-peaking = Mostrar ou ocultar realce de focagem
palette-histogram = Mostrar ou ocultar histograma
palette-pixel-inspector = Mostrar ou ocultar inspetor de píxeis
# machine-drafted, needs native review
palette-crosshair = Mira do teclado
palette-announce-cursor = Ler o valor sob o cursor

## Painel do histograma
histogram-title = Histograma
# Proporção de píxeis visíveis nos níveis de recorte ou além deles; os valores são percentagens.
histogram-clipped = Recorte: altas luzes { $high }, sombras { $low }
histogram-empty = Nenhuma imagem visível
# Resumo para leitores de ecrã. $mean é o brilho médio (0–255), $high/$low são percentagens.
histogram-a11y = Histograma da imagem visível: brilho médio { $mean }, altas luzes recortadas { $high }, sombras recortadas { $low }

## Painel do inspetor de píxeis
inspector-title = Inspetor de píxeis
inspector-position = Posição
inspector-source-rgb = RGB de origem
inspector-source-rgba = RGBA de origem
inspector-source-ycc = Y′CbCr de origem
inspector-source-gray = Cinzento de origem
inspector-source-gray16 = Cinzento de origem (16 bits)
inspector-source-gray-alpha16 = Cinzento de origem + alfa (16 bits)
inspector-source-rgb16 = RGB de origem (16 bits)
inspector-source-rgba16 = RGBA de origem (16 bits)
inspector-source-pending = Origem
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Linear
# Mostrado quando os valores vêm de uma pré-visualização reduzida e não dos píxeis em resolução total.
inspector-approximate = De uma pré-visualização reduzida; amplie para obter valores exatos
inspector-outside = Aponte para a imagem para inspecionar um píxel
# Resumo para leitores de ecrã: posição do píxel e o seu valor sRGB.
inspector-a11y = Píxel em { $x }, { $y }: sRGB { $value }

## Definições
settings-section-inspection = Inspeção
settings-inspection-clip-highlight = Nível de recorte das altas luzes
settings-inspection-clip-highlight-help = Os píxeis com algum canal igual ou superior a este valor de 8 bits são marcados como altas luzes recortadas.
settings-inspection-clip-shadow = Nível de recorte das sombras
settings-inspection-clip-shadow-help = Os píxeis com algum canal igual ou inferior a este valor de 8 bits são marcados como sombras recortadas.
settings-inspection-highlight-color = Cor do aviso de altas luzes
settings-inspection-highlight-color-help = Cor que marca as altas luzes recortadas.
settings-inspection-shadow-color = Cor do aviso de sombras
settings-inspection-shadow-color-help = Cor que marca as sombras recortadas.
settings-inspection-peaking-sensitivity = Sensibilidade do realce de focagem
settings-inspection-peaking-sensitivity-help = Valores mais altos marcam como focadas arestas mais suaves.
settings-inspection-peaking-color = Cor do realce de focagem
settings-inspection-peaking-color-help = Cor que marca as arestas nítidas.
settings-inspection-histogram-channels = Canais do histograma
settings-inspection-histogram-channels-help = As curvas que o histograma mostra.
settings-inspection-histogram-channels-colour = Vermelho, verde e azul
settings-inspection-histogram-channels-luminance = Luminância
settings-inspection-histogram-channels-both = Cor e luminância
settings-inspection-histogram-log = Histograma logarítmico
settings-inspection-histogram-log-help = Escala logarítmica para que as contagens pequenas continuem visíveis.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Mostrar o canal vermelho
palette-channel-green = Mostrar o canal verde
palette-channel-blue = Mostrar o canal azul
palette-channel-alpha = Mostrar o canal alfa (novamente: sobre xadrez, depois sobre uma cor)
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
settings-inspection-alpha-matte-color-help = Cor sólida por trás da imagem na vista «Transparência sobre cor».

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Mostrar ou ocultar falsa cor
palette-next-colour-map = Mapa de cores seguinte
palette-channel-colours = Cores de canal seguintes (vermelho–verde, magenta–verde, magenta–amarelo–ciano)
palette-hilo = Mostrar ou ocultar HiLo (píxeis no máximo e no mínimo da profundidade de bits)
palette-colour-vision = Simulação de visão de cores seguinte
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
badge-hilo = HiLo: a marcar píxeis no máximo e no mínimo
badge-colour-vision-protanopia = A simular protanopia
badge-colour-vision-deuteranopia = A simular deuteranopia
badge-colour-vision-tritanopia = A simular tritanopia
badge-colour-vision-achromatopsia = A simular acromatopsia
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
settings-inspection-auto-range = Intervalo automático para tons de cinzento de alta profundidade
settings-inspection-auto-range-help = Esticar imagens em tons de cinzento de 16 bits (TIFF, PNG, FITS) do valor mais escuro ao mais claro. As imagens DICOM mantêm a sua própria janela.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Mira ativada. As setas movem-na, Shift para passos maiores. Enter clica. Esc termina.
crosshair-off = Mira desativada
announce-cursor-nothing = Nada sob o cursor
