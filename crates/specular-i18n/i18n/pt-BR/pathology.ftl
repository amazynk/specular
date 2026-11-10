# machine-drafted, needs native review
# Specular — Patologia: separação de corantes, exportação e importação de anotações, volumes de rótulos sobre volumes. Português (Brasil).

## Paleta
palette-stain-separation = Separar corantes
palette-stain-1 = Mostrar ou ocultar o corante 1
palette-stain-2 = Mostrar ou ocultar o corante 2
palette-stain-3 = Mostrar ou ocultar o corante 3
palette-stain-mode = Corantes como luz ou densidade óptica
palette-stain-estimate = Estimar corantes a partir da região
palette-stain-normalise = Normalizar as cores dos corantes
palette-stain-reference = Usar a região como referência de coloração
palette-stain-reset = Vetores de coloração publicados
palette-export-annotations = Exportar anotações…
palette-import-annotations = Importar anotações…
palette-label-volume = Sobrepor volume de rótulos…

## Motivos de desativação
disabled-not-colour-image = Requer uma imagem colorida
disabled-stains-off = Ative primeiro a separação de corantes
disabled-no-region = Desenhe primeiro uma região
disabled-no-slide-annotations = Não há anotações, regiões ou pontos para exportar
disabled-no-label-target = Abra um volume em vistas multiplanares, ou uma imagem médica

## Nomes dos corantes
stain-hematoxylin = Hematoxilina
stain-eosin = Eosina
stain-dab = DAB
stain-residual = Resíduo
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Avisos e legenda dos corantes
stain-off = Separação de corantes desativada
# { $set }: H&E, H-DAB ou H&E-DAB.
stain-on = Corantes: { $set }
stain-shown = { $stain } visível
stain-hidden = { $stain } não visível
stain-mode-light = Corantes como luz
stain-mode-od = Corantes como densidade óptica
# Título da legenda: “H&E · luz”, “H-DAB · densidade óptica”.
stain-legend-light = { $set } · luz
stain-legend-od = { $set } · densidade óptica
stain-legend-estimated = Estimados a partir da região
stain-legend-normalised = Normalizados
stain-legend-published = Vetores publicados
# { $pixels }: pixels corados usados.
stain-estimated = Corantes estimados a partir de { $pixels } pixels
stain-estimate-failed = Não é possível estimar os corantes: { $reason }
stain-estimate-background = a região é quase toda fundo
stain-estimate-one-colour = a região tem uma só cor
stain-estimate-three = a estimativa requer H&E ou H-DAB
stain-estimate-no-pixels = os pixels da região ainda não foram carregados
stain-reference-set = Região definida como referência de coloração
stain-normalise-on = Cores dos corantes normalizadas
stain-normalise-off = Cores dos corantes como digitalizadas
stain-reset-done = Vetores de coloração publicados

## Diálogo de exportação de anotações
annot-export-title = Exportar anotações
annot-export-format = Formato
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Máscaras de rótulos
annot-export-output = Máscaras como
annot-export-output-png = Blocos PNG
annot-export-output-tiff = Blocos TIFF
annot-export-output-ome = Pirâmide OME-TIFF
annot-export-region = Região
annot-export-region-slide = Lâmina inteira
annot-export-region-view = Área visível
annot-export-region-drawn = Última região desenhada
annot-export-level = Nível
# { $level }: 0 para a resolução total; { $width } × { $height } seu tamanho em pixels.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Tamanho do bloco
annot-export-empty = Blocos vazios
annot-export-empty-skip = Pular
annot-export-empty-keep = Gravar
annot-export-what = { $count ->
    [one] { $count } forma
    [many] { $count } de formas
   *[other] { $count } formas
}, { $classes ->
    [one] { $classes } classe
    [many] { $classes } de classes
   *[other] { $classes } classes
}
annot-export-lines = As linhas ficam fora do COCO e das máscaras.
annot-export-export = Exportar…
annot-export-writing = Gravando…
# { $files }: arquivos gravados.
annot-export-done = { $files ->
    [one] { $files } arquivo exportado
    [many] { $files } de arquivos exportados
   *[other] { $files } arquivos exportados
}
annot-export-failed = Falha na exportação: { $reason }
annot-import-done = { $count ->
    [one] { $count } anotação importada
    [many] { $count } de anotações importadas
   *[other] { $count } anotações importadas
}
annot-import-failed = Não é possível importar: { $reason }
annot-import-skipped = { $count ->
    [one] { $count } forma não tinha contorno utilizável
    [many] { $count } de formas não tinham contorno utilizável
   *[other] { $count } formas não tinham contorno utilizável
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Volumes de rótulos
label-volume-file = Volume de rótulos
label-volume-table = Tabela de cores
# { $file }: nome do arquivo do volume de rótulos; { $count }: seus rótulos.
label-volume-loaded = { $count ->
    [one] { $file }: { $count } rótulo
    [many] { $file }: { $count } de rótulos
   *[other] { $file }: { $count } rótulos
}
label-volume-failed = Não é possível sobrepor { $file }: { $reason }
label-volume-outside = { $file } não se sobrepõe a este volume
label-volume-not-whole = seus valores não são números inteiros
label-volume-colour = contém cores, não rótulos
label-volume-empty = não tem rótulos
label-volume-too-many = tem rótulos demais
label-volume-too-large = é grande demais para reamostrar
label-volume-degenerate = sua orientação é inválida
# Um rótulo sem nome na tabela de cores.
label-volume-unnamed = Rótulo { $value }
