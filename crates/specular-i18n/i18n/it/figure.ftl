# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Esporta figura…
# machine-drafted, needs native review
palette-export-figure-help = Salva la vista come figura con barra di scala, barra dei colori ed etichette.
# machine-drafted, needs native review
palette-toggle-key-image = Contrassegna come immagine chiave
# machine-drafted, needs native review
palette-toggle-key-image-help = Contrassegna questa immagine come immagine chiave, oppure rimuovi il contrassegno.
# machine-drafted, needs native review
palette-copy-key-images = Copia immagini chiave
# machine-drafted, needs native review
palette-copy-key-images-help = Copia le immagini chiave come un’unica immagine.
# machine-drafted, needs native review
palette-export-key-images = Esporta immagini chiave…
# machine-drafted, needs native review
palette-export-key-images-help = Salva le immagini chiave come diapositive o come cartella di PNG.
# machine-drafted, needs native review
palette-export-teaching-file = Esporta caso didattico…
# machine-drafted, needs native review
palette-export-teaching-file-help = Salva le immagini chiave senza i dati del paziente, per la didattica.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Esporta figura
# machine-drafted, needs native review
figure-format = Formato
# machine-drafted, needs native review
figure-format-png = PNG, 16 bit
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16 bit
# machine-drafted, needs native review
figure-region = Regione
# machine-drafted, needs native review
figure-region-visible = Area visibile
# machine-drafted, needs native review
figure-region-whole = Immagine intera
# machine-drafted, needs native review
figure-size = Dimensione
# machine-drafted, needs native review
figure-size-pixels = Pixel dell’immagine (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = { $width } px di larghezza
# machine-drafted, needs native review
figure-scale-bar = Barra di scala
# machine-drafted, needs native review
figure-scale-bar-none = Nessuna calibrazione
# machine-drafted, needs native review
figure-colour-bar = Barra dei colori
# machine-drafted, needs native review
figure-colour-bar-none = Nessuna mappa colori né finestra
# machine-drafted, needs native review
figure-inset = Inserto
# machine-drafted, needs native review
figure-inset-rectangle = Ultimo rettangolo
# machine-drafted, needs native review
figure-inset-none = Disegna prima un rettangolo
# machine-drafted, needs native review
figure-channels = Canali
# machine-drafted, needs native review
figure-channels-merged = Uniti
# machine-drafted, needs native review
figure-channels-split = Separati e uniti
# machine-drafted, needs native review
figure-channels-one = Un canale
# machine-drafted, needs native review
figure-measurements = Misure
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Attive ({ $count })
    [many] Attive ({ $count })
   *[other] Attive ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Nessuna
# machine-drafted, needs native review
figure-title-row = Titolo
# machine-drafted, needs native review
figure-provenance = Provenienza
# machine-drafted, needs native review
figure-provenance-metadata = Nel file
# machine-drafted, needs native review
figure-provenance-drawn = Nel file e disegnata
# machine-drafted, needs native review
figure-file = File
# machine-drafted, needs native review
figure-choose = Scegli…
# machine-drafted, needs native review
figure-on = Attivo
# machine-drafted, needs native review
figure-off = Disattivato
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Attivo · { $source }
# machine-drafted, needs native review
figure-export = Esporta
# machine-drafted, needs native review
figure-working = Esportazione…
# machine-drafted, needs native review
figure-not-full-res = I pixel a piena risoluzione non sono caricati. Ingrandisci al { PERCENT(100) } o scegli una dimensione.
# machine-drafted, needs native review
figure-channel-red = Rosso
# machine-drafted, needs native review
figure-channel-green = Verde
# machine-drafted, needs native review
figure-channel-blue = Blu
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, da { $lo } a { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Finestra da { $lo } a { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Colori dei canali { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Canale { $channel }
# machine-drafted, needs native review
figure-display-hilo = Indicatori HiLo
# machine-drafted, needs native review
figure-display-as-stored = Come salvata
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Calibrazione: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Visualizzazione: { $value }
# machine-drafted, needs native review
figure-not-image = Le figure si creano da un’immagine nel visualizzatore.
# machine-drafted, needs native review
figure-not-ready = L’immagine è ancora in caricamento.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Troppo grande: { $width } × { $height } px. Scegli l’area visibile o una dimensione.

## Key images
# machine-drafted, needs native review
figure-keys-title = Esporta immagini chiave
# machine-drafted, needs native review
figure-teaching-title = Esporta caso didattico
# machine-drafted, needs native review
figure-keys-images = Immagini
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } immagine chiave
    [many] { $count } immagini chiave
   *[other] { $count } immagini chiave
}
# machine-drafted, needs native review
figure-keys-pptx = Diapositive (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Cartella di PNG
# machine-drafted, needs native review
figure-keys-preset = Preimpostazione
# machine-drafted, needs native review
figure-keys-standard = Standard
# machine-drafted, needs native review
figure-keys-teaching = Caso didattico
# machine-drafted, needs native review
figure-keys-overlays-on = Disegnate e nelle note
# machine-drafted, needs native review
figure-keys-masks = Testo incorporato
# machine-drafted, needs native review
figure-keys-masks-auto = Oscurato
# machine-drafted, needs native review
figure-keys-notes = Note del caso
# machine-drafted, needs native review
figure-keys-destination = Salva in
# machine-drafted, needs native review
figure-keys-empty = Nessuna immagine chiave per ora. Contrassegna prima un’immagine come immagine chiave.
# machine-drafted, needs native review
figure-keys-teaching-note = I dati del paziente vengono omessi e il testo incorporato viene oscurato. Controlla ogni immagine prima di condividere.
# machine-drafted, needs native review
figure-keys-none-left = Nessuna immagine è stata esportata senza dati del paziente.
# machine-drafted, needs native review
figure-keys-copied = Immagini chiave copiate
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Contrassegnata come immagine chiave ({ $count })
    [many] Contrassegnata come immagine chiave ({ $count })
   *[other] Contrassegnata come immagine chiave ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Non più immagine chiave (ne restano { $count })
    [many] Non più immagine chiave (ne restano { $count })
   *[other] Non più immagine chiave (ne restano { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Immagine { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Immagine chiave { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } salvato
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } salvato. { $count } immagine è stata esclusa: non è stato trovato il suo testo incorporato.
    [many] { $name } salvato. { $count } immagini sono state escluse: non è stato trovato il loro testo incorporato.
   *[other] { $name } salvato. { $count } immagini sono state escluse: non è stato trovato il loro testo incorporato.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Impossibile esportare: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } è già in quella cartella. Scegli una cartella vuota.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Immagine chiave
