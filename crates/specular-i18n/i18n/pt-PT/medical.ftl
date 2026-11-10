# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Imagens médicas
palette-window-level = Ferramenta de janela/nível (arrastar com o esquerdo)
palette-window-default = Janela: predefinição do ficheiro
palette-window-lung = Janela: pulmão (W 1500, L −600)
palette-window-mediastinum = Janela: mediastino (W 350, L 50)
palette-window-bone = Janela: osso (W 1800, L 400)
palette-window-brain = Janela: cérebro (W 80, L 40)
palette-window-stroke = Janela: AVC (W 40, L 40)
palette-window-soft-tissue = Janela: tecidos moles (W 400, L 50)
palette-window-liver = Janela: fígado (W 150, L 30)
palette-window-subdural = Janela: subdural (W 200, L 75)
palette-window-full-range = Janela: intervalo completo
palette-invert-grey = Inverter tons de cinzento
palette-next-slice = Fotograma seguinte
palette-prev-slice = Fotograma anterior
disabled-not-medical = Apenas para imagens médicas em tons de cinzento
disabled-one-frame = A imagem tem um só fotograma
settings-section-medical = Imagens médicas
settings-medical-keep-window = Manter janela/nível na série seguinte
settings-medical-keep-window-help = Manter a janela ajustada ao passar para outra série, em vez de começar pela predefinição dela. Os fotogramas de um ficheiro e os cortes de uma série partilham-na sempre.
medical-window = C { $center }  W { $width }
medical-inverted = invertido
medical-frame = Fotograma { $frame } de { $count }
medical-tool = Ferramenta de janela/nível: arraste
medical-tool-on = Ferramenta de janela/nível ativada: arraste para ajustar
medical-tool-off = Ferramenta de janela/nível desativada
medical-preset-not-ct = As predefinições de TC precisam de valores em unidades Hounsfield
medical-file-window-none = Não há janela { $index } neste ficheiro
medical-file-window-unnamed = Janela do ficheiro { $index } de { $count }
medical-file-window = Janela do ficheiro { $index } de { $count }: { $name }
measure-templates-need-medical = Os modelos geométricos vêm com o perfil Medicina
medical-off = Sem ferramentas médicas para esta imagem: { $reason }
medical-readout = Valor e janela

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Reproduzir imagens (cine)
palette-measure-length = Medir comprimento
palette-measure-angle = Medir ângulo
palette-measure-ellipse = Medir região elíptica
palette-measure-rectangle = Medir região retangular
palette-clear-measurements = Limpar medições
palette-measure-templates = Modelos geométricos…
palette-overlays = Mostrar ou ocultar sobreposições
medical-cine = A reproduzir a { $rate } imagens/s
medical-overlays-shown = Sobreposições visíveis
medical-overlays-hidden = Sobreposições ocultas
measure-hint-length = Comprimento: arraste ou clique nas duas extremidades
measure-hint-angle = Ângulo: clique em três pontos, o vértice em segundo
measure-hint-ellipse = Elipse: arraste sobre a região
measure-hint-rectangle = Retângulo: arraste sobre a região
measure-off = Ferramenta de medição desativada
measure-cleared = Medições limpas
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Área { $value } mm²
measure-area-px = Área { $value } px²
measure-angle = { $value }°
measure-mean = Média { $mean } ± { $sd } { $unit }
measure-range = Mín { $min }  Máx { $max }  ({ $count } px)
measure-caveat-detector = No detetor: sem correção da ampliação
measure-caveat-magnification = Correção de ampliação estimada
measure-caveat-uncalibrated = Sem espaçamento de píxeis: em píxeis
measure-caveat-outside-region = Fora de uma região de ecografia calibrada: em píxeis

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Mostrar etiquetas de orientação
settings-medical-orientation-labels-help = Letras nas margens da imagem com as direções do doente (R/L direita/esquerda, A/P anterior/posterior, S/I cabeça/pés), lidas do ficheiro e acompanhando rotações e inversões. L mostra-as ou oculta-as durante a sessão.
settings-medical-wheel-steps-frames = A roda do rato percorre os fotogramas
settings-medical-wheel-steps-frames-help = Num ficheiro ou série com vários fotogramas, a roda passa de um fotograma para outro em vez de fazer zoom ou mudar de imagem. Ctrl+roda continua a fazer zoom.
# machine-drafted, needs native review
settings-medical-display-function = Apresentação de cinzentos
settings-medical-display-function-help = Como os tons de cinzento das imagens médicas são apresentados. O DICOM GSDF (função de apresentação padrão em escala de cinzentos) distribui-os em passos de visibilidade igual entre as luminâncias do preto e do branco do ecrã (DICOM PS3.14), como num monitor de diagnóstico calibrado. O perfil de cor do ecrã continua a aplicar-se.
settings-medical-display-function-srgb = Padrão (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Luminância do preto do ecrã
settings-medical-display-black-help = A luminância do preto do ecrã, incluindo a luz ambiente, obtida por medição ou a partir da ficha técnica do ecrã. Usada pelo DICOM GSDF.
settings-medical-display-white = Luminância do branco do ecrã
settings-medical-display-white-help = A luminância do branco do ecrã, obtida por medição ou a partir da ficha técnica do ecrã. Usada pelo DICOM GSDF.
palette-orientation-labels = Mostrar ou ocultar etiquetas de orientação
palette-presentation-state = Estado de apresentação seguinte
disabled-no-presentation-state = Nenhum estado de apresentação se refere a esta imagem
medical-labels-shown = Etiquetas de orientação visíveis
medical-labels-hidden = Etiquetas de orientação ocultas
medical-presentation = Estado de apresentação: { $label } ({ $index } de { $count })
medical-presentation-none = Estado de apresentação desativado
medical-presentation-readout = Apresentação: { $label }
medical-preset-presentation = A tabela de consulta do estado de apresentação define os níveis de cinzento

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientação: esquerda { $left }, direita { $right }, topo { $top }, base { $bottom }
