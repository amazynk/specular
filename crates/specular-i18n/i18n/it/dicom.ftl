# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Mostra tag DICOM
palette-next-series = Serie successiva
palette-prev-series = Serie precedente
disabled-not-dicom = L’immagine non è un file DICOM
disabled-no-other-series = Non c’è un’altra serie in questa cartella
dicom-tags-title = Tag DICOM
dicom-tags-title-file = Tag DICOM: { $file }
dicom-tags-filter = Filtra tag
dicom-tags-show-patient = Mostra dati del paziente
dicom-tags-col-tag = Tag
dicom-tags-col-keyword = Parola chiave
dicom-tags-col-vr = VR
dicom-tags-col-length = Lunghezza
dicom-tags-col-value = Valore
dicom-tags-hidden = Nascosto
dicom-tags-hidden-count = { $count ->
    [one] { $count } valore del paziente nascosto
   *[other] { $count } valori del paziente nascosti
}
dicom-tags-no-match = Nessun tag corrisponde
dicom-tags-reading = Lettura dei tag…
dicom-tags-error = Impossibile leggere i tag: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } elemento
   *[other] { $count } elementi
}
dicom-tags-fragments = { $count ->
    [one] { $count } frammento
   *[other] { $count } frammenti
}
dicom-tags-item = Elemento { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } byte
}
dicom-tags-empty = (vuoto)
dicom-tags-omitted = { $count ->
    [one] Altro { $count } elemento non elencato
   *[other] Altri { $count } elementi non elencati
}
dicom-series-group = Serie
dicom-series-slice = Sezione
dicom-series-slice-value = { $index } di { $count }
dicom-series-frames = { $frames ->
        [one] { $frames } fotogramma
       *[other] { $frames } fotogrammi
    }
dicom-series-location = Posizione della sezione
dicom-series-mm = { $mm } mm
dicom-series-spacing = Spaziatura delle sezioni
dicom-series-which = Serie
dicom-series-which-value = { $index } di { $count }
dicom-series-order = Ordine
dicom-series-order-position = Per posizione
dicom-series-order-instance = Per numero di istanza
dicom-series-order-time = Per ora di acquisizione
dicom-series-order-name = Per nome file
dicom-series-gaps = Sezioni mancanti
dicom-series-gaps-value = { $count ->
    [one] Manca { $count } sezione
   *[other] Mancano { $count } sezioni
}
dicom-series-gaps-where = Dopo la sezione { $slices }
dicom-series-irregular = Spaziatura irregolare
dicom-series-tilt = Inclinazione del gantry
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Duplicati saltati
dicom-series-toast = { $series ->
    [one] Serie – sezioni: { $count }
   *[other] Serie { $index } di { $series } – sezioni: { $count }
}
dicom-series-capped = Sono stati letti solo i primi { $count } file della cartella

studies-title = Studi
studies-title-medium = Studi: { $medium }
studies-show-patient = Mostra dati del paziente
studies-patient-hidden = Nascosto
studies-patient-hidden-note = Nomi e date di nascita dei pazienti sono nascosti
studies-none = Nessuno studio trovato
studies-reading = Lettura del disco…
studies-reading-count = Lettura del disco: { $done } di { $total ->
        [one] { $total } file
       *[other] { $total } file
    }
studies-opening = Apertura della serie…
studies-error = Impossibile leggere il disco: { $reason }
studies-missing = { $count ->
    [one] Manca { $count } file elencato sul disco
   *[other] Mancano { $count } file elencati sul disco
}
studies-capped = Sono stati letti solo i primi { $count } file
# machine-drafted, needs native review
studies-stopped = Lettura interrotta dopo { $done } di { $total ->
        [one] { $total } file
       *[other] { $total } file
    }: l’elenco è incompleto
studies-stopped-early = Lettura interrotta: l’elenco è incompleto
studies-cancel-description = Interrompe la lettura del disco. Ciò che è già stato letto resta nell’elenco.
studies-patient-n = Paziente { $index }
studies-patient-unknown = Paziente sconosciuto
studies-born = Nato/a il { $date }
studies-no-description = Nessuna descrizione
studies-series = Serie { $number }: { $description }
studies-series-number = Serie { $number }
studies-images = { $count ->
    [one] { $count } immagine
   *[other] { $count } immagini
}
studies-images-missing = { $count } mancanti
studies-others = { $count ->
    [one] { $count } altro oggetto
   *[other] { $count } altri oggetti
}
studies-no-images = Questa serie non ha immagini sul disco
studies-medium-inserted = È stato inserito un disco con studi medici
studies-medium-show = Mostra studi
palette-studies = Mostra gli studi su questo disco
