# machine-drafted, needs native review
# Specular — Relatório de abertura: por que a imagem demorou o que demorou para abrir (painel de
# informações, seção “Desempenho”; DESIGN.md §12.7).

open-report-title = Desempenho

## Headlines
open-report-fast = Aberta em { $ms } ms
open-report-display-cache = Exibida a partir do cache de exibição
open-report-slow-read = Armazenamento lento: { $percent } do tempo foi gasto lendo o arquivo ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Armazenamento lento: { $percent } do tempo foi gasto lendo o arquivo (arquivo pequeno: sobretudo esperando o armazenamento responder)
open-report-archive-read = { $percent } do tempo foi gasto descompactando o arquivo compactado
open-report-startup = { $percent } do tempo foi gasto iniciando o { -app-name }
open-report-checked = Decodificada duas vezes: o primeiro resultado do decodificador de hardware foi comparado com o da CPU ({ $percent } do tempo)
open-report-hardware = Decodificada pelo decodificador de hardware, que levou { $percent } do tempo
open-report-decode = Imagem grande: decodificar { $mpx } megapixels levou { $percent } do tempo
open-report-upload = { $percent } do tempo foi gasto enviando a imagem à GPU
open-report-waiting = { $percent } do tempo foi gasto esperando outras tarefas terminarem
open-report-stage = { $stage } levou { $percent } do tempo
open-report-memory-short = Faltava memória, então o sistema pode ter precisado usar a troca (swap)

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Decodificada por software: o decodificador de hardware ainda estava sendo detectado
open-report-sw-no-device = Decodificada por software: { $api } não está disponível neste computador
open-report-sw-no-codec = Decodificada por software: { $api } não tem decodificador { $codec } nesta GPU
open-report-sw-blocked-mismatch = Decodificada por software: o decodificador de hardware foi desativado porque seu resultado não coincidiu com o da CPU
open-report-sw-blocked-error = Decodificada por software: o decodificador de hardware foi desativado após um erro
open-report-sw-blocked-lost = Decodificada por software: o decodificador de hardware foi desativado porque parou de responder
open-report-sw-failed = Decodificada por software: o decodificador de hardware falhou nesta imagem
open-report-sw-busy = Decodificada por software: o decodificador de hardware estava ocupado
open-report-sw-unsupported = Decodificada por software: o decodificador de hardware não aceita esta imagem
open-report-sw-unavailable = Decodificada por software: a decodificação por hardware fica desativada até o { -app-name } reiniciar
open-report-sw-cpu-only = Decodificada por software: o processamento está em “Somente a CPU”
open-report-sw-setting-off = Decodificada por software: a decodificação por hardware está desativada nas configurações
open-report-sw-unchecked = Decodificada por software: o decodificador de hardware ainda não foi verificado (ele é testado primeiro em uma imagem preparada em segundo plano)
open-report-sw-lower-power = Decodificada por software: na bateria, a CPU gasta menos energia para isso
open-report-sw-faster = Decodificada por software: mais rápido neste computador (cerca de { $cpu } ms, contra { $hw } ms no decodificador de hardware)
open-report-sw-scaled = Decodificada por software: o decodificador de hardware só decodifica imagens em tamanho total

## Hints
open-report-hint-copy-local = Copie o arquivo para um disco local para abri-lo mais rápido.
open-report-hint-extract = Extraia o arquivo compactado para uma pasta para abrir as páginas mais rápido.
open-report-hint-try-again = Para tentar o decodificador de hardware de novo: Configurações → Desempenho → Tentar novamente.
open-report-hint-automatic = Defina “Onde as imagens são processadas” como Automático em Configurações → Desempenho.
open-report-hint-turn-on = Defina “Decodificação por hardware” como Automático em Configurações → Desempenho.
open-report-hint-free-memory = Feche outros programas para liberar memória.
open-report-hint-once-only = Somente a primeira imagem é verificada assim.

## Rows
open-report-total = Até ficar nítida
open-report-first-pixels = Primeiros pixels
# machine-drafted, needs native review
open-report-whole = Imagem inteira exibida
open-report-stage-startup = Inicialização
open-report-stage-read = Leitura do arquivo
open-report-stage-parse = Leitura do cabeçalho
open-report-stage-colour = Preparação do gerenciamento de cores
open-report-stage-decode = Decodificação
open-report-stage-upload = Envio à GPU
open-report-stage-waiting = Espera por outras tarefas
open-report-decoder = Decodificada por
open-report-decoder-checked = { $engine }, conferido com a CPU
open-report-decoder-cache = Cache de exibição
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, em 1/{ $scale } do tamanho
open-report-decoder-scaled-full = { $engine }, em tamanho total, depois reduzida para 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, em 1/{ $scale } do tamanho, sem o índice de início rápido
open-report-colour-conversion = Cores convertidas por

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Recorrente
open-report-advice-change = Ajudaria
open-report-advice-gain = Ganho estimado
open-report-advice-software = { $count } das últimas { $total } imagens { $format } lentas { $count ->
    [one] foi decodificada
   *[other] foram decodificadas
} por software por esse motivo
open-report-advice-slow-storage = { $count } das últimas { $total } imagens lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela leitura do arquivo (cerca de { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } das últimas { $total } imagens lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela leitura do arquivo (arquivos pequenos: sobretudo esperando o armazenamento responder)
open-report-advice-archive = { $count } das últimas { $total } imagens lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela descompactação do arquivo compactado
open-report-advice-hardware-decoder = Uma GPU com decodificador de hardware { $codec }
open-report-advice-gain-range = Cerca de { $low } a { $high } vezes mais rápido
open-report-advice-gain-one = Cerca de { $low } vezes mais rápido

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Dicas de desempenho
open-report-insights-indicator = Por que está lento?
open-report-insights-announce = Dica de desempenho: as imagens continuam abrindo devagar pelo mesmo motivo. Abra “Por que está lento?” para ver o que ajudaria.
open-report-insights-hide = Ocultar
open-report-insights-last = Última imagem lenta: { $total } até ficar nítida
open-report-insights-now = O que você pode fazer agora
open-report-insights-hardware = Hardware que ajudaria
open-report-insights-none = Nenhuma lentidão recorrente
open-report-insights-none-detail = As dicas aparecem quando várias imagens abrem devagar pelo mesmo motivo. Para encontrá-las, { -app-name } guarda uma semana de aberturas lentas neste computador; nada é enviado a lugar nenhum.
open-report-insights-software = Imagens { $format } são decodificadas na CPU
open-report-insights-storage = A leitura dos arquivos atrasa as imagens
open-report-insights-archive = A descompactação de arquivos compactados atrasa as imagens
open-report-insights-nothing-now = Nenhuma configuração de { -app-name } muda isso.
open-report-insights-display-cache = Ative o cache de exibição: as imagens que você já abriu passam a abrir de uma cópia rápida neste computador.
open-report-insights-prefetch = Mantenha mais imagens vizinhas prontas, para que as próximas sejam preparadas enquanto você olha.
open-report-insights-hw-ssd = Uma unidade interna rápida (um SSD NVMe) para estes arquivos
open-report-insights-action-turn-on = Ativar
open-report-insights-action-try-again = Tentar novamente
open-report-insights-action-automatic = Definir como Automático
open-report-insights-dismiss-storage = Não mostrar de novo para armazenamento
open-report-insights-dismiss-decoder = Não mostrar de novo para decodificação
open-report-insights-off = Desativar as dicas
open-report-insights-on = Ativar as dicas
open-report-insights-restore = Mostrar novamente as dicas ocultas
open-report-insights-power = Este computador está economizando energia (na bateria ou em um modo de economia de energia), o que deixa a decodificação na CPU mais lenta. Ligado na tomada, as imagens podem abrir mais rápido.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Dica de desempenho: as miniaturas continuam aparecendo devagar pelo mesmo motivo. Abra “Por que está lento?” para ver o que ajudaria.
open-report-insights-last-thumbnail = Última miniatura lenta: { $total } para ser criada
open-report-insights-thumbnails-software = Miniaturas de imagens { $format } são decodificadas na CPU
open-report-insights-thumbnails-storage = A leitura dos arquivos atrasa as miniaturas
open-report-insights-thumbnails-archive = A descompactação de arquivos compactados atrasa as miniaturas
open-report-insights-thumbnail-cache = Não é possível guardar miniaturas neste computador (o cache de miniaturas não está disponível), então elas são criadas de novo sempre que a pasta é exibida.
open-report-advice-thumbnails-software = { $count } das últimas { $total } miniaturas { $format } lentas { $count ->
    [one] foi decodificada
   *[other] foram decodificadas
} por software por esse motivo
open-report-advice-thumbnails-slow-storage = { $count } das últimas { $total } miniaturas lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela leitura do arquivo (cerca de { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } das últimas { $total } miniaturas lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela leitura do arquivo (arquivos pequenos: sobretudo esperando o armazenamento responder)
open-report-advice-thumbnails-archive = { $count } das últimas { $total } miniaturas lentas { $count ->
    [one] foi atrasada
   *[other] foram atrasadas
} pela descompactação do arquivo compactado
