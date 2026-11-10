# Informes estructurados DICOM, formas de onda, dosis de radioterapia y tomografía oftálmica. machine-drafted, needs native review

## Árbol de contenido del informe estructurado (el explorador de etiquetas en modo informe)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Elemento
sr-col-concept = Concepto
sr-col-type = Tipo
sr-col-relationship = Relación
sr-col-value = Valor
sr-show-image = Mostrar la imagen
sr-jump-missing = La imagen referenciada no está en esta carpeta
sr-jump-frame = Ninguna imagen de este marco de referencia contiene ese punto

## Gráficos (histogramas dosis-volumen, trazados de ECG)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Arrastre sobre un gráfico para medir
plot-adjust-hint = + y − cambian la ganancia, [ y ] la velocidad

## Trazado de ECG
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] No se muestra { $count } grupo de canales más
    [many] No se muestran { $count } grupos de canales más
   *[other] No se muestran { $count } grupos de canales más
}

## Dosis de radioterapia sobre la MPR (líneas de isodosis, mapa de color, leyenda, lectura)
dose-value = Dosis { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dosis oculta (Shift+U la muestra)
dose-on-rx = Dosis: { $dose } { $unit } prescrita · máx. { $max } { $unit }
dose-on-max = Dosis: porcentaje del máximo, { $max } { $unit }
dose-colour = Dosis en { $colormap } al { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: el histograma dosis-volumen de una estructura
    [many] Shift+K: los histogramas dosis-volumen de { $count } estructuras
   *[other] Shift+K: los histogramas dosis-volumen de { $count } estructuras
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dosis como líneas de isodosis
dose-mode-colour = Dosis como mapa de color

## Histogramas dosis-volumen (la hoja de gráficos)
dvh-title = Histogramas dosis-volumen
dvh-col-roi = Estructura
dvh-col-volume = Volumen
dvh-axis-dose = Dosis ({ $unit })
dvh-axis-volume = Volumen (%)
dvh-prescription = Prescripción { $dose } Gy · acumulado
dvh-max = Dosis máxima { $dose } { $unit } · acumulado
dvh-none = No hay ningún conjunto de estructuras de este marco de referencia sobre el que calcular el histograma de dosis

## Tomografía oftálmica (el control deslizante de B-scan; la imagen de localización en una esquina)
oct-bscan = B-scan
oct-depth = { $depth } µm de profundidad
