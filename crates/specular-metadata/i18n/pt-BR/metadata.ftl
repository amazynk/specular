# machine-drafted, needs native review
# Painel de metadados do Specular — Português do Brasil.
# Veja i18n/en/metadata.ftl para as convenções. Números e datas chegam já
# formatados pelo ICU4X para a localidade.

## Títulos dos grupos do resumo

group-camera = Câmera
group-exposure = Exposição
group-lens = Lente
group-date = Data e hora
group-location = Local
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drone
label-gimbal = Gimbal
label-flight = Aeronave
label-relative-altitude = Acima do ponto de decolagem
drone-attitude = guinada { $yaw }, arfagem { $pitch }, rolagem { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperaturas
drone-thermal-refused = Não exibido: a licença do SDK da DJI não permite
drone-thermal-refused-detail = Ler temperaturas da DJI exige o Thermal SDK da DJI, cuja licença só permite usá-lo em programas que o incluem. A imagem e seus metadados são exibidos.
group-image = Imagem
group-file = Arquivo
group-rights = Direitos e descrição
group-credentials = Credenciais de conteúdo
# machine-drafted, needs native review
group-medical = Imagem médica
# machine-drafted, needs native review
group-inspection = Componente e inspeção
group-nde-image = Imagem de inspeção (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Imagem de inspeção
group-science = Imagem científica
group-em = Microscopia eletrônica
group-spm = Microscopia de sonda de varredura
group-slide = Lâmina
group-microscopy = Microscopia
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Difração
group-surface = Topografia de superfície
group-ebsd = Difração de elétrons retroespalhados
group-seismic = Dados sísmicos
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = Imagem CASA
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Tratografia
# machine-drafted, needs native review
group-annotations = Anotações
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Imageamento por espectrometria de massas
# machine-drafted, needs native review
group-thermal = Imagem térmica
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Citometria de fluxo
# machine-drafted, needs native review
group-spectroscopy = Espectroscopia
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Malha não estruturada
group-drawing = Desenho
# machine-drafted, needs native review
group-volume = Volume

## Rótulos dos itens do resumo

label-camera = Câmera
label-software = Software
label-application = Aplicativo
label-shutter = Velocidade do obturador
label-aperture = Abertura
label-iso = ISO
label-focal-length = Distância focal
label-exposure-bias = Compensação de exposição
label-program = Modo
label-metering = Medição
label-white-balance = Balanço de branco
label-flash = Flash
label-lens = Lente
label-lens-make = Fabricante da lente
label-lens-range = Faixa da lente
# machine-drafted, needs native review
label-focus-mode = Modo de foco
label-stabilisation = Estabilização de imagem
label-taken = Capturada em
label-digitised = Digitalizada em
label-edited = Última edição
label-place = Local
label-coordinates = Coordenadas
label-altitude = Altitude
label-direction = Direção
label-accuracy = Precisão
label-gps-time = Hora do GPS
label-dimensions = Dimensões
label-encoding = Codificação
# machine-drafted, needs native review
label-motion = Movimento
motion-kind-motion-photo = Foto com movimento
motion-kind-micro-video = Foto com movimento (MicroVideo)
motion-kind-samsung = Foto com movimento Samsung
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Perfil de cor
label-orientation = Orientação
label-file-name = Nome
label-folder = Pasta
label-file-size = Tamanho
label-format = Formato
label-modified = Modificado em
label-title = Título
label-description = Descrição
label-creator = Autor
label-copyright = Direitos autorais
label-keywords = Palavras-chave
label-rating = Classificação
label-colour-label = Rótulo
label-credentials = Credenciais de conteúdo

## Títulos dos grupos de "Todos os campos" (os nomes das tags ficam como nos padrões)

fields-ifd0 = IFD0 (imagem principal)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperabilidade
fields-ifd1 = IFD1 (miniatura)
fields-icc = Cabeçalho do perfil ICC
# $namespace: de xmp-namespace; $prefix: por exemplo "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Básico
    [xmpRights] Direitos
    [xmpMM] Gerenciamento de mídia
    [xmpDM] Mídia dinâmica
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Acessórios da câmera
    [photoshop] Photoshop
    [crs] Configurações do Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] Extensão IPTC
    [plus] Licenciamento PLUS
    [GPano] Foto esférica
    [GCamera] Câmera do Google
    [hdrgm] Mapa de ganho HDR
    [mwg-rs] Regiões
    [pdf] PDF
    [stEvt] Eventos do recurso
    [stRef] Referências do recurso
   *[other] Outro
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Tag desconhecida { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } byte)
   *[other] ({ $num } bytes)
}
maker-note-bytes = ({ $num ->
        [one] { $num } byte
       *[other] { $num } bytes
    }, nota do fabricante, não decodificada)
more-values = { $shown }, … ({ $total ->
        [one] { $total } valor
       *[other] { $total } valores
    })

## Ligações genéricas

# Separa as partes de uma lista montada no código (descrição do flash, áreas do local).
list-separator = {", "}
# Separa obturador · abertura · ISO · distância focal no resumo de uma linha.
one-line-separator = {" · "}

## Exposição

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm (equiv. { $equiv } mm)
focal-length-equiv-only = equiv. { $equiv } mm
# $ev é o módulo; o sinal vem do seletor.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manual
    [2] Programa (automático)
    [3] Prioridade de abertura
    [4] Prioridade de obturador
    [5] Criativo (profundidade de campo)
    [6] Ação (obturador rápido)
    [7] Retrato
    [8] Paisagem
   *[other] Desconhecido
}
metering-mode = { $value ->
    [1] Média
    [2] Média ponderada ao centro
    [3] Pontual
    [4] Multipontual
    [5] Matricial (avaliativa)
    [6] Parcial
   *[other] Outro
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] AF One-Shot
    [1] AF AI Servo
    [2] AF AI Focus
    [3] Foco manual
    [4] Único
    [5] Contínuo
    [6] Foco manual
    [16] Pan Focus
    [256] AF One-Shot (Live View)
    [257] AF AI Servo (Live View)
    [258] AF AI Focus (Live View)
    [512] Foco Movie Snap
    [519] AF Servo de vídeo
   *[other] Outro
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Desativada
    [1] Ativada
    [2] Somente ao fotografar
    [3] Panning
    [4] Dinâmica
    [256] Desativada
    [257] Ativada
    [258] Somente ao fotografar
    [259] Panning
    [260] Dinâmica
   *[other] Outro
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Automático
    [2] Foco manual
    [4] Automático (botão de foco)
    [5] Automático (contínuo)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Outro
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Ligado (óptico)
    [3] Desligado
    [4] Ligado (modo 2)
    [5] Ligado (óptico, panorâmica)
    [6] Ligado (só no corpo)
    [7] Ligado (só no corpo, panorâmica)
    [9] Dual IS
    [10] Dual IS (panorâmica)
    [11] Dual IS 2
    [12] Dual IS 2 (panorâmica)
   *[other] Outro
}
label-comment = Comentário
fields-tags = Tags { $kind }
group-picture = Ajustes de imagem
fields-fujifilm = Nota do fabricante Fujifilm
label-film-simulation = Simulação de filme
label-dynamic-range = Faixa dinâmica
label-highlight-tone = Realces
label-shadow-tone = Sombras
label-colour-setting = Cor
label-sharpness = Nitidez
label-noise-reduction = Redução de ruído
label-clarity = Claridade
label-grain-effect = Efeito de granulação
label-colour-chrome = Cor cromada
label-colour-chrome-blue = Cor cromada azul
label-shutter-type = Tipo de obturador
label-drive-mode = Modo de disparo
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (padrão)
    [256] Retrato de estúdio
    [272] Retrato de estúdio (mais saturação)
    [288] Astia (suave)
    [304] Retrato de estúdio (mais nítido)
    [512] Velvia (vívido)
    [768] Retrato de estúdio Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Outro
}
fuji-monochrome = { $value ->
    [768] Monocromático
    [769] Monocromático, filtro vermelho
    [770] Monocromático, filtro amarelo
    [771] Monocromático, filtro verde
    [784] Sépia
    [1280] Acros
    [1281] Acros, filtro vermelho
    [1282] Acros, filtro amarelo
    [1283] Acros, filtro verde
   *[other] Monocromático
}
fuji-white-balance = { $value ->
    [0] Automático
    [1] Automático (prioridade ao branco)
    [2] Automático (prioridade ao ambiente)
    [256] Luz do dia
    [512] Nublado
    [768] Fluorescente luz do dia
    [769] Fluorescente branco dia
    [770] Fluorescente branco
    [771] Fluorescente branco quente
    [772] Fluorescente branco quente residencial
    [1024] Incandescente
    [1280] Flash
    [1536] Subaquático
    [3840] Personalizado 1
    [3841] Personalizado 2
    [3842] Personalizado 3
    [3843] Personalizado 4
    [3844] Personalizado 5
    [4080] Temperatura de cor
   *[other] Outro
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Automático (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Automático
    [256] Padrão (DR100)
    [512] Amplo 1 (DR230)
    [513] Amplo 2 (DR400)
   *[other] Outro
}
fuji-effect = { $value ->
    [0] Desligado
    [32] Fraco
    [64] Forte
   *[other] Outro
}
fuji-grain-size = { $value ->
    [16] pequeno
    [32] grande
   *[other] outro
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mecânico
    [1] Eletrônico
    [2] Eletrônico (exposições longas)
    [3] Primeira cortina eletrônica
   *[other] Outro
}
fuji-drive-mode = { $value ->
    [0] Foto única
    [1] Contínuo, baixa velocidade
    [2] Contínuo, alta velocidade
   *[other] Outro
}
fuji-focus-mode = { $value ->
    [0] Manual
    [1] AF-S
    [2] AF-C
    [100] Automático
    [101] Manual
   *[other] Outro
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Nenhuma
    [1] Óptica
    [2] Deslocamento do sensor
    [3] Lente
    [258] Deslocamento do sensor e digital
    [512] Digital
   *[other] Outro
}
fuji-stabilisation-mode = { $value ->
    [0] Desligado
    [1] ligada, contínua
    [2] ligada, ao fotografar
   *[other] Outro
}
light-source = { $value ->
    [1] Luz do dia
    [2] Fluorescente
    [3] Tungstênio
    [4] Flash
    [9] Tempo bom
    [10] Nublado
    [11] Sombra
    [12] Fluorescente luz do dia
    [13] Fluorescente branco-dia
    [14] Fluorescente branco-frio
    [15] Fluorescente branco
    [16] Fluorescente branco-quente
    [17] Luz padrão A
    [18] Luz padrão B
    [19] Luz padrão C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Tungstênio de estúdio
   *[other] Outra
}
white-balance-auto = Automático
white-balance-manual = Manual
white-balance-manual-with-light = Manual ({ $light })
# A descrição do flash é montada com estas partes, unidas por list-separator.
flash-none = Sem função de flash
flash-fired = Flash disparado
flash-not-fired = Flash não disparado
flash-forced-on = forçado
flash-forced-off = desativado à força
flash-auto = modo automático
flash-return-not-detected = luz de retorno não detectada
flash-return-detected = luz de retorno detectada
flash-red-eye = redução de olhos vermelhos

## Lente

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Data e hora

# $date: "terça-feira, 14 de maio de 2024"; $time: "18:32"
# $weekday: "terça-feira" (ICU); $date: "14 de mai. de 2024" (ICU)
date-with-weekday = { $weekday }, { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Fuso horário deduzido do relógio do GPS

## Local

# $lat e $lon não têm sinal; o hemisfério vem do seletor.
coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] O
   *[east] L
}
altitude = { $ref ->
    [above] { $m } m acima do nível do mar
    [below] { $m } m abaixo do nível do mar
   *[sea] No nível do mar
}
compass-point = { $point ->
    [n] norte
    [nne] norte-nordeste
    [ne] nordeste
    [ene] leste-nordeste
    [e] leste
    [ese] leste-sudeste
    [se] sudeste
    [sse] sul-sudeste
    [s] sul
    [ssw] sul-sudoeste
    [sw] sudoeste
    [wsw] oeste-sudoeste
    [w] oeste
    [wnw] oeste-noroeste
    [nw] noroeste
   *[nnw] norte-noroeste
}
# $point: de compass-point; $degrees: "287"
facing = Voltada para { $point } ({ $degrees }°)
detail-true-north = Em relação ao norte verdadeiro
detail-magnetic-north = Em relação ao norte magnético
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# A mais de 30 km da cidade encontrada.
place-near = perto de { $place }
place-detail = { $areas } · { $distance } de { $city }
place-detail-distance = { $distance } de { $city }

## Imagem

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } bits
# $format: "YCbCr 4:2:0 de 8 bits"
encoding-progressive = { $format }, progressivo
encoding-baseline = { $format }, baseline
encoding-hierarchical = { $format }, hierárquico
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] com alfa pré-multiplicado
    [transparency] com transparência
   *[alpha] com alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Tons de cinza
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Cor indexada
    [Multichannel] Multicanal
    [Duotone] Duotônico
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminância/crominância (YC)
    [Bayer] RAW Bayer
    [X-Trans] RAW X-Trans
    [CFA] RAW com matriz de filtros de cor
    [LinearRaw] RAW linear
    [RGBE] RGBE (expoente compartilhado)
    [XYZE] XYZE (expoente compartilhado)
    [R] Canal único (R)
    [RG] Dois canais (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (pela tag EXIF; sem perfil incorporado)
orientation = { $value ->
    [1] Na posição normal (sem rotação)
    [2] Espelhada horizontalmente (aplicado)
    [3] Girada 180° (aplicado)
    [4] Espelhada verticalmente (aplicado)
    [5] Espelhada horizontalmente e girada 90° no sentido anti-horário (aplicado)
    [6] Girada 90° no sentido horário (aplicado)
    [7] Espelhada horizontalmente e girada 90° no sentido horário (aplicado)
    [8] Girada 90° no sentido anti-horário (aplicado)
   *[other] Desconhecida ({ $value })
}
# Forma curta para avisos.
orientation-short = { $value ->
    [1] Na posição normal
    [2] Espelhada horizontalmente
    [3] Girada 180°
    [4] Espelhada verticalmente
    [5] Espelhada horizontalmente e girada 90° no sentido anti-horário
    [6] Girada 90° no sentido horário
    [7] Espelhada horizontalmente e girada 90° no sentido horário
    [8] Girada 90° no sentido anti-horário
   *[other] valor { $value }
}

## Arquivo

file-size-bytes = { $num ->
    [one] { $num } byte
   *[other] { $num } bytes
}
# Unidades decimais (1 KB = 1000 bytes).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Direitos

rating-rejected = Rejeitada
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n } de 5)

## Credenciais de conteúdo

c2pa-present = Presente (não verificado)
c2pa-detail = Este arquivo contém um manifesto C2PA; a assinatura dele não foi verificada.

## Valores do cabeçalho ICC

icc-class = { $class ->
    [scnr] Dispositivo de entrada
    [mntr] Dispositivo de exibição
    [prtr] Dispositivo de saída
    [link] Vínculo de dispositivos
    [spac] Espaço de cor
    [abst] Abstrato
    [nmcl] Cor nomeada
   *[other] Desconhecido
}
icc-intent = { $intent ->
    [0] Perceptual
    [1] Colorimétrico relativo
    [2] Saturação
    [3] Colorimétrico absoluto
   *[other] Desconhecido
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Não especificada
   *[other] Desconhecida
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalidade
label-body-part = Parte do corpo
label-study = Estudo
label-series = Série
label-slice-thickness = Espessura do corte
label-pixel-spacing = Espaçamento de pixels
label-imager-pixel-spacing = Espaçamento de pixels no detector
label-frames = Quadros
label-transfer-syntax = Sintaxe de transferência
label-document = Documento
label-document-type = Tipo de documento
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (compressão com perdas)
detail-pixel-spacing = Distância entre os centros dos pixels no paciente (linhas × colunas).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Distância entre os centros dos pixels no objeto (linhas × colunas).
detail-imager-pixel-spacing = Medido no detector, sem correção da ampliação: as distâncias no paciente costumam ser menores.

## Inspeção industrial (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Forma do componente
label-nde-component-manufacturer = Fabricante do componente
label-nde-material = Material
label-nde-thickness = Espessura do material
label-nde-pipe-diameter = Diâmetro do tubo
label-nde-source = Fonte
label-nde-environment = Ambiente
label-nde-iqi = Indicador de qualidade de imagem
label-nde-indications = Indicações
label-nde-standard = Norma

## Volumes de pesquisa (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Tamanho do voxel
label-data-type = Tipo de dados
label-time-points = Pontos temporais
label-orientation-source = Orientação de
label-intent = Valores
label-compression = Compressão
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Cabeçalho Analyze (não confiável)
volume-orient-space = Direções espaciais NRRD
volume-orient-transform = Transformação MetaImage
volume-orient-spacings = Apenas espaçamento de voxel
volume-orient-none = Não informada
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = Padrão do FreeSurfer (não confiável)
volume-orient-minc = Cossenos diretores do MINC
volume-orient-object = Coordenadas do objeto (sem orientação do paciente)
volume-content-projections = Projeções (aquisição bruta)
volume-content-reconstruction = Volume reconstruído
volume-coords-rectilinear = Retilínea (espaçamento irregular, exibida uniforme)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Tempo de repetição
label-volume-te = Tempo de eco
label-volume-ti = Tempo de inversão
label-volume-flip = Ângulo de inclinação
label-volume-fov = Campo de visão
label-volume-field = Intensidade do campo
label-volume-pedir = Codificação de fase
label-volume-cmdline = Linha de comando
label-volume-xform = Transformação de Talairach
label-volume-content = Conteúdo
label-volume-projections = Projeções
label-volume-angles = Faixa de rotação
label-volume-angle-step = Passo de rotação
label-volume-kv = Tensão do tubo
label-volume-current = Corrente do tubo
label-volume-power = Potência da fonte
label-volume-exposure = Exposição
label-volume-sod = Fonte–objeto
label-volume-odd = Objeto–detector
label-volume-sdd = Fonte–detector
label-volume-magnification = Ampliação óptica
label-volume-binning = Binning
label-volume-objective = Objetiva
label-volume-filter = Filtro da fonte
label-volume-camera = Detector
label-volume-instrument = Instrumento
label-volume-software = Software
label-volume-date = Adquirido
label-volume-ring = Correção de artefatos em anel
label-volume-smoothing = Suavização
label-volume-coords = Grade
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Olho
volume-eye-right = Direito (OD)
volume-eye-left = Esquerdo (OS)
detail-volume-unreliable = O arquivo não indica para onde seus eixos apontam no paciente: esquerda e direita podem estar trocadas, por isso nenhum rótulo de orientação é exibido.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Os eixos são os do scanner, não os de um paciente, por isso nenhum rótulo de orientação é exibido.

## Imagens científicas (FITS, XISF)

label-object = Objeto
label-telescope = Telescópio
label-instrument = Instrumento
label-filter = Filtro
label-observed = Observado
label-exposure-time = Exposição
label-sample = Amostras
label-unit = Unidade
label-hdu = Extensão
label-centre = Centro
label-centre-deg = Centro (graus)
label-frame = Sistema de coordenadas
# machine-drafted, needs native review
science-frame-galactic = Galáctico
science-frame-ecliptic = Eclíptico
label-projection = Projeção
label-pixel-scale = Escala do pixel
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ por pixel
label-distortion = Distorção
label-wcs = Coordenadas celestes
science-wcs-unsupported = Não suportado: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Aproximadas (apenas apontamento, sem distorção)
label-observer = Observador
label-bayer = Padrão Bayer
label-capture-start = Início da captura (UTC)
label-capture-end = Fim da captura (UTC)
label-frame-rate = Taxa de quadros
science-fps = { $fps } quadros/s
science-frames-cut = { $frames } de { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Conjunto de dados
label-pixel-size = Tamanho do pixel
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } por pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Microscópio
label-voltage = Tensão de aceleração
label-magnification = Ampliação
label-imaging-mode = Modo
label-detector = Detector
label-acquired = Adquirido
label-field-of-view = Campo de visão
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Fabricante
label-scanner = Scanner
label-objective = Objetiva
label-pyramid-levels = Níveis
# machine-drafted, needs native review
label-emissivity = Emissividade
label-object-distance = Distância do objeto
label-reflected-temperature = Temperatura refletida
label-atmospheric-temperature = Temperatura atmosférica
label-humidity = Umidade relativa
label-raw-range = Intervalo bruto
label-temperature-range = Intervalo de temperatura
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Comprimento de onda
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Energia do fóton
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Distância do detector
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Centro do feixe
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = 2θ do detector
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Fi (φ)
label-omega = Ômega (ω)
label-chi = Qui (χ)
label-kappa = Capa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscilação
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° em torno de { $axis }
# The count at which a pixel saturates.
label-overload = Valor de saturação
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Resolução
xtal-resolution = { $edge } Å na borda, { $corner } Å nos cantos
label-geometry = Geometria
xtal-curved = Placa de imagem curva: sem q, d nem anéis de resolução
xtal-incomplete = Cabeçalho incompleto: sem q, d nem anéis de resolução
# All-fields group title: "FITS header".
fields-science-header = Cabeçalho { $format }
# machine-drafted, needs native review
fields-asdf-tree = Árvore ASDF
label-asdf-model = Modelo de dados
label-asdf-version = Versão do arquivo
label-asdf-standard = Padrão ASDF
label-asdf-library = Gravado por
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = Chaves ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = Versão do ROOT
label-root-compression = Compressão
label-root-keys = Chaves
label-root-class = Classe
label-root-name = Nome
label-root-title = Título
label-root-entries = Entradas
label-root-sum-weights = Soma dos pesos
# A graph's number of points.
label-root-points = Pontos
# $axis is "x", "y" or "z".
label-root-axis = Eixo { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
        [many] { $bins } bins
       *[other] { $bins } bins
    }, de { $min } a { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } bin variável
        [many] { $bins } bins variáveis
       *[other] { $bins } bins variáveis
    }, de { $min } a { $max }
label-root-mean-rms = Média e RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Árvores
root-tree = { $name } ({ $entries ->
        [one] { $entries } entrada
        [many] { $entries } entradas
       *[other] { $entries } entradas
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Tipo de imagem
casa-type-image = Imagem restaurada
casa-type-residual = Resíduo
casa-type-model = Modelo
casa-type-pb = Feixe primário
casa-type-mask = Máscara de limpeza
casa-type-psf = Função de espalhamento de ponto
casa-type-sumwt = Soma dos pesos
casa-type-weight = Pesos
label-casa-beam = Feixe de restauração
casa-beam = { $major }″ × { $minor }″, AP { $pa }°
label-casa-plane-beams = Feixes por plano
casa-plane-beams = { $count ->
        [one] { $count } feixe
        [many] { $count } de feixes
       *[other] { $count } feixes
    }, eixo maior de { $min }″ a { $max }″
label-casa-rest-frequency = Frequência de repouso
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Referencial espectral
label-casa-velocity = Definição de velocidade
casa-velocity-radio = Rádio
casa-velocity-optical = Óptica
casa-velocity-relativistic = Relativística
label-casa-mask = Máscara padrão
label-casa-storage = Armazenamento
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Malha
label-ugrid-convention = Convenção
label-ugrid-nodes = Nós
label-ugrid-faces = Faces
label-ugrid-edges = Arestas
label-ugrid-shapes = Formas das faces
label-ugrid-coordinates = Coordenadas
label-ugrid-values = Valores em
label-ugrid-split = Divididas para desenho
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } triângulo
    [many] { $count } de triângulos
   *[other] { $count } triângulos
}
ugrid-quads = { $count ->
    [one] { $count } quadrilátero
    [many] { $count } de quadriláteros
   *[other] { $count } quadriláteros
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } polígono de { $max } nós
    [many] { $count } de polígonos de até { $max } nós
   *[other] { $count } polígonos de até { $max } nós
}
ugrid-lonlat = Longitude e latitude
ugrid-projected = x e y projetados
ugrid-projected-unit = x e y projetados ({ $unit })
ugrid-on-faces = Faces (desenhadas em cor uniforme)
ugrid-on-nodes = Nós (interpolados nas faces)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Nenhum: a imagem mostra os nós por face
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } face no antimeridiano ou em um polo
    [many] { $count } de faces no antimeridiano ou em um polo
   *[other] { $count } faces no antimeridiano ou em um polo
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Arquivo
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Formas
label-annotation-groups = Grupos
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Omitidas
annotation-polygon = Polígono
annotation-polyline = Linha
annotation-rectangle = Retângulo
annotation-ellipse = Elipse
annotation-point = Ponto
annotation-points = Pontos
annotation-ruler = Régua
annotation-arrow = Seta
# The list of annotations is cut short: "and 1,200 more".
annotation-more = e mais { $count }
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Anotações em { $file }
label-drawing-version = Versão
label-drawing-units = Unidades
label-drawing-extents = Extensão
label-drawing-sheet = Folha
label-drawing-layouts = Layouts
label-drawing-layers = Camadas
label-drawing-entities = Entidades
label-drawing-codepage = Página de código
label-drawing-missing-fonts = Fontes substituídas
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (estilos { $styles }), substituída por { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (estilos { $styles }), substituída por fontes alternativas
# Survey data (LandXML).
label-survey-crs = Sistema de coordenadas
survey-crs-none = Não informado
label-survey-project = Nome do projeto
label-survey-application = Gravado por
label-survey-surface = Modelo de terreno
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } ponto
       *[other] { $points } pontos
    }, { $triangles ->
        [one] { $triangles } triângulo
       *[other] { $triangles } triângulos
    }; área em planta { $area } { $unit }², área real { $area3d } { $unit }²; { $volume } { $unit }³ acima do ponto mais baixo; cotas { $zmin }–{ $zmax } { $unit }; curvas de nível a cada { $interval } { $unit }
label-survey-alignment = Eixo
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, estacas { $start } a { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = Fontes SHX
label-drawing-warnings = Problemas
label-plot-paper = Papel
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } de { $pages }
label-plot-page = Página
label-plot-pens = Canetas usadas
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Trabalho de impressão
# What a plot draws: strokes, fills and labels.
label-plot-items = Elementos desenhados
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Modelo
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } desativadas ou congeladas
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Polegadas
    [2] Pés
    [3] Milhas
    [4] Milímetros
    [5] Centímetros
    [6] Metros
    [7] Quilômetros
    [8] Micropolegadas
    [9] Milésimos de polegada
    [10] Jardas
    [11] Ångströms
    [12] Nanômetros
    [13] Mícrons
    [14] Decímetros
    [15] Decâmetros
    [16] Hectômetros
    [17] Gigâmetros
    [18] Unidades astronômicas
    [19] Anos-luz
    [20] Parsecs
    [21] Pés de levantamento dos EUA
   *[other] Sem unidade
}
# All-fields group title: "DXF entities".
fields-drawing-entities = Entidades { $format }

## Layouts de circuitos integrados (GDSII)

# machine-drafted, needs native review
label-layout-library = Biblioteca
label-layout-units = Unidades
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } unidade de banco de dados, { $user } unidade de usuário
# The top cell shown (each top cell is a page).
label-layout-cell = Célula
label-layout-cells = Células
# Cells no other cell places.
label-layout-top-cells = Células de topo
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } e mais { $more }
# Layer/datatype pairs.
label-layout-layers = Camadas
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Formas
label-layout-texts = Rótulos
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Referências
layout-references = { $refs } ({ $placements } { $placements ->
    [one] instância
    [many] de instâncias
   *[other] instâncias
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Formas achatadas
label-layout-bounds = Limites
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } a { $x1 }, { $y1 } µm

## Placas de circuito e esquemáticos (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Placa
group-schematic = Esquemático
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Vista
label-pcb-project = Projeto
label-pcb-revision = Revisão
label-pcb-date = Data
# The CAD program that wrote the file.
label-pcb-generator = Criado com
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Tamanho da placa
label-pcb-thickness = Espessura
label-pcb-copper-layers = Camadas de cobre
label-pcb-sheets = Folhas
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Camadas
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Footprints
label-pcb-symbols = Símbolos
label-pcb-nets = Redes
label-pcb-tracks = Trilhas
label-pcb-wires = Fios
label-pcb-vias = Vias
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Furos
pcb-holes = { $holes } em { $sizes } { $sizes ->
    [one] tamanho
    [many] de tamanhos
   *[other] tamanhos
}
label-pcb-apertures = Aberturas
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Cobre interno { $n }
pcb-role-top-copper = Cobre superior
pcb-role-bottom-copper = Cobre inferior
pcb-role-top-solder-mask = Máscara de solda superior
pcb-role-bottom-solder-mask = Máscara de solda inferior
pcb-role-top-silkscreen = Serigrafia superior
pcb-role-bottom-silkscreen = Serigrafia inferior
pcb-role-top-paste = Pasta superior
pcb-role-bottom-paste = Pasta inferior
pcb-role-top-solder-paste = Pasta de solda superior
pcb-role-bottom-solder-paste = Pasta de solda inferior
pcb-role-top-adhesive = Adesivo superior
pcb-role-bottom-adhesive = Adesivo inferior
pcb-role-top-courtyard = Área de ocupação superior
pcb-role-bottom-courtyard = Área de ocupação inferior
pcb-role-top-fabrication = Fabricação superior
pcb-role-bottom-fabrication = Fabricação inferior
pcb-role-board-outline = Contorno da placa
pcb-role-plated-drill = Furação metalizada
pcb-role-non-plated-drill = Furação não metalizada
pcb-role-drill = Furação
pcb-role-drill-holes = Furos
pcb-role-mechanical = Mecânica
pcb-role-margin = Margem
pcb-role-drawings = Desenhos
pcb-role-comments = Comentários
pcb-role-engineering-change-1 = Alteração de engenharia 1
pcb-role-engineering-change-2 = Alteração de engenharia 2
pcb-role-vias = Vias
pcb-role-other = Outra
pcb-role-unknown = Desconhecida
label-pcb-macros = Macros de abertura

## Espectros (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Espectro
label-spectrum-format = Formato
label-spectrum-technique = Técnica
label-spectrum-title = Título
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Página
spectrum-page-of = { $page } de { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = Eixo X
label-spectrum-y = Eixo Y
label-spectrum-nucleus = Núcleo
label-spectrum-frequency = Frequência do espectrômetro
label-spectrum-solvent = Solvente
label-spectrum-pulse-program = Sequência de pulsos
label-spectrum-spectra = Espectros
label-spectrum-points = Pontos
label-spectrum-instrument = Instrumento
# Who or what lab measured it.
label-spectrum-origin = Origem
label-spectrum-date = Medido
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Processamento
spectrum-technique-ir = Infravermelho
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescência
spectrum-technique-nmr = RMN
spectrum-technique-ms = Espectro de massa
spectrum-technique-chromatogram = Cromatograma
spectrum-technique-other = Espectro

## Modelos 3D (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = Modelo 3D
label-triangles = Triângulos
label-vertices = Vértices
label-materials = Materiais
label-textures = Texturas
label-up-axis = Eixo vertical
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } ausentes)

## Estruturas moleculares (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Estrutura
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Entrada
label-structure-classification = Classificação
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Método
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Resolução
structure-resolution = { $value } Å
label-structure-deposited = Depositada
label-structure-formula = Fórmula
label-structure-chains = Cadeias
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Resíduos
label-structure-atoms = Átomos
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } na unidade assimétrica)
label-structure-bonds = Ligações
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modelos
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Registros
structure-first-of = { $count } (o primeiro é exibido)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligantes
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } e mais { $more }
label-structure-cell = Célula unitária
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Grupo espacial
label-structure-secondary = Estrutura secundária
structure-ss-file = Do arquivo
structure-ss-computed = Calculada a partir da cadeia principal

## HDF4 e HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Biblioteca
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Conjuntos de dados
label-hdf4-images = Imagens raster
label-hdf4-vgroups = Grupos
label-hdf4-vdatas = Tabelas
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Versão
# The grids, swaths and point structures the file defines.
label-eos-structures = Estruturas
eos-grids = { $count ->
    [one] { $count } grade ({ $names })
    [many] { $count } grades ({ $names })
   *[other] { $count } grades ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } faixa ({ $names })
    [many] { $count } faixas ({ $names })
   *[other] { $count } faixas ({ $names })
}
eos-points = { $count ->
    [one] { $count } estrutura de pontos
    [many] { $count } estruturas de pontos
   *[other] { $count } estruturas de pontos
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Estrutura
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projeção
label-eos-grid-size = Tamanho da grade
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Cantos
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Produto
# When the data was acquired.
label-eos-time = Data
eos-time = { $begin } a { $end }

## Imagens planetárias (VICAR, PDS3, PDS4: dados de missões da NASA e da ESA)

# machine-drafted, needs native review
group-planetary = Imagem planetária
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Alvo
label-planetary-mission = Missão
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Espaçonave
label-planetary-start = Hora de início
label-planetary-stop = Hora de término
# The product's identifier in the archive.
label-planetary-product = ID do produto
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Projeção cartográfica
label-planetary-resolution = Resolução
planetary-metres-per-pixel = { $metres } m por pixel
planetary-pixels-per-degree = { $pixels } pixels por grau
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Centro da projeção
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Cobertura
planetary-coverage = latitude { $south }° a { $north }°, longitude { $west }° a { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Raio do corpo
planetary-km = { $km } km

## OMERO (imagens abertas de um servidor OMERO ou do Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Servidor
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (como { $user })
omero-server-public = { $server } (público)
label-omero-image = Imagem
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Projeto
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Triagem
label-omero-owner = Proprietário
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Aquisição
label-omero-dimensions = Dimensões
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } plano Z
   *[other] { $count } planos Z
}
omero-c = { $count ->
    [one] { $count } canal
   *[other] { $count } canais
}
omero-t = { $count ->
    [one] { $count } ponto temporal
   *[other] { $count } pontos temporais
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Tamanho do pixel
label-omero-channels = Canais
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (oculto)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Obtido como
omero-fetched-tiles = Mosaicos { $encoding } de { $width } × { $height }, { $levels ->
    [one] { $levels } nível
   *[other] { $levels } níveis
}
omero-fetched-whole = Planos inteiros em { $encoding }

## Imagens de transformação de refletância (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Transformação de refletância
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Base
label-rti-order = Ordem
# Coefficients per colour channel.
label-rti-terms = Coeficientes
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Disposição das cores
# Coefficient images stored in the file.
label-rti-planes = Planos
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Escala
label-rti-bias = Deslocamento

## Nuvens de pontos

# machine-drafted, needs native review
group-points = Nuvem de pontos
label-points = Pontos
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } de { $total }
# The LAS point data record format number (0–10).
label-point-format = Formato de ponto
# The coordinate reference system.
label-crs = Sistema de coordenadas
# What each point carries besides its position.
label-attributes = Atributos
points-channel-rgb = RGB
points-channel-intensity = Intensidade
points-channel-classification = Classificação
points-channel-returns = Retornos
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Classes
# E57 scans, with point counts.
label-scans = Varreduras
point-class-0 = Nunca classificado
point-class-1 = Não classificado
point-class-2 = Solo
point-class-3 = Vegetação baixa
point-class-4 = Vegetação média
point-class-5 = Vegetação alta
point-class-6 = Edificação
point-class-7 = Ponto baixo (ruído)
point-class-8 = Ponto-chave do modelo
point-class-9 = Água
point-class-10 = Ferrovia
point-class-11 = Superfície de estrada
point-class-12 = Sobreposição
point-class-13 = Cabo para-raios
point-class-14 = Condutor
point-class-15 = Torre de transmissão
point-class-16 = Conector de cabo
point-class-17 = Tabuleiro de ponte
point-class-18 = Ruído alto
point-class-19 = Estrutura suspensa
point-class-20 = Solo ignorado
point-class-21 = Neve
point-class-22 = Exclusão temporal
# A class without a standard name, by number.
point-class-other = Classe { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Splats gaussianos
label-splats = Splats
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } de { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Cor dependente da vista
splats-harmonics-none = Nenhum
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, treinado com antisserrilhamento

## Avisos

# machine-drafted, needs native review
warn-not-diagnostic = Não destinado a diagnóstico. O Specular é um visualizador de imagens geral, não um dispositivo médico: a exibição não é calibrada para diagnóstico.

warn-gps = Este arquivo registra onde foi feito. Remova a localização antes de compartilhar se ela deve ficar privada.
warn-null-island = A posição GPS é exatamente 0°, 0°; a câmera provavelmente não tinha sinal, então a localização deve estar errada.
warn-date-future = A data de captura está no futuro; o relógio da câmera provavelmente estava errado.
warn-icc-damaged = O perfil de cor incorporado está danificado e não pôde ser lido.
warn-metadata-failed = Não foi possível ler os metadados por causa de um erro no Specular. A imagem em si está boa.
warn-orientation-conflict = EXIF e XMP discordam sobre a orientação (EXIF: { $exif }, XMP: { $xmp }). O valor EXIF é usado.
warn-exif-damaged = Alguns dados EXIF não puderam ser lidos ({ $error }); o restante é mostrado.
warn-xmp-damaged = Os metadados XMP estão malformados ({ $error }); só uma parte pôde ser lida.

## Janela Sobre

geonames-attribution = Nomes de lugares: GeoNames (www.geonames.org), licenciados sob Creative Commons Atribuição 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Consulta offline pela crate reverse_geocoder, de Grant Miner (MIT ou Apache-2.0). Os dados foram convertidos em uma lista de cidades; são fornecidos no estado em que se encontram, sem garantia.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Imagem em resolução total
    [1] Imagem em resolução reduzida
    [2] Uma página de uma imagem com várias páginas
    [4] Máscara de transparência
   *[other] Outro
}
predictor = { $value ->
    [1] Nenhum
    [2] Diferença horizontal
    [3] Diferença de ponto flutuante
   *[other] Outro
}
extra-samples = { $value ->
    [0] Não especificado
    [1] Alfa pré-multiplicado
    [2] Alfa não pré-multiplicado
   *[other] Outro
}
sample-format = { $value ->
    [1] Inteiro sem sinal
    [2] Inteiro com sinal
    [3] Ponto flutuante
    [4] Indefinido
   *[other] Outro
}

## Dados geoespaciais (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Geoespacial
label-geo-crs = Sistema de coordenadas
label-geo-datum = Datum
label-geo-projection = Projeção
label-geo-unit = Unidades
label-geo-bounds = Extensão
label-geo-lonlat-bounds = Longitude e latitude
label-geo-pixel-size = Tamanho do pixel
label-geo-overviews = Visões gerais
label-geo-nodata = Valor sem dados
label-geo-raster-type = Tipo de raster
label-geo-vertical = Referência vertical
label-geo-citation = Citação
label-geo-features = Feições
label-geo-vertices = Vértices
label-geo-layer = Camada
label-geo-layers = Camadas
label-geo-tiles = Blocos
label-geo-encoding = Codificação de texto
label-geo-warnings = Problemas
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Arquivos complementares
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (coordenadas não compatíveis)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Para WGS 84
geo-to-wgs84-same = Não necessária
geo-to-wgs84-helmert = Mudança de datum (Helmert)
geo-to-wgs84-by-area = Mudança de datum (Helmert, escolhida pela área)
geo-to-wgs84-unknown = Desconhecida: as coordenadas estão no datum do próprio arquivo
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Não informado
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } a { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° a { $east }° de longitude, { $south }° a { $north }° de latitude
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } visão geral: { $sizes }
   *[other] { $n } visões gerais: { $sizes }
}
geo-pixel-is-point = Os pixels são pontos
geo-layer = { $features ->
    [one] { $features } feição
   *[other] { $features } feições
}, { $fields ->
    [one] { $fields } campo
   *[other] { $fields } campos
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoom { $zoom }, { $tiles ->
    [one] { $tiles } bloco
   *[other] { $tiles } blocos
}
fields-geo-geokeys = Chaves GeoTIFF
fields-geo-gdal = Metadados GDAL
fields-geo-wkt = Sistema de coordenadas (WKT)
fields-geo-projjson = Sistema de coordenadas (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Conjunto de blocos
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Créditos
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Cabeçalho
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = Versão do GIFTI
label-gifti-role = Contém
gifti-role-surface = Superfície
gifti-role-labels = Rótulos por vértice
gifti-role-data = Valores por vértice
label-gifti-arrays = Arrays de dados
label-gifti-encoding = Codificação
label-gifti-vertices = Vértices
label-gifti-triangles = Triângulos
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Estrutura
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Tipo geométrico
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Coordenadas
gifti-transform = { $from } (transformação para { $to } desnecessária)
gifti-transform-applied = { $from } → { $to } (aplicada)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Exibido sobre
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Array
label-gifti-intent = Intent
label-gifti-data-type = Tipo de dados
# Entries of the label table (names and colours of regions).
label-gifti-labels = Tabela de rótulos
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = Versão do CIFTI
label-cifti-intent = Intent
label-cifti-matrix = Matriz
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Mapas
cifti-maps-scalars = { $count ->
        [one] { $count } mapa escalar
       *[other] { $count } mapas escalares
    }
cifti-maps-labels = { $count ->
        [one] { $count } mapa de rótulos
       *[other] { $count } mapas de rótulos
    }
cifti-maps-series = { $count ->
        [one] { $count } instante
       *[other] { $count } instantes
    }
cifti-maps-parcels = { $count ->
        [one] { $count } parcela
       *[other] { $count } parcelas
    }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Conectividade
cifti-dense = { $seeds ->
        [one] { $seeds } semente
       *[other] { $seeds } sementes
    }, { $size } lidos por semente
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Série
cifti-series = a partir de { $start }, a cada { $step } ({ $unit })
label-cifti-models = Modelos cerebrais
label-cifti-parcels = Parcelas
label-cifti-structures = Estruturas
label-cifti-voxels = Voxels subcorticais
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Superfície
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } vértice
       *[other] { $vertices } vértices
    }, { $file }
cifti-surface-missing = { $structure }: precisa de uma superfície com { $vertices ->
        [one] { $vertices } vértice
       *[other] { $vertices } vértices
    } (nenhuma ao lado do arquivo)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Exibido como
cifti-shown-surfaces = Superfícies coloridas
cifti-shown-voxels = Volume dos voxels subcorticais
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Conectividade da semente escolhida
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Mapas omitidos
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Formato
label-tract-streamlines = Linhas de corrente
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } exibidas)
label-tract-points = Pontos
label-tract-mean-length = Comprimento médio
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Escalares por ponto
label-tract-properties = Propriedades por linha de corrente
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Grade
label-tract-voxel-size = Tamanho do voxel
label-tract-voxel-order = Ordem dos voxels
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Coordenadas
tract-space-vox-to-ras = A partir da matriz voxel-para-RAS
tract-space-voxel-order = A partir da ordem dos voxels (sem matriz voxel-para-RAS)
tract-space-scanner = Espaço do scanner
# MRtrix: the tracking step size.
label-tract-step = Tamanho do passo

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Cabeçalho do arquivo
label-header-created = Criado
# The program or person that wrote the file.
label-header-creator = Autor
label-header-copyright = Direitos autorais
label-header-project = Projeto
label-header-comment = Comentário
# The scanner, camera or other device the image came from.
label-header-device = Dispositivo
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Timecode
# A film frame's edge code (KeyKode).
label-header-edge-code = Código de borda
label-header-frame-rate = Taxa de quadros
# Where the frame sits in its sequence.
label-header-frame = Quadro
# Film stock and slate.
label-header-film = Filme
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Função de transferência
# The colour primaries the file states.
label-header-colorimetric = Colorimetria
# The X window a screen dump was taken of.
label-header-window = Janela
label-header-resolution = Resolução
label-header-compression = Compressão
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Perfil
# machine-drafted, needs native review
label-header-level = Nível
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Níveis de wavelet
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Transformação de cor
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Pranchetas
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Não desenhado
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Desenhado de forma aproximada

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Fonte
label-font-family = Família
label-font-style = Estilo
# The face's place in a font collection: "2 of 4".
label-font-face = Face
font-face-of = { $index } de { $count }
label-font-version = Versão
label-font-designer = Designer
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Fundidora
label-font-glyphs = Glifos
label-font-outlines = Contornos
font-outlines-truetype = TrueType (quadráticos)
font-outlines-cff = PostScript (CFF, cúbicos)
font-outlines-cff2 = PostScript (CFF2, cúbicos)
font-outlines-bitmap = Só bitmaps
font-outlines-none = Nenhum
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Incorporação
font-embedding-installable = Pode ser incorporada e instalada.
font-embedding-restricted = Não pode ser incorporada.
font-embedding-preview-print = Pode ser incorporada só para ver e imprimir.
font-embedding-editable = Pode ser incorporada em documentos editáveis.
font-embedding-no-subsetting = Só inteira, não como subconjunto.
font-embedding-bitmap-only = Só como bitmaps.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Eixos de variação
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Estilos nomeados
label-font-colour = Glifos coloridos
font-colour-colr0 = Camadas COLR
font-colour-colr1 = Gradientes COLR v1
font-colour-sbix = Bitmaps (sbix)
font-colour-cbdt = Bitmaps (CBDT)
font-colour-svg = SVG
label-font-weight = Peso
label-font-units-per-em = Unidades por em
label-font-tables = Tabelas
label-font-licence = Licença
label-font-licence-url = URL da licença
label-font-copyright = Direitos autorais
label-font-trademark = Marca registrada

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Documento
label-document-kind = Tipo
document-kind-word = Documento do Word
document-kind-excel = Pasta de trabalho do Excel
document-kind-powerpoint = Apresentação do PowerPoint
document-kind-pages = Documento do Pages
document-kind-numbers = Planilha do Numbers
document-kind-keynote = Apresentação do Keynote
document-kind-odf-text = Texto OpenDocument
document-kind-odf-spreadsheet = Planilha OpenDocument
document-kind-odf-presentation = Apresentação OpenDocument
document-kind-odf-graphics = Desenho OpenDocument
document-kind-odf-other = Arquivo OpenDocument
# machine-drafted, needs native review
document-kind-playlist = Playlist
# machine-drafted, needs native review
label-playlist-items = Itens
# Appended to the kind: "Word document, template, with macros".
document-template = modelo
document-macros = com macros
document-legacy = formato Office 97–2003
label-document-preview = Visualização
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = Imagem { $format }
document-preview-none = Nenhuma neste arquivo
label-document-subject = Assunto
label-document-author = Autor
label-document-description = Descrição
label-document-category = Categoria
label-document-last-saved-by = Salvo pela última vez por
label-document-created = Criado
label-document-modified = Modificado
label-document-printed = Última impressão
label-document-pages = Páginas
label-document-slides = Slides
label-document-hidden-slides = Slides ocultos
label-document-notes = Páginas de anotações
label-document-sheets = Planilhas
label-document-words = Palavras
label-document-characters = Caracteres
label-document-paragraphs = Parágrafos
label-document-lines = Linhas
label-document-editing-time = Tempo de edição
document-minutes = { $minutes } min
label-document-revision = Revisão
label-document-application = Salvo com
label-document-company = Empresa
label-document-template = Modelo

## Books (EPUB)

# machine-drafted, needs native review
group-book = Livro
label-book-authors = Autores
label-book-publisher = Editora
label-book-date = Publicado
label-book-language = Idioma
label-book-pages = Páginas
# Entries in the book's table of contents.
label-book-contents = Entradas do sumário
label-book-layout = Layout
book-layout-reflowable = Refluível
book-layout-fixed = Layout fixo
book-right-to-left = da direita para a esquerda
label-book-version = Formato
book-version = EPUB { $version }
label-book-fonts = Fontes incorporadas
book-fonts-obfuscated = { $count } ({ $obfuscated } ofuscadas)
label-book-subjects = Assuntos
label-book-identifier = Identificador
label-book-rights = Direitos

unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

# machine-drafted, needs native review
volume-orient-parrec = Angulação e descentralização Philips
volume-orient-ecat = Tamanhos de voxel e deslocamentos ECAT (não confiáveis)
label-volume-technique = Técnica
label-volume-version = Versão do formato
label-volume-isotope = Isótopo
label-volume-tracer = Traçador
label-volume-units = Unidades

# Text, tables and subtitles.
label-header-encoding = Conjunto de caracteres
label-header-lines = Linhas
label-header-line-ends = Fins de linha
label-header-words = Palavras
label-header-pages = Páginas
label-header-pictures = Imagens
label-header-pictures-not-shown = Imagens não mostradas
label-header-tables = Tabelas
label-header-rows = Linhas da tabela
label-header-columns = Colunas
label-header-delimiter = Delimitador
label-header-column-names = Nomes das colunas
label-header-cues = Legendas
label-header-cue-span = Legendas de – a
