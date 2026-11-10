# machine-drafted, needs native review
# Specular — Relatório de abertura: porque é que a imagem demorou o que demorou a abrir (painel de
# informações, secção «Desempenho»; DESIGN.md §12.7).

open-report-title = Desempenho

## Headlines
open-report-fast = Aberta em { $ms } ms
open-report-display-cache = Mostrada a partir da cache de visualização
open-report-slow-read = Armazenamento lento: { $percent } do tempo foi gasto a ler o ficheiro ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Armazenamento lento: { $percent } do tempo foi gasto a ler o ficheiro (ficheiro pequeno: sobretudo a aguardar que o armazenamento responda)
open-report-archive-read = { $percent } do tempo foi gasto a descomprimir o arquivo
open-report-startup = { $percent } do tempo foi gasto a iniciar o { -app-name }
open-report-checked = Descodificada duas vezes: o primeiro resultado do descodificador de hardware foi comparado com o da CPU ({ $percent } do tempo)
open-report-hardware = Descodificada pelo descodificador de hardware, que demorou { $percent } do tempo
open-report-decode = Imagem grande: descodificar { $mpx } megapíxeis demorou { $percent } do tempo
open-report-upload = { $percent } do tempo foi gasto a enviar a imagem para a GPU
open-report-waiting = { $percent } do tempo foi gasto à espera de que outras tarefas terminassem
open-report-stage = { $stage } demorou { $percent } do tempo
open-report-memory-short = A memória era escassa, pelo que o sistema pode ter tido de usar a memória virtual

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Descodificada por software: o descodificador de hardware ainda estava a ser detetado
open-report-sw-no-device = Descodificada por software: { $api } não está disponível neste computador
open-report-sw-no-codec = Descodificada por software: { $api } não tem descodificador { $codec } nesta GPU
open-report-sw-blocked-mismatch = Descodificada por software: o descodificador de hardware foi desativado porque o resultado não coincidiu com o da CPU
open-report-sw-blocked-error = Descodificada por software: o descodificador de hardware foi desativado após um erro
open-report-sw-blocked-lost = Descodificada por software: o descodificador de hardware foi desativado porque deixou de responder
open-report-sw-failed = Descodificada por software: o descodificador de hardware falhou nesta imagem
open-report-sw-busy = Descodificada por software: o descodificador de hardware estava ocupado
open-report-sw-unsupported = Descodificada por software: o descodificador de hardware não suporta esta imagem
open-report-sw-unavailable = Descodificada por software: a descodificação por hardware está desligada até o { -app-name } reiniciar
open-report-sw-cpu-only = Descodificada por software: o processamento está definido como «Apenas a CPU»
open-report-sw-setting-off = Descodificada por software: a descodificação por hardware está desligada nas definições
open-report-sw-unchecked = Descodificada por software: o descodificador de hardware ainda não foi verificado (é experimentado primeiro numa imagem preparada em segundo plano)
open-report-sw-lower-power = Descodificada por software: com bateria, a CPU gasta menos energia para isto
open-report-sw-faster = Descodificada por software: mais rápido neste computador (cerca de { $cpu } ms, contra { $hw } ms no descodificador de hardware)
open-report-sw-scaled = Descodificada por software: o descodificador de hardware só descodifica imagens em tamanho total

## Hints
open-report-hint-copy-local = Copie o ficheiro para um disco local para o abrir mais depressa.
open-report-hint-extract = Extraia o arquivo para uma pasta para abrir as páginas mais depressa.
open-report-hint-try-again = Para voltar a tentar o descodificador de hardware: Definições → Desempenho → Tentar novamente.
open-report-hint-automatic = Defina «Onde as imagens são processadas» como Automático em Definições → Desempenho.
open-report-hint-turn-on = Defina «Descodificação por hardware» como Automático em Definições → Desempenho.
open-report-hint-free-memory = Feche outros programas para libertar memória.
open-report-hint-once-only = Só a primeira imagem é verificada assim.

## Rows
open-report-total = Até ficar nítida
open-report-first-pixels = Primeiros píxeis
# machine-drafted, needs native review
open-report-whole = Imagem inteira visível
open-report-stage-startup = Arranque
open-report-stage-read = Leitura do ficheiro
open-report-stage-parse = Leitura do cabeçalho
open-report-stage-colour = Preparação da gestão de cor
open-report-stage-decode = Descodificação
open-report-stage-upload = Envio para a GPU
open-report-stage-waiting = Espera por outras tarefas
open-report-decoder = Descodificada por
open-report-decoder-checked = { $engine }, verificado com a CPU
open-report-decoder-cache = Cache de visualização
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, a 1/{ $scale } do tamanho
open-report-decoder-scaled-full = { $engine }, em tamanho total, depois reduzida para 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, a 1/{ $scale } do tamanho, sem o índice de arranque rápido
open-report-colour-conversion = Cores convertidas por

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Recorrente
open-report-advice-change = Ajudaria
open-report-advice-gain = Ganho estimado
open-report-advice-software = { $count } das últimas { $total } imagens { $format } lentas { $count ->
    [one] foi descodificada
   *[other] foram descodificadas
} por software por este motivo
open-report-advice-slow-storage = { $count } das últimas { $total } imagens lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela leitura do ficheiro (cerca de { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } das últimas { $total } imagens lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela leitura do ficheiro (ficheiros pequenos: sobretudo a aguardar que o armazenamento responda)
open-report-advice-archive = { $count } das últimas { $total } imagens lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela descompressão do arquivo
open-report-advice-hardware-decoder = Uma GPU com descodificador de hardware { $codec }
open-report-advice-gain-range = Cerca de { $low } a { $high } vezes mais rápido
open-report-advice-gain-one = Cerca de { $low } vezes mais rápido

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Sugestões de desempenho
open-report-insights-indicator = Porque está lento?
open-report-insights-announce = Sugestão de desempenho: as imagens continuam a abrir lentamente pelo mesmo motivo. Abra «Porque está lento?» para ver o que ajudaria.
open-report-insights-hide = Ocultar
open-report-insights-last = Última imagem lenta: { $total } até ficar nítida
open-report-insights-now = O que pode fazer agora
open-report-insights-hardware = Hardware que ajudaria
open-report-insights-none = Nenhum abrandamento recorrente
open-report-insights-none-detail = As sugestões aparecem quando várias imagens abrem lentamente pelo mesmo motivo. Para as encontrar, { -app-name } guarda uma semana de aberturas lentas neste computador; nada é enviado para lado nenhum.
open-report-insights-software = As imagens { $format } são descodificadas na CPU
open-report-insights-storage = A leitura dos ficheiros atrasa as imagens
open-report-insights-archive = A descompressão de arquivos atrasa as imagens
open-report-insights-nothing-now = Nenhuma definição de { -app-name } altera isto.
open-report-insights-display-cache = Ative a cache de visualização: as imagens que já abriu passam a abrir a partir de uma cópia rápida neste computador.
open-report-insights-prefetch = Mantenha mais imagens vizinhas prontas, para que as seguintes sejam preparadas enquanto vê.
open-report-insights-hw-ssd = Uma unidade interna rápida (um SSD NVMe) para estes ficheiros
open-report-insights-action-turn-on = Ativar
open-report-insights-action-try-again = Tentar novamente
open-report-insights-action-automatic = Definir como Automático
open-report-insights-dismiss-storage = Não mostrar novamente para o armazenamento
open-report-insights-dismiss-decoder = Não mostrar novamente para a descodificação
open-report-insights-off = Desativar as sugestões
open-report-insights-on = Ativar as sugestões
open-report-insights-restore = Mostrar novamente as sugestões ocultas
open-report-insights-power = Este computador está a poupar energia (com bateria ou num modo de poupança de energia), o que torna mais lenta a descodificação na CPU. Ligado à corrente, as imagens podem abrir mais depressa.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Sugestão de desempenho: as miniaturas continuam a aparecer lentamente pelo mesmo motivo. Abra «Porque está lento?» para ver o que ajudaria.
open-report-insights-last-thumbnail = Última miniatura lenta: { $total } para ser criada
open-report-insights-thumbnails-software = As miniaturas das imagens { $format } são descodificadas na CPU
open-report-insights-thumbnails-storage = A leitura dos ficheiros atrasa as miniaturas
open-report-insights-thumbnails-archive = A descompressão de arquivos atrasa as miniaturas
open-report-insights-thumbnail-cache = Não é possível guardar miniaturas neste computador (a cache de miniaturas não está disponível), por isso são criadas de novo sempre que a pasta é apresentada.
open-report-advice-thumbnails-software = { $count } das últimas { $total } miniaturas { $format } lentas { $count ->
    [one] foi descodificada
   *[other] foram descodificadas
} por software por este motivo
open-report-advice-thumbnails-slow-storage = { $count } das últimas { $total } miniaturas lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela leitura do ficheiro (cerca de { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } das últimas { $total } miniaturas lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela leitura do ficheiro (ficheiros pequenos: sobretudo a aguardar que o armazenamento responda)
open-report-advice-thumbnails-archive = { $count } das últimas { $total } miniaturas lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela descompressão do arquivo
