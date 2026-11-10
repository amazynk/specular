# machine-drafted, needs native review
# Specular — janela de Configurações. Português do Brasil.

## Elementos da janela

settings-title = Configurações
settings-search-placeholder = Buscar configurações
settings-search-no-results = Nenhuma configuração corresponde a { $query }
settings-reset = Redefinir
settings-reset-section = Redefinir seção
settings-modified = Modificado
settings-default-value = Padrão: { $value }
settings-restart-required = Entra em vigor após reiniciar
settings-restart-now = Reiniciar agora
settings-undo = Desfazer
settings-redo = Refazer
settings-value-on = Ativado
settings-value-off = Desativado
settings-value-auto = Automático
settings-value-system = Seguir o sistema
settings-value-system-language = Padrão do sistema
settings-value-follow-language = Seguir o idioma da interface
settings-value-default-location = Local padrão
settings-choose = Escolher…
settings-clear = Limpar
settings-file-reloaded = settings.toml foi alterado fora do { -app-name } e foi recarregado.
settings-warning-unknown-key = Configuração desconhecida { $key } em settings.toml. Ela é mantida no arquivo, mas ignorada.
settings-warning-invalid-value = Valor inválido para { $key }: { $reason } O padrão é usado no lugar.
settings-warning-newer-version = settings.toml foi gravado por uma versão mais recente do { -app-name }. As configurações que esta versão não conhece são mantidas.
settings-warning-syntax = Não é possível ler settings.toml (linha { $line }): { $reason } Os padrões são usados até que seja corrigido.

## Erros de validação (também exibidos por `specular --set`)

settings-error-unknown-key = Não existe nenhuma configuração chamada { $key }.
settings-error-unknown-key-suggestion = Não existe nenhuma configuração chamada { $key }. Você quis dizer { $suggestion }?
settings-error-not-bool = Esperado on ou off (true ou false).
settings-error-not-integer = Esperado um número inteiro.
settings-error-not-number = Esperado um número.
settings-error-out-of-range = Deve estar entre { $min } e { $max }.
settings-error-out-of-range-auto = Deve ser 0 (automático) ou estar entre { $min } e { $max }.
settings-error-unknown-option = Esperado um destes: { $options }.
settings-error-bad-colour = Esperada uma cor como #1E1E1E ou #1E1E1E80.
settings-error-bad-language = Esperado um código de idioma como de, pt-BR ou zh-Hans, ou “system”.
settings-error-bad-path = Esperado o caminho de um arquivo ou pasta.
settings-error-not-settable = Esta entrada é um comando e não tem valor.
settings-error-syntax = Esperado chave=valor, por exemplo viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Não pode conter tabulações, quebras de linha ou outros caracteres de controle.
settings-error-invalid = { $reason }
settings-error-tile-url = Precisa de um endereço http ou https com {"{"}z{"}"}, {"{"}x{"}"} e {"{"}y{"}"}.

## Unidades

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } imagem
       *[other] { $value } imagens
    }
unit-threads =
    { $value ->
        [one] { $value } thread
       *[other] { $value } threads
    }
unit-bytes =
    { $value ->
        [one] { $value } byte
       *[other] { $value } bytes
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
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

## Seções

settings-section-general = Geral
settings-section-viewing = Visualização
settings-section-browsing = Navegação
settings-section-colour = Cor
settings-section-metadata = Metadados e privacidade
# machine-drafted, needs native review
settings-section-privacy = Privacidade
settings-section-performance = Desempenho
settings-section-accessibility = Acessibilidade
settings-section-keys = Teclas
settings-section-advanced = Avançado

## Geral

settings-general-language = Idioma da interface
settings-general-language-help = As alterações entram em vigor imediatamente. “Padrão do sistema” segue o idioma do seu sistema operacional.
settings-general-region-formats = Formatos regionais
settings-general-region-formats-help = Como datas, horas, números e tamanhos são escritos. Por padrão, seguem o idioma da interface.
settings-general-theme = Tema
settings-general-theme-help = Aparência clara ou escura de janelas e controles. As imagens nunca são alteradas.
settings-general-theme-system = Seguir o sistema
settings-general-theme-light = Claro
settings-general-theme-dark = Escuro
# machine-drafted, needs native review
settings-general-on-start = Quando o { -app-name } abre sem arquivo
settings-general-on-start-help = O que mostrar quando o { -app-name } é aberto sozinho. Um arquivo aberto pelo gerenciador de arquivos ou pela linha de comando sempre aparece no visualizador.
settings-general-on-start-start-screen = A tela inicial
settings-general-on-start-gallery = Galeria da última pasta
settings-general-on-start-library = Biblioteca
settings-general-start-tips = Mostrar “Para experimentar” na tela inicial
settings-general-start-tips-help = Uma fileira de cartões com recursos úteis e suas teclas. “Mostrar boas-vindas” na paleta de comandos a traz de volta.
settings-general-tips = Mostrar dicas
settings-general-tips-help = Uma dica curta, uma única vez, na primeira vez que um recurso pode ajudar, como a tecla que mostra uma pasta como galeria. Nunca durante uma apresentação de slides.
settings-general-tips-shown = Dicas já mostradas
settings-general-tips-shown-help = As dicas únicas já dadas, para não serem repetidas.
# machine-drafted, needs native review
settings-general-your-name = Seu nome
settings-general-your-name-help = Aparece nos carimbos e marcações que você adiciona. Salvo só nos arquivos de marcações que você cria.
settings-general-single-instance = Abrir arquivos na janela existente
settings-general-single-instance-help = Quando o { -app-name } já está em execução, abrir outra imagem a mostra naquela janela em vez de iniciar uma nova.
settings-general-file-associations = Associações de arquivos…
settings-general-file-associations-help = Torna o { -app-name } o visualizador padrão dos formatos de imagem compatíveis.

## Visualização

settings-viewing-initial-zoom = Zoom inicial
settings-viewing-initial-zoom-help = O tamanho com que uma imagem recém-aberta é mostrada.
settings-viewing-initial-zoom-fit = Ajustar à janela
settings-viewing-initial-zoom-fit-if-larger = Ajustar imagens grandes, mostrar as pequenas em { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Tamanho real ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Preencher a janela
settings-viewing-wheel-behaviour = Roda do mouse
settings-viewing-wheel-behaviour-help = O que girar a roda faz. Mantenha Ctrl pressionado para a outra ação.
settings-viewing-wheel-behaviour-help-macos = O que girar a roda faz. Mantenha ⌘ pressionado para a outra ação.
settings-viewing-wheel-behaviour-zoom = Zoom
settings-viewing-wheel-behaviour-navigate = Próxima e anterior
settings-viewing-keep-zoom = Manter o zoom entre imagens
settings-viewing-keep-zoom-help = Com zoom aplicado, a imagem seguinte e a anterior abrem com o mesmo zoom e mostram a mesma parte do quadro: útil para conferir o foco de uma sequência.
settings-viewing-zoom-animation = Animar o zoom
settings-viewing-zoom-animation-help = Aplica o zoom suavemente em vez de saltar. Desativado automaticamente quando “Reduzir movimento” está ativado.
settings-viewing-zoom-animation-speed = Velocidade da animação de zoom
settings-viewing-zoom-animation-speed-help = Com que rapidez os zooms animados se estabilizam.
settings-viewing-zoom-animation-speed-relaxed = Tranquila
settings-viewing-zoom-animation-speed-normal = Normal
settings-viewing-zoom-animation-speed-fast = Rápida
settings-viewing-background = Plano de fundo
settings-viewing-background-help = A cor atrás da imagem em uma janela.
settings-viewing-background-theme = Igual ao tema
settings-viewing-background-black = Preto
settings-viewing-background-dark-grey = Cinza-escuro
settings-viewing-background-light-grey = Cinza-claro
settings-viewing-background-white = Branco
settings-viewing-background-custom = Cor personalizada
settings-viewing-background-color = Cor de fundo personalizada
settings-viewing-background-color-help = Usada quando Plano de fundo está definido como “Cor personalizada”.
settings-viewing-fullscreen-background = Fundo em tela cheia
settings-viewing-fullscreen-background-help = A cor atrás da imagem em tela cheia.
settings-viewing-transparency = Áreas transparentes
settings-viewing-transparency-help = Como as partes transparentes de uma imagem são desenhadas.
settings-viewing-transparency-checkerboard = Quadriculado
settings-viewing-transparency-background = Cor de fundo
settings-viewing-transparency-black = Preto
settings-viewing-transparency-white = Branco
settings-viewing-pixel-grid-threshold = Mostrar grade de pixels a partir de
settings-viewing-pixel-grid-threshold-help = Desenha uma grade entre os pixels quando o zoom é pelo menos este.
settings-viewing-smoothing-high-zoom = Pixels ampliados
settings-viewing-smoothing-high-zoom-help = A aparência dos pixels quando a imagem é mostrada acima de { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Suave, nítido a partir de { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Sempre suave
settings-viewing-smoothing-high-zoom-pixelated = Sempre quadrados nítidos
settings-viewing-double-click = Clique duplo na imagem
settings-viewing-double-click-help = Clicar duas vezes no fundo vazio sempre alterna a tela cheia.
settings-viewing-double-click-toggle-zoom = Alternar entre ajustado e { PERCENT(100) }
settings-viewing-double-click-fullscreen = Alternar tela cheia
settings-viewing-double-click-nothing = Não fazer nada
settings-viewing-fullscreen-on-open = Abrir imagens em tela cheia
settings-viewing-fullscreen-on-open-help = Inicia em tela cheia quando uma imagem é aberta de fora do { -app-name }.
settings-viewing-fullscreen-mode = Estilo de tela cheia
settings-viewing-fullscreen-mode-help = A tela cheia nativa usa seu próprio Space; a tela cheia instantânea cobre o atual sem animação.
settings-viewing-fullscreen-mode-native = Nativa (Space próprio)
settings-viewing-fullscreen-mode-borderless = Instantânea
settings-viewing-toolbar-autohide = Ocultar a barra de ferramentas automaticamente
settings-viewing-toolbar-autohide-help = A barra de ferramentas e as setas laterais somem quando o ponteiro fica parado e voltam quando ele se move.
settings-viewing-toolbar-autohide-delay = Ocultar a barra de ferramentas após
settings-viewing-toolbar-autohide-delay-help = Quanto tempo o ponteiro precisa ficar parado antes que a barra de ferramentas suma.
settings-viewing-toolbar-items = Botões da barra de ferramentas
settings-viewing-toolbar-items-help = Quais botões a barra de ferramentas mostra, em ordem.
settings-viewing-toolbar-drag = Reorganizar a barra de ferramentas arrastando
settings-viewing-toolbar-drag-help = Arraste um botão ao longo da barra de ferramentas para movê-lo. Um clique curto continua acionando o botão; Esc devolve um botão arrastado ao seu lugar.
settings-viewing-toolbar-items-open = Abrir
settings-viewing-toolbar-items-prev = Imagem anterior
settings-viewing-toolbar-items-next = Próxima imagem
settings-viewing-toolbar-items-zoom-out = Reduzir
settings-viewing-toolbar-items-zoom-level = Nível de zoom
settings-viewing-toolbar-items-zoom-in = Ampliar
settings-viewing-toolbar-items-fit = Ajustar à janela
settings-viewing-toolbar-items-rotate-left = Girar para a esquerda
settings-viewing-toolbar-items-rotate-right = Girar para a direita
settings-viewing-toolbar-items-info = Informações
settings-viewing-toolbar-items-copy = Copiar imagem
settings-viewing-toolbar-items-delete = Mover para a Lixeira
settings-viewing-toolbar-items-palette = Paleta de comandos
settings-viewing-toolbar-items-settings = Configurações
settings-viewing-toolbar-items-fullscreen = Tela cheia
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Espaço flexível
settings-viewing-toolbar-items-separator = Separador
settings-toolbar-preview = Prévia da barra de ferramentas
settings-toolbar-hidden = Botões ocultos
settings-toolbar-add-spacer = Adicionar espaço flexível
settings-toolbar-add-separator = Adicionar separador
settings-toolbar-reset = Restaurar padrão
settings-toolbar-remove = Remover { $item }
settings-toolbar-hint = Arraste uma linha para movê-la, ou selecione-a e pressione Alt+↑ ou Alt+↓. Tab alterna entre o campo de busca e a lista.
settings-toolbar-move-up = Mover para cima
settings-toolbar-move-down = Mover para baixo
settings-toolbar-move-start = Mover para o início
settings-toolbar-move-end = Mover para o fim
settings-toolbar-menu-hide = Ocultar
settings-toolbar-menu-show = Mostrar
settings-toolbar-menu-remove = Remover
settings-toolbar-row-menu = Mais ações para { $item }
settings-toolbar-position = Posição { $index } de { $count }
settings-toolbar-hidden-state = Oculto

## Navegação

settings-browsing-sort-order = Ordenar imagens por
settings-browsing-sort-order-help = A ordem usada ao percorrer uma pasta. A ordem por nome coloca “img2” antes de “img10” e segue o alfabeto do seu idioma.
settings-browsing-sort-order-natural = Nome
settings-browsing-sort-order-date-taken = Data da foto
settings-browsing-sort-order-modified = Data de modificação
settings-browsing-sort-order-size = Tamanho do arquivo
settings-browsing-sort-order-type = Tipo de arquivo
settings-browsing-sort-descending = Ordem inversa
settings-browsing-sort-descending-help = Ordena do último para o primeiro.
settings-browsing-loop-folder = Voltar ao início no fim da pasta
settings-browsing-loop-folder-help = Depois da última imagem, continua com a primeira, e vice-versa.
settings-browsing-include-subfolders = Incluir subpastas
settings-browsing-include-subfolders-help = Percorre também as imagens das subpastas.
settings-browsing-hold-acceleration = Aceleração ao manter próxima ou anterior
settings-browsing-hold-acceleration-help = Com que rapidez a troca de imagens acelera enquanto uma tecla ou botão é mantido pressionado.
settings-browsing-hold-acceleration-off = Sem aceleração
settings-browsing-hold-acceleration-gentle = Suave
settings-browsing-hold-acceleration-normal = Normal
settings-browsing-hold-acceleration-fast = Rápida
settings-browsing-confirm-trash = Perguntar antes de mover para a Lixeira
settings-browsing-confirm-trash-help = Mover para a Lixeira sempre pode ser desfeito, por isso o { -app-name } não pergunta por padrão.
settings-browsing-arrows-follow-reading-direction = Setas seguem a direção de leitura
settings-browsing-arrows-follow-reading-direction-help = Em idiomas da direita para a esquerda, faz a seta para a esquerda ir para a próxima imagem.

## Cor

settings-colour-management = Gerenciamento de cores
settings-colour-management-help = Mostra as cores como deveriam ser, usando o perfil de cor da imagem e o do seu monitor.
settings-colour-display-profile = Perfil do monitor
settings-colour-display-profile-help = O perfil de cor do seu monitor. Automático usa o informado pelo sistema operacional.
settings-colour-rendering-intent = Tipo de renderização
settings-colour-rendering-intent-help = Como as cores que o monitor não consegue mostrar são trazidas para a sua gama.
settings-colour-rendering-intent-perceptual = Perceptual
settings-colour-rendering-intent-relative-colorimetric = Colorimétrico relativo
settings-colour-rendering-intent-saturation = Saturação
settings-colour-rendering-intent-absolute-colorimetric = Colorimétrico absoluto
settings-colour-black-point-compensation = Compensação do ponto preto
settings-colour-black-point-compensation-help = Preserva os detalhes das sombras quando o preto do monitor é mais claro que o da imagem.
settings-colour-untagged-images = Imagens sem perfil de cor
settings-colour-untagged-images-help = Como mostrar imagens que não dizem quais cores usam.
settings-colour-untagged-images-srgb = Tratar como sRGB
settings-colour-untagged-images-display = Mostrar sem alteração
settings-colour-dithering = Pontilhamento
settings-colour-dithering-help = Adiciona um ruído azul fino ao reduzir a profundidade de cor, o que evita faixas em céus e degradês.
settings-colour-hdr-output = Saída HDR
settings-colour-hdr-output-help = Mostra imagens HDR com todo o brilho em monitores compatíveis. Automático usa HDR quando o monitor informa brilho extra; caso contrário, as áreas claras são comprimidas suavemente para caber.
settings-colour-hdr-output-auto = Automático
settings-colour-hdr-output-on = Ativado
settings-colour-hdr-output-off = Desativado
# machine-drafted, needs native review
settings-colour-ocio-config = Configuração OCIO
settings-colour-ocio-config-help = Uma configuração OpenColorIO (.ocio) para imagens EXR, Radiance HDR e PFM. Vazio usa a indicada por $OCIO.
settings-colour-ocio-display = Display OCIO
settings-colour-ocio-display-help = O display da configuração OCIO que corresponde ao seu monitor.
settings-colour-ocio-display-default = O primeiro da configuração
settings-colour-ocio-view = Visualização OCIO
settings-colour-ocio-view-help = Como as imagens scene-linear são mostradas nesse display, como uma visualização com mapeamento de tons.
settings-colour-ocio-view-default = A primeira do display
settings-colour-ocio-input = Espaço de cor de entrada OCIO
settings-colour-ocio-input-help = O espaço de cor dos valores das imagens.
settings-colour-ocio-input-default = A função scene-linear da configuração
# machine-drafted, needs native review
settings-colour-proof-profile = Perfil da prova na tela
settings-colour-proof-profile-help = O perfil de impressora que a prova na tela da Revelação usou por último.
settings-colour-proof-intent = Intenção da prova na tela
settings-colour-proof-intent-help = Como as cores que a impressora não imprime são ajustadas.
settings-colour-proof-intent-perceptual = Perceptual
settings-colour-proof-intent-relative = Colorimétrica relativa
settings-colour-proof-bpc = Compensação de ponto preto na prova
settings-colour-proof-bpc-help = Leva o preto ao preto da impressora.
settings-colour-proof-paper = Branco do papel na prova
settings-colour-proof-paper-help = Mostra a cor do próprio papel.
settings-colour-proof-ink = Preto da tinta na prova
settings-colour-proof-ink-help = Mostra quão escuro é o preto da tinta.

## Metadados e privacidade

settings-metadata-show-info-on-open = Mostrar o painel de informações ao abrir
settings-metadata-show-info-on-open-help = Abre o painel com detalhes de câmera, data e local sempre que uma imagem é aberta.
settings-metadata-info-detail = O painel de informações mostra
settings-metadata-info-detail-help = O resumo explica os detalhes mais úteis em linguagem simples; todos os campos lista tudo o que está armazenado no arquivo.
settings-metadata-info-detail-summary = Resumo
settings-metadata-info-detail-all = Todos os campos
settings-metadata-reverse-geocoding = Mostrar nomes de lugares para posições GPS
settings-metadata-reverse-geocoding-help = Procura a cidade mais próxima em dados integrados. Funciona offline: as posições nunca saem do seu computador.
settings-metadata-hide-gps = Ocultar posições GPS na tela
settings-metadata-hide-gps-help = Não mostra coordenadas nem lugares, por exemplo ao compartilhar a tela. Os arquivos não são alterados.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Salvar as edições de Revelação ao lado da foto
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Grava IMG_1234.CR2.specular.xmp ao lado do arquivo, para que as edições acompanhem a foto. Desligado: as edições ficam só na biblioteca.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importar ajustes do Lightroom em Revelar
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Uma foto com ajustes do Lightroom e sem edição abre com eles, de forma aproximada. Desligado: Revelar oferece um botão.
settings-metadata-units = Unidades
settings-metadata-units-help = Unidades de altitude, distâncias e velocidades.
settings-metadata-units-auto = Seguir os formatos regionais
settings-metadata-units-metric = Métrico
settings-metadata-units-imperial = Imperial
# machine-drafted, needs native review
settings-metadata-basemap = Mapa base
settings-metadata-basemap-help = O mapa sob os locais das fotos. Mapas online baixam blocos da área visualizada.
settings-metadata-basemap-off = Só o mapa integrado (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Outro servidor de blocos (online)
settings-metadata-basemap-file = Meu próprio arquivo de mapa
settings-metadata-basemap-url = Endereço do servidor de blocos
settings-metadata-basemap-url-help = Um endereço com {"{"}z{"}"}, {"{"}x{"}"} e {"{"}y{"}"}, por exemplo https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Arquivo de mapa
settings-metadata-basemap-path-help = Um arquivo MBTiles ou PMTiles. Funciona offline.
settings-metadata-basemap-cache = Cache de blocos de mapa
settings-metadata-basemap-cache-help = Espaço em disco para blocos baixados. Os mais antigos saem primeiro.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Servidores de blocos permitidos
settings-metadata-basemap-allowed-help = Servidores de mapas dos quais você aceitou baixar blocos.

## Desempenho

settings-performance-ram-cache = Cache na memória
settings-performance-ram-cache-help = Memória usada para manter imagens decodificadas prontas. Automático escolhe com base na quantidade de memória do computador.
settings-performance-vram-cache = Cache na memória da GPU
settings-performance-vram-cache-help = Memória da GPU usada para imagens e blocos. Automático escolhe com base na sua GPU.
settings-performance-prefetch-depth = Imagens a preparar antecipadamente
settings-performance-prefetch-depth-help = Quantas imagens na direção em que você avança são decodificadas com antecedência.
settings-performance-decoder-threads = Threads de decodificação
settings-performance-decoder-threads-help = Threads da CPU usadas para decodificar imagens. Automático usa todos os núcleos de desempenho e se adapta ao arquivo.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Qual GPU usar quando o computador tem mais de uma.
settings-performance-gpu-adapter-high-performance = Maior desempenho
settings-performance-gpu-adapter-low-power = Menor consumo de energia
settings-performance-processing = Onde as imagens são processadas
settings-performance-processing-help = Parte do trabalho pode rodar na CPU, na GPU ou em um decodificador de hardware. Automático mede este computador e escolhe o caminho mais rápido para a imagem que você está vendo e o mais econômico para o trabalho em segundo plano. O que ele escolheu neste computador, e por quê, aparece abaixo.
settings-performance-processing-automatic = Automático
settings-performance-processing-prefer-gpu = Preferir a GPU
settings-performance-processing-cpu-only = Somente a CPU
settings-performance-drag-preview = Prévia ao arrastar
settings-performance-drag-preview-help = Como a Revelação mostra a foto enquanto você arrasta um controle deslizante. Automático usa uma imagem menor só quando o computador não acompanha.
settings-performance-drag-preview-automatic = Automático
settings-performance-drag-preview-sharp = Sempre nítida
settings-performance-drag-preview-fast = Sempre rápida

## Configurações → Desempenho → Como o { -app-name } funciona neste computador
settings-paths-title = Como o { -app-name } funciona neste computador
settings-paths-intro = O { -app-name } cronometra cada forma de executar essas tarefas neste computador e usa a melhor. Os tempos são para uma foto de { $mpx } megapixels.
settings-paths-intro-measuring = Medição ainda não concluída: o { -app-name } mede enquanto o computador está ocioso.
settings-paths-intro-battery = Na bateria: as escolhas priorizam um menor consumo de energia.
settings-paths-keywords = GPU CPU NPU placa de vídeo aceleração de hardware decodificador benchmark velocidade desempenho
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, todos os núcleos
paths-engine-gpu = GPU
paths-engine-hw-decoder = Decodificador de hardware
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (sua configuração)
settings-paths-measured-faster = Medido neste computador: { $ratio } vezes mais rápido que via { $other }.
settings-paths-measured-similar = Medido neste computador: quase tão rápido quanto via { $other }.
settings-paths-measured-power = Na bateria, esta opção usa menos energia (medido neste computador).
settings-paths-not-measured = Ainda não medido: a escolha habitual para este tipo de computador.
settings-paths-set-by = Escolhido pela sua configuração “{ $setting }: { $option }”.
settings-paths-blocked-check = O processamento via { $engine } foi desativado em { $date }: o resultado não correspondeu ao da CPU.
settings-paths-blocked-error = O processamento via { $engine } foi desativado em { $date } após um erro.
settings-paths-blocked-lost = O processamento via { $engine } foi desativado em { $date } depois de parar de responder.
settings-paths-blocked-undated = O processamento via { $engine } foi desativado após um erro.
settings-paths-checking = O processamento via { $engine } é comparado com a CPU antes de ser usado na imagem que você está vendo.
settings-paths-exploring = Testando o processamento via { $engine } em segundo plano agora.
settings-paths-unavailable = Este computador não tem { $engine } para isso.
settings-paths-background-cheaper = Em segundo plano: processamento via { $engine }, que nas medições usou { $percent } menos tempo de CPU.
settings-paths-background-power = Em segundo plano: processamento via { $engine }, para economizar energia.
settings-paths-background = Em segundo plano: processamento via { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms de tempo de CPU, { $runs ->
    [one] { $runs } medição
   *[other] { $runs } medições
} ({ $confidence })
settings-paths-detail-guess = { $engine }: cerca de { $time } ms (estimativa)
settings-paths-confidence-low = confiança baixa
settings-paths-confidence-medium = confiança média
settings-paths-confidence-high = confiança alta
settings-paths-machine = Medido para { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Tentar novamente
settings-paths-measure-again = Medir novamente
settings-paths-measure-again-question = Esquecer o que foi medido neste computador, incluindo os caminhos desativados, e medir novamente quando o computador estiver ocioso?
settings-paths-measure-again-confirm = Esquecer medições
settings-performance-display-cache = Manter um cache de exibição no disco
settings-performance-display-cache-help = Armazena cópias das imagens do tamanho da tela para que pastas já vistas abram instantaneamente.
settings-performance-display-cache-size = Tamanho do cache de exibição
settings-performance-display-cache-size-help = As entradas mais antigas são removidas quando o cache fica cheio.
settings-performance-display-cache-location = Local do cache de exibição
settings-performance-display-cache-location-help = Uma pasta em uma unidade rápida funciona melhor.
settings-performance-fast-simd = Usar instruções SIMD
settings-performance-fast-simd-help = Usa as instruções vetoriais da CPU para decodificar. Desative apenas para diagnosticar um problema.
settings-performance-fast-dct-scaling = Decodificar no tamanho da tela
settings-performance-fast-dct-scaling-help = Decodifica JPEGs grandes diretamente no tamanho em que são mostrados. Desative apenas para diagnosticar um problema.
settings-performance-fast-parallel-decode = Decodificar em vários núcleos
settings-performance-fast-parallel-decode-help = Divide imagens grandes entre os núcleos da CPU. Desative apenas para diagnosticar um problema.
settings-performance-fast-restart-index = Indexar JPEGs grandes para zoom
settings-performance-fast-restart-index-help = Lembra onde começa cada parte de um JPEG, para que regiões ampliadas sejam decodificadas rapidamente. Desative apenas para diagnosticar um problema.
settings-performance-fast-yuv-upload = Converter cores na GPU
settings-performance-fast-yuv-upload-help = Converte as cores de JPEG na GPU ou na CPU. Automático escolhe o que for mais rápido neste computador. Desative apenas para diagnosticar um problema.
settings-performance-fast-yuv-upload-auto = Automático
settings-performance-fast-yuv-upload-on = Ativado
settings-performance-fast-yuv-upload-off = Desativado
settings-performance-fast-async-io = Leitura assíncrona de arquivos
settings-performance-fast-async-io-help = Lê antecipadamente com a interface de arquivos mais rápida do sistema operacional. Desative apenas para diagnosticar um problema.
settings-performance-fast-mmap = Arquivos mapeados na memória
settings-performance-fast-mmap-help = Lê arquivos sem copiá-los. Desative apenas para diagnosticar um problema.
settings-performance-hardware-decode = Decodificação por hardware
settings-performance-hardware-decode-help = Permite que o decodificador integrado da GPU abra fotos JPEG. Automático o usa onde ajuda, geralmente para imagens preparadas em segundo plano. Ele é primeiro verificado em relação ao decodificador normal, e o que ele não conseguir tratar abre normalmente.
settings-performance-hardware-decode-auto = Automático
settings-performance-hardware-decode-on = Ativado
settings-performance-hardware-decode-off = Desativado
settings-performance-frame-time-hud = HUD de tempo de quadro
settings-performance-frame-time-hud-help = Mostra os tempos de decodificação e de quadro e quais caminhos rápidos estão em uso.
settings-performance-stall-warning = Registrar carregamentos travados após
settings-performance-stall-warning-help = Quando uma imagem ainda está carregando após esse tempo, grava seu estado de carregamento e as tarefas pendentes no log (saída de erro padrão ou SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Dicas de desempenho
settings-performance-insights-help = Quando as imagens continuarem abrindo devagar pelo mesmo motivo, mostrar “Por que está lento?” no canto com o que ajudaria. O que ele aprende fica neste computador.
settings-performance-insights-storage = Dicas sobre armazenamento lento
settings-performance-insights-storage-help = Mostrar dicas quando a leitura de arquivos (de um compartilhamento de rede, uma unidade lenta ou arquivos compactados) continuar atrasando a abertura das imagens.
settings-performance-insights-decoder = Dicas sobre decodificação
settings-performance-insights-decoder-help = Mostrar dicas quando as imagens continuarem sendo decodificadas na CPU, embora um decodificador de hardware pudesse ajudar.
# machine-drafted, needs native review
settings-performance-insights-restore = Mostrar novamente as dicas ocultas
settings-performance-insights-restore-help = Traz de volta as dicas que você ocultou com × em “Por que está lento?”, tanto para abrir imagens quanto para miniaturas. Indisponível enquanto nenhuma estiver oculta.

## Acessibilidade

settings-accessibility-reduce-motion = Reduzir movimento
settings-accessibility-reduce-motion-help = Substitui as animações de zoom, deslizamento e esmaecimento por mudanças instantâneas.
settings-accessibility-reduce-motion-system = Seguir o sistema
settings-accessibility-reduce-motion-on = Ativado
settings-accessibility-reduce-motion-off = Desativado
settings-accessibility-increase-contrast = Aumentar contraste
settings-accessibility-increase-contrast-help = Bordas e texto mais fortes e menos transparência.
settings-accessibility-increase-contrast-system = Seguir o sistema
settings-accessibility-increase-contrast-on = Ativado
settings-accessibility-increase-contrast-off = Desativado
settings-accessibility-text-scale = Tamanho do texto
settings-accessibility-text-scale-help = Tamanho do texto em relação ao tamanho padrão.
settings-accessibility-ui-scale = Escala da interface
settings-accessibility-ui-scale-help = Tamanho de toda a interface, além da escala de exibição definida no sistema operacional.
settings-accessibility-focus-ring-always = Sempre mostrar o contorno de foco
settings-accessibility-focus-ring-always-help = Mostra qual controle tem o foco do teclado mesmo ao usar o mouse.
settings-accessibility-screen-reader-verbosity = Detalhes do leitor de tela
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Quanto um leitor de tela anuncia quando você passa de uma imagem para outra e quanto da leitura sob o cursor ele lê.
settings-accessibility-screen-reader-verbosity-concise = Breve
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Detalhado
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Ler o valor sob o cursor
settings-accessibility-announce-cursor-help = Um leitor de tela lê o valor do pixel, HU, temperatura ou posição sob o cursor ou a mira do teclado quando ele para de se mover.

## Privacidade

# machine-drafted, needs native review
settings-privacy-no-network = Sem rede
# machine-drafted, needs native review
settings-privacy-no-network-help = Desligado: só estes se conectam, cada um depois que você o liga ou abre algo com ele: verificação de atualizações, downloads de modelos de IA local, mapas online, imagens de um endereço da web ou armazenamento em nuvem e servidores que você adiciona (DICOMweb, OMERO, mapas). Ligado: nada se conecta.

## Avançado

settings-advanced-portable-mode = Modo portátil
settings-advanced-portable-mode-help = Mantém configurações e caches na pasta ao lado do programa em vez do seu perfil de usuário.
settings-advanced-log-level = Detalhe do log
settings-advanced-log-level-help = Quanto o { -app-name } grava no arquivo de log.
settings-advanced-log-level-error = Somente erros
settings-advanced-log-level-warn = Avisos
settings-advanced-log-level-info = Informações
settings-advanced-log-level-debug = Depuração
settings-advanced-log-level-trace = Tudo
settings-advanced-developer-mode = Modo de desenvolvedor
settings-advanced-developer-mode-help = Mostra ferramentas para desenvolver e testar o { -app-name }, como os pseudoidiomas na lista de idiomas.
# machine-drafted, needs native review
settings-advanced-audit-log = Registro de auditoria local
# machine-drafted, needs native review
settings-advanced-audit-log-help = Registra imagens médicas abertas, exportações, anonimizações, impressões e envios a um PACS em audit.jsonl, na pasta de dados. O registro fica neste computador.
settings-advanced-hw-report = Relatório de hardware…
settings-advanced-hw-report-help = Mostra quais recursos de CPU e GPU o { -app-name } detectou e quais caminhos rápidos ele usa.
# machine-drafted, needs native review
settings-advanced-renderer-info = Informações do renderizador…
settings-advanced-renderer-info-help = Mostra o adaptador gráfico, o driver, os formatos de tela e os limites de memória que o { -app-name } usa, prontos para copiar em um relatório de bug.
settings-advanced-edit-settings-file = Editar settings.toml
settings-advanced-edit-settings-file-help = Edite o arquivo de configurações no editor próprio do { -app-name }. Ele verifica o arquivo enquanto você digita e só o salva quando não há erros.
settings-advanced-import = Importar configurações…
settings-advanced-import-help = Carrega configurações de um arquivo exportado pelo { -app-name }.
settings-advanced-export = Exportar configurações…
settings-advanced-export-help = Salva todas as configurações em um arquivo, para backup ou outro computador.
settings-advanced-reset-all = Redefinir todas as configurações…
settings-advanced-reset-all-help = Volta cada configuração ao padrão. Isso pode ser desfeito.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Salvar
config-editor-revert = Reverter
config-editor-close = Fechar
config-editor-modified = Modificado
config-editor-text = Arquivo de configurações
config-editor-position = Linha { $line }, coluna { $column }
config-editor-saved = settings.toml salvo e aplicado.
config-editor-cant-save =
    { $count ->
        [one] Não é possível salvar: 1 problema. Corrija-o primeiro.
       *[other] Não é possível salvar: { $count } problemas. Corrija-os primeiro.
    }
config-editor-save-failed = Não foi possível salvar settings.toml: { $reason }
config-editor-conflict = settings.toml foi alterado em outro lugar.
config-editor-reload = Recarregar
config-editor-keep-mine = Manter os meus
config-editor-discard-question = Descartar as alterações não salvas em settings.toml?
config-editor-discard = Descartar
config-editor-keep-editing = Continuar editando
config-editor-find-placeholder = Localizar no arquivo
config-editor-find-count = { $current } de { $total }
config-editor-find-none = Nenhuma ocorrência
config-editor-find-previous = Ocorrência anterior
config-editor-find-next = Próxima ocorrência
config-editor-find-close = Fechar a busca
config-editor-problems = Problemas
config-editor-problems-count =
    { $count ->
        [0] Nenhum problema
        [one] 1 problema
       *[other] { $count } problemas
    }
config-editor-no-problems = Nenhum problema. O arquivo pode ser salvo.
config-editor-problem-at = Linha { $line }: { $message }
config-editor-severity-error = Erro
config-editor-severity-warning = Aviso
config-editor-severity-info = Observação
config-editor-problem-syntax = TOML inválido: { $reason }
config-editor-problem-duplicate = { $key } está definido duas vezes na mesma tabela. Remova um deles.
config-editor-problem-unknown-section = Não existe a seção de configurações [{ $key }].
config-editor-problem-unknown-section-suggestion = Não existe a seção de configurações [{ $key }]. Você quis dizer [{ $suggestion }]?
config-editor-problem-unknown-key = Não existe a configuração { $key }.
config-editor-problem-unknown-key-suggestion = Não existe a configuração { $key }. Você quis dizer { $suggestion }?
config-editor-problem-invalid = Valor inválido para { $key }: { $reason }
config-editor-problem-migrated = { $key } está escrito como uma versão anterior do { -app-name } o escrevia. Ele é convertido na leitura.
config-editor-problem-newer-version = Este arquivo foi escrito por uma versão mais recente do { -app-name } (formato { $found }; esta versão lê { $supported }). As configurações desconhecidas são mantidas.
config-editor-problem-command = Comando { $index } em [[commands]]: { $reason }
config-editor-problem-hook = Gancho { $index } em [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Arquivo de atalhos de teclado
config-editor-keymap-saved = keymap.toml salvo e aplicado.
config-editor-keymap-save-failed = Não é possível salvar keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml foi alterado em outro lugar.
config-editor-keymap-discard-question = Descartar as alterações não salvas em keymap.toml?
config-editor-new-file = arquivo novo, criado ao salvar
config-editor-no-folder = não há uma pasta de configurações onde salvá-lo.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Redefinir todas as configurações para o padrão?
settings-reset-all-confirm-detail = Isso afeta todas as seções das Configurações. Pode ser desfeito depois.
settings-reset-all-confirm-ok = Redefinir tudo
settings-reset-all-done = Todas as configurações foram redefinidas para o padrão.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Codificador de hardware

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = O processamento via { $engine } é comparado com a CPU antes de ser usado.
settings-paths-unit-thumbnail = Tempos para uma miniatura de { $size }
settings-paths-unit-frame = Tempos por quadro de vídeo de { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Pasta de quadros de calibração
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flats e darks para Revelar. Os nomes de arquivo ou pasta contêm "flat" ou "dark".

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Branco
settings-colour-light-grey = Cinza-claro
settings-colour-grey = Cinza
settings-colour-dark-grey = Cinza-escuro
settings-colour-black = Preto
settings-colour-yellow = Amarelo
settings-colour-orange = Laranja
settings-colour-red = Vermelho
settings-colour-magenta = Magenta
settings-colour-blue = Azul
settings-colour-cyan = Ciano
settings-colour-green = Verde
