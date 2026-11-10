# Laudos estruturados DICOM, formas de onda, dose de radioterapia e tomografia oftálmica. machine-drafted, needs native review

## Árvore de conteúdo do laudo estruturado (o navegador de tags no modo laudo)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Item
sr-col-concept = Conceito
sr-col-type = Tipo
sr-col-relationship = Relação
sr-col-value = Valor
sr-show-image = Mostrar a imagem
sr-jump-missing = A imagem referenciada não está nesta pasta
sr-jump-frame = Nenhuma imagem deste referencial contém esse ponto

## Gráficos (histogramas dose-volume, traçados de ECG)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Arraste sobre um gráfico para medir
plot-adjust-hint = + e − alteram o ganho, [ e ] a velocidade

## Traçado de ECG
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Mais { $count } grupo de canais não é mostrado
    [many] Mais { $count } grupos de canais não são mostrados
   *[other] Mais { $count } grupos de canais não são mostrados
}

## Dose de radioterapia sobre a MPR (linhas de isodose, mapa de cores, legenda, leitura)
dose-value = Dose { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dose oculta (Shift+U a mostra)
dose-on-rx = Dose: { $dose } { $unit } prescrita · máx. { $max } { $unit }
dose-on-max = Dose: porcentagem do máximo, { $max } { $unit }
dose-colour = Dose em { $colormap } a { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: o histograma dose-volume de { $count } estrutura
    [many] Shift+K: os histogramas dose-volume de { $count } estruturas
   *[other] Shift+K: os histogramas dose-volume de { $count } estruturas
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dose como linhas de isodose
dose-mode-colour = Dose como mapa de cores

## Histogramas dose-volume (a folha de gráficos)
dvh-title = Histogramas dose-volume
dvh-col-roi = Estrutura
dvh-col-volume = Volume
dvh-axis-dose = Dose ({ $unit })
dvh-axis-volume = Volume (%)
dvh-prescription = Prescrição { $dose } Gy · cumulativo
dvh-max = Dose máxima { $dose } { $unit } · cumulativo
dvh-none = Nenhum conjunto de estruturas deste referencial sobre o qual calcular o histograma de dose

## Tomografia oftálmica (o controle deslizante de B-scan; a imagem de localização num canto)
oct-bscan = B-scan
oct-depth = { $depth } µm de profundidade
