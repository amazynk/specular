# Specular — Ferramenta de medição (comprimentos, ângulos, regiões e respetivas estatísticas em qualquer imagem). Rascunho automático, requer revisão nativa.

palette-measure = Medir
palette-measure-copy-csv = Copiar medições como CSV
palette-measure-export = Exportar medições…
toast-copied-measurements = Medições copiadas como CSV

measure-tool-title = Medir
measure-tool-list = Medições
measure-tool-count = { $count ->
    [0] Nenhuma medição
    [one] Uma medição
   *[other] { $count } medições
}
measure-tool-empty = Escolha uma ferramenta e desenhe na imagem.

measure-tool-point = Ponto
measure-tool-line = Linha
measure-tool-polyline = Polilinha
measure-tool-angle = Ângulo
measure-tool-rect = Retângulo
measure-tool-ellipse = Elipse
measure-tool-polygon = Polígono
measure-tool-freehand = À mão livre
measure-tool-calibrate = Calibrar
# machine-drafted, needs native review
measure-tool-spot = Ponto

measure-tool-undo = Anular
measure-tool-delete = Eliminar
measure-tool-copy = Copiar CSV
measure-tool-export = Exportar…
measure-tool-import = Importar…
measure-tool-done = Concluído
measure-tool-unit = Unidade: { $unit }
measure-tool-apply = Aplicar

measure-tool-hint-line = Arraste ou clique em dois pontos. As setas movem a mira, Espaço marca um ponto.
measure-tool-hint-angle = Clique em três pontos; o segundo é o vértice.
measure-tool-hint-rect = Arraste ou clique em dois cantos.
measure-tool-hint-ellipse = Arraste ou clique em dois cantos da caixa.
measure-tool-hint-polygon = Clique nos cantos; Enter ou um clique no primeiro canto fecha a forma.
measure-tool-hint-calibrate = Desenhe uma linha sobre algo de comprimento conhecido.
# machine-drafted, needs native review
measure-tool-hint-spot = Clique num ponto para ler a sua temperatura.
measure-tool-known-length = Comprimento conhecido: { $value } { $unit } (escreva o número, U muda a unidade, Enter aplica)
measure-tool-calibrated = Calibrado: um píxel mede { $size } { $unit }
measure-tool-bad-length = Escreva um comprimento maior que zero.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Área { $value }
measure-tool-mean = Média { $mean } ± { $sd }
measure-tool-means = Média { $values }
measure-tool-range = Intervalo { $min } – { $max } em { $count } px
measure-tool-pixels = Em { $count } px
measure-tool-stats-pending = A ler píxeis…
measure-tool-stats-none = Sem valores de píxel aqui
measure-tool-approximate = De uma cópia reduzida (aproximado)

measure-tool-deleted = Medição eliminada
measure-tool-undone = Anulado
measure-tool-nothing = Nenhuma medição nesta imagem
measure-tool-exported = Medições guardadas em { $name }
measure-tool-export-failed = Não foi possível guardar as medições: { $reason }
measure-tool-imported = { $count ->
    [one] Uma medição importada.
   *[other] { $count } medições importadas.
}
measure-tool-skipped = { $count ->
    [one] Não foi possível ler uma forma.
   *[other] Não foi possível ler { $count } formas.
}
measure-tool-import-failed = Não foi possível ler as medições: { $reason }
measure-tool-too-large = O ficheiro é demasiado grande.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR precisa de uma imagem DICOM. Guarde como CSV ou GeoJSON.

measure-source-uncalibrated = Píxeis (não calibrado)
measure-source-dicom-pixel-spacing = Espaçamento de píxeis DICOM
measure-source-dicom-calibrated = Espaçamento calibrado DICOM
measure-source-dicom-imager-pixel-spacing = Espaçamento DICOM no detetor
measure-source-dicom-magnification = Espaçamento DICOM corrigido da ampliação
measure-source-dicom-ultrasound-region = Região de ecografia DICOM
measure-source-tiff-resolution = Resolução TIFF
measure-source-exif-resolution = Resolução EXIF
measure-source-ome-physical-size = Tamanho físico OME
measure-source-fits-wcs = Coordenadas celestes FITS
measure-source-vendor-tag = Metadados do fabricante
measure-source-drawing-units = Unidades do desenho
# machine-drafted, needs native review
measure-source-drawing-scale = Da escala do desenho
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Do desenho (escala { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Do desenho (escala { $scale }, viewport { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Escala do mapa no centro (aproximada; Mapas mede sobre a Terra)
measure-source-user = A sua calibração
