# machine-drafted, needs native review
# Specular — Patologia: separazione dei coloranti, esportazione e importazione di annotazioni, volumi di etichette sui volumi. Italiano.

## Tavolozza
palette-stain-separation = Separa i coloranti
palette-stain-1 = Mostra o nascondi il colorante 1
palette-stain-2 = Mostra o nascondi il colorante 2
palette-stain-3 = Mostra o nascondi il colorante 3
palette-stain-mode = Coloranti come luce o densità ottica
palette-stain-estimate = Stima i coloranti dalla regione
palette-stain-normalise = Normalizza i colori dei coloranti
palette-stain-reference = Usa la regione come riferimento di colorazione
palette-stain-reset = Vettori di colorazione pubblicati
palette-export-annotations = Esporta annotazioni…
palette-import-annotations = Importa annotazioni…
palette-label-volume = Sovrapponi volume di etichette…

## Motivi di disattivazione
disabled-not-colour-image = Serve un’immagine a colori
disabled-stains-off = Attiva prima la separazione dei coloranti
disabled-no-region = Disegna prima una regione
disabled-no-slide-annotations = Nessuna annotazione, regione o punto da esportare
disabled-no-label-target = Apri un volume nelle viste multiplanari, o un’immagine medica

## Nomi dei coloranti
stain-hematoxylin = Ematossilina
stain-eosin = Eosina
stain-dab = DAB
stain-residual = Residuo
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Notifiche e legenda dei coloranti
stain-off = Separazione dei coloranti disattivata
# { $set }: H&E, H-DAB o H&E-DAB.
stain-on = Coloranti: { $set }
stain-shown = { $stain } visibile
stain-hidden = { $stain } non visibile
stain-mode-light = Coloranti come luce
stain-mode-od = Coloranti come densità ottica
# Titolo della legenda: «H&E · luce», «H-DAB · densità ottica».
stain-legend-light = { $set } · luce
stain-legend-od = { $set } · densità ottica
stain-legend-estimated = Stimati dalla regione
stain-legend-normalised = Normalizzati
stain-legend-published = Vettori pubblicati
# { $pixels }: pixel colorati usati.
stain-estimated = Coloranti stimati da { $pixels } pixel
stain-estimate-failed = Impossibile stimare i coloranti: { $reason }
stain-estimate-background = la regione è quasi tutta sfondo
stain-estimate-one-colour = la regione contiene un solo colore
stain-estimate-three = la stima richiede H&E o H-DAB
stain-estimate-no-pixels = i pixel della regione non sono ancora caricati
stain-reference-set = Regione impostata come riferimento di colorazione
stain-normalise-on = Colori dei coloranti normalizzati
stain-normalise-off = Colori dei coloranti come scansionati
stain-reset-done = Vettori di colorazione pubblicati

## Finestra di esportazione delle annotazioni
annot-export-title = Esporta annotazioni
annot-export-format = Formato
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Maschere di etichette
annot-export-output = Maschere come
annot-export-output-png = Tile PNG
annot-export-output-tiff = Tile TIFF
annot-export-output-ome = Piramide OME-TIFF
annot-export-region = Regione
annot-export-region-slide = Vetrino intero
annot-export-region-view = Area visibile
annot-export-region-drawn = Ultima regione disegnata
annot-export-level = Livello
# { $level }: 0 per la risoluzione piena; { $width } × { $height } la sua dimensione in pixel.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Dimensione dei tile
annot-export-empty = Tile vuoti
annot-export-empty-skip = Salta
annot-export-empty-keep = Scrivi
annot-export-what = { $count ->
    [one] 1 forma
    [many] { $count } di forme
   *[other] { $count } forme
}, { $classes ->
    [one] 1 classe
    [many] { $classes } di classi
   *[other] { $classes } classi
}
annot-export-lines = Le linee sono escluse da COCO e dalle maschere.
annot-export-export = Esporta…
annot-export-writing = Scrittura…
# { $files }: file scritti.
annot-export-done = { $files ->
    [one] 1 file esportato
    [many] { $files } di file esportati
   *[other] { $files } file esportati
}
annot-export-failed = Esportazione non riuscita: { $reason }
annot-import-done = { $count ->
    [one] 1 annotazione importata
    [many] { $count } di annotazioni importate
   *[other] { $count } annotazioni importate
}
annot-import-failed = Impossibile importare: { $reason }
annot-import-skipped = { $count ->
    [one] 1 forma non aveva un contorno utilizzabile
    [many] { $count } di forme non avevano un contorno utilizzabile
   *[other] { $count } forme non avevano un contorno utilizzabile
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Volumi di etichette
label-volume-file = Volume di etichette
label-volume-table = Tabella dei colori
# { $file }: nome del file del volume di etichette; { $count }: le sue etichette.
label-volume-loaded = { $count ->
    [one] { $file }: 1 etichetta
    [many] { $file }: { $count } di etichette
   *[other] { $file }: { $count } etichette
}
label-volume-failed = Impossibile sovrapporre { $file }: { $reason }
label-volume-outside = { $file } non si sovrappone a questo volume
label-volume-not-whole = i suoi valori non sono numeri interi
label-volume-colour = contiene colori, non etichette
label-volume-empty = non ha etichette
label-volume-too-many = ha troppe etichette
label-volume-too-large = è troppo grande da ricampionare
label-volume-degenerate = il suo orientamento non è valido
# Un’etichetta senza nome nella tabella dei colori.
label-volume-unnamed = Etichetta { $value }
