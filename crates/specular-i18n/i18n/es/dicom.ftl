# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Mostrar etiquetas DICOM
palette-next-series = Serie siguiente
palette-prev-series = Serie anterior
disabled-not-dicom = La imagen no es un archivo DICOM
disabled-no-other-series = No hay otra serie en esta carpeta
dicom-tags-title = Etiquetas DICOM
dicom-tags-title-file = Etiquetas DICOM: { $file }
dicom-tags-filter = Filtrar etiquetas
dicom-tags-show-patient = Mostrar datos del paciente
dicom-tags-col-tag = Etiqueta
dicom-tags-col-keyword = Palabra clave
dicom-tags-col-vr = VR
dicom-tags-col-length = Longitud
dicom-tags-col-value = Valor
dicom-tags-hidden = Oculto
dicom-tags-hidden-count = { $count ->
    [one] { $count } valor del paciente oculto
   *[other] { $count } valores del paciente ocultos
}
dicom-tags-no-match = Ninguna etiqueta coincide
dicom-tags-reading = Leyendo etiquetas…
dicom-tags-error = No se pudieron leer las etiquetas: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } elemento
   *[other] { $count } elementos
}
dicom-tags-fragments = { $count ->
    [one] { $count } fragmento
   *[other] { $count } fragmentos
}
dicom-tags-item = Elemento { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } bytes
}
dicom-tags-empty = (vacío)
dicom-tags-omitted = { $count ->
    [one] { $count } elemento más no listado
   *[other] { $count } elementos más no listados
}
dicom-series-group = Serie
dicom-series-slice = Corte
dicom-series-slice-value = { $index } de { $count }
dicom-series-frames = { $frames ->
        [one] { $frames } fotograma
       *[other] { $frames } fotogramas
    }
dicom-series-location = Posición del corte
dicom-series-mm = { $mm } mm
dicom-series-spacing = Espaciado de cortes
dicom-series-which = Serie
dicom-series-which-value = { $index } de { $count }
dicom-series-order = Orden
dicom-series-order-position = Por posición
dicom-series-order-instance = Por número de instancia
dicom-series-order-time = Por hora de adquisición
dicom-series-order-name = Por nombre de archivo
dicom-series-gaps = Cortes que faltan
dicom-series-gaps-value = { $count ->
    [one] Falta { $count } corte
   *[other] Faltan { $count } cortes
}
dicom-series-gaps-where = Tras el corte { $slices }
dicom-series-irregular = Espaciado irregular
dicom-series-tilt = Inclinación del gantry
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Duplicados omitidos
dicom-series-toast = { $series ->
    [one] Serie – cortes: { $count }
   *[other] Serie { $index } de { $series } – cortes: { $count }
}
dicom-series-capped = Solo se leyeron los primeros { $count } archivos de la carpeta

studies-title = Estudios
studies-title-medium = Estudios: { $medium }
studies-show-patient = Mostrar datos del paciente
studies-patient-hidden = Oculto
studies-patient-hidden-note = Los nombres y fechas de nacimiento de los pacientes están ocultos
studies-none = No se encontraron estudios
studies-reading = Leyendo el disco…
studies-reading-count = Leyendo el disco: { $done } de { $total ->
        [one] { $total } archivo
       *[other] { $total } archivos
    }
studies-opening = Abriendo la serie…
studies-error = No se pudo leer el disco: { $reason }
studies-missing = { $count ->
    [one] Falta { $count } archivo listado en el disco
   *[other] Faltan { $count } archivos listados en el disco
}
studies-capped = Solo se leyeron los primeros { $count } archivos
# machine-drafted, needs native review
studies-stopped = Lectura detenida tras { $done } de { $total ->
        [one] { $total } archivo
       *[other] { $total } archivos
    }: la lista está incompleta
studies-stopped-early = Lectura detenida: la lista está incompleta
studies-cancel-description = Detiene la lectura del disco. Lo leído hasta ahora sigue en la lista.
studies-patient-n = Paciente { $index }
studies-patient-unknown = Paciente desconocido
studies-born = Nacimiento: { $date }
studies-no-description = Sin descripción
studies-series = Serie { $number }: { $description }
studies-series-number = Serie { $number }
studies-images = { $count ->
    [one] { $count } imagen
   *[other] { $count } imágenes
}
studies-images-missing = Faltan { $count }
studies-others = { $count ->
    [one] { $count } otro objeto
   *[other] { $count } otros objetos
}
studies-no-images = Esta serie no tiene imágenes en el disco
studies-medium-inserted = Se insertó un disco con estudios médicos
studies-medium-show = Mostrar estudios
palette-studies = Mostrar los estudios de este disco
