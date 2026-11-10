# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Inspecionar
ndt-probe = Sonda: { $name }
ndt-probe-profile = Perfil de linha
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = IQI duplex de fios
ndt-probe-wire-iqi = IQI de fio único
ndt-probe-crack = Largura da fissura
ndt-probe-density = Ponto de densidade

ndt-hint-profile = Arraste uma linha para traçar os valores de cinzento ao longo dela. [ e ] mudam a sonda.
ndt-hint-snr = Arraste uma região numa zona uniforme. É alargada para pelo menos 20 × 55 píxeis.
ndt-hint-duplex-iqi = Arraste uma linha sobre todos os pares de fios, ao longo do IQI.
ndt-hint-wire-iqi = Arraste uma linha curta através do fio mais fino que consegue ver.
ndt-hint-crack = Clique ao longo da fissura e prima Enter. A largura é lida em 10 pontos.
ndt-hint-density = Clique num ponto para ler o seu valor de cinzento. Escreva a sua densidade conhecida para calibrar.

ndt-refused-needs-scale = Defina primeiro a escala: o tamanho dos fios exige milímetros. Use Calibrar num comprimento conhecido.
ndt-refused-nothing-found = Nada se destaca do ruído ao longo desta linha.
ndt-refused-too-small = A imagem é mais pequena do que a região de SNR de 20 × 55 píxeis.
ndt-refused-no-noise = Esta região não tem ruído para medir (todos os píxeis são iguais).

ndt-reading = A ler…
ndt-no-pixels = Não foi possível ler os píxeis.

ndt-profile-range = Cinzento { $min } a { $max }
ndt-profile-mean = Cinzento médio { $mean }
ndt-profile-density = Densidade { $min } a { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = O SNRn precisa da SRb: meça um IQI duplex ou escreva o valor.
ndt-snr-stats = Média { $mean }, σ { $sd }, região { $w } × { $h } px

ndt-duplex-result = Primeiro par não resolvido { $pair }: SRb { $srb } µm, falta de nitidez { $unsharpness }
ndt-duplex-all-resolved = Todos os pares encontrados estão resolvidos.
ndt-duplex-dips = Mínimos: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpolado: iSRb { $isrb } µm, falta de nitidez { $unsharpness }

ndt-wire-result = Fio { $wire }, { $diameter }
ndt-wire-measured = Largura medida { $width } (FWHM), cerca de { $diameter } de diâmetro

ndt-crack-mean = Largura média { $width }
ndt-crack-range = Mínima { $min }, máxima { $max }
ndt-crack-cuts = Pontos medidos: { $count }

ndt-density-value = Cinzento { $grey }, densidade { $density }
ndt-density-grey = Cinzento { $grey }
ndt-density-uncalibrated = Ainda sem calibração de densidade: escreva densidades conhecidas em dois pontos.

ndt-ask-density = Densidade conhecida aqui: { $value } (Enter aplica, Enter vazio ignora)
ndt-ask-srb = Resolução espacial básica SRb: { $value } µm (Enter aplica, Esc ignora)
ndt-bad-density = Escreva uma densidade de 0 a 10.
ndt-bad-srb = Escreva a SRb em micrómetros, por exemplo 80.
ndt-srb-set = SRb definida como { $srb } µm para esta imagem.
ndt-density-one-more = Ponto de densidade guardado. Escreva a densidade em mais um ponto.
ndt-density-calibrated = Densidade calibrada a partir de dois pontos.
ndt-sidecar-failed = Não foi possível guardar as definições de inspeção ao lado do ficheiro: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m acima do ponto de descolagem
map-export-kml = Guardar fotografias como KMZ ou KML…
map-export-kmz-full = Guardar fotografias como KMZ com as fotografias completas…
map-kml-heading = Rumo { $degrees }°
map-kml-height = { $metres } m acima do ponto de descolagem
map-kml-nothing = Nenhuma fotografia com posição para guardar.
map-kml-saved = Fotografias guardadas em { $name }: { $count }.
map-kml-failed = Não foi possível guardar o ficheiro de mapa: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Relatório de inspeção
inspection-report-default-title = Relatório de inspeção
inspection-report-folder = Adicionar as outras fotografias desta pasta
inspection-report-docx = Guardar também um documento Word (DOCX)
inspection-report-kmz = Guardar também um KMZ com os locais das fotografias
inspection-report-caption = Fotografia com as medições
inspection-report-col-value = Valor
inspection-report-col-scale = Escala a partir de
inspection-report-docx-failed = Não foi possível escrever o documento Word.
inspection-report-unreadable = Não foi possível ler nenhuma das fotografias.
