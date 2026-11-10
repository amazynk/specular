# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Mostrar tags DICOM
palette-next-series = Próxima série
palette-prev-series = Série anterior
disabled-not-dicom = A imagem não é um arquivo DICOM
disabled-no-other-series = Não há outra série nesta pasta
dicom-tags-title = Tags DICOM
dicom-tags-title-file = Tags DICOM: { $file }
dicom-tags-filter = Filtrar tags
dicom-tags-show-patient = Mostrar dados do paciente
dicom-tags-col-tag = Tag
dicom-tags-col-keyword = Palavra-chave
dicom-tags-col-vr = VR
dicom-tags-col-length = Comprimento
dicom-tags-col-value = Valor
dicom-tags-hidden = Oculto
dicom-tags-hidden-count = { $count ->
    [one] { $count } valor do paciente oculto
   *[other] { $count } valores do paciente ocultos
}
dicom-tags-no-match = Nenhuma tag corresponde
dicom-tags-reading = Lendo tags…
dicom-tags-error = Não foi possível ler as tags: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } item
   *[other] { $count } itens
}
dicom-tags-fragments = { $count ->
    [one] { $count } fragmento
   *[other] { $count } fragmentos
}
dicom-tags-item = Item { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } bytes
}
dicom-tags-empty = (vazio)
dicom-tags-omitted = { $count ->
    [one] Mais { $count } elemento não listado
   *[other] Mais { $count } elementos não listados
}
dicom-series-group = Série
dicom-series-slice = Corte
dicom-series-slice-value = { $index } de { $count }
dicom-series-frames = { $frames ->
        [one] { $frames } quadro
       *[other] { $frames } quadros
    }
dicom-series-location = Posição do corte
dicom-series-mm = { $mm } mm
dicom-series-spacing = Espaçamento dos cortes
dicom-series-which = Série
dicom-series-which-value = { $index } de { $count }
dicom-series-order = Ordem
dicom-series-order-position = Por posição
dicom-series-order-instance = Por número da instância
dicom-series-order-time = Por hora de aquisição
dicom-series-order-name = Por nome do arquivo
dicom-series-gaps = Cortes ausentes
dicom-series-gaps-value = { $count ->
    [one] { $count } corte ausente
   *[other] { $count } cortes ausentes
}
dicom-series-gaps-where = Após o corte { $slices }
dicom-series-irregular = Espaçamento irregular
dicom-series-tilt = Inclinação do gantry
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Duplicatas ignoradas
dicom-series-toast = { $series ->
    [one] Série – cortes: { $count }
   *[other] Série { $index } de { $series } – cortes: { $count }
}
dicom-series-capped = Só os primeiros { $count } arquivos da pasta foram lidos

studies-title = Estudos
studies-title-medium = Estudos: { $medium }
studies-show-patient = Mostrar dados do paciente
studies-patient-hidden = Oculto
studies-patient-hidden-note = Nomes e datas de nascimento dos pacientes estão ocultos
studies-none = Nenhum estudo encontrado
studies-reading = Lendo o disco…
studies-reading-count = Lendo o disco: { $done } de { $total ->
        [one] { $total } arquivo
       *[other] { $total } arquivos
    }
studies-opening = Abrindo a série…
studies-error = Não foi possível ler o disco: { $reason }
studies-missing = { $count ->
    [one] Falta { $count } arquivo listado no disco
   *[other] Faltam { $count } arquivos listados no disco
}
studies-capped = Somente os primeiros { $count } arquivos foram lidos
# machine-drafted, needs native review
studies-stopped = A leitura parou após { $done } de { $total ->
        [one] { $total } arquivo
       *[other] { $total } arquivos
    }: a lista está incompleta
studies-stopped-early = A leitura parou: a lista está incompleta
studies-cancel-description = Para de ler o disco. O que já foi lido continua na lista.
studies-patient-n = Paciente { $index }
studies-patient-unknown = Paciente desconhecido
studies-born = Nascimento: { $date }
studies-no-description = Sem descrição
studies-series = Série { $number }: { $description }
studies-series-number = Série { $number }
studies-images = { $count ->
    [one] { $count } imagem
   *[other] { $count } imagens
}
studies-images-missing = { $count } ausentes
studies-others = { $count ->
    [one] { $count } outro objeto
   *[other] { $count } outros objetos
}
studies-no-images = Esta série não tem imagens no disco
studies-medium-inserted = Foi inserido um disco com estudos médicos
studies-medium-show = Mostrar estudos
palette-studies = Mostrar os estudos deste disco
