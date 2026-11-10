# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Imagens médicas
palette-window-level = Ferramenta de janela/nível (arrastar com o esquerdo)
palette-window-default = Janela: padrão do arquivo
palette-window-lung = Janela: pulmão (W 1500, L −600)
palette-window-mediastinum = Janela: mediastino (W 350, L 50)
palette-window-bone = Janela: osso (W 1800, L 400)
palette-window-brain = Janela: cérebro (W 80, L 40)
palette-window-stroke = Janela: AVC (W 40, L 40)
palette-window-soft-tissue = Janela: partes moles (W 400, L 50)
palette-window-liver = Janela: fígado (W 150, L 30)
palette-window-subdural = Janela: subdural (W 200, L 75)
palette-window-full-range = Janela: intervalo completo
palette-invert-grey = Inverter tons de cinza
palette-next-slice = Próximo quadro
palette-prev-slice = Quadro anterior
disabled-not-medical = Somente para imagens médicas em tons de cinza
disabled-one-frame = A imagem tem um só quadro
settings-section-medical = Imagens médicas
settings-medical-keep-window = Manter janela/nível na próxima série
settings-medical-keep-window-help = Manter a janela ajustada ao passar para outra série, em vez de começar pelo padrão dela. Os quadros de um arquivo e os cortes de uma série sempre a compartilham.
medical-window = C { $center }  W { $width }
medical-inverted = invertido
medical-frame = Quadro { $frame } de { $count }
medical-tool = Ferramenta de janela/nível: arraste
medical-tool-on = Ferramenta de janela/nível ativada: arraste para ajustar
medical-tool-off = Ferramenta de janela/nível desativada
medical-preset-not-ct = Predefinições de TC precisam de valores em unidades Hounsfield
medical-file-window-none = Não há janela { $index } neste arquivo
medical-file-window-unnamed = Janela do arquivo { $index } de { $count }
medical-file-window = Janela do arquivo { $index } de { $count }: { $name }
measure-templates-need-medical = Os modelos geométricos vêm com o perfil Medicina
medical-off = Sem ferramentas médicas para esta imagem: { $reason }
medical-readout = Valor e janela

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Reproduzir quadros (cine)
palette-measure-length = Medir comprimento
palette-measure-angle = Medir ângulo
palette-measure-ellipse = Medir região elíptica
palette-measure-rectangle = Medir região retangular
palette-clear-measurements = Limpar medições
palette-measure-templates = Modelos geométricos…
palette-overlays = Mostrar ou ocultar sobreposições
medical-cine = Reproduzindo a { $rate } quadros/s
medical-overlays-shown = Sobreposições visíveis
medical-overlays-hidden = Sobreposições ocultas
measure-hint-length = Comprimento: arraste ou clique nas duas pontas
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
measure-caveat-detector = No detector: sem correção de ampliação
measure-caveat-magnification = Correção de ampliação estimada
measure-caveat-uncalibrated = Sem espaçamento de pixels: em pixels
measure-caveat-outside-region = Fora de uma região de ultrassom calibrada: em pixels

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Mostrar rótulos de orientação
settings-medical-orientation-labels-help = Letras nas bordas da imagem com as direções do paciente (R/L direita/esquerda, A/P anterior/posterior, S/I cabeça/pés), lidas do arquivo e acompanhando rotações e espelhamentos. L as mostra ou oculta durante a sessão.
settings-medical-wheel-steps-frames = Roda do mouse percorre os quadros
settings-medical-wheel-steps-frames-help = Em um arquivo ou série com vários quadros, a roda passa de um quadro a outro em vez de dar zoom ou trocar de imagem. Ctrl+roda continua dando zoom.
# machine-drafted, needs native review
settings-medical-display-function = Exibição de cinzas
settings-medical-display-function-help = Como os tons de cinza das imagens médicas são exibidos. O DICOM GSDF (função de exibição padrão em escala de cinza) os distribui em passos de visibilidade igual entre as luminâncias do preto e do branco do monitor (DICOM PS3.14), como em um monitor de diagnóstico calibrado. O perfil de cor do monitor continua sendo aplicado.
settings-medical-display-function-srgb = Padrão (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Luminância do preto do monitor
settings-medical-display-black-help = A luminância do preto do monitor, incluindo a luz ambiente, obtida por medição ou pela ficha técnica do monitor. Usada pelo DICOM GSDF.
settings-medical-display-white = Luminância do branco do monitor
settings-medical-display-white-help = A luminância do branco do monitor, obtida por medição ou pela ficha técnica do monitor. Usada pelo DICOM GSDF.
palette-orientation-labels = Mostrar ou ocultar rótulos de orientação
palette-presentation-state = Próximo estado de apresentação
disabled-no-presentation-state = Nenhum estado de apresentação se refere a esta imagem
medical-labels-shown = Rótulos de orientação visíveis
medical-labels-hidden = Rótulos de orientação ocultos
medical-presentation = Estado de apresentação: { $label } ({ $index } de { $count })
medical-presentation-none = Estado de apresentação desativado
medical-presentation-readout = Apresentação: { $label }
medical-preset-presentation = A tabela de consulta do estado de apresentação define os níveis de cinza

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientação: esquerda { $left }, direita { $right }, topo { $top }, base { $bottom }
