# machine-drafted, needs native review
# Reprodução de animações e modo de leitura (arquivos, banda desenhada). Português europeu.

disabled-not-animated = Esta imagem não é animada
disabled-in-archive = As páginas dentro de um arquivo não podem ser movidas para o Lixo

## Paleta de comandos e atalhos de teclado

palette-play-pause = Reproduzir ou pausar animação
palette-next-frame = Fotograma seguinte
palette-prev-frame = Fotograma anterior
palette-faster = Reproduzir mais depressa
palette-slower = Reproduzir mais devagar
palette-normal-speed = Velocidade de reprodução normal
palette-loop-forever = Repetir animação sem fim
palette-right-to-left = Ordem das páginas da direita para a esquerda
# machine-drafted, needs native review
palette-contents = Índice
palette-spreads = Páginas duplas

## Controlos de animação (barra por cima da barra de ferramentas)

# Nome de acessibilidade do grupo de controlos de reprodução.
transport-region = Controlos de animação
transport-play = Reproduzir
transport-pause = Pausa
transport-prev-frame = Fotograma anterior
transport-next-frame = Fotograma seguinte
transport-slower = Mais devagar
transport-faster = Mais depressa
# $index é o fotograma mostrado, $count o número de fotogramas.
transport-frame = Fotograma { $index } / { $count }
# O número de fotogramas só é conhecido depois de a animação ser reproduzida uma vez.
transport-frame-unknown = Fotograma { $index }
# $speed é um número formatado, como 1,5.
transport-speed = { $speed }×
transport-speed-name = Velocidade de reprodução { $speed }×
# machine-drafted, needs native review
motion-badge = Movimento
motion-badge-live = Live
motion-play = Reproduzir o clipe em movimento
motion-play-live = Reproduzir a Live Photo
motion-stop = Voltar à fotografia estática
toast-motion-failed = Não é possível reproduzir o clipe em movimento: { $reason }

## Notificações

toast-speed = Velocidade de reprodução { $speed }×
toast-loop-forever-on = Repetição sem fim
toast-loop-forever-off = Repetição conforme o ficheiro
toast-right-to-left-on = Páginas lidas da direita para a esquerda
toast-right-to-left-off = Páginas lidas da esquerda para a direita
toast-spreads-on = Páginas duplas
toast-spreads-off = Uma página de cada vez

## Arquivos

# Título da janela para uma página de um arquivo.
title-archive-page = { $page } — { $archive }
# Indicador de posição quando são mostradas duas páginas.
readout-position-spread = { $first }–{ $last } / { $count }
# $format é RAR.
archive-needs-sandbox = Os arquivos { $format } só podem ser abertos pelo anfitrião de descodificação isolado, que esta compilação ainda não inclui.
archive-empty = Este arquivo não contém imagens.
archive-failed = Não foi possível abrir o arquivo: { $error }
archive-encrypted = Este arquivo está encriptado. Abra-o novamente para introduzir a palavra-passe.
archive-wrong-password = A palavra-passe deste arquivo está incorreta.
password-title = Palavra-passe necessária
password-archive = «{ $archive }» está encriptado.
password-label = Palavra-passe
password-show = Mostrar palavra-passe
password-wrong = Palavra-passe incorreta. Tente novamente.
password-open = Abrir
password-cancel = Cancelar

## Definições

settings-viewing-animation-autoplay = Reproduzir animações automaticamente
settings-viewing-animation-autoplay-help = Iniciar as imagens animadas (GIF, APNG, WebP…) assim que abrem.
settings-viewing-animation-loop = Repetição de animações
settings-viewing-animation-loop-help = Quantas vezes as animações se repetem.
settings-viewing-animation-loop-file = Conforme o ficheiro
settings-viewing-animation-loop-forever = Sem fim
settings-viewing-animation-loop-once = Reproduzir uma vez
settings-viewing-animation-short-delays = Intervalos de fotograma muito curtos
settings-viewing-animation-short-delays-help = Fotogramas que deviam durar 10 ms ou menos: abrandá-los para 100 ms como fazem os navegadores, ou reproduzi-los como estão guardados.
settings-viewing-animation-short-delays-browser = Como os navegadores (100 ms)
settings-viewing-animation-short-delays-as-stored = Como guardado
settings-browsing-reading-order = Ordem das páginas em arquivos
settings-browsing-reading-order-help = Os arquivos de banda desenhada são lidos da direita para a esquerda quando o seu ComicInfo.xml o indica (manga), exceto se for definido aqui.
settings-browsing-reading-order-auto = Do arquivo
settings-browsing-reading-order-left-to-right = Da esquerda para a direita
settings-browsing-reading-order-right-to-left = Da direita para a esquerda
settings-browsing-page-layout = Páginas em arquivos
settings-browsing-page-layout-help = Mostrar uma página ou duas lado a lado; a capa é sempre mostrada sozinha.
settings-browsing-page-layout-auto = Do arquivo
settings-browsing-page-layout-single = Uma página
settings-browsing-page-layout-spreads = Páginas duplas

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Página seguinte
palette-prev-page = Página anterior
palette-first-page = Primeira página
palette-last-page = Última página
disabled-one-page = A imagem tem apenas uma página
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
page-label-slide-macro = Vista geral
page-label-slide-thumbnail = Miniatura
# machine-drafted, needs native review
page-label-thermal = Imagem térmica
page-label-visible = Fotografia visível
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
page-label-product-composite = Bandas predefinidas
page-label-product-bands = Todas as bandas
page-label-product-true-colour = Cor verdadeira
page-label-product-false-colour = Falsa cor (infravermelho próximo)
page-label-product-swir = Infravermelho de onda curta
page-label-product-agriculture = Agricultura
page-label-product-scene-class = Classificação da cena
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
ocio-builtin-config = $OCIO indica uma configuração integrada (ocio://); guarde-a como ficheiro e escolha esse ficheiro.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } bits

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Imagem unida
page-label-from-layers = Composta a partir das camadas
page-label-layer-hidden = { $name } (oculta)
page-label-layer-unnamed = Camada sem nome { $index }
