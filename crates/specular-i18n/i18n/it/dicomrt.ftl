# Referti strutturati DICOM, forme d’onda, dose di radioterapia e tomografia oftalmica. machine-drafted, needs native review

## Albero dei contenuti del referto strutturato (il browser dei tag in modalità referto)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Elemento
sr-col-concept = Concetto
sr-col-type = Tipo
sr-col-relationship = Relazione
sr-col-value = Valore
sr-show-image = Mostra l’immagine
sr-jump-missing = L’immagine di riferimento non è in questa cartella
sr-jump-frame = Nessuna immagine di questo sistema di riferimento contiene quel punto

## Grafici (istogrammi dose-volume, tracciati ECG)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Trascina su un grafico per misurare
plot-adjust-hint = + e − cambiano il guadagno, [ e ] la velocità

## Tracciato ECG
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Un altro gruppo di canali non è mostrato
    [many] Altri { $count } gruppi di canali non sono mostrati
   *[other] Altri { $count } gruppi di canali non sono mostrati
}

## Dose di radioterapia sulla MPR (linee isodose, mappa di colori, legenda, lettura)
dose-value = Dose { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dose nascosta (Shift+U la mostra)
dose-on-rx = Dose: { $dose } { $unit } prescritta · max { $max } { $unit }
dose-on-max = Dose: percentuale del massimo, { $max } { $unit }
dose-colour = Dose in { $colormap } al { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: l’istogramma dose-volume di una struttura
    [many] Shift+K: gli istogrammi dose-volume di { $count } strutture
   *[other] Shift+K: gli istogrammi dose-volume di { $count } strutture
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dose come linee isodose
dose-mode-colour = Dose come mappa di colori

## Istogrammi dose-volume (il foglio dei grafici)
dvh-title = Istogrammi dose-volume
dvh-col-roi = Struttura
dvh-col-volume = Volume
dvh-axis-dose = Dose ({ $unit })
dvh-axis-volume = Volume (%)
dvh-prescription = Prescrizione { $dose } Gy · cumulativo
dvh-max = Dose massima { $dose } { $unit } · cumulativo
dvh-none = Nessun set di strutture di questo sistema di riferimento su cui calcolare l’istogramma della dose

## Tomografia oftalmica (il cursore dei B-scan; l’immagine di localizzazione in un angolo)
oct-bscan = B-scan
oct-depth = { $depth } µm di profondità
