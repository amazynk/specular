# machine-drafted, needs native review
# Specular — interface do visualizador, paleta de comandos, erros, Acerca de. Português europeu.

-app-name = Specular

## Dicas da barra de ferramentas e etiquetas de acessibilidade

toolbar-open = Abrir imagem…
toolbar-prev = Imagem anterior
toolbar-next = Imagem seguinte
toolbar-back-gallery = Voltar à galeria
toolbar-zoom-out = Reduzir
toolbar-zoom-in = Ampliar
toolbar-zoom-level = Nível de zoom e predefinições
toolbar-zoom-fit = Ajustar à janela
toolbar-zoom-100 = Tamanho real ({ PERCENT(100) })
toolbar-rotate-left = Rodar para a esquerda
toolbar-rotate-right = Rodar para a direita
toolbar-fullscreen-enter = Entrar em ecrã inteiro
toolbar-fullscreen-exit = Sair do ecrã inteiro
toolbar-info = Informações da imagem
toolbar-settings = Definições
toolbar-delete = Mover para o Lixo
toolbar-copy = Copiar imagem
toolbar-palette = Paleta de comandos
toolbar-go-to = Ir para a imagem…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Personalizar barra de ferramentas…
toolbar-more = Mais
toolbar-announce-moved = { $item }: posição { $index } de { $count }
toolbar-announce-added = { $item } adicionado: posição { $index } de { $count }
toolbar-announce-removed = { $item } removido da barra de ferramentas
toolbar-announce-reset = Barra de ferramentas reposta
# Uma etiqueta de dica seguida do seu atalho de teclado.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Dica de um controlo que não pode ser usado agora; $reason é uma das mensagens disabled-*.
tooltip-unavailable = { $label } (indisponível: { $reason })
# Porque é que um controlo ou comando está indisponível (dicas, paleta de comandos, leitores de ecrã).
disabled-no-image = Nenhuma imagem aberta
disabled-still-loading = A imagem ainda está a carregar
disabled-decode-failed = Não foi possível descodificar esta imagem
disabled-no-other-images = Não há outras imagens nesta pasta

## Indicadores

# Posição na pasta, p. ex. "12 / 348".
readout-position = { $index } / { $count }
# Lido pelos leitores de ecrã para o indicador de posição.
readout-position-a11y = Imagem { $index } de { $count }
# $zoom é uma percentagem já formatada, p. ex. "150%".
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Ajustar ({ $zoom })
zoom-preset-fit = Ajustar à janela
zoom-preset-fill = Preencher a janela
zoom-preset-actual = Tamanho real
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Ecrã inteiro
# $zoom é uma percentagem já formatada.
zoom-preset-percent = { $zoom }
# Dimensões da imagem em píxeis, p. ex. "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Número de imagens na pasta atual.
status-image-count =
    { $count ->
        [one] { $count } imagem
       *[other] { $count } imagens
    }

## Paleta de comandos

palette-placeholder = Escreva um comando ou definição…
palette-no-results = Nenhum comando correspondente
# Alterar uma definição a partir da paleta, p. ex. "Definir: Roda do rato → Imagem seguinte".
palette-set-setting = Definir: { $setting } → { $value }
palette-next = Imagem seguinte
palette-prev = Imagem anterior
palette-first = Primeira imagem
palette-last = Última imagem
palette-go-to = Ir para a imagem…
palette-zoom-in = Ampliar
palette-zoom-out = Reduzir
palette-zoom-fit = Ajustar à janela
palette-zoom-fill = Preencher a janela
# machine-drafted, needs native review
palette-zoom-fit-width = Ajustar à largura
palette-zoom-fit-height = Ajustar à altura
palette-zoom-100 = Zoom a { PERCENT(100) }
palette-zoom-200 = Zoom a { PERCENT(200) }
palette-zoom-300 = Zoom a { PERCENT(300) }
palette-zoom-400 = Zoom a { PERCENT(400) }
palette-zoom-rectangle = Zoom na seleção
palette-rotate-left = Rodar para a esquerda
palette-rotate-right = Rodar para a direita
palette-flip-horizontal = Inverter na horizontal
palette-flip-vertical = Inverter na vertical
palette-toggle-fullscreen = Alternar ecrã inteiro
palette-navigator = Mostrar ou ocultar navegador
palette-loupe = Lupa
palette-pixel-grid = Mostrar ou ocultar grelha de píxeis
palette-info-panel = Mostrar ou ocultar painel de informações
palette-copy-image = Copiar imagem
# machine-drafted, needs native review
palette-read-codes = Ler códigos
# machine-drafted, needs native review
palette-find-text = Encontrar texto
# machine-drafted, needs native review
palette-face-boxes = Mostrar rostos
palette-copy-path = Copiar caminho do ficheiro
palette-move-to-trash = Mover para o Lixo
palette-undo-trash = Anular mover para o Lixo
palette-open = Abrir imagem…
# machine-drafted, needs native review
palette-open-address = Abrir endereço…
palette-settings = Definições
# machine-drafted, needs native review
palette-customise-toolbar = Personalizar barra de ferramentas…
palette-cheat-sheet = Atalhos de teclado
palette-hud = Mostrar ou ocultar HUD de tempo de fotograma
palette-language = Idioma
palette-theme = Tema
# machine-drafted, needs native review
palette-interface-larger = Interface maior
palette-interface-smaller = Interface menor
palette-interface-reset = Interface a { PERCENT(100) }
palette-quit = Sair do { -app-name }

## Atalhos de teclado

cheatsheet-title = Atalhos de teclado
cheatsheet-navigation = Navegação
cheatsheet-zoom-pan = Zoom e deslocação
cheatsheet-view = Ver
cheatsheet-file = Ficheiro
cheatsheet-window = Janela
cheatsheet-help = Ajuda
cheatsheet-hold = Manter { $key }
cheatsheet-close-hint = Prima { $key } ou Esc para fechar

## Erros

# machine-drafted, needs native review
viewer-no-preview = Nenhuma pré-visualização neste ficheiro.
error-cannot-open = Não é possível abrir { $file }
error-cannot-open-reason = Não é possível abrir { $file }: { $reason }
error-unsupported-format = { $file } está num formato que o { -app-name } ainda não consegue ler.
error-file-truncated = { $file } está incompleto. É mostrada a parte que foi possível ler.
error-file-too-large = { $file } é demasiado grande para abrir ({ $megapixels } megapíxeis; o limite é { $limit } megapíxeis).
error-not-found = { $file } já não existe.
error-decoder-stopped = o descodificador parou sem produzir uma imagem
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = é necessária uma superfície cerebral (um ficheiro .surf.gii com { $vertices } vértices) na mesma pasta
error-permission-denied = Não tem permissão para ler { $file }.
error-details = Detalhes
crash-last-time = O { -app-name } fechou inesperadamente da última vez; foi guardado um relatório em { $path }
error-trash-failed = Não foi possível mover { $file } para o Lixo: { $reason }
# machine-drafted, needs native review
# Motivos, mostrados após "Não é possível abrir FICHEIRO:" e outras mensagens de erro, por isso
# começam com minúscula. Dizem o que aconteceu e, quando ajuda, o que fazer.
error-reason-not-found = já não está lá. Pode ter sido movido, mudado de nome ou eliminado.
error-reason-permission = não tem permissão para o ler. Verifique o proprietário e as permissões.
error-reason-disk-full = o disco está cheio. Liberte espaço e tente novamente.
error-reason-read-only = o disco é só de leitura.
error-reason-truncated = o ficheiro está incompleto. Pode ainda estar a ser copiado ou transferido.
error-reason-is-folder = é uma pasta, não um ficheiro.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = o servidor recusou o acesso. Os dados podem ser privados; para um bucket S3, configure primeiro as credenciais da AWS.
error-reason-remote-missing = não há dados Zarr nesse endereço.
# $status is an HTTP status code ("500").
error-reason-remote-status = o servidor respondeu com o erro { $status }.
error-reason-offline = não é possível contactar o servidor. Verifique o endereço e a ligação.
# machine-drafted, needs native review
error-reason-network-off = a rede está desligada em Definições › Privacidade.
error-reason-timed-out = a unidade ou o servidor não respondeu a tempo. Verifique a ligação e tente novamente.
error-reason-memory = não há memória suficiente. Feche outras imagens ou aplicações e tente novamente.
error-reason-unsupported = está num formato que o { -app-name } ainda não consegue ler.
# $detail são as palavras do próprio descodificador, técnicas e em inglês ("12-bit lossless JPEG").
error-reason-unsupported-detail = usa uma funcionalidade que o { -app-name } ainda não consegue ler ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = não é nenhum dos ficheiros .{ $ext } que o { -app-name } lê ({ $formats }).
error-reason-damaged = o ficheiro está danificado ou não é o que o nome indica ({ $detail }).
error-reason-too-large = é demasiado grande para abrir.
# machine-drafted, needs native review
error-reason-needs-memory = precisa de { $needed } de RAM livre, mas só { $free } estão livres neste momento. Feche algumas outras aplicações e tente novamente, ou abra-o num computador com mais memória.
error-needs-memory = Esta imagem precisa de { $needed } de RAM livre, mas só { $free } estão livres neste momento. Feche algumas outras aplicações e tente novamente, ou abra-a num computador com mais memória.
error-needs-memory-short = Precisa de { $needed } de RAM livre
error-reason-cancelled = a abertura foi interrompida.
error-reason-trash-access = já não está lá, ou não tem permissão para o alterar.
error-reason-restore-collision = já existe um ficheiro com o mesmo nome na pasta.
error-reason-trash-root = não é possível mover uma unidade inteira para o Lixo.
error-reason-not-in-trash = já não está no Lixo.
error-reason-program-missing = o programa não está instalado ou foi movido. Escolha-o novamente ou reinstale-o.
error-reason-program-not-allowed = o programa não tem permissão para ser executado. Verifique se é executável.
# machine-drafted, needs native review
error-reason-permission-write = não tem permissão para o alterar. Verifique o proprietário e as permissões.
error-reason-file-has-errors = tem erros. Corrija-os primeiro, para que nada se perca.
error-reason-not-dicom = não é um ficheiro DICOM.
error-reason-no-image = não contém nenhuma imagem.
error-reason-needs-layout = é um volume sem cabeçalho. Para o abrir, são necessários o tamanho, o tipo de amostra e a ordem dos bytes.
error-reason-not-archive = não é um arquivo que o { -app-name } consiga ler.
error-reason-needs-sandbox = este tipo de arquivo só é aberto pelo leitor protegido do { -app-name }, que não está disponível.
error-reason-encrypted = está encriptado e precisa da palavra-passe.
error-reason-wrong-password = a palavra-passe está errada.
error-reason-checksum = o ficheiro está danificado: o conteúdo não corresponde à soma de verificação.
error-reason-missing-entry = falta um ficheiro no seu interior.
error-reason-busy = outro programa está a utilizá-lo. Feche-o aí e tente novamente.
error-reason-catalogue-damaged = o ficheiro do catálogo está danificado.
error-reason-cannot-open = não é possível abrir o ficheiro. Verifique se a pasta existe e se pode escrever nela.
error-reason-newer-version = foi escrito por uma versão mais recente do { -app-name }. Atualize o { -app-name } para o alterar.
error-reason-internal = algo correu mal no { -app-name }. O registo tem os detalhes.
error-reason-volume-too-few = { $count ->
    [one] um corte é insuficiente para construir um volume.
    [many] { $count } cortes são insuficientes para construir um volume.
   *[other] { $count } cortes são insuficientes para construir um volume.
}
error-reason-volume-no-geometry = um corte não indica onde se encontra nem para que lado está virado.
error-reason-volume-not-parallel = os cortes não são paralelos.
error-reason-volume-shared-position = várias imagens partilham { $count ->
        [one] { $count } posição
       *[other] cada uma das { $count } posições
    } (fases, ecos ou uma série temporal).
error-reason-volume-missing = { $count ->
        [one] falta cerca de { $count } corte.
       *[other] faltam cerca de { $count } cortes.
    }
error-reason-volume-mismatch = os cortes diferem no tamanho ou no espaçamento dos píxeis.
error-reason-volume-not-grey = os cortes são a cores, não em tons de cinzento.
error-reason-volume-skewed = os eixos do volume não estão em ângulo reto.
error-reason-volume-too-large = precisa de { $need } MiB e só são permitidos { $budget } MiB.
error-reason-url-not-specular = não é uma ligação do { -app-name }.
error-reason-url-too-long = é demasiado longa.
error-reason-url-damaged = está danificada. Pode ter sido cortada ou alterada ao ser copiada.
error-reason-url-unknown = pede algo que esta versão do { -app-name } não conhece. Atualizar o { -app-name } pode ajudar.
error-reason-url-not-absolute = os caminhos dos ficheiros têm de ser completos.
error-reason-url-no-paths = não indica nenhum ficheiro.
error-reason-url-too-many = indica demasiados ficheiros.

## Notificações

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Não foi possível concluir: { $reason }
toast-moved-to-trash = Movido para o Lixo
toast-moved-to-trash-named = { $file } movido para o Lixo
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } imagem movida para o Lixo
       *[other] { $count } imagens movidas para o Lixo
    }
toast-undo = Anular
toast-restored = { $file } reposto
toast-copied-image = Imagem copiada
toast-copied-path = Caminho copiado
toast-copied-report = Relatório copiado
toast-copied-value = Valor copiado
toast-loop-start = De volta à primeira imagem
toast-loop-end = Saltou para a última imagem
# machine-drafted, needs native review
toast-interface-size = Tamanho da interface { PERCENT($percent) }
toast-language-changed = Idioma alterado para { $language }

## Estados vazios

empty-drop = Largue aqui uma imagem ou prima { $shortcut }
empty-open-button = Abrir imagem…
empty-drop-release = Solte para abrir
empty-folder = Esta pasta não tem imagens que o { -app-name } consiga abrir.
# machine-drafted, needs native review
# Sob "Abrir imagem…" na janela vazia: onde encontrar tudo o resto.
empty-hint-keys = Prima { $cheatsheet } para ver os atalhos de teclado, ou { $palette } para encontrar qualquer comando.
empty-hint-cheatsheet = Prima { $cheatsheet } para ver os atalhos de teclado.
empty-hint-palette = Prima { $palette } para encontrar qualquer comando.
# Sob o indicador de progresso quando um ficheiro demora mais do que um instante a abrir.
viewer-opening = A abrir { $file }…
fullscreen-exit-hint = Prima Esc para sair do ecrã inteiro
# Mostrado ao entrar em ecrã inteiro; com zoom aplicado, Esc primeiro repõe o zoom.
fullscreen-back-hint = Prima Esc para repor o zoom ou sair do ecrã inteiro
# Atalhos e paleta de comandos: o que faz Esc, um passo de cada vez.
action-back = Fechar o painel (numa caixa de diálogo, recuar primeiro um passo), repor o zoom, sair do ecrã inteiro; na galeria, subir uma pasta

## Seletor de idioma

language-picker-title = Idioma
# machine-drafted, needs native review
language-picker-search = Pesquisar idiomas
# $percent é uma percentagem já formatada, p. ex. "87%".
language-completeness = { $percent } traduzido
language-system-default = Predefinição do sistema ({ $language })
language-pseudo-accented = Pseudoidioma: acentuado e mais longo
language-pseudo-rtl = Pseudoidioma: espelhado da direita para a esquerda

## Acerca de

about-title = Acerca do { -app-name }
about-version = Versão { $version }
about-tagline = Um visualizador de imagens rápido e fiel.
about-copyright = © { $year } Os autores do { -app-name }
about-license = Licenciado ao abrigo da MIT License / Apache License 2.0.
about-geonames = Nomes de locais do GeoNames (geonames.org), licenciados ao abrigo da Creative Commons Atribuição 4.0.
about-unicode = Dados regionais do Unicode CLDR através do ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Perfis de objetivas da base de dados Lensfun (lensfun.github.io), licenciados ao abrigo da Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Licenças de terceiros
about-third-party-file = O { -app-name } inclui software e dados de terceiros sob as suas próprias licenças; consulte o THIRD-PARTY.md, instalado com o { -app-name }.
about-copy-info = Copiar informações do sistema
about-close = Fechar

## Interface do visualizador (aplicação da fase 1)

app-close = Fechar
# Distintivo mostrado numa imagem cujo ficheiro terminou antes do tempo.
app-truncated-badge = Ficheiro incompleto
app-info-summary = Resumo
app-info-all-fields = Todos os campos
app-info-copy-value = Copiar valor
app-info-copy-field = Copiar o valor de { $field }
app-info-loading = A ler metadados…
# machine-drafted, needs native review
# Todos os campos, para um ficheiro que não guarda metadados além dos píxeis.
app-info-no-fields = Este ficheiro não tem campos de metadados.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = Mapa de ganho HDR
app-info-gain-map-headroom = Até { $headroom }× o brilho SDR
app-copy-failed = Não foi possível copiar: { $reason }
app-restore-failed = Não foi possível repor { $file }: { $reason }
app-open-dialog-title = Abrir imagem
app-open-dialog-filter = Ficheiros suportados
app-open-dialog-all = Todos os ficheiros
app-open-group-images = Imagens
app-open-group-raw = RAW de câmara
app-open-group-video = Vídeo
app-open-group-sound = Som
app-open-group-documents = Documentos
app-open-group-drawings = Desenhos e CAD
app-open-group-models = Modelos 3D
app-open-group-maps = Mapas e percursos
app-open-group-science = Ciência e medicina
app-open-group-fonts = Tipos de letra
app-open-group-other = Outros

## Informações do renderizador (Acerca, Definições › Avançadas)

# machine-drafted, needs native review
renderer-title = Informações do renderizador
renderer-section-adapter = Placa gráfica
renderer-section-limits = Limites e funcionalidades
renderer-section-display = Ecrã
renderer-section-memory = Memória
renderer-section-decoders = Descodificadores opcionais
renderer-unknown = Desconhecido
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ID do fabricante e do dispositivo
# Wayland, X11, Windows or macOS.
renderer-display-server = Servidor de visualização
renderer-features = Funcionalidades utilizadas
renderer-surface-format = Formato da janela
renderer-surface-formats = Formatos disponíveis
renderer-present-mode = Modo de apresentação
renderer-present-modes = Modos disponíveis
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Ligado (margem de { $headroom }×)
renderer-hdr-off = Desligado (o ecrã pode mostrar HDR)
renderer-hdr-unavailable = Indisponível neste ecrã
renderer-decode-budget = A descodificação pode utilizar
renderer-decode-budget-detail = { PERCENT(80) } da memória livre, partilhados por todas as imagens a ser abertas.
renderer-display-cache = Cache de visualização

## HUD de desempenho (F12)

app-hud-scale = Escala
app-hud-level = Nível
app-hud-level-thumbnail = Miniatura incorporada
app-hud-level-full = Resolução total
# $denominator é 2, 4 ou 8: a imagem descodificada a 1/2, 1/4 ou 1/8 do tamanho.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Abrir
app-hud-decode = Descodificar
app-hud-upload = Envio
app-hud-frame = Fotograma
app-hud-threads = Threads
app-hud-threads-value = { $threads } ({ $queued } em espera)
app-hud-tiles = Mosaicos
app-hud-tiles-value = { $tiles } de { $capacity }
app-hud-ram = Memória
app-hud-vram = Memória da GPU
app-hud-gpu = GPU

# Confirmação de mover para o Lixo
app-confirm-trash-title = Mover { $file } para o Lixo?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Os ficheiros auxiliares também vão para o lixo: { $files }
app-cancel = Cancelar
# Aviso mostrado antes de descodificar uma imagem muito grande. $need e $available são
# tamanhos de memória ("3,2 GiB").
toast-low-memory = Esta imagem pode precisar de cerca de { $need } de memória, mas só há { $available } disponíveis. Pode carregar devagar ou falhar.

# O relatório de hardware (Definições → Avançadas). Os valores são preenchidos pela aplicação.
hw-report-title = Relatório de hardware
hw-report-copy = Copiar relatório
hw-report-version = Versão
hw-report-cpu = CPU
hw-report-cores = Núcleos
hw-report-cores-value = { $logical } lógicos, { $physical } físicos
hw-report-performance-cores = Núcleos de desempenho
hw-report-cpu-features = Caminhos rápidos da CPU
hw-report-none = Nenhum
hw-report-memory = Memória
hw-report-memory-value = { $total } no total, { $available } disponíveis
hw-report-graphics-api = API gráfica
hw-report-driver = Controlador
hw-report-max-texture = Maior textura
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Threads de descodificação
hw-report-ram-cache = Cache de imagens em memória
hw-report-vram-cache = Cache de memória da GPU
hw-report-processing = Processamento
hw-report-path-jpeg-decode = Descodificação JPEG
hw-report-path-ycbcr-to-rgb = Conversão de cor
hw-report-path-png-unfilter = Descodificação PNG
hw-report-path-heic-decode = Descodificação HEIC
hw-report-path-inference = Análise de imagem
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, em paralelo
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = descodificador de hardware
hw-report-engine-npu = NPU
hw-report-build-debug = compilação de depuração
hw-report-build-release = compilação de lançamento

## Acessibilidade (leitores de ecrã)
# A imagem tal como um leitor de ecrã a anuncia: nome do ficheiro e tamanho em píxeis.
a11y-image = { $name }, { $width } × { $height } píxeis
# A região onde aparecem as notificações curtas.
a11y-notifications = Notificações
# machine-drafted, needs native review
a11y-transport-playback = Reprodução
a11y-transport-pages = Páginas
a11y-settings-sections = Secções

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Códigos nesta imagem
codes-looking = A procurar códigos…
codes-none = Não foram encontrados códigos nesta imagem.
codes-failed = Não foi possível ler a imagem: { $reason }
codes-copy = Copiar «{ $text }»
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Abrir ligação em { $host }…
codes-network-off = Nenhuma rede está ligada, por isso as ligações não abrem.
codes-open-title = Abrir uma ligação para { $host }?
codes-open-detail = Abre no seu navegador. Abra apenas ligações em que confia.
codes-open = Abrir no navegador
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Texto nesta imagem
# machine-drafted, needs native review
ocr-looking = A ler o texto…
# machine-drafted, needs native review
ocr-none = Não foi encontrado texto nesta imagem.
# machine-drafted, needs native review
ocr-failed = Não foi possível ler o texto: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Encontrar texto precisa dos modelos de texto do pacote de IA local (Definições › IA local).
# machine-drafted, needs native review
ocr-loading = A carregar os modelos de texto…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Copiar «{ $text }»
# machine-drafted, needs native review
ocr-copy-all = Copiar todo o texto
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Confiança: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Sistema
hw-report-section-gpu = GPU
hw-report-section-decoding = Descodificação
hw-report-operating-system = Sistema operativo
hw-report-adapter = Adaptador
hw-report-gpu-type = Tipo
hw-report-gpu-init = Tempo de arranque
hw-report-max-layers = Camadas de textura
hw-report-compression = Compressão de texturas
hw-report-unified-memory = Memória partilhada com a CPU
hw-report-display = Ecrã
hw-report-hw-decode = Descodificação por hardware
hw-report-core-cluster = Grupo de núcleos
hw-report-power = Energia
hw-report-calibration = Medições de velocidade
hw-report-codec-others = Outros
hw-report-performance-cores-value = { $count } de { $total }
hw-report-cluster-value = { $name }: { $physical } físicos, { $logical } lógicos
hw-report-thermal-nominal = Temperatura normal
hw-report-thermal-fair = Quente
hw-report-thermal-serious = Muito quente, a abrandar
hw-report-thermal-critical = Sobreaquecido
hw-report-power-battery = Com bateria ({ $percent })
hw-report-power-battery-unknown = Com bateria
hw-report-power-mains = Ligado à corrente
hw-report-power-mains-percent = Ligado à corrente ({ $percent })
hw-report-power-saving = Poupança de energia ativada
hw-report-gpu-discrete = Dedicado
hw-report-gpu-integrated = Integrado
hw-report-gpu-virtual = Virtual
hw-report-gpu-software = Software, na CPU
hw-report-gpu-other = Outro
hw-report-gpu-unavailable = Indisponível
hw-report-ms = { $ms } ms
hw-report-yes = Sim
hw-report-no = Não
hw-report-unified-platform-differs = O sistema indica o contrário
hw-report-display-value = { $fps } Hz, margem HDR { $now } agora, até { $max }
hw-report-hw-decode-value = { $api } em { $device }
hw-report-hw-decode-checking = Ainda a verificar…
hw-report-codec-up-to = Até { $width } × { $height }
hw-report-codec-none = Indisponível
hw-report-codec-h264-encode = Codificação H.264
hw-report-supported = suportado: { $list }
hw-report-unsupported = não suportado: { $list }
hw-report-calibration-done = Medido neste computador
hw-report-calibration-pending = Ainda não medido
hw-report-paths-power-saving = Poupança de energia: são preferidos os caminhos que gastam menos energia
hw-report-paths-caption = Caminhos para uma imagem de { $mpx } megapíxeis
hw-report-col-task = Tarefa
hw-report-col-path = Caminho
hw-report-col-time = Tempo
hw-report-col-cpu = Tempo de CPU (todas as threads)
hw-report-col-runs = Medições
hw-report-col-use = Usado para
hw-report-runs-untested = Não testado
hw-report-use-viewing = Visualização ({ $reason })
hw-report-use-background = Segundo plano ({ $reason })
hw-report-reason-only = a única opção
hw-report-reason-faster = mais rápido
hw-report-reason-cheaper = menos tempo de CPU
hw-report-reason-lower-power = menos energia
hw-report-reason-explore = em teste
hw-report-reason-forced = definido nas Definições
hw-report-reason-prefer-gpu = Preferir a GPU
hw-report-reason-cpu-only = Apenas a CPU
hw-report-reason-calibrate = a ser medido
hw-report-reason-scaled = tamanho reduzido
hw-report-trust-unverified = Ainda não verificado
hw-report-trust-blocked = Desativado após um erro
hw-report-trust-absent = Não existe neste computador

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galeria
cheatsheet-mode-compare = Comparar
cheatsheet-gallery-browse = Navegar
cheatsheet-gallery-select = Selecionar
cheatsheet-gallery-mark = Classificar e marcar
cheatsheet-gallery-move = Mover entre imagens
cheatsheet-gallery-page = Página acima ou abaixo
cheatsheet-gallery-ends = Primeira ou última imagem
cheatsheet-gallery-up = Subir para a pasta que a contém
cheatsheet-gallery-history-back = Voltar à pasta anterior
cheatsheet-gallery-history-forward = Avançar para a pasta seguinte
cheatsheet-gallery-back = Fechar a vista de cima e depois voltar
cheatsheet-gallery-viewer = Voltar ao visualizador
cheatsheet-gallery-album-add = Adicionar ao último álbum
cheatsheet-gallery-select-one = Selecionar ou desmarcar
cheatsheet-gallery-quick-look = Pré-visualização rápida
cheatsheet-gallery-filter = Filtrar
cheatsheet-gallery-bigger = Miniaturas maiores
cheatsheet-gallery-smaller = Miniaturas mais pequenas
cheatsheet-gallery-default-size = Tamanho padrão das miniaturas
cheatsheet-gallery-refresh = Atualizar a pasta
cheatsheet-gallery-menu = Menu de contexto
cheatsheet-compare-views = Comparar
cheatsheet-compare-overlay = Sobreposição ou lado a lado
cheatsheet-compare-difference = Diferença (de novo: tipo seguinte)
cheatsheet-compare-step-down = Menos mistura ou ganho da diferença
cheatsheet-compare-step-up = Mais mistura ou ganho da diferença
cheatsheet-compare-pick = Imagem sobre a qual as teclas atuam
cheatsheet-compare-close = Fechar a comparação
cheatsheet-channels = Canais
cheatsheet-medical-window = Janela e nível
cheatsheet-medical-measure = Medir e anotar
cheatsheet-medical-frames = Fotogramas e séries
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D e reformatação
cheatsheet-medical-analysis = Segmentação, fusão e PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Estudos e partilha
cheatsheet-palette-only-title = Na paleta de comandos ({ $key })
cheatsheet-palette-only-title-nokey = Na paleta de comandos
cheatsheet-palette-only-tag = Paleta
cheatsheet-palette-only-line = { $action }: sem tecla; na paleta de comandos ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: sem tecla; na paleta de comandos
# Esc na folha de atalhos (a descrição longa da paleta não cabe).
cheatsheet-back = Fechar ou recuar

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Exportação de vídeo
hw-report-engine-hw-encoder = codificador de hardware

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (por miniatura de { $size })
hw-report-task-frame = { $task } (por fotograma de { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Apenas software

about-natural-earth = Dados do mapa do Natural Earth (naturalearthdata.com), domínio público.
app-info-show-on-map = Mostrar no mapa
