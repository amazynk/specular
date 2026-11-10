# Specular — Herramienta de medición (longitudes, ángulos, regiones y sus estadísticas en cualquier imagen). Traducción automática, pendiente de revisión nativa.

palette-measure = Medir
palette-measure-copy-csv = Copiar mediciones como CSV
palette-measure-export = Exportar mediciones…
toast-copied-measurements = Mediciones copiadas como CSV

measure-tool-title = Medir
measure-tool-list = Mediciones
measure-tool-count = { $count ->
    [0] Sin mediciones
    [one] Una medición
   *[other] { $count } mediciones
}
measure-tool-empty = Elija una herramienta y dibuje sobre la imagen.

measure-tool-point = Punto
measure-tool-line = Línea
measure-tool-polyline = Polilínea
measure-tool-angle = Ángulo
measure-tool-rect = Rectángulo
measure-tool-ellipse = Elipse
measure-tool-polygon = Polígono
measure-tool-freehand = A mano alzada
measure-tool-calibrate = Calibrar
# machine-drafted, needs native review
measure-tool-spot = Punto

measure-tool-undo = Deshacer
measure-tool-delete = Eliminar
measure-tool-copy = Copiar CSV
measure-tool-export = Exportar…
measure-tool-import = Importar…
measure-tool-done = Hecho
measure-tool-unit = Unidad: { $unit }
measure-tool-apply = Aplicar

measure-tool-hint-line = Arrastre o haga clic en dos puntos. Las flechas mueven la cruz, Espacio coloca un punto.
measure-tool-hint-angle = Haga clic en tres puntos; el segundo es el vértice.
measure-tool-hint-rect = Arrastre o haga clic en dos esquinas.
measure-tool-hint-ellipse = Arrastre o haga clic en dos esquinas de su recuadro.
measure-tool-hint-polygon = Haga clic en las esquinas; Intro o un clic en la primera esquina lo cierra.
measure-tool-hint-calibrate = Trace una línea sobre algo de longitud conocida.
# machine-drafted, needs native review
measure-tool-hint-spot = Haz clic en un punto para leer su temperatura.
measure-tool-known-length = Longitud conocida: { $value } { $unit } (escriba el número, U cambia la unidad, Intro aplica)
measure-tool-calibrated = Calibrado: un píxel mide { $size } { $unit }
measure-tool-bad-length = Escriba una longitud mayor que cero.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Área { $value }
measure-tool-mean = Media { $mean } ± { $sd }
measure-tool-means = Media { $values }
measure-tool-range = Rango { $min } – { $max } en { $count } px
measure-tool-pixels = En { $count } px
measure-tool-stats-pending = Leyendo píxeles…
measure-tool-stats-none = No hay valores de píxel aquí
measure-tool-approximate = De una copia reducida (aproximado)

measure-tool-deleted = Medición eliminada
measure-tool-undone = Deshecho
measure-tool-nothing = No hay mediciones en esta imagen
measure-tool-exported = Mediciones guardadas en { $name }
measure-tool-export-failed = No se pudieron guardar las mediciones: { $reason }
measure-tool-imported = { $count ->
    [one] Una medición importada.
   *[other] { $count } mediciones importadas.
}
measure-tool-skipped = { $count ->
    [one] No se pudo leer una forma.
   *[other] No se pudieron leer { $count } formas.
}
measure-tool-import-failed = No se pudieron leer las mediciones: { $reason }
measure-tool-too-large = El archivo es demasiado grande.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR necesita una imagen DICOM. Guarde como CSV o GeoJSON.

measure-source-uncalibrated = Píxeles (sin calibrar)
measure-source-dicom-pixel-spacing = Espaciado de píxeles DICOM
measure-source-dicom-calibrated = Espaciado calibrado DICOM
measure-source-dicom-imager-pixel-spacing = Espaciado DICOM en el detector
measure-source-dicom-magnification = Espaciado DICOM corregido por magnificación
measure-source-dicom-ultrasound-region = Región de ecografía DICOM
measure-source-tiff-resolution = Resolución TIFF
measure-source-exif-resolution = Resolución EXIF
measure-source-ome-physical-size = Tamaño físico OME
measure-source-fits-wcs = Coordenadas celestes FITS
measure-source-vendor-tag = Metadatos del fabricante
measure-source-drawing-units = Unidades del dibujo
# machine-drafted, needs native review
measure-source-drawing-scale = De la escala del dibujo
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Del dibujo (escala { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Del dibujo (escala { $scale }, vista { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Escala del mapa en el centro (aproximada; las herramientas de mapas miden sobre la Tierra)
measure-source-user = Su calibración
