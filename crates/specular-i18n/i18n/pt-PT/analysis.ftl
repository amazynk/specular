# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Português (Portugal). machine-drafted, needs native review

palette-segments = Mostrar ou ocultar segmentações
palette-segment-opacity = Opacidade seguinte do preenchimento da segmentação
palette-fusion = Fundir as séries do mesmo referencial
palette-fusion-colormap = Mapa de cores seguinte da fusão
palette-fusion-blend = Opacidade seguinte da fusão
palette-fusion-nudge = Mover a série fundida à mão
palette-tmtv = Ferramenta TMTV (esfera, caixa, desligada)
palette-tmtv-threshold = Limiar TMTV (SUV { NUMBER(2.5) } ou { PERCENT(41) } do SUVmax)
palette-time-curve = Curva tempo-intensidade da última região
palette-export-curve = Guardar a curva tempo-intensidade como CSV

analysis-searching = Ainda à procura de segmentações e séries para fundir
segments-none = Nenhuma segmentação deste referencial foi encontrada
segments-shown = Segmentações mostradas
segments-hidden = Segmentações ocultas
segments-fill = Preenchimento da segmentação { PERCENT($percent) }
segments-failed = Segmentação não mostrada: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = Fator SUVbw { $factor }, { $minutes } min após a injeção
suv-refused = Sem SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = não é uma imagem PET
suv-refused-units = as unidades dos píxeis não são Bq/mL
suv-refused-corrected = sem correção de atenuação e decaimento
suv-refused-decay = correção de decaimento não suportada
suv-refused-weight = sem peso do paciente
suv-refused-dose = sem dose injetada
suv-refused-half-life = sem meia-vida do radionuclídeo
suv-refused-injection = sem hora da injeção
suv-refused-scan-time = sem hora da aquisição
suv-refused-delay = a injeção é posterior ao exame
suv-refused-scale = sem fator de escala SUV da Philips
suv-refused-height = sem altura do paciente
suv-refused-sex = sem sexo do paciente
suv-refused-lbm = massa magra fora do intervalo

colormap-hot-iron = Ferro em brasa
colormap-rainbow = Arco-íris
colormap-viridis = Viridis
fusion-none = Nenhuma série PET, SPECT, CT ou MR deste referencial para fundir
fusion-needed = Funda uma série primeiro (Shift+U)
fusion-opening = A fundir { $modality }
fusion-off = Fusão desligada
fusion-refused = Não é possível fundir: { $reason }
fusion-on = { $modality } fundida: { $colormap }, { PERCENT($blend) }
fusion-hidden = { $modality } fundida oculta
fusion-window-suv = Janela da fusão SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Opacidade da fusão { PERCENT($percent) }
fusion-moved = Movida à mão: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = As setas movem a série fundida 1 mm, Shift+Up/Down ao longo da vista, [ e ] rodam-na
fusion-nudge-off = Série fundida fixada

tmtv-hint-sphere = TMTV: arraste uma esfera a partir do centro
tmtv-hint-box = TMTV: arraste uma caixa
tmtv-off = TMTV desligado
tmtv-threshold-suv = Limiar SUV { $suv }
tmtv-threshold-share = Limiar { PERCENT($share) } do SUVmax
tmtv-result = TMTV { $ml } mL em { $count ->
        [one] { $count } região
       *[other] { $count } regiões
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = A região está fora do volume
tmtv-no-pet = O TMTV precisa de um volume PET com SUV

analysis-mpr-only = Fusão e TMTV funcionam em MPR (Shift+R)
curve-not-dynamic = Não é uma imagem multi-imagem nem uma série dinâmica
curve-not-mpr = As curvas tempo-intensidade são traçadas na imagem, fora do MPR
curve-no-roi = Desenhe primeiro uma elipse ou um retângulo
curve-loading = { $count ->
        [one] A ler { $count } ponto no tempo…
       *[other] A ler { $count } pontos no tempo…
    }
curve-title = Curva tempo-intensidade
curve-mean = Média
curve-max = Máx.
curve-seconds = s
curve-saved = Curva guardada
curve-none = Nenhuma curva para guardar
curve-failed = Curva não criada: { $reason }
