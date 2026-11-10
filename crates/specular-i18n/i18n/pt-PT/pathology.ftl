# machine-drafted, needs native review
# Specular — Patologia: separação de corantes, exportação e importação de anotações, volumes de etiquetas sobre volumes. Português (Portugal).

## Paleta
palette-stain-separation = Separar corantes
palette-stain-1 = Mostrar ou ocultar o corante 1
palette-stain-2 = Mostrar ou ocultar o corante 2
palette-stain-3 = Mostrar ou ocultar o corante 3
palette-stain-mode = Corantes como luz ou densidade ótica
palette-stain-estimate = Estimar corantes a partir da região
palette-stain-normalise = Normalizar as cores dos corantes
palette-stain-reference = Usar a região como referência de coloração
palette-stain-reset = Vetores de coloração publicados
palette-export-annotations = Exportar anotações…
palette-import-annotations = Importar anotações…
palette-label-volume = Sobrepor volume de etiquetas…

## Motivos de desativação
disabled-not-colour-image = Requer uma imagem a cores
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
stain-mode-od = Corantes como densidade ótica
# Título da legenda: «H&E · luz», «H-DAB · densidade ótica».
stain-legend-light = { $set } · luz
stain-legend-od = { $set } · densidade ótica
stain-legend-estimated = Estimados a partir da região
stain-legend-normalised = Normalizados
stain-legend-published = Vetores publicados
# { $pixels }: píxeis corados usados.
stain-estimated = Corantes estimados a partir de { $pixels } píxeis
stain-estimate-failed = Não é possível estimar os corantes: { $reason }
stain-estimate-background = a região é quase toda fundo
stain-estimate-one-colour = a região tem uma só cor
stain-estimate-three = a estimativa requer H&E ou H-DAB
stain-estimate-no-pixels = os píxeis da região ainda não foram carregados
stain-reference-set = Região definida como referência de coloração
stain-normalise-on = Cores dos corantes normalizadas
stain-normalise-off = Cores dos corantes tal como digitalizadas
stain-reset-done = Vetores de coloração publicados

## Diálogo de exportação de anotações
annot-export-title = Exportar anotações
annot-export-format = Formato
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Máscaras de etiquetas
annot-export-output = Máscaras como
annot-export-output-png = Mosaicos PNG
annot-export-output-tiff = Mosaicos TIFF
annot-export-output-ome = Pirâmide OME-TIFF
annot-export-region = Região
annot-export-region-slide = Lâmina inteira
annot-export-region-view = Área visível
annot-export-region-drawn = Última região desenhada
annot-export-level = Nível
# { $level }: 0 para a resolução total; { $width } × { $height } o seu tamanho em píxeis.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Tamanho do mosaico
annot-export-empty = Mosaicos vazios
annot-export-empty-skip = Saltar
annot-export-empty-keep = Escrever
annot-export-what = { $count ->
    [one] 1 forma
    [many] { $count } de formas
   *[other] { $count } formas
}, { $classes ->
    [one] 1 classe
    [many] { $classes } de classes
   *[other] { $classes } classes
}
annot-export-lines = As linhas ficam fora do COCO e das máscaras.
annot-export-export = Exportar…
annot-export-writing = A escrever…
# { $files }: ficheiros escritos.
annot-export-done = { $files ->
    [one] 1 ficheiro exportado
    [many] { $files } de ficheiros exportados
   *[other] { $files } ficheiros exportados
}
annot-export-failed = A exportação falhou: { $reason }
annot-import-done = { $count ->
    [one] 1 anotação importada
    [many] { $count } de anotações importadas
   *[other] { $count } anotações importadas
}
annot-import-failed = Não é possível importar: { $reason }
annot-import-skipped = { $count ->
    [one] 1 forma não tinha contorno utilizável
    [many] { $count } de formas não tinham contorno utilizável
   *[other] { $count } formas não tinham contorno utilizável
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Volumes de etiquetas
label-volume-file = Volume de etiquetas
label-volume-table = Tabela de cores
# { $file }: nome do ficheiro do volume de etiquetas; { $count }: as suas etiquetas.
label-volume-loaded = { $count ->
    [one] { $file }: 1 etiqueta
    [many] { $file }: { $count } de etiquetas
   *[other] { $file }: { $count } etiquetas
}
label-volume-failed = Não é possível sobrepor { $file }: { $reason }
label-volume-outside = { $file } não se sobrepõe a este volume
label-volume-not-whole = os seus valores não são números inteiros
label-volume-colour = contém cores, não etiquetas
label-volume-empty = não tem etiquetas
label-volume-too-many = tem demasiadas etiquetas
label-volume-too-large = é demasiado grande para reamostrar
label-volume-degenerate = a sua orientação é inválida
# Uma etiqueta sem nome na tabela de cores.
label-volume-unnamed = Etiqueta { $value }
