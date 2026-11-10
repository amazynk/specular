# machine-drafted, needs native review
# Reprodução de animações e modo de leitura (arquivos compactados, quadrinhos). Português do Brasil.

disabled-not-animated = Esta imagem não é animada
disabled-in-archive = Páginas dentro de um arquivo compactado não podem ser movidas para a Lixeira

## Paleta de comandos e folha de atalhos

palette-play-pause = Reproduzir ou pausar animação
palette-next-frame = Próximo quadro
palette-prev-frame = Quadro anterior
palette-faster = Reproduzir mais rápido
palette-slower = Reproduzir mais devagar
palette-normal-speed = Velocidade normal de reprodução
palette-loop-forever = Repetir animação sem parar
palette-right-to-left = Páginas da direita para a esquerda
# machine-drafted, needs native review
palette-contents = Sumário
palette-spreads = Páginas duplas

## Controles de animação (barra acima da barra de ferramentas)

# Nome de acessibilidade do grupo de controles de reprodução.
transport-region = Controles de animação
transport-play = Reproduzir
transport-pause = Pausar
transport-prev-frame = Quadro anterior
transport-next-frame = Próximo quadro
transport-slower = Mais devagar
transport-faster = Mais rápido
# $index é o quadro mostrado, $count o número de quadros.
transport-frame = Quadro { $index } / { $count }
# O número de quadros só é conhecido depois que a animação é reproduzida uma vez.
transport-frame-unknown = Quadro { $index }
# $speed é um número formatado, como 1,5.
transport-speed = { $speed }×
transport-speed-name = Velocidade de reprodução { $speed }×
# machine-drafted, needs native review
motion-badge = Movimento
motion-badge-live = Live
motion-play = Reproduzir o clipe em movimento
motion-play-live = Reproduzir a Live Photo
motion-stop = Voltar à foto estática
toast-motion-failed = Não é possível reproduzir o clipe em movimento: { $reason }

## Notificações

toast-speed = Velocidade de reprodução { $speed }×
toast-loop-forever-on = Repetindo sem parar
toast-loop-forever-off = Repetindo como o arquivo indica
toast-right-to-left-on = Páginas lidas da direita para a esquerda
toast-right-to-left-off = Páginas lidas da esquerda para a direita
toast-spreads-on = Páginas duplas
toast-spreads-off = Uma página por vez

## Arquivos compactados

# Título da janela para uma página de um arquivo compactado.
title-archive-page = { $page } — { $archive }
# Indicador de posição enquanto duas páginas são mostradas.
readout-position-spread = { $first }–{ $last } / { $count }
# $format é RAR.
archive-needs-sandbox = Arquivos { $format } só podem ser abertos pelo decodificador isolado (sandbox), que esta versão ainda não inclui.
archive-empty = Este arquivo compactado não contém imagens.
archive-failed = Não foi possível abrir o arquivo compactado: { $error }
archive-encrypted = Este arquivo compactado está criptografado. Abra-o novamente para digitar a senha.
archive-wrong-password = A senha deste arquivo compactado está incorreta.
password-title = Senha necessária
password-archive = “{ $archive }” está criptografado.
password-label = Senha
password-show = Mostrar senha
password-wrong = Senha incorreta. Tente novamente.
password-open = Abrir
password-cancel = Cancelar

## Configurações

settings-viewing-animation-autoplay = Reproduzir animações automaticamente
settings-viewing-animation-autoplay-help = Inicia imagens animadas (GIF, APNG, WebP…) assim que são abertas.
settings-viewing-animation-loop = Repetição de animações
settings-viewing-animation-loop-help = Quantas vezes as animações se repetem.
settings-viewing-animation-loop-file = Como o arquivo indica
settings-viewing-animation-loop-forever = Sem parar
settings-viewing-animation-loop-once = Reproduzir uma vez
settings-viewing-animation-short-delays = Atrasos de quadro muito curtos
settings-viewing-animation-short-delays-help = Quadros que deveriam durar 10 ms ou menos: desacelerar para 100 ms como os navegadores fazem, ou reproduzir como armazenados.
settings-viewing-animation-short-delays-browser = Como navegadores (100 ms)
settings-viewing-animation-short-delays-as-stored = Como armazenado
settings-browsing-reading-order = Ordem das páginas em arquivos compactados
settings-browsing-reading-order-help = Arquivos de quadrinhos são lidos da direita para a esquerda quando seu ComicInfo.xml indica (mangá), a menos que definido aqui.
settings-browsing-reading-order-auto = Do arquivo compactado
settings-browsing-reading-order-left-to-right = Da esquerda para a direita
settings-browsing-reading-order-right-to-left = Da direita para a esquerda
settings-browsing-page-layout = Páginas em arquivos compactados
settings-browsing-page-layout-help = Mostra uma página ou duas lado a lado; a capa é sempre mostrada sozinha.
settings-browsing-page-layout-auto = Do arquivo compactado
settings-browsing-page-layout-single = Uma página
settings-browsing-page-layout-spreads = Páginas duplas

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Próxima página
palette-prev-page = Página anterior
palette-first-page = Primeira página
palette-last-page = Última página
disabled-one-page = A imagem tem só uma página
page-indicator = Página { $index } de { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Camada { $index }
page-label-face = Face { $face }
page-label-sphere = Mapa esférico
page-label-slice = Corte { $index }
page-label-join = { $a } · { $b }
page-label-slide = Lâmina
page-label-slide-label = Etiqueta
page-label-slide-macro = Visão geral
page-label-slide-thumbnail = Miniatura
# machine-drafted, needs native review
page-label-thermal = Imagem térmica
page-label-visible = Foto visível
# machine-drafted, needs native review
page-label-left-view = Vista esquerda
# machine-drafted, needs native review
page-label-right-view = Vista direita
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } de { $count }
page-label-t = T { $index } de { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Bandas padrão
page-label-product-bands = Todas as bandas
page-label-product-true-colour = Cor verdadeira
page-label-product-false-colour = Falsa cor (infravermelho próximo)
page-label-product-swir = Infravermelho de ondas curtas
page-label-product-agriculture = Agricultura
page-label-product-scene-class = Classificação de cena
page-label-product-cloud = Probabilidade de nuvens
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Página
info-page-position = Posição
info-page-contents = Conteúdo

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = O OCIO não é usado: { $reason }
ocio-builtin-config = $OCIO indica uma configuração integrada (ocio://); salve-a como arquivo e escolha esse arquivo.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } bits

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Imagem mesclada
page-label-from-layers = Composta a partir das camadas
page-label-layer-hidden = { $name } (oculta)
page-label-layer-unnamed = Camada sem nome { $index }
