# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Mostrar etiquetas DICOM
palette-next-series = Série seguinte
palette-prev-series = Série anterior
disabled-not-dicom = A imagem não é um ficheiro DICOM
disabled-no-other-series = Não há outra série nesta pasta
dicom-tags-title = Etiquetas DICOM
dicom-tags-title-file = Etiquetas DICOM: { $file }
dicom-tags-filter = Filtrar etiquetas
dicom-tags-show-patient = Mostrar dados do doente
dicom-tags-col-tag = Etiqueta
dicom-tags-col-keyword = Palavra-chave
dicom-tags-col-vr = VR
dicom-tags-col-length = Comprimento
dicom-tags-col-value = Valor
dicom-tags-hidden = Oculto
dicom-tags-hidden-count = { $count ->
    [one] { $count } valor do doente oculto
   *[other] { $count } valores do doente ocultos
}
dicom-tags-no-match = Nenhuma etiqueta corresponde
dicom-tags-reading = A ler etiquetas…
dicom-tags-error = Não foi possível ler as etiquetas: { $reason }
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
        [one] { $frames } fotograma
       *[other] { $frames } fotogramas
    }
dicom-series-location = Posição do corte
dicom-series-mm = { $mm } mm
dicom-series-spacing = Espaçamento dos cortes
dicom-series-which = Série
dicom-series-which-value = { $index } de { $count }
dicom-series-order = Ordem
dicom-series-order-position = Por posição
dicom-series-order-instance = Por número de instância
dicom-series-order-time = Por hora de aquisição
dicom-series-order-name = Por nome do ficheiro
dicom-series-gaps = Cortes em falta
dicom-series-gaps-value = { $count ->
    [one] { $count } corte em falta
   *[other] { $count } cortes em falta
}
dicom-series-gaps-where = Após o corte { $slices }
dicom-series-irregular = Espaçamento irregular
dicom-series-tilt = Inclinação da gantry
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Duplicados ignorados
dicom-series-toast = { $series ->
    [one] Série – cortes: { $count }
   *[other] Série { $index } de { $series } – cortes: { $count }
}
dicom-series-capped = Só foram lidos os primeiros { $count } ficheiros da pasta

studies-title = Estudos
studies-title-medium = Estudos: { $medium }
studies-show-patient = Mostrar dados do doente
studies-patient-hidden = Oculto
studies-patient-hidden-note = Os nomes e datas de nascimento dos doentes estão ocultos
studies-none = Nenhum estudo encontrado
studies-reading = A ler o disco…
studies-reading-count = A ler o disco: { $done } de { $total ->
        [one] { $total } ficheiro
       *[other] { $total } ficheiros
    }
studies-opening = A abrir a série…
studies-error = Não foi possível ler o disco: { $reason }
studies-missing = { $count ->
    [one] Falta { $count } ficheiro listado no disco
   *[other] Faltam { $count } ficheiros listados no disco
}
studies-capped = Apenas os primeiros { $count } ficheiros foram lidos
# machine-drafted, needs native review
studies-stopped = A leitura parou após { $done } de { $total ->
        [one] { $total } ficheiro
       *[other] { $total } ficheiros
    }: a lista está incompleta
studies-stopped-early = A leitura parou: a lista está incompleta
studies-cancel-description = Para de ler o disco. O que já foi lido continua na lista.
studies-patient-n = Doente { $index }
studies-patient-unknown = Doente desconhecido
studies-born = Nascimento: { $date }
studies-no-description = Sem descrição
studies-series = Série { $number }: { $description }
studies-series-number = Série { $number }
studies-images = { $count ->
    [one] { $count } imagem
   *[other] { $count } imagens
}
studies-images-missing = { $count } em falta
studies-others = { $count ->
    [one] { $count } outro objeto
   *[other] { $count } outros objetos
}
studies-no-images = Esta série não tem imagens no disco
studies-medium-inserted = Foi inserido um disco com estudos médicos
studies-medium-show = Mostrar estudos
palette-studies = Mostrar os estudos deste disco
