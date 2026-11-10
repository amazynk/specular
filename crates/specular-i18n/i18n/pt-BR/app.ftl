# machine-drafted, needs native review
# Specular — interface do visualizador, paleta de comandos, erros, Sobre. Português do Brasil.

-app-name = Specular

## Dicas da barra de ferramentas e rótulos de acessibilidade

toolbar-open = Abrir imagem…
toolbar-prev = Imagem anterior
toolbar-next = Próxima imagem
toolbar-back-gallery = Voltar à galeria
toolbar-zoom-out = Reduzir
toolbar-zoom-in = Ampliar
toolbar-zoom-level = Nível de zoom e predefinições
toolbar-zoom-fit = Ajustar à janela
toolbar-zoom-100 = Tamanho real ({ PERCENT(100) })
toolbar-rotate-left = Girar para a esquerda
toolbar-rotate-right = Girar para a direita
toolbar-fullscreen-enter = Tela cheia
toolbar-fullscreen-exit = Sair da tela cheia
toolbar-info = Informações da imagem
toolbar-settings = Configurações
toolbar-delete = Mover para a Lixeira
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
toolbar-announce-reset = Barra de ferramentas restaurada ao padrão
# Rótulo de dica seguido do atalho de teclado.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Dica de um controle indisponível no momento; $reason é uma das mensagens disabled-*.
tooltip-unavailable = { $label } (indisponível: { $reason })
# Por que um controle ou comando está indisponível (dicas, paleta de comandos, leitores de tela).
disabled-no-image = Nenhuma imagem aberta
disabled-still-loading = A imagem ainda está carregando
disabled-decode-failed = Não foi possível decodificar esta imagem
disabled-no-other-images = Não há outras imagens nesta pasta

## Indicadores

# Posição na pasta, por exemplo "12 / 348".
readout-position = { $index } / { $count }
# Lido pelos leitores de tela para o indicador de posição.
readout-position-a11y = Imagem { $index } de { $count }
# $zoom é uma porcentagem já formatada, por exemplo "150%".
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Ajustado ({ $zoom })
zoom-preset-fit = Ajustar à janela
zoom-preset-fill = Preencher a janela
zoom-preset-actual = Tamanho real
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Tela cheia
# $zoom é uma porcentagem já formatada.
zoom-preset-percent = { $zoom }
# Dimensões da imagem em pixels, por exemplo "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Número de imagens na pasta atual.
status-image-count =
    { $count ->
        [one] { $count } imagem
       *[other] { $count } imagens
    }

## Paleta de comandos

palette-placeholder = Digite um comando ou configuração…
palette-no-results = Nenhum comando correspondente
# Alterar uma configuração pela paleta, por exemplo "Definir: Roda do mouse → Próxima imagem".
palette-set-setting = Definir: { $setting } → { $value }
palette-next = Próxima imagem
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
palette-zoom-100 = Zoom em { PERCENT(100) }
palette-zoom-200 = Zoom em { PERCENT(200) }
palette-zoom-300 = Zoom em { PERCENT(300) }
palette-zoom-400 = Zoom em { PERCENT(400) }
palette-zoom-rectangle = Zoom na seleção
palette-rotate-left = Girar para a esquerda
palette-rotate-right = Girar para a direita
palette-flip-horizontal = Espelhar horizontalmente
palette-flip-vertical = Espelhar verticalmente
palette-toggle-fullscreen = Alternar tela cheia
palette-navigator = Mostrar ou ocultar navegador
palette-loupe = Lupa
palette-pixel-grid = Mostrar ou ocultar grade de pixels
palette-info-panel = Mostrar ou ocultar painel de informações
palette-copy-image = Copiar imagem
# machine-drafted, needs native review
palette-read-codes = Ler códigos
# machine-drafted, needs native review
palette-find-text = Encontrar texto
# machine-drafted, needs native review
palette-face-boxes = Mostrar rostos
palette-copy-path = Copiar caminho do arquivo
palette-move-to-trash = Mover para a Lixeira
palette-undo-trash = Desfazer mover para a Lixeira
palette-open = Abrir imagem…
# machine-drafted, needs native review
palette-open-address = Abrir endereço…
palette-settings = Configurações
# machine-drafted, needs native review
palette-customise-toolbar = Personalizar barra de ferramentas…
palette-cheat-sheet = Atalhos de teclado
palette-hud = Mostrar ou ocultar HUD de tempo de quadro
palette-language = Idioma
palette-theme = Tema
# machine-drafted, needs native review
palette-interface-larger = Interface maior
palette-interface-smaller = Interface menor
palette-interface-reset = Interface em { PERCENT(100) }
palette-quit = Sair do { -app-name }

## Folha de atalhos

cheatsheet-title = Atalhos de teclado
cheatsheet-navigation = Navegação
cheatsheet-zoom-pan = Zoom e movimento
cheatsheet-view = Exibir
cheatsheet-file = Arquivo
cheatsheet-window = Janela
cheatsheet-help = Ajuda
cheatsheet-hold = Manter { $key } pressionada
cheatsheet-close-hint = Pressione { $key } ou Esc para fechar

## Erros

# machine-drafted, needs native review
viewer-no-preview = Nenhuma visualização neste arquivo.
error-cannot-open = Não é possível abrir { $file }
error-cannot-open-reason = Não é possível abrir { $file }: { $reason }
error-unsupported-format = { $file } está em um formato que o { -app-name } ainda não consegue ler.
error-file-truncated = { $file } está incompleto. Mostrando a parte que pôde ser lida.
error-file-too-large = { $file } é grande demais para abrir ({ $megapixels } megapixels; o limite é { $limit } megapixels).
error-not-found = { $file } não existe mais.
error-decoder-stopped = o decodificador parou sem produzir uma imagem
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = é preciso uma superfície cerebral (um arquivo .surf.gii com { $vertices } vértices) na mesma pasta
error-permission-denied = Você não tem permissão para ler { $file }.
error-details = Detalhes
crash-last-time = O { -app-name } foi fechado inesperadamente da última vez; um relatório foi salvo em { $path }
error-trash-failed = Não foi possível mover { $file } para a Lixeira: { $reason }
# machine-drafted, needs native review
# Motivos, mostrados após "Não é possível abrir ARQUIVO:" e outras mensagens de erro, por isso
# começam com minúscula. Dizem o que aconteceu e, quando ajuda, o que fazer.
error-reason-not-found = ele não está mais lá. Pode ter sido movido, renomeado ou excluído.
error-reason-permission = você não tem permissão para lê-lo. Verifique o proprietário e as permissões.
error-reason-disk-full = o disco está cheio. Libere espaço e tente novamente.
error-reason-read-only = o disco é somente leitura.
error-reason-truncated = o arquivo está incompleto. Talvez ainda esteja sendo copiado ou baixado.
error-reason-is-folder = é uma pasta, não um arquivo.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = o servidor recusou o acesso. Os dados podem ser privados; para um bucket S3, configure primeiro as credenciais da AWS.
error-reason-remote-missing = não há dados Zarr nesse endereço.
# $status is an HTTP status code ("500").
error-reason-remote-status = o servidor respondeu com o erro { $status }.
error-reason-offline = não é possível acessar o servidor. Verifique o endereço e a conexão.
# machine-drafted, needs native review
error-reason-network-off = a rede está desligada em Configurações › Privacidade.
error-reason-timed-out = a unidade ou o servidor não respondeu a tempo. Verifique a conexão e tente novamente.
error-reason-memory = não há memória suficiente. Feche outras imagens ou aplicativos e tente novamente.
error-reason-unsupported = está em um formato que o { -app-name } ainda não consegue ler.
# $detail são as palavras do próprio decodificador, técnicas e em inglês ("12-bit lossless JPEG").
error-reason-unsupported-detail = usa um recurso que o { -app-name } ainda não consegue ler ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = não é nenhum dos arquivos .{ $ext } que o { -app-name } lê ({ $formats }).
error-reason-damaged = o arquivo está danificado ou não é o que o nome indica ({ $detail }).
error-reason-too-large = é grande demais para abrir.
# machine-drafted, needs native review
error-reason-needs-memory = precisa de { $needed } de RAM livre, mas só { $free } estão livres agora. Feche alguns outros aplicativos e tente novamente, ou abra em um computador com mais memória.
error-needs-memory = Esta imagem precisa de { $needed } de RAM livre, mas só { $free } estão livres agora. Feche alguns outros aplicativos e tente novamente, ou abra-a em um computador com mais memória.
error-needs-memory-short = Precisa de { $needed } de RAM livre
error-reason-cancelled = a abertura foi interrompida.
error-reason-trash-access = ele não está mais lá, ou você não tem permissão para alterá-lo.
error-reason-restore-collision = já existe um arquivo com o mesmo nome na pasta.
error-reason-trash-root = não é possível mover uma unidade inteira para a Lixeira.
error-reason-not-in-trash = ele não está mais na Lixeira.
error-reason-program-missing = o programa não está instalado ou foi movido. Escolha-o de novo ou reinstale-o.
error-reason-program-not-allowed = o programa não tem permissão para ser executado. Verifique se ele é executável.
# machine-drafted, needs native review
error-reason-permission-write = você não tem permissão para alterá-lo. Verifique o proprietário e as permissões.
error-reason-file-has-errors = ele tem erros. Corrija-os primeiro, para que nada nele se perca.
error-reason-not-dicom = não é um arquivo DICOM.
error-reason-no-image = não contém nenhuma imagem.
error-reason-needs-layout = é um volume sem cabeçalho. É preciso informar o tamanho, o tipo de amostra e a ordem dos bytes para abri-lo.
error-reason-not-archive = não é um arquivo compactado que o { -app-name } consiga ler.
error-reason-needs-sandbox = este tipo de arquivo compactado só é aberto pelo leitor protegido do { -app-name }, que não está disponível.
error-reason-encrypted = está criptografado e precisa da senha.
error-reason-wrong-password = a senha está errada.
error-reason-checksum = o arquivo está danificado: o conteúdo não corresponde à soma de verificação.
error-reason-missing-entry = falta um arquivo dentro dele.
error-reason-busy = outro programa o está usando. Feche-o lá e tente novamente.
error-reason-catalogue-damaged = o arquivo de catálogo está danificado.
error-reason-cannot-open = não é possível abrir o arquivo. Verifique se a pasta existe e se você pode gravar nela.
error-reason-newer-version = foi gravado por uma versão mais nova do { -app-name }. Atualize o { -app-name } para alterá-lo.
error-reason-internal = algo deu errado dentro do { -app-name }. O log tem os detalhes.
error-reason-volume-too-few = { $count ->
    [one] um corte é pouco para montar um volume.
    [many] { $count } cortes são poucos para montar um volume.
   *[other] { $count } cortes são poucos para montar um volume.
}
error-reason-volume-no-geometry = um corte não informa onde está nem para que lado está voltado.
error-reason-volume-not-parallel = os cortes não são paralelos.
error-reason-volume-shared-position = várias imagens compartilham { $count ->
        [one] { $count } posição
       *[other] cada uma das { $count } posições
    } (fases, ecos ou uma série temporal).
error-reason-volume-missing = { $count ->
        [one] falta cerca de { $count } corte.
       *[other] faltam cerca de { $count } cortes.
    }
error-reason-volume-mismatch = os cortes diferem em tamanho ou espaçamento de pixels.
error-reason-volume-not-grey = os cortes são coloridos, não em tons de cinza.
error-reason-volume-skewed = os eixos do volume não estão em ângulo reto.
error-reason-volume-too-large = ele precisa de { $need } MiB e só { $budget } MiB são permitidos.
error-reason-url-not-specular = não é um link do { -app-name }.
error-reason-url-too-long = é longo demais.
error-reason-url-damaged = está danificado. Pode ter sido cortado ou alterado ao ser copiado.
error-reason-url-unknown = pede algo que esta versão do { -app-name } não conhece. Atualizar o { -app-name } pode ajudar.
error-reason-url-not-absolute = os caminhos de arquivo precisam ser completos.
error-reason-url-no-paths = não indica nenhum arquivo.
error-reason-url-too-many = indica arquivos demais.

## Notificações

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Não foi possível concluir: { $reason }
toast-moved-to-trash = Movida para a Lixeira
toast-moved-to-trash-named = { $file } movido para a Lixeira
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } imagem movida para a Lixeira
       *[other] { $count } imagens movidas para a Lixeira
    }
toast-undo = Desfazer
toast-restored = { $file } restaurado
toast-copied-image = Imagem copiada
toast-copied-path = Caminho copiado
toast-copied-report = Relatório copiado
toast-copied-value = Valor copiado
toast-loop-start = De volta à primeira imagem
toast-loop-end = Pulou para a última imagem
# machine-drafted, needs native review
toast-interface-size = Tamanho da interface { PERCENT($percent) }
toast-language-changed = Idioma alterado para { $language }

## Estados vazios

empty-drop = Solte uma imagem aqui ou pressione { $shortcut }
empty-open-button = Abrir imagem…
empty-drop-release = Solte para abrir
empty-folder = Esta pasta não tem imagens que o { -app-name } consiga abrir.
# machine-drafted, needs native review
# Sob "Abrir imagem…" na janela vazia: onde encontrar todo o resto.
empty-hint-keys = Pressione { $cheatsheet } para ver os atalhos de teclado, ou { $palette } para encontrar qualquer comando.
empty-hint-cheatsheet = Pressione { $cheatsheet } para ver os atalhos de teclado.
empty-hint-palette = Pressione { $palette } para encontrar qualquer comando.
# Sob o indicador de carregamento quando um arquivo demora mais que um instante para abrir.
viewer-opening = Abrindo { $file }…
fullscreen-exit-hint = Pressione Esc para sair da tela cheia
# Mostrado ao entrar em tela cheia; com zoom aplicado, Esc primeiro redefine o zoom.
fullscreen-back-hint = Pressione Esc para redefinir o zoom ou sair da tela cheia
# Folha de atalhos e paleta de comandos: o que Esc faz, um passo por vez.
action-back = Fechar o painel (em uma caixa de diálogo, voltar um passo primeiro), redefinir o zoom, sair da tela cheia; na galeria, subir uma pasta

## Seletor de idioma

language-picker-title = Idioma
# machine-drafted, needs native review
language-picker-search = Buscar idiomas
# $percent é uma porcentagem já formatada, por exemplo "87%".
language-completeness = { $percent } traduzido
language-system-default = Padrão do sistema ({ $language })
language-pseudo-accented = Pseudoidioma: acentuado e mais longo
language-pseudo-rtl = Pseudoidioma: espelhado da direita para a esquerda

## Janela Sobre

about-title = Sobre o { -app-name }
about-version = Versão { $version }
about-tagline = Um visualizador de imagens rápido e fiel.
about-copyright = © { $year } Os autores do { -app-name }
about-license = Licenciado sob a MIT License / Apache License 2.0.
about-geonames = Nomes de lugares do GeoNames (geonames.org), licenciados sob Creative Commons Atribuição 4.0.
about-unicode = Dados de localidade do Unicode CLDR via ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Perfis de lente do banco de dados Lensfun (lensfun.github.io), licenciados sob Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Licenças de terceiros
about-third-party-file = O { -app-name } inclui software e dados de terceiros sob suas próprias licenças; consulte THIRD-PARTY.md, instalado com o { -app-name }.
about-copy-info = Copiar informações do sistema
about-close = Fechar

## Interface do visualizador (app da Fase 1)

app-close = Fechar
# Selo mostrado em uma imagem cujo arquivo terminou antes do esperado.
app-truncated-badge = Arquivo incompleto
app-info-summary = Resumo
app-info-all-fields = Todos os campos
app-info-copy-value = Copiar valor
app-info-copy-field = Copiar o valor de { $field }
app-info-loading = Lendo metadados…
# machine-drafted, needs native review
# Todos os campos, para um arquivo que não guarda metadados além dos pixels.
app-info-no-fields = Este arquivo não tem campos de metadados.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = Mapa de ganho HDR
app-info-gain-map-headroom = Até { $headroom }× o brilho SDR
app-copy-failed = Não foi possível copiar: { $reason }
app-restore-failed = Não foi possível restaurar { $file }: { $reason }
app-open-dialog-title = Abrir imagem
app-open-dialog-filter = Arquivos compatíveis
app-open-dialog-all = Todos os arquivos
app-open-group-images = Imagens
app-open-group-raw = RAW de câmera
app-open-group-video = Vídeo
app-open-group-sound = Som
app-open-group-documents = Documentos
app-open-group-drawings = Desenhos e CAD
app-open-group-models = Modelos 3D
app-open-group-maps = Mapas e trajetos
app-open-group-science = Ciência e medicina
app-open-group-fonts = Fontes
app-open-group-other = Outros

## Informações do renderizador (Sobre, Configurações › Avançado)

# machine-drafted, needs native review
renderer-title = Informações do renderizador
renderer-section-adapter = Adaptador gráfico
renderer-section-limits = Limites e recursos
renderer-section-display = Tela
renderer-section-memory = Memória
renderer-section-decoders = Decodificadores opcionais
renderer-unknown = Desconhecido
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ID do fabricante e do dispositivo
# Wayland, X11, Windows or macOS.
renderer-display-server = Servidor de exibição
renderer-features = Recursos usados
renderer-surface-format = Formato da janela
renderer-surface-formats = Formatos oferecidos
renderer-present-mode = Modo de apresentação
renderer-present-modes = Modos oferecidos
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Ligado (margem de { $headroom }×)
renderer-hdr-off = Desligado (a tela pode mostrar HDR)
renderer-hdr-unavailable = Indisponível nesta tela
renderer-decode-budget = A decodificação pode usar
renderer-decode-budget-detail = { PERCENT(80) } da memória livre, compartilhados por todas as imagens sendo abertas.
renderer-display-cache = Cache de exibição

## HUD de desempenho (F12)

app-hud-scale = Escala
app-hud-level = Nível
app-hud-level-thumbnail = Miniatura incorporada
app-hud-level-full = Resolução total
# $denominator é 2, 4 ou 8: a imagem decodificada em 1/2, 1/4 ou 1/8 do tamanho.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Abrir
app-hud-decode = Decodificar
app-hud-upload = Envio
app-hud-frame = Quadro
app-hud-threads = Threads
app-hud-threads-value = { $threads } ({ $queued } na fila)
app-hud-tiles = Blocos
app-hud-tiles-value = { $tiles } de { $capacity }
app-hud-ram = Memória
app-hud-vram = Memória da GPU
app-hud-gpu = GPU

# Confirmação de mover para a Lixeira
app-confirm-trash-title = Mover { $file } para a Lixeira?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Os arquivos auxiliares também vão para a lixeira: { $files }
app-cancel = Cancelar
# Aviso mostrado antes de decodificar uma imagem muito grande. $need e $available são
# tamanhos de memória ("3,2 GiB").
toast-low-memory = Esta imagem pode precisar de cerca de { $need } de memória, mas só { $available } está disponível. Ela pode carregar devagar ou falhar.

# O relatório de hardware (Configurações → Avançado). Os valores são preenchidos pelo app.
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
hw-report-memory-value = { $total } no total, { $available } disponível
hw-report-graphics-api = API gráfica
hw-report-driver = Driver
hw-report-max-texture = Maior textura
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Threads de decodificação
hw-report-ram-cache = Cache de imagens na memória
hw-report-vram-cache = Cache na memória da GPU
hw-report-processing = Processamento
hw-report-path-jpeg-decode = Decodificação JPEG
hw-report-path-ycbcr-to-rgb = Conversão de cores
hw-report-path-png-unfilter = Decodificação PNG
hw-report-path-heic-decode = Decodificação HEIC
hw-report-path-inference = Análise de imagem
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, em paralelo
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = decodificador de hardware
hw-report-engine-npu = NPU
hw-report-build-debug = build de depuração
hw-report-build-release = build de lançamento

## Acessibilidade (leitores de tela)
# A imagem como um leitor de tela a anuncia: nome do arquivo e tamanho em pixels.
a11y-image = { $name }, { $width } × { $height } pixels
# A região onde aparecem notificações curtas.
a11y-notifications = Notificações
# machine-drafted, needs native review
a11y-transport-playback = Reprodução
a11y-transport-pages = Páginas
a11y-settings-sections = Seções

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Códigos nesta imagem
codes-looking = Procurando códigos…
codes-none = Nenhum código encontrado nesta imagem.
codes-failed = Não foi possível ler a imagem: { $reason }
codes-copy = Copiar “{ $text }”
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Abrir link em { $host }…
codes-network-off = Nenhuma rede está ligada, então os links não abrem.
codes-open-title = Abrir um link para { $host }?
codes-open-detail = Ele abre no seu navegador. Abra somente links em que você confia.
codes-open = Abrir no navegador
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Texto nesta imagem
# machine-drafted, needs native review
ocr-looking = Lendo o texto…
# machine-drafted, needs native review
ocr-none = Nenhum texto encontrado nesta imagem.
# machine-drafted, needs native review
ocr-failed = Não foi possível ler o texto: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Encontrar texto precisa dos modelos de texto do pacote de IA local (Configurações › IA local).
# machine-drafted, needs native review
ocr-loading = Carregando os modelos de texto…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Copiar “{ $text }”
# machine-drafted, needs native review
ocr-copy-all = Copiar todo o texto
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Confiança: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Sistema
hw-report-section-gpu = GPU
hw-report-section-decoding = Decodificação
hw-report-operating-system = Sistema operacional
hw-report-adapter = Adaptador
hw-report-gpu-type = Tipo
hw-report-gpu-init = Tempo de inicialização
hw-report-max-layers = Camadas de textura
hw-report-compression = Compressão de texturas
hw-report-unified-memory = Memória compartilhada com a CPU
hw-report-display = Tela
hw-report-hw-decode = Decodificação por hardware
hw-report-core-cluster = Grupo de núcleos
hw-report-power = Energia
hw-report-calibration = Medições de velocidade
hw-report-codec-others = Outros
hw-report-performance-cores-value = { $count } de { $total }
hw-report-cluster-value = { $name }: { $physical } físicos, { $logical } lógicos
hw-report-thermal-nominal = Temperatura normal
hw-report-thermal-fair = Quente
hw-report-thermal-serious = Muito quente, desacelerando
hw-report-thermal-critical = Superaquecido
hw-report-power-battery = Na bateria ({ $percent })
hw-report-power-battery-unknown = Na bateria
hw-report-power-mains = Na tomada
hw-report-power-mains-percent = Na tomada ({ $percent })
hw-report-power-saving = Economia de energia ativada
hw-report-gpu-discrete = Dedicado
hw-report-gpu-integrated = Integrado
hw-report-gpu-virtual = Virtual
hw-report-gpu-software = Software, na CPU
hw-report-gpu-other = Outro
hw-report-gpu-unavailable = Indisponível
hw-report-ms = { $ms } ms
hw-report-yes = Sim
hw-report-no = Não
hw-report-unified-platform-differs = O sistema informa o contrário
hw-report-display-value = { $fps } Hz, margem HDR { $now } agora, até { $max }
hw-report-hw-decode-value = { $api } em { $device }
hw-report-hw-decode-checking = Ainda verificando…
hw-report-codec-up-to = Até { $width } × { $height }
hw-report-codec-none = Indisponível
hw-report-codec-h264-encode = Codificação H.264
hw-report-supported = suportado: { $list }
hw-report-unsupported = não suportado: { $list }
hw-report-calibration-done = Medido neste computador
hw-report-calibration-pending = Ainda não medido
hw-report-paths-power-saving = Economia de energia: caminhos que gastam menos energia são preferidos
hw-report-paths-caption = Caminhos para uma imagem de { $mpx } megapixels
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
hw-report-reason-forced = definido nas Configurações
hw-report-reason-prefer-gpu = Preferir a GPU
hw-report-reason-cpu-only = Somente a CPU
hw-report-reason-calibrate = sendo medido
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
cheatsheet-gallery-history-back = Voltar para a pasta anterior
cheatsheet-gallery-history-forward = Avançar para a próxima pasta
cheatsheet-gallery-back = Fechar a visualização de cima e depois voltar
cheatsheet-gallery-viewer = Voltar ao visualizador
cheatsheet-gallery-album-add = Adicionar ao último álbum
cheatsheet-gallery-select-one = Selecionar ou desmarcar
cheatsheet-gallery-quick-look = Visualização rápida
cheatsheet-gallery-filter = Filtrar
cheatsheet-gallery-bigger = Miniaturas maiores
cheatsheet-gallery-smaller = Miniaturas menores
cheatsheet-gallery-default-size = Tamanho padrão das miniaturas
cheatsheet-gallery-refresh = Atualizar a pasta
cheatsheet-gallery-menu = Menu de contexto
cheatsheet-compare-views = Comparar
cheatsheet-compare-overlay = Sobreposição ou lado a lado
cheatsheet-compare-difference = Diferença (de novo: próximo tipo)
cheatsheet-compare-step-down = Menos mesclagem ou ganho da diferença
cheatsheet-compare-step-up = Mais mesclagem ou ganho da diferença
cheatsheet-compare-pick = Imagem em que as teclas atuam
cheatsheet-compare-close = Fechar a comparação
cheatsheet-channels = Canais
cheatsheet-medical-window = Janela e nível
cheatsheet-medical-measure = Medir e anotar
cheatsheet-medical-frames = Quadros e séries
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D e reformatação
cheatsheet-medical-analysis = Segmentação, fusão e PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Estudos e compartilhamento
cheatsheet-palette-only-title = Na paleta de comandos ({ $key })
cheatsheet-palette-only-title-nokey = Na paleta de comandos
cheatsheet-palette-only-tag = Paleta
cheatsheet-palette-only-line = { $action }: sem tecla; na paleta de comandos ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: sem tecla; na paleta de comandos
# Esc na folha de atalhos (a descrição longa da paleta não cabe).
cheatsheet-back = Fechar ou voltar

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Exportação de vídeo
hw-report-engine-hw-encoder = codificador de hardware

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (por miniatura de { $size })
hw-report-task-frame = { $task } (por quadro de { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Somente software

about-natural-earth = Dados do mapa do Natural Earth (naturalearthdata.com), domínio público.
app-info-show-on-map = Mostrar no mapa
