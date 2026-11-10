# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

palette-export-anonymised = Exportar copia anonimizada…
palette-export-patient-disc = Exportar carpeta de disco del paciente…
palette-mask-burned-in-text = Enmascarar texto incrustado
anon-title = Exportar copia anonimizada
anon-disc-title = Exportar carpeta de disco del paciente
anon-images = Imágenes
anon-images-current = Esta imagen
anon-images-series = { $count ->
    [one] Esta serie ({ $count } archivo)
   *[other] Esta serie ({ $count } archivos)
}
anon-images-study = { $count ->
    [one] Este estudio ({ $count } archivo)
   *[other] Este estudio ({ $count } archivos)
}
anon-images-reading = Leyendo la carpeta…
anon-anonymise = Anonimizar
anon-dates = Fechas y horas
anon-dates-remove = Eliminar
anon-dates-keep = Conservar
anon-dates-shift = Desplazar
anon-patient = Características del paciente
anon-device = Identidad del equipo
anon-institution = Identidad de la institución
anon-descriptions = Descripciones
anon-descriptions-remove = Eliminar
anon-descriptions-clean = Limpiar
anon-structured = Contenido de informes
anon-uids = UID
anon-uids-replace = Reemplazar
anon-uids-keep = Conservar
anon-overlays = Superposiciones y curvas
anon-keep = Conservar
anon-remove = Eliminar
anon-on = Activado
anon-off = Desactivado
anon-burned-in = Texto incrustado
anon-masks = { $count ->
    [one] { $count } área enmascarada
   *[other] { $count } áreas enmascaradas
}
anon-masks-none = Sin máscaras
anon-folder = Carpeta
anon-folder-choose = Elegir…
anon-export = Exportar
anon-risk-present = Estas imágenes indican que muestran texto del paciente: enmascárelo antes de exportar.
anon-risk-possible = Las imágenes de este tipo suelen mostrar texto del paciente: revíselas antes de exportar.
anon-progress = Exportando { $done } de { $total }…
anon-stopping = Deteniendo…
anon-mask-title = Enmascarar texto incrustado
anon-mask-hint = Arrastre sobre el texto para enmascararlo. Las áreas enmascaradas quedan negras en todos los fotogramas.
anon-mask-suggest = Sugerir
anon-mask-remove = Eliminar
anon-mask-clear = Borrar
anon-mask-done = Listo
anon-mask-area = Área { $n }, { $width } × { $height } píxeles
anon-mask-suggested = { $count ->
    [0] No se encontró texto cerca de los bordes
    [one] { $count } área de texto sugerida
   *[other] { $count } áreas de texto sugeridas
}
anon-done = { $count ->
    [one] { $count } archivo anonimizado escrito en { $folder }
   *[other] { $count } archivos anonimizados escritos en { $folder }
}
anon-disc-done = { $count ->
    [one] Carpeta de disco del paciente con { $count } imagen escrita en { $folder }
   *[other] Carpeta de disco del paciente con { $count } imágenes escrita en { $folder }
}
anon-skipped-burned-in = { $count ->
    [one] Se omitió { $count } archivo con texto incrustado y sin máscaras
   *[other] Se omitieron { $count } archivos con texto incrustado y sin máscaras
}
anon-skipped-failed = { $count ->
    [one] No se pudo exportar { $count } archivo
   *[other] No se pudieron exportar { $count } archivos
}
anon-stopped = Exportación detenida
anon-failed = No se pudo exportar: { $reason }
anon-not-empty = La carpeta ya contiene un disco del paciente
