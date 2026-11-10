# machine-drafted, needs native review
# Specular — janela de Definições. Português europeu.
#
# Esquema de ids, gerado a partir do esquema de definições em crates/specular-settings:
#   settings-section-<secção>               título da secção
#   settings-<secção>-<chave>               nome da definição
#   settings-<secção>-<chave>-help          texto de ajuda por baixo do nome
#   settings-<secção>-<chave>-<valor>       uma opção de uma definição de escolha
# As chaves e os valores são os nomes do settings.toml com "_" escrito como "-".

## Janela

settings-title = Definições
settings-search-placeholder = Pesquisar definições
settings-search-no-results = Nenhuma definição corresponde a { $query }
settings-reset = Repor
settings-reset-section = Repor secção
settings-modified = Modificado
settings-default-value = Predefinição: { $value }
settings-restart-required = Tem efeito após reiniciar
settings-restart-now = Reiniciar agora
settings-undo = Anular
settings-redo = Refazer
settings-value-on = Ativado
settings-value-off = Desativado
settings-value-auto = Automático
settings-value-system = Seguir o sistema
settings-value-system-language = Predefinição do sistema
settings-value-follow-language = Seguir o idioma da interface
settings-value-default-location = Localização predefinida
settings-choose = Escolher…
settings-clear = Limpar
settings-file-reloaded = O settings.toml foi alterado fora do { -app-name } e foi recarregado.
settings-warning-unknown-key = Definição desconhecida { $key } no settings.toml. É mantida no ficheiro, mas ignorada.
settings-warning-invalid-value = Valor inválido para { $key }: { $reason } É usada a predefinição.
settings-warning-newer-version = O settings.toml foi escrito por uma versão mais recente do { -app-name }. As definições que esta versão não conhece são mantidas.
settings-warning-syntax = Não é possível ler o settings.toml (linha { $line }): { $reason } São usadas as predefinições até ser corrigido.

## Erros de validação (também mostrados por `specular --set`)

settings-error-unknown-key = Não existe nenhuma definição chamada { $key }.
settings-error-unknown-key-suggestion = Não existe nenhuma definição chamada { $key }. Quis dizer { $suggestion }?
settings-error-not-bool = Esperava-se on ou off (true ou false).
settings-error-not-integer = Esperava-se um número inteiro.
settings-error-not-number = Esperava-se um número.
settings-error-out-of-range = Tem de estar entre { $min } e { $max }.
settings-error-out-of-range-auto = Tem de ser 0 (automático) ou estar entre { $min } e { $max }.
settings-error-unknown-option = Esperava-se um de: { $options }.
settings-error-bad-colour = Esperava-se uma cor como #1E1E1E ou #1E1E1E80.
settings-error-bad-language = Esperava-se uma etiqueta de idioma como de, pt-BR ou zh-Hans, ou «system».
settings-error-bad-path = Esperava-se o caminho de um ficheiro ou pasta.
settings-error-not-settable = Esta entrada é um comando e não tem valor.
settings-error-syntax = Esperava-se chave=valor, por exemplo viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Não pode conter tabulações, quebras de linha ou outros caracteres de controlo.
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

## Secções

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
settings-section-advanced = Avançadas

## Geral

settings-general-language = Idioma da interface
settings-general-language-help = As alterações têm efeito imediato. «Predefinição do sistema» segue o idioma do sistema operativo.
settings-general-region-formats = Formatos regionais
settings-general-region-formats-help = Como são escritas as datas, horas, números e tamanhos. Por predefinição, seguem o idioma da interface.
settings-general-theme = Tema
settings-general-theme-help = Aspeto claro ou escuro das janelas e dos controlos. As imagens nunca são alteradas.
settings-general-theme-system = Seguir o sistema
settings-general-theme-light = Claro
settings-general-theme-dark = Escuro
# machine-drafted, needs native review
settings-general-on-start = Quando o { -app-name } abre sem ficheiro
settings-general-on-start-help = O que mostrar quando o { -app-name } é aberto sozinho. Um ficheiro aberto a partir do gestor de ficheiros ou da linha de comandos aparece sempre no visualizador.
settings-general-on-start-start-screen = O ecrã inicial
settings-general-on-start-gallery = Galeria da última pasta
settings-general-on-start-library = Biblioteca
settings-general-start-tips = Mostrar «Para experimentar» no ecrã inicial
settings-general-start-tips-help = Uma fila de cartões com funcionalidades úteis e as respetivas teclas. «Mostrar boas-vindas» na paleta de comandos volta a mostrá-la.
settings-general-tips = Mostrar sugestões
settings-general-tips-help = Uma sugestão curta, uma única vez, na primeira vez que uma funcionalidade pode ajudar, como a tecla que mostra uma pasta como galeria. Nunca durante uma apresentação de diapositivos.
settings-general-tips-shown = Sugestões já mostradas
settings-general-tips-shown-help = As sugestões únicas já dadas, para não serem repetidas.
# machine-drafted, needs native review
settings-general-your-name = O seu nome
settings-general-your-name-help = Aparece nos carimbos e anotações que adicionar. Guardado só nos ficheiros de anotações que criar.
settings-general-single-instance = Abrir ficheiros na janela existente
settings-general-single-instance-help = Quando o { -app-name } já está em execução, abrir outra imagem mostra-a nessa janela em vez de iniciar uma nova.
settings-general-file-associations = Associações de ficheiros…
settings-general-file-associations-help = Tornar o { -app-name } o visualizador predefinido para os formatos de imagem que suporta.

## Visualização

settings-viewing-initial-zoom = Zoom inicial
settings-viewing-initial-zoom-help = O tamanho com que é mostrada uma imagem acabada de abrir.
settings-viewing-initial-zoom-fit = Ajustar à janela
settings-viewing-initial-zoom-fit-if-larger = Ajustar as grandes, mostrar as pequenas a { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Tamanho real ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Preencher a janela
settings-viewing-wheel-behaviour = Roda do rato
settings-viewing-wheel-behaviour-help = O que faz a roda do rato. Mantenha Ctrl premido para a outra ação.
settings-viewing-wheel-behaviour-help-macos = O que faz a roda do rato. Mantenha ⌘ premido para a outra ação.
settings-viewing-wheel-behaviour-zoom = Zoom
settings-viewing-wheel-behaviour-navigate = Imagem seguinte e anterior
settings-viewing-keep-zoom = Manter o zoom entre imagens
settings-viewing-keep-zoom-help = Com zoom aplicado, a imagem seguinte e a anterior abrem com o mesmo zoom e mostram a mesma parte do enquadramento: útil para verificar a focagem de uma sequência.
settings-viewing-zoom-animation = Animar o zoom
settings-viewing-zoom-animation-help = Fazer zoom de forma suave em vez de saltar. Desativado automaticamente quando «Reduzir movimento» está ativo.
settings-viewing-zoom-animation-speed = Velocidade da animação de zoom
settings-viewing-zoom-animation-speed-help = A rapidez com que o zoom animado estabiliza.
settings-viewing-zoom-animation-speed-relaxed = Calma
settings-viewing-zoom-animation-speed-normal = Normal
settings-viewing-zoom-animation-speed-fast = Rápida
settings-viewing-background = Fundo
settings-viewing-background-help = A cor por trás da imagem numa janela.
settings-viewing-background-theme = Igual ao tema
settings-viewing-background-black = Preto
settings-viewing-background-dark-grey = Cinzento escuro
settings-viewing-background-light-grey = Cinzento claro
settings-viewing-background-white = Branco
settings-viewing-background-custom = Cor personalizada
settings-viewing-background-color = Cor de fundo personalizada
settings-viewing-background-color-help = Usada quando o Fundo está definido como «Cor personalizada».
settings-viewing-fullscreen-background = Fundo em ecrã inteiro
settings-viewing-fullscreen-background-help = A cor por trás da imagem em ecrã inteiro.
settings-viewing-transparency = Áreas transparentes
settings-viewing-transparency-help = Como são desenhadas as partes transparentes de uma imagem.
settings-viewing-transparency-checkerboard = Xadrez
settings-viewing-transparency-background = Cor de fundo
settings-viewing-transparency-black = Preto
settings-viewing-transparency-white = Branco
settings-viewing-pixel-grid-threshold = Mostrar grelha de píxeis a partir de
settings-viewing-pixel-grid-threshold-help = Desenhar uma grelha entre os píxeis com um zoom de pelo menos este valor.
settings-viewing-smoothing-high-zoom = Píxeis ampliados
settings-viewing-smoothing-high-zoom-help = O aspeto dos píxeis quando a imagem é mostrada acima de { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Suaves, nítidos a partir de { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Sempre suaves
settings-viewing-smoothing-high-zoom-pixelated = Sempre quadrados nítidos
settings-viewing-double-click = Duplo clique na imagem
settings-viewing-double-click-help = O duplo clique no fundo vazio alterna sempre o ecrã inteiro.
settings-viewing-double-click-toggle-zoom = Alternar entre ajustar e { PERCENT(100) }
settings-viewing-double-click-fullscreen = Alternar ecrã inteiro
settings-viewing-double-click-nothing = Não fazer nada
settings-viewing-fullscreen-on-open = Abrir imagens em ecrã inteiro
settings-viewing-fullscreen-on-open-help = Iniciar em ecrã inteiro quando uma imagem é aberta a partir de fora do { -app-name }.
settings-viewing-fullscreen-mode = Estilo de ecrã inteiro
settings-viewing-fullscreen-mode-help = O ecrã inteiro nativo usa o seu próprio Space; o ecrã inteiro instantâneo cobre o atual sem animação.
settings-viewing-fullscreen-mode-native = Nativo (Space próprio)
settings-viewing-fullscreen-mode-borderless = Instantâneo
settings-viewing-toolbar-autohide = Ocultar a barra de ferramentas automaticamente
settings-viewing-toolbar-autohide-help = A barra de ferramentas e as setas laterais desaparecem quando o ponteiro está parado e voltam quando se move.
settings-viewing-toolbar-autohide-delay = Ocultar a barra de ferramentas após
settings-viewing-toolbar-autohide-delay-help = Quanto tempo o ponteiro tem de estar parado antes de a barra de ferramentas desaparecer.
settings-viewing-toolbar-items = Botões da barra de ferramentas
settings-viewing-toolbar-items-help = Os botões que a barra de ferramentas mostra, por ordem.
settings-viewing-toolbar-drag = Reorganizar a barra de ferramentas arrastando
settings-viewing-toolbar-drag-help = Arraste um botão ao longo da barra de ferramentas para o mover. Um clique curto continua a acionar o botão; Esc devolve um botão arrastado ao seu lugar.
settings-viewing-toolbar-items-open = Abrir
settings-viewing-toolbar-items-prev = Imagem anterior
settings-viewing-toolbar-items-next = Imagem seguinte
settings-viewing-toolbar-items-zoom-out = Reduzir
settings-viewing-toolbar-items-zoom-level = Nível de zoom
settings-viewing-toolbar-items-zoom-in = Ampliar
settings-viewing-toolbar-items-fit = Ajustar à janela
settings-viewing-toolbar-items-rotate-left = Rodar para a esquerda
settings-viewing-toolbar-items-rotate-right = Rodar para a direita
settings-viewing-toolbar-items-info = Informações
settings-viewing-toolbar-items-copy = Copiar imagem
settings-viewing-toolbar-items-delete = Mover para o Lixo
settings-viewing-toolbar-items-palette = Paleta de comandos
settings-viewing-toolbar-items-settings = Definições
settings-viewing-toolbar-items-fullscreen = Ecrã inteiro
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Espaço flexível
settings-viewing-toolbar-items-separator = Separador
settings-toolbar-preview = Pré-visualização da barra de ferramentas
settings-toolbar-hidden = Botões ocultos
settings-toolbar-add-spacer = Adicionar espaço flexível
settings-toolbar-add-separator = Adicionar separador
settings-toolbar-reset = Repor predefinições
settings-toolbar-remove = Remover { $item }
settings-toolbar-hint = Arraste uma linha para a mover, ou selecione-a e prima Alt+↑ ou Alt+↓. Tab alterna entre o campo de pesquisa e a lista.
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
settings-browsing-sort-order-help = A ordem usada ao percorrer uma pasta. A ordem por nome põe «img2» antes de «img10» e segue o alfabeto do seu idioma.
settings-browsing-sort-order-natural = Nome
settings-browsing-sort-order-date-taken = Data de captura
settings-browsing-sort-order-modified = Data de modificação
settings-browsing-sort-order-size = Tamanho do ficheiro
settings-browsing-sort-order-type = Tipo de ficheiro
settings-browsing-sort-descending = Ordem inversa
settings-browsing-sort-descending-help = Ordenar do último para o primeiro.
settings-browsing-loop-folder = Recomeçar no fim da pasta
settings-browsing-loop-folder-help = Depois da última imagem, continuar com a primeira, e vice-versa.
settings-browsing-include-subfolders = Incluir subpastas
settings-browsing-include-subfolders-help = Percorrer também as imagens das subpastas.
settings-browsing-hold-acceleration = Acelerar ao manter seguinte ou anterior
settings-browsing-hold-acceleration-help = A rapidez com que a passagem de imagens acelera enquanto uma tecla ou botão está premido.
settings-browsing-hold-acceleration-off = Sem aceleração
settings-browsing-hold-acceleration-gentle = Suave
settings-browsing-hold-acceleration-normal = Normal
settings-browsing-hold-acceleration-fast = Rápida
settings-browsing-confirm-trash = Perguntar antes de mover para o Lixo
settings-browsing-confirm-trash-help = Mover para o Lixo pode sempre ser anulado, por isso o { -app-name } não pergunta por predefinição.
settings-browsing-arrows-follow-reading-direction = Setas seguem o sentido de leitura
settings-browsing-arrows-follow-reading-direction-help = Nos idiomas da direita para a esquerda, a seta para a esquerda vai para a imagem seguinte.

## Cor

settings-colour-management = Gestão de cor
settings-colour-management-help = Mostrar as cores como deviam ser vistas, usando o perfil de cor da imagem e o do ecrã.
settings-colour-display-profile = Perfil do ecrã
settings-colour-display-profile-help = O perfil de cor do ecrã. Automático usa o que o sistema operativo indica.
settings-colour-rendering-intent = Intenção de composição
settings-colour-rendering-intent-help = Como as cores que o ecrã não consegue mostrar são trazidas para a sua gama.
settings-colour-rendering-intent-perceptual = Percetual
settings-colour-rendering-intent-relative-colorimetric = Colorimétrica relativa
settings-colour-rendering-intent-saturation = Saturação
settings-colour-rendering-intent-absolute-colorimetric = Colorimétrica absoluta
settings-colour-black-point-compensation = Compensação do ponto preto
settings-colour-black-point-compensation-help = Manter o detalhe nas sombras quando o preto do ecrã é mais claro do que o da imagem.
settings-colour-untagged-images = Imagens sem perfil de cor
settings-colour-untagged-images-help = Como mostrar as imagens que não indicam que cores usam.
settings-colour-untagged-images-srgb = Tratar como sRGB
settings-colour-untagged-images-display = Mostrar sem alterações
settings-colour-dithering = Pontilhado
settings-colour-dithering-help = Adicionar um ruído azul fino ao reduzir a profundidade de cor, o que evita faixas em céus e gradientes.
settings-colour-hdr-output = Saída HDR
settings-colour-hdr-output-help = Mostrar as imagens HDR com todo o seu brilho nos ecrãs que o suportam. Automático usa HDR quando o ecrã indica brilho extra; caso contrário, as zonas claras são suavemente comprimidas para caber.
settings-colour-hdr-output-auto = Automático
settings-colour-hdr-output-on = Ativado
settings-colour-hdr-output-off = Desativado
# machine-drafted, needs native review
settings-colour-ocio-config = Configuração OCIO
settings-colour-ocio-config-help = Uma configuração OpenColorIO (.ocio) para imagens EXR, Radiance HDR e PFM. Vazio usa a indicada por $OCIO.
settings-colour-ocio-display = Ecrã OCIO
settings-colour-ocio-display-help = O ecrã (display) da configuração OCIO que corresponde ao seu monitor.
settings-colour-ocio-display-default = O primeiro da configuração
settings-colour-ocio-view = Vista OCIO
settings-colour-ocio-view-help = Como as imagens scene-linear são mostradas nesse ecrã, por exemplo numa vista com mapeamento de tons.
settings-colour-ocio-view-default = A primeira do ecrã
settings-colour-ocio-input = Espaço de cor de entrada OCIO
settings-colour-ocio-input-help = O espaço de cor dos valores das imagens.
settings-colour-ocio-input-default = A função scene-linear da configuração
# machine-drafted, needs native review
settings-colour-proof-profile = Perfil da prova no ecrã
settings-colour-proof-profile-help = O perfil de impressora que a prova no ecrã da Revelação usou por último.
settings-colour-proof-intent = Intenção da prova no ecrã
settings-colour-proof-intent-help = Como as cores que a impressora não imprime são ajustadas.
settings-colour-proof-intent-perceptual = Percetual
settings-colour-proof-intent-relative = Colorimétrica relativa
settings-colour-proof-bpc = Compensação de ponto preto na prova
settings-colour-proof-bpc-help = Leva o preto ao preto da impressora.
settings-colour-proof-paper = Branco do papel na prova
settings-colour-proof-paper-help = Mostra a cor do próprio papel.
settings-colour-proof-ink = Preto da tinta na prova
settings-colour-proof-ink-help = Mostra quão escuro é o preto da tinta.

## Metadados e privacidade

settings-metadata-show-info-on-open = Mostrar o painel de informações ao abrir
settings-metadata-show-info-on-open-help = Abrir o painel com os detalhes da câmara, data e local sempre que é aberta uma imagem.
settings-metadata-info-detail = O painel de informações mostra
settings-metadata-info-detail-help = O resumo explica os detalhes mais úteis em linguagem simples; todos os campos lista tudo o que está guardado no ficheiro.
settings-metadata-info-detail-summary = Resumo
settings-metadata-info-detail-all = Todos os campos
settings-metadata-reverse-geocoding = Mostrar nomes de locais para posições GPS
settings-metadata-reverse-geocoding-help = Procurar a localidade mais próxima nos dados incorporados. Funciona offline: as posições nunca saem do computador.
settings-metadata-hide-gps = Ocultar posições GPS no ecrã
settings-metadata-hide-gps-help = Não mostrar coordenadas nem locais, por exemplo ao partilhar o ecrã. Os ficheiros não são alterados.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Guardar as edições de Revelação junto da fotografia
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Escreve IMG_1234.CR2.specular.xmp junto do ficheiro, para que as edições acompanhem a fotografia. Desligado: as edições ficam só na biblioteca.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importar definições do Lightroom em Revelar
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Uma foto com definições do Lightroom e sem edição abre com elas, de forma aproximada. Desligado: Revelar oferece um botão.
settings-metadata-units = Unidades
settings-metadata-units-help = Unidades de altitude, distâncias e velocidades.
settings-metadata-units-auto = Seguir os formatos regionais
settings-metadata-units-metric = Métricas
settings-metadata-units-imperial = Imperiais
# machine-drafted, needs native review
settings-metadata-basemap = Mapa base
settings-metadata-basemap-help = O mapa sob os locais das fotografias. Os mapas online transferem mosaicos da zona visualizada.
settings-metadata-basemap-off = Só o mapa incorporado (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Outro servidor de mosaicos (online)
settings-metadata-basemap-file = O meu ficheiro de mapa
settings-metadata-basemap-url = Endereço do servidor de mosaicos
settings-metadata-basemap-url-help = Um endereço com {"{"}z{"}"}, {"{"}x{"}"} e {"{"}y{"}"}, por exemplo https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Ficheiro de mapa
settings-metadata-basemap-path-help = Um ficheiro MBTiles ou PMTiles. Funciona offline.
settings-metadata-basemap-cache = Cache de mosaicos de mapa
settings-metadata-basemap-cache-help = Espaço em disco para mosaicos transferidos. Os mais antigos saem primeiro.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Servidores de mosaicos permitidos
settings-metadata-basemap-allowed-help = Servidores de mapas dos quais aceitou transferir mosaicos.

## Desempenho

settings-performance-ram-cache = Cache de memória
settings-performance-ram-cache-help = Memória usada para manter as imagens descodificadas prontas. Automático escolhe com base na memória do computador.
settings-performance-vram-cache = Cache na memória da GPU
settings-performance-vram-cache-help = Memória da GPU usada para imagens e mosaicos. Automático escolhe com base na GPU.
settings-performance-prefetch-depth = Imagens a preparar antecipadamente
settings-performance-prefetch-depth-help = Quantas imagens no sentido em que avança são descodificadas antecipadamente.
settings-performance-decoder-threads = Threads de descodificação
settings-performance-decoder-threads-help = Threads da CPU usadas para descodificar imagens. Automático usa todos os núcleos de desempenho e adapta-se ao ficheiro.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = A GPU a usar quando o computador tem mais do que uma.
settings-performance-gpu-adapter-high-performance = Máximo desempenho
settings-performance-gpu-adapter-low-power = Mínimo consumo
settings-performance-processing = Onde as imagens são processadas
settings-performance-processing-help = Algum trabalho pode ser feito na CPU, na GPU ou num descodificador de hardware. Automático mede este computador e escolhe o modo mais rápido para a imagem que está a ver e o mais económico para o trabalho em segundo plano. O que escolheu neste computador, e porquê, é indicado abaixo.
settings-performance-processing-automatic = Automático
settings-performance-processing-prefer-gpu = Preferir a GPU
settings-performance-processing-cpu-only = Apenas a CPU
settings-performance-drag-preview = Pré-visualização ao arrastar
settings-performance-drag-preview-help = Como a Revelação mostra a fotografia enquanto arrasta um cursor. Automático usa uma imagem mais pequena só quando o computador não acompanha.
settings-performance-drag-preview-automatic = Automático
settings-performance-drag-preview-sharp = Sempre nítida
settings-performance-drag-preview-fast = Sempre rápida

## Definições → Desempenho → Como o { -app-name } funciona neste computador
settings-paths-title = Como o { -app-name } funciona neste computador
settings-paths-intro = O { -app-name } cronometra cada forma de executar estas tarefas neste computador e utiliza a melhor. Os tempos referem-se a uma fotografia de { $mpx } megapíxeis.
settings-paths-intro-measuring = Medição ainda não concluída: o { -app-name } mede enquanto o computador está inativo.
settings-paths-intro-battery = Com bateria: as escolhas privilegiam um menor consumo de energia.
settings-paths-keywords = GPU CPU NPU placa gráfica aceleração de hardware descodificador benchmark velocidade desempenho
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, todos os núcleos
paths-engine-gpu = GPU
paths-engine-hw-decoder = Descodificador de hardware
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (a sua definição)
settings-paths-measured-faster = Medido neste computador: { $ratio } vezes mais rápido do que via { $other }.
settings-paths-measured-similar = Medido neste computador: aproximadamente tão rápido como via { $other }.
settings-paths-measured-power = Com bateria, esta opção gasta menos energia (medido neste computador).
settings-paths-not-measured = Ainda não medido: a escolha habitual para este tipo de computador.
settings-paths-set-by = Escolhido pela sua definição «{ $setting }: { $option }».
settings-paths-blocked-check = O processamento via { $engine } foi desativado a { $date }: o resultado não coincidia com o da CPU.
settings-paths-blocked-error = O processamento via { $engine } foi desativado a { $date } após um erro.
settings-paths-blocked-lost = O processamento via { $engine } foi desativado a { $date } depois de deixar de responder.
settings-paths-blocked-undated = O processamento via { $engine } foi desativado após um erro.
settings-paths-checking = O processamento via { $engine } é comparado com a CPU antes de ser utilizado na imagem que está a ver.
settings-paths-exploring = A experimentar o processamento via { $engine } em segundo plano.
settings-paths-unavailable = Este computador não tem { $engine } para isto.
settings-paths-background-cheaper = Em segundo plano: processamento via { $engine }, que nas medições gastou menos { $percent } de tempo de CPU.
settings-paths-background-power = Em segundo plano: processamento via { $engine }, para poupar energia.
settings-paths-background = Em segundo plano: processamento via { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms de tempo de CPU, { $runs ->
    [one] { $runs } medição
   *[other] { $runs } medições
} ({ $confidence })
settings-paths-detail-guess = { $engine }: cerca de { $time } ms (estimativa)
settings-paths-confidence-low = confiança baixa
settings-paths-confidence-medium = confiança média
settings-paths-confidence-high = confiança elevada
settings-paths-machine = Medido para { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Tentar novamente
settings-paths-measure-again = Medir novamente
settings-paths-measure-again-question = Esquecer o que foi medido neste computador, incluindo os caminhos desativados, e medir novamente quando o computador estiver inativo?
settings-paths-measure-again-confirm = Esquecer medições
settings-performance-display-cache = Manter uma cache de visualização no disco
settings-performance-display-cache-help = Guardar cópias das imagens ao tamanho do ecrã, para que as pastas já vistas abram instantaneamente.
settings-performance-display-cache-size = Tamanho da cache de visualização
settings-performance-display-cache-size-help = As entradas mais antigas são removidas quando a cache está cheia.
settings-performance-display-cache-location = Localização da cache de visualização
settings-performance-display-cache-location-help = Funciona melhor numa pasta de uma unidade rápida.
settings-performance-fast-simd = Usar instruções SIMD
settings-performance-fast-simd-help = Usar as instruções vetoriais da CPU para descodificar. Desative apenas para diagnosticar um problema.
settings-performance-fast-dct-scaling = Descodificar ao tamanho do ecrã
settings-performance-fast-dct-scaling-help = Descodificar JPEG grandes diretamente ao tamanho em que são mostrados. Desative apenas para diagnosticar um problema.
settings-performance-fast-parallel-decode = Descodificar em vários núcleos
settings-performance-fast-parallel-decode-help = Dividir as imagens grandes pelos núcleos da CPU. Desative apenas para diagnosticar um problema.
settings-performance-fast-restart-index = Indexar JPEG grandes para zoom
settings-performance-fast-restart-index-help = Memorizar onde começa cada parte de um JPEG, para que as zonas ampliadas sejam descodificadas depressa. Desative apenas para diagnosticar um problema.
settings-performance-fast-yuv-upload = Converter cores na GPU
settings-performance-fast-yuv-upload-help = Converter as cores dos JPEG na GPU ou na CPU. Automático escolhe o que for mais rápido neste computador. Desative apenas para diagnosticar um problema.
settings-performance-fast-yuv-upload-auto = Automático
settings-performance-fast-yuv-upload-on = Ativado
settings-performance-fast-yuv-upload-off = Desativado
settings-performance-fast-async-io = Leitura assíncrona de ficheiros
settings-performance-fast-async-io-help = Ler antecipadamente com a interface de ficheiros mais rápida do sistema operativo. Desative apenas para diagnosticar um problema.
settings-performance-fast-mmap = Ficheiros mapeados em memória
settings-performance-fast-mmap-help = Ler os ficheiros sem os copiar. Desative apenas para diagnosticar um problema.
settings-performance-hardware-decode = Descodificação por hardware
settings-performance-hardware-decode-help = Deixar o descodificador incorporado da GPU abrir fotografias JPEG. Automático usa-o onde ajuda, normalmente para imagens preparadas em segundo plano. Primeiro é verificado em relação ao descodificador normal, e o que não conseguir tratar abre normalmente.
settings-performance-hardware-decode-auto = Automático
settings-performance-hardware-decode-on = Ativado
settings-performance-hardware-decode-off = Desativado
settings-performance-frame-time-hud = HUD de tempo de fotograma
settings-performance-frame-time-hud-help = Mostrar os tempos de descodificação e de fotograma e os caminhos rápidos em uso.
settings-performance-stall-warning = Comunicar carregamentos parados após
settings-performance-stall-warning-help = Quando uma imagem continua a carregar após este tempo, escrever o seu estado de carregamento e as tarefas pendentes no registo (saída de erro padrão ou SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Sugestões de desempenho
settings-performance-insights-help = Quando as imagens continuam a abrir lentamente pelo mesmo motivo, mostrar «Porque está lento?» no canto com o que ajudaria. O que aprende fica neste computador.
settings-performance-insights-storage = Sugestões sobre armazenamento lento
settings-performance-insights-storage-help = Mostrar sugestões quando a leitura de ficheiros (de uma partilha de rede, uma unidade lenta ou arquivos) continua a atrasar a abertura das imagens.
settings-performance-insights-decoder = Sugestões sobre descodificação
settings-performance-insights-decoder-help = Mostrar sugestões quando as imagens continuam a ser descodificadas na CPU, embora um descodificador de hardware pudesse ajudar.
# machine-drafted, needs native review
settings-performance-insights-restore = Mostrar novamente as sugestões ocultas
settings-performance-insights-restore-help = Repõe as sugestões que ocultou com × em «Porque está lento?», tanto para abrir imagens como para miniaturas. Indisponível enquanto nenhuma estiver oculta.

## Acessibilidade

settings-accessibility-reduce-motion = Reduzir movimento
settings-accessibility-reduce-motion-help = Substituir as animações de zoom, deslize e desvanecimento por mudanças instantâneas.
settings-accessibility-reduce-motion-system = Seguir o sistema
settings-accessibility-reduce-motion-on = Ativado
settings-accessibility-reduce-motion-off = Desativado
settings-accessibility-increase-contrast = Aumentar contraste
settings-accessibility-increase-contrast-help = Contornos e texto mais fortes e menos transparência.
settings-accessibility-increase-contrast-system = Seguir o sistema
settings-accessibility-increase-contrast-on = Ativado
settings-accessibility-increase-contrast-off = Desativado
settings-accessibility-text-scale = Tamanho do texto
settings-accessibility-text-scale-help = Tamanho do texto em relação ao tamanho padrão.
settings-accessibility-ui-scale = Escala da interface
settings-accessibility-ui-scale-help = Tamanho de toda a interface, além da escala de ecrã definida no sistema operativo.
settings-accessibility-focus-ring-always = Mostrar sempre o contorno de foco
settings-accessibility-focus-ring-always-help = Mostrar que controlo tem o foco do teclado mesmo ao usar o rato.
settings-accessibility-screen-reader-verbosity = Detalhe do leitor de ecrã
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Quanto anuncia um leitor de ecrã ao passar de uma imagem para outra e quanto da leitura sob o cursor lê.
settings-accessibility-screen-reader-verbosity-concise = Breve
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Detalhado
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Ler o valor sob o cursor
settings-accessibility-announce-cursor-help = Um leitor de ecrã lê o valor do píxel, HU, temperatura ou posição sob o cursor ou a mira do teclado quando deixa de se mover.

## Privacidade

# machine-drafted, needs native review
settings-privacy-no-network = Sem rede
# machine-drafted, needs native review
settings-privacy-no-network-help = Desligado: só estes se ligam, cada um depois de o ativar ou de abrir algo com ele: verificação de atualizações, transferências de modelos de IA local, mapas online, imagens de um endereço web ou armazenamento na nuvem e servidores que adicionar (DICOMweb, OMERO, mapas). Ligado: nada se liga.

## Avançadas

settings-advanced-portable-mode = Modo portátil
settings-advanced-portable-mode-help = Guardar as definições e as caches na pasta junto ao programa em vez do perfil de utilizador.
settings-advanced-log-level = Detalhe do registo
settings-advanced-log-level-help = Quanto o { -app-name } escreve no ficheiro de registo.
settings-advanced-log-level-error = Apenas erros
settings-advanced-log-level-warn = Avisos
settings-advanced-log-level-info = Informações
settings-advanced-log-level-debug = Depuração
settings-advanced-log-level-trace = Tudo
settings-advanced-developer-mode = Modo de programador
settings-advanced-developer-mode-help = Mostrar ferramentas para desenvolver e testar o { -app-name }, como os pseudoidiomas na lista de idiomas.
# machine-drafted, needs native review
settings-advanced-audit-log = Registo de auditoria local
# machine-drafted, needs native review
settings-advanced-audit-log-help = Registar imagens médicas abertas, exportações, anonimizações, impressões e envios para um PACS em audit.jsonl, na pasta de dados. O registo fica neste computador.
settings-advanced-hw-report = Relatório de hardware…
settings-advanced-hw-report-help = Mostrar que funcionalidades da CPU e da GPU o { -app-name } detetou e que caminhos rápidos usa.
# machine-drafted, needs native review
settings-advanced-renderer-info = Informações do renderizador…
settings-advanced-renderer-info-help = Mostra a placa gráfica, o controlador, os formatos de ecrã e os limites de memória que o { -app-name } utiliza, prontos a copiar para um relatório de erro.
settings-advanced-edit-settings-file = Editar settings.toml
settings-advanced-edit-settings-file-help = Edite o ficheiro de definições no editor próprio do { -app-name }. Este verifica o ficheiro enquanto escreve e só o guarda quando não tem erros.
settings-advanced-import = Importar definições…
settings-advanced-import-help = Carregar definições de um ficheiro exportado do { -app-name }.
settings-advanced-export = Exportar definições…
settings-advanced-export-help = Guardar todas as definições num ficheiro, como cópia de segurança ou para outro computador.
settings-advanced-reset-all = Repor todas as definições…
settings-advanced-reset-all-help = Repor a predefinição de todas as definições. Esta ação pode ser anulada.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Guardar
config-editor-revert = Reverter
config-editor-close = Fechar
config-editor-modified = Modificado
config-editor-text = Ficheiro de definições
config-editor-position = Linha { $line }, coluna { $column }
config-editor-saved = settings.toml guardado e aplicado.
config-editor-cant-save =
    { $count ->
        [one] Não é possível guardar: 1 problema. Corrija-o primeiro.
       *[other] Não é possível guardar: { $count } problemas. Corrija-os primeiro.
    }
config-editor-save-failed = Não foi possível guardar settings.toml: { $reason }
config-editor-conflict = settings.toml foi alterado noutro local.
config-editor-reload = Recarregar
config-editor-keep-mine = Manter os meus
config-editor-discard-question = Descartar as alterações não guardadas em settings.toml?
config-editor-discard = Descartar
config-editor-keep-editing = Continuar a editar
config-editor-find-placeholder = Procurar no ficheiro
config-editor-find-count = { $current } de { $total }
config-editor-find-none = Sem correspondências
config-editor-find-previous = Correspondência anterior
config-editor-find-next = Correspondência seguinte
config-editor-find-close = Fechar a pesquisa
config-editor-problems = Problemas
config-editor-problems-count =
    { $count ->
        [0] Sem problemas
        [one] 1 problema
       *[other] { $count } problemas
    }
config-editor-no-problems = Sem problemas. O ficheiro pode ser guardado.
config-editor-problem-at = Linha { $line }: { $message }
config-editor-severity-error = Erro
config-editor-severity-warning = Aviso
config-editor-severity-info = Nota
config-editor-problem-syntax = TOML inválido: { $reason }
config-editor-problem-duplicate = { $key } está definido duas vezes na mesma tabela. Remova um deles.
config-editor-problem-unknown-section = Não existe a secção de definições [{ $key }].
config-editor-problem-unknown-section-suggestion = Não existe a secção de definições [{ $key }]. Queria dizer [{ $suggestion }]?
config-editor-problem-unknown-key = Não existe a definição { $key }.
config-editor-problem-unknown-key-suggestion = Não existe a definição { $key }. Queria dizer { $suggestion }?
config-editor-problem-invalid = Valor inválido para { $key }: { $reason }
config-editor-problem-migrated = { $key } está escrito como uma versão anterior do { -app-name } o escrevia. É convertido na leitura.
config-editor-problem-newer-version = Este ficheiro foi escrito por uma versão mais recente do { -app-name } (formato { $found }; esta versão lê { $supported }). As definições desconhecidas são mantidas.
config-editor-problem-command = Comando { $index } em [[commands]]: { $reason }
config-editor-problem-hook = Gancho { $index } em [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Ficheiro de atalhos de teclado
config-editor-keymap-saved = keymap.toml guardado e aplicado.
config-editor-keymap-save-failed = Não é possível guardar keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml foi alterado noutro local.
config-editor-keymap-discard-question = Descartar as alterações não guardadas em keymap.toml?
config-editor-new-file = ficheiro novo, criado ao guardar
config-editor-no-folder = não existe uma pasta de definições onde o guardar.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Repor todas as definições para os valores predefinidos?
settings-reset-all-confirm-detail = Isto afeta todas as secções das Definições. Pode ser anulado depois.
settings-reset-all-confirm-ok = Repor tudo
settings-reset-all-done = Todas as definições foram repostas para os valores predefinidos.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Codificador de hardware

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = O processamento via { $engine } é comparado com a CPU antes de ser utilizado.
settings-paths-unit-thumbnail = Tempos para uma miniatura de { $size }
settings-paths-unit-frame = Tempos por fotograma de vídeo de { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Pasta de fotogramas de calibração
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flats e darks para Revelar. Os nomes de ficheiro ou pasta contêm "flat" ou "dark".

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Branco
settings-colour-light-grey = Cinzento-claro
settings-colour-grey = Cinzento
settings-colour-dark-grey = Cinzento-escuro
settings-colour-black = Preto
settings-colour-yellow = Amarelo
settings-colour-orange = Laranja
settings-colour-red = Vermelho
settings-colour-magenta = Magenta
settings-colour-blue = Azul
settings-colour-cyan = Ciano
settings-colour-green = Verde
