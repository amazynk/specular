# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Português (Brasil).

keys-mode-science = Imagens científicas
cheatsheet-science = Imagens científicas
cheatsheet-science-look = Esticamento e limites
cheatsheet-science-planes = Planos e canais
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Mostrar ou ocultar os canais 1 a 9
cheatsheet-science-channels-10-16 = Mostrar ou ocultar os canais 10 a 16
palette-science-stretch = Próximo esticamento
palette-science-limits = Próximos limites
palette-science-next-plane = Próximo plano
palette-science-prev-plane = Plano anterior
palette-science-next-axis = Percorrer outro eixo
palette-science-composite = Compor os canais em cores
palette-science-channel-1 = Mostrar ou ocultar o canal 1
palette-science-channel-2 = Mostrar ou ocultar o canal 2
palette-science-channel-3 = Mostrar ou ocultar o canal 3
palette-science-channel-4 = Mostrar ou ocultar o canal 4
palette-science-channel-5 = Mostrar ou ocultar o canal 5
palette-science-channel-6 = Mostrar ou ocultar o canal 6
palette-science-channel-7 = Mostrar ou ocultar o canal 7
palette-science-channel-8 = Mostrar ou ocultar o canal 8
palette-science-channel-9 = Mostrar ou ocultar o canal 9
# machine-drafted, needs native review
palette-science-channel-10 = Mostrar ou ocultar o canal 10
palette-science-channel-11 = Mostrar ou ocultar o canal 11
palette-science-channel-12 = Mostrar ou ocultar o canal 12
palette-science-channel-13 = Mostrar ou ocultar o canal 13
palette-science-channel-14 = Mostrar ou ocultar o canal 14
palette-science-channel-15 = Mostrar ou ocultar o canal 15
palette-science-channel-16 = Mostrar ou ocultar o canal 16
palette-science-grid = Mostrar ou ocultar a grade de coordenadas
palette-science-reset = Aparência automática
palette-science-level = Nivelar mapas de altura (subtrair o plano médio)
palette-science-wiggle = Traços sísmicos wiggle com área variável, sobre as cores
# machine-drafted, needs native review
palette-science-wireframe = Arestas da malha não estruturada (aramado), sobre as cores
palette-science-import-lut = Tabela de cores de um arquivo (ImageJ .lut)…
palette-science-hillshade = Sombrear o relevo de modelos de elevação (sombreamento)
palette-science-fft = Espectro de potência (FFT) do plano
palette-science-catalogue = Sobrepor um catálogo de fontes (VOTable, tabela FITS, CSV)…
palette-science-clear-catalogues = Remover os catálogos sobrepostos
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] uma fonte na imagem
       *[other] { $count } fontes na imagem
    }
science-catalogue-capped = { $name }: exibindo { $shown } de { $count } fontes na imagem
science-catalogue-none = { $name }: nenhuma fonte na imagem
science-catalogue-failed = Não é um catálogo com posições celestes: { $name }
science-catalogues-cleared = Catálogos removidos
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Catálogos de fontes
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Imagem iônica em um valor de m/z (espectrometria de massas)…
disabled-not-science = Somente para imagens científicas (astronomia, microscopia eletrônica e de sonda de varredura)
disabled-one-plane = A imagem tem um só plano
disabled-one-axis = A imagem não tem outro eixo para percorrer
disabled-no-channels = A imagem não tem esse canal para compor
disabled-no-coordinates = A imagem não tem coordenadas celestes
disabled-not-height-map = Somente para mapas de altura (Gwyddion, Igor)
disabled-not-elevation = Somente para modelos de elevação georreferenciados de uma banda
disabled-not-seismic = Somente para dados sísmicos (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Somente para imagens de espectrometria de massas (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Somente para malhas não estruturadas (NetCDF UGRID, ICON e MPAS)
disabled-not-bayer = Apenas para quadros brutos de câmeras coloridas (FITS com padrão Bayer, vídeo SER)
palette-debayer = Mostrar quadros de câmeras coloridas com debayer ou brutos
palette-bayer-pattern = Próximo padrão Bayer para este arquivo
science-debayer-on = Com debayer ({ $pattern })
science-debayer-off = Matriz de filtros de cor bruta
science-bayer-pattern = Padrão Bayer { $pattern }
# machine-drafted, needs native review
palette-annotations = Mostrar ou ocultar as anotações
disabled-no-annotations = A imagem não tem anotações ao lado (NDPA, Aperio ou ASAP XML)
science-annotations-on = Anotações visíveis
science-annotations-off = Anotações ocultas
science-stretch-auto = Automático (por cor)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Linear
science-stretch-sqrt = Raiz quadrada
science-stretch-log = Logarítmico
science-stretch-asinh = Asinh
science-stretch-histeq = Equalização de histograma
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Mín/máx
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Intervalo do arquivo
# Toasts when the look changes.
science-stretch-toast = Esticamento: { $stretch }
science-limits-toast = Limites: { $limits }
science-reset-toast = Aparência automática
# An imported colour table: $name is its file name.
science-lut-toast = Tabela de cores { $name }
science-lut-failed = Não é uma tabela de cores: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = Tabelas de cores do ImageJ
science-composite-on = Canais compostos em cores
science-composite-off = Um canal em cinza
science-channel-on = { $channel } visível
science-channel-off = { $channel } oculto
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Canal { $number }
science-grid-on = Grade de coordenadas visível
science-grid-off = Grade de coordenadas oculta
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Anéis de resolução visíveis
science-rings-off = Anéis de resolução ocultos
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Traços wiggle ativados (aproxime para vê-los)
science-wiggle-off = Traços wiggle desativados
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Arestas da malha ativadas
science-wireframe-off = Arestas da malha desativadas
science-wireframe-dense = Arestas da malha ativadas (aproxime para vê-las)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Nivelamento do plano ativado
science-level-off = Nivelamento do plano desativado
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Sombreamento ativado: sol a { $azimuth }°, { $altitude }° de altura
science-hillshade-off = Sombreamento desativado
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Espectro de potência exibido
science-fft-off = Espectro de potência oculto
science-axis-toast = Percorrendo: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Valor { $value }
science-value-unit = Valor { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Valor { $value } (bits { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Valor { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Sem dados
s2-class-defective = Saturado ou defeituoso
s2-class-dark = Pixels de áreas escuras
s2-class-cloud-shadow = Sombras de nuvens
s2-class-vegetation = Vegetação
s2-class-not-vegetated = Sem vegetação
s2-class-water = Água
s2-class-unclassified = Não classificado
s2-class-cloud-medium = Nuvens de probabilidade média
s2-class-cloud-high = Nuvens de probabilidade alta
s2-class-cirrus = Cirros finos
s2-class-snow = Neve ou gelo
s2-class-clear = Céu limpo
s2-class-opaque-cloud = Nuvens opacas
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = AR { $ra }  Dec { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS não suportado: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Marcadores do catálogo: ICRS → FK4 convertido
science-fk4-markers-galactic = Marcadores do catálogo: galáctico → FK4 convertido
science-fk4-samp = SAMP: FK4 → ICRS convertido
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Plano nivelado
# Shown under the look while the hillshade is on.
science-hillshaded = Sombreamento: sol a { $azimuth }°, { $altitude }° de altura
# Shown in the readout while the power spectrum is shown.
science-fft = Espectro de potência (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ potência { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frequência u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Período { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Fase { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Espectro em { $x }, { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (bin { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = { $count ->
        [one] { $count } bin de underflow/overflow oculto (soma { $sum })
        [many] { $count } bins de underflow/overflow ocultos (soma { $sum })
       *[other] { $count } bins de underflow/overflow ocultos (soma { $sum })
    }
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Bins de largura variável reamostrados para uma grade uniforme
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } evento
    [many] { $count } de eventos
   *[other] { $count } eventos
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Compensado ({ $keyword })
science-cytometry-uncompensated = Não compensado
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Traço { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Sem traço
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Tempo { $time } ms
science-seismic-depth = Profundidade { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Face { $face }
science-mesh-face-node = Face { $face }  Nó { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Fora da malha
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } de { $count }
science-plane-world = { $axis } { $index } de { $count } ({ $world })
science-axis-unnamed = Plano
science-axis-extension = Imagem
science-axis-channel = Canal
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Campo
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energia
science-band-all = todas as energias
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Parte
science-part-magnitude = módulo
science-part-real = parte real
science-part-imaginary = parte imaginária
science-part-phase = fase (radianos)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensidade (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Quadro
science-readout = Valor e coordenadas
# Accessible names of the plane sliders.
science-sliders = Planos
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Barra de escala: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Corrente iônica total
science-ion-window = Imagem iônica m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Imagem iônica em m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolerância · Enter mostrar · Esc cancelar
science-ion-bad = Não é um valor de m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Ponto x { $x }  y { $y }
science-ms-spot-z = Ponto x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Nenhum espectro neste ponto
# machine-drafted, needs native review
palette-science-temperature-unit = Unidade de temperatura (°C, °F, K)
palette-science-isotherm = Isoterma acima ou abaixo da temperatura sob o ponteiro
palette-science-thermal-parameters = Emissividade e temperatura refletida…
palette-science-export-temperatures = Exportar temperaturas (TIFF de ponto flutuante ou CSV)…
palette-thermal-visible = Foto visível de uma imagem térmica
disabled-not-thermal = Somente para imagens térmicas
disabled-not-radiometric = O arquivo armazena temperaturas: seus parâmetros não podem ser alterados
disabled-no-visible-photo = A imagem térmica não tem foto visível
science-temperature-unit-toast = Temperaturas em { $unit }
science-isotherm-above = Isoterma acima de { $level }
science-isotherm-below = Isoterma abaixo de { $level }
science-isotherm-off = Isoterma desativada
science-thermal-emissivity-prompt = Emissividade: { $text }
science-thermal-reflected-prompt = Temperatura refletida ({ $unit }): { $text }
science-thermal-prompt-hint = Tab próximo campo · Enter recalcular · Esc cancelar
science-thermal-bad = Valor inválido: { $text }
science-thermal-applied = Recalculado para emissividade { $emissivity }, refletida { $reflected }
science-thermal-conditions = Emissividade { $emissivity } · refletida { $reflected }
science-thermal-export-tiff = TIFF de ponto flutuante de 32 bits
science-thermal-export-csv = CSV
science-thermal-export-done = Temperaturas salvas em { $name }
science-thermal-export-failed = Não foi possível salvar as temperaturas: { $reason }
# machine-drafted, needs native review
science-values-failed = Não foi possível mostrar os valores: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Reproduzir ou pausar a sequência térmica
palette-science-thermal-series-csv = Salvar as temperaturas ao longo do tempo (CSV)…
disabled-not-thermal-sequence = Somente para sequências térmicas
thermal-seq-title = Temperaturas ao longo do tempo
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Quadro { $frame } de { $frames } · { $time }
thermal-seq-play = Reproduzir
thermal-seq-pause = Pausar
thermal-seq-playing = Reproduzindo
thermal-seq-paused = Pausado
thermal-seq-save-csv = Salvar CSV…
thermal-seq-hint = Adicione um ponto, uma linha ou uma área (M) para traçar sua curva no tempo.
thermal-seq-reading = Lendo o quadro { $done } de { $total }…
thermal-seq-failed = Não foi possível ler os quadros: { $reason }
thermal-seq-no-series = Nada para salvar ainda: adicione um ponto, uma linha ou uma área.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Gráfico: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Relatório (PDF)…
thermal-report-heading = Relatório térmico
thermal-report-default-title = Relatório de inspeção termográfica
thermal-report-title-label = Título
thermal-report-notes-label = Notas
thermal-report-photo = Incluir a foto visível
thermal-report-folder = Adicionar as outras imagens térmicas desta pasta
thermal-report-save = Salvar PDF…
thermal-report-busy = Gerando o relatório…
thermal-report-nothing = As temperaturas ainda não estão sendo exibidas.
thermal-report-saved = Relatório salvo em { $name } ({ $count ->
    [one] { $count } imagem
    [many] { $count } imagens
   *[other] { $count } imagens
})
thermal-report-failed = Não foi possível salvar o relatório: { $reason }
# In the PDF.
thermal-report-notes = Notas
thermal-report-measurements = Medições
thermal-report-no-measurements = Nenhuma medição.
thermal-report-thermal-caption = Imagem térmica
thermal-report-visible-caption = Foto visível
thermal-report-col-measurement = Medição
thermal-report-col-min = Mín.
thermal-report-col-max = Máx.
thermal-report-col-mean = Média
thermal-report-col-emissivity = Emissividade
thermal-report-whole-image = Imagem inteira
thermal-report-camera = Câmera
thermal-report-lens = Lente
thermal-report-date = Capturada
thermal-report-place = Local
thermal-report-frame = Quadro
thermal-report-emissivity = Emissividade
thermal-report-reflected = Temperatura refletida
thermal-report-distance = Distância
thermal-report-humidity = Umidade
thermal-report-range = Faixa
thermal-report-format = Formato
thermal-report-page = Página { $page } de { $pages }
