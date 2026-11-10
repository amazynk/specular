# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Sobrepor regiões DS9…
palette-science-load-regions-help = Desenhar na imagem as formas de um ficheiro de regiões DS9 (.reg).
palette-science-export-regions = Guardar regiões como DS9…
palette-science-export-regions-help = Guardar as medições e as regiões sobrepostas como ficheiro de regiões DS9.
palette-science-edit-regions = Editar regiões como medições
palette-science-edit-regions-help = Transformar as regiões sobrepostas em medições que pode mover e alterar.
palette-science-clear-regions = Remover as regiões sobrepostas
palette-science-clear-regions-help = Remover as regiões DS9 da imagem.
palette-science-plate-solve = Resolução astrométrica
palette-science-plate-solve-help = Encontrar as coordenadas celestes da imagem com ASTAP ou astrometry.net. Executar de novo para parar.
palette-science-save-plate-solution = Guardar a solução astrométrica
palette-science-save-plate-solution-help = Guardar as coordenadas celestes encontradas junto à imagem, como ficheiro .wcs.
palette-science-sky-survey = Levantamento do céu como fundo
palette-science-sky-survey-help = Mostrar o levantamento do céu seguinte à volta da imagem, ou nenhum.
palette-science-samp-send-image = Enviar imagem para aplicações SAMP
palette-science-samp-send-image-help = Abrir esta imagem no DS9, no Aladin e nas outras aplicações do hub SAMP.
palette-science-samp-send-position = Enviar posição para aplicações SAMP
palette-science-samp-send-position-help = Apontar as outras aplicações para a posição celeste sob o ponteiro.

disabled-samp-off = Ligue-se primeiro a um hub SAMP (Definições › Integração).

settings-integration-samp = Ligar a um hub SAMP
settings-integration-samp-help = Trocar imagens, tabelas e posições celestes com DS9, TOPCAT, Aladin e outras aplicações de astronomia. Só é usado um hub neste computador.
settings-integration-astap-path = Programa ASTAP
settings-integration-astap-path-help = O resolvedor astrométrico ASTAP, quando não é encontrado automaticamente.
settings-integration-solve-field-path = Programa astrometry.net
settings-integration-solve-field-path-help = O programa solve-field do astrometry.net, quando não é encontrado automaticamente.

astro-regions-filter = Regiões DS9
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } região.
       *[other] { $count } regiões.
    }
astro-regions-refused = { $count ->
        [one] { $count } linha não lida.
       *[other] { $count } linhas não lidas.
    } Primeira: linha { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda desenhado pelos rebordos interior e exterior.
       *[other] { $count } pandas desenhados pelos rebordos interior e exterior.
    }
astro-regions-none-here = { $name }: nenhuma região fica nesta imagem.
astro-regions-failed = Não foi possível ler { $name }: { $reason }
astro-regions-cleared = Regiões removidas.
astro-regions-nothing = Nenhuma região ou medição para guardar.
astro-regions-saved = { $name } guardado.
astro-regions-skipped = { $count ->
        [one] { $count } forma fora das coordenadas celestes foi omitida.
       *[other] { $count } formas fora das coordenadas celestes foram omitidas.
    }
astro-regions-save-failed = Não foi possível guardar as regiões: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } região é agora uma medição.
       *[other] { $count } regiões são agora medições.
    }
astro-regions-no-wcs = As regiões celestes precisam de uma imagem com coordenadas celestes.

astro-solve-no-solver = Nenhum resolvedor astrométrico encontrado. Instale o ASTAP ou o astrometry.net, ou escolha um em Definições › Integração.
astro-solve-running = A resolver com { $solver }… { $seconds } s
astro-solve-stop = Parar
astro-solve-stopped = Resolução astrométrica parada.
astro-solve-done = Resolvido: { $ra }, { $dec }, { $scale } por píxel.
astro-solve-failed = A resolução astrométrica falhou: { $reason }
astro-solve-not-local = Só é possível resolver imagens de uma pasta neste computador.
astro-solve-none = Ainda não há solução astrométrica para esta imagem.
astro-solve-saved = { $name } guardado. A imagem em si não foi alterada.
astro-solve-sidecar = Coordenadas celestes de { $name }.

astro-samp-connected = Ligado ao hub SAMP.
astro-samp-no-hub = SAMP: { $reason }. Nova tentativa quando um hub arrancar.
astro-samp-lost = O hub SAMP parou. Nova tentativa quando um arrancar.
astro-samp-sent = { $count ->
        [one] Enviado para { $count } aplicação.
       *[other] Enviado para { $count } aplicações.
    }
astro-samp-nobody = Nenhuma aplicação do hub escuta isto.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } não está neste computador, por isso não é aberto.
astro-samp-pointed = Apontado para { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } não está nesta imagem.
astro-samp-no-table = SAMP: essa tabela não está sobreposta aqui.
astro-samp-rows = { $count ->
        [one] { $count } fonte do catálogo realçada.
       *[other] { $count } fontes do catálogo realçadas.
    }

astro-survey-off = Levantamento do céu desligado.
astro-survey-on = Levantamento do céu: { $name }. { $credit }
astro-survey-failed = Não é possível mostrar { $name }: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Mosaicos do levantamento do céu de { $host }
