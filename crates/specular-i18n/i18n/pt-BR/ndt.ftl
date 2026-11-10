# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Inspecionar
ndt-probe = Sonda: { $name }
ndt-probe-profile = Perfil de linha
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = IQI duplex de fios
ndt-probe-wire-iqi = IQI de fio único
ndt-probe-crack = Largura da trinca
ndt-probe-density = Ponto de densidade

ndt-hint-profile = Arraste uma linha para traçar os valores de cinza ao longo dela. [ e ] trocam a sonda.
ndt-hint-snr = Arraste uma região numa área uniforme. Ela é ampliada para pelo menos 20 × 55 pixels.
ndt-hint-duplex-iqi = Arraste uma linha sobre todos os pares de fios, ao longo do IQI.
ndt-hint-wire-iqi = Arraste uma linha curta através do fio mais fino que você consegue ver.
ndt-hint-crack = Clique ao longo da trinca e pressione Enter. A largura é lida em 10 pontos.
ndt-hint-density = Clique num ponto para ler seu valor de cinza. Digite a densidade conhecida dele para calibrar.

ndt-refused-needs-scale = Defina a escala primeiro: o tamanho dos fios exige milímetros. Use Calibrar num comprimento conhecido.
ndt-refused-nothing-found = Nada se destaca do ruído ao longo desta linha.
ndt-refused-too-small = A imagem é menor que a região de SNR de 20 × 55 pixels.
ndt-refused-no-noise = Esta região não tem ruído para medir (todos os pixels são iguais).

ndt-reading = Lendo…
ndt-no-pixels = Não foi possível ler os pixels.

ndt-profile-range = Cinza { $min } a { $max }
ndt-profile-mean = Cinza médio { $mean }
ndt-profile-density = Densidade { $min } a { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = O SNRn precisa da SRb: meça um IQI duplex ou digite o valor.
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

ndt-density-value = Cinza { $grey }, densidade { $density }
ndt-density-grey = Cinza { $grey }
ndt-density-uncalibrated = Ainda sem calibração de densidade: digite densidades conhecidas em dois pontos.

ndt-ask-density = Densidade conhecida aqui: { $value } (Enter aplica, Enter vazio pula)
ndt-ask-srb = Resolução espacial básica SRb: { $value } µm (Enter aplica, Esc pula)
ndt-bad-density = Digite uma densidade de 0 a 10.
ndt-bad-srb = Digite a SRb em micrômetros, por exemplo 80.
ndt-srb-set = SRb definida como { $srb } µm para esta imagem.
ndt-density-one-more = Ponto de densidade mantido. Digite a densidade em mais um ponto.
ndt-density-calibrated = Densidade calibrada a partir de dois pontos.
ndt-sidecar-failed = Não foi possível salvar as configurações de inspeção ao lado do arquivo: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m acima do ponto de decolagem
map-export-kml = Salvar fotos como KMZ ou KML…
map-export-kmz-full = Salvar fotos como KMZ com as fotos completas…
map-kml-heading = Rumo { $degrees }°
map-kml-height = { $metres } m acima do ponto de decolagem
map-kml-nothing = Nenhuma foto com posição para salvar.
map-kml-saved = Fotos salvas em { $name }: { $count }.
map-kml-failed = Não foi possível salvar o arquivo de mapa: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Relatório de inspeção
inspection-report-default-title = Relatório de inspeção
inspection-report-folder = Adicionar as outras fotos desta pasta
inspection-report-docx = Salvar também um documento do Word (DOCX)
inspection-report-kmz = Salvar também um KMZ com os locais das fotos
inspection-report-caption = Foto com as medições
inspection-report-col-value = Valor
inspection-report-col-scale = Escala a partir de
inspection-report-docx-failed = Não foi possível gravar o documento do Word.
inspection-report-unreadable = Não foi possível ler nenhuma das fotos.
