# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Inspeccionar
ndt-probe = Sonda: { $name }
ndt-probe-profile = Perfil de línea
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = IQI dúplex de hilos
ndt-probe-wire-iqi = IQI de hilo único
ndt-probe-crack = Anchura de grieta
ndt-probe-density = Punto de densidad

ndt-hint-profile = Trace una línea para representar los valores de gris a lo largo de ella. [ y ] cambian la sonda.
ndt-hint-snr = Trace una región en una zona uniforme. Se amplía hasta al menos 20 × 55 píxeles.
ndt-hint-duplex-iqi = Trace una línea sobre todos los pares de hilos, a lo largo del IQI.
ndt-hint-wire-iqi = Trace una línea corta a través del hilo más fino que se vea.
ndt-hint-crack = Haga clic a lo largo de la grieta y pulse Intro. La anchura se lee en 10 puntos.
ndt-hint-density = Haga clic en un punto para leer su valor de gris. Escriba su densidad conocida para calibrar.

ndt-refused-needs-scale = Defina primero la escala: el tamaño de los hilos requiere milímetros. Use Calibrar sobre una longitud conocida.
ndt-refused-nothing-found = Nada destaca sobre el ruido a lo largo de esta línea.
ndt-refused-too-small = La imagen es más pequeña que la región SNR de 20 × 55 píxeles.
ndt-refused-no-noise = Esta región no tiene ruido que medir (todos los píxeles son iguales).

ndt-reading = Leyendo…
ndt-no-pixels = No se pudieron leer los píxeles.

ndt-profile-range = Gris { $min } a { $max }
ndt-profile-mean = Gris medio { $mean }
ndt-profile-density = Densidad { $min } a { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = La SNRn necesita la SRb: mida un IQI dúplex o escríbala.
ndt-snr-stats = Media { $mean }, σ { $sd }, región { $w } × { $h } px

ndt-duplex-result = Primer par no resuelto { $pair }: SRb { $srb } µm, falta de nitidez { $unsharpness }
ndt-duplex-all-resolved = Todos los pares encontrados están resueltos.
ndt-duplex-dips = Mínimos: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpolado: iSRb { $isrb } µm, falta de nitidez { $unsharpness }

ndt-wire-result = Hilo { $wire }, { $diameter }
ndt-wire-measured = Anchura medida { $width } (FWHM), unos { $diameter } de diámetro

ndt-crack-mean = Anchura media { $width }
ndt-crack-range = Mínima { $min }, máxima { $max }
ndt-crack-cuts = Puntos medidos: { $count }

ndt-density-value = Gris { $grey }, densidad { $density }
ndt-density-grey = Gris { $grey }
ndt-density-uncalibrated = Aún no hay calibración de densidad: escriba densidades conocidas en dos puntos.

ndt-ask-density = Densidad conocida aquí: { $value } (Intro aplica, Intro sin valor omite)
ndt-ask-srb = Resolución espacial básica SRb: { $value } µm (Intro aplica, Esc omite)
ndt-bad-density = Escriba una densidad de 0 a 10.
ndt-bad-srb = Escriba la SRb en micrómetros, por ejemplo 80.
ndt-srb-set = SRb fijada en { $srb } µm para esta imagen.
ndt-density-one-more = Punto de densidad guardado. Escriba la densidad en un punto más.
ndt-density-calibrated = Densidad calibrada a partir de dos puntos.
ndt-sidecar-failed = No se pudieron guardar los ajustes de inspección junto al archivo: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m sobre el punto de despegue
map-export-kml = Guardar fotos como KMZ o KML…
map-export-kmz-full = Guardar fotos como KMZ con las fotos completas…
map-kml-heading = Rumbo { $degrees }°
map-kml-height = { $metres } m sobre el punto de despegue
map-kml-nothing = No hay fotos con posición que guardar.
map-kml-saved = Fotos guardadas en { $name }: { $count }.
map-kml-failed = No se pudo guardar el archivo de mapa: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Informe de inspección
inspection-report-default-title = Informe de inspección
inspection-report-folder = Añadir las demás fotos de esta carpeta
inspection-report-docx = Guardar también un documento de Word (DOCX)
inspection-report-kmz = Guardar también un KMZ con los lugares de las fotos
inspection-report-caption = Foto con las mediciones
inspection-report-col-value = Valor
inspection-report-col-scale = Escala según
inspection-report-docx-failed = No se pudo escribir el documento de Word.
inspection-report-unreadable = No se pudo leer ninguna de las fotos.
