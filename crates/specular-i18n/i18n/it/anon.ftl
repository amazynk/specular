# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

palette-export-anonymised = Esporta copia anonimizzata…
palette-export-patient-disc = Esporta cartella disco paziente…
palette-mask-burned-in-text = Maschera testo impresso
anon-title = Esporta copia anonimizzata
anon-disc-title = Esporta cartella disco paziente
anon-images = Immagini
anon-images-current = Questa immagine
anon-images-series = { $count ->
    [one] Questa serie ({ $count } file)
   *[other] Questa serie ({ $count } file)
}
anon-images-study = { $count ->
    [one] Questo studio ({ $count } file)
   *[other] Questo studio ({ $count } file)
}
anon-images-reading = Lettura della cartella…
anon-anonymise = Anonimizza
anon-dates = Date e ore
anon-dates-remove = Rimuovi
anon-dates-keep = Mantieni
anon-dates-shift = Sposta
anon-patient = Caratteristiche del paziente
anon-device = Identità del dispositivo
anon-institution = Identità della struttura
anon-descriptions = Descrizioni
anon-descriptions-remove = Rimuovi
anon-descriptions-clean = Pulisci
anon-structured = Contenuto dei referti
anon-uids = UID
anon-uids-replace = Sostituisci
anon-uids-keep = Mantieni
anon-overlays = Overlay e curve
anon-keep = Mantieni
anon-remove = Rimuovi
anon-on = Attivo
anon-off = Disattivo
anon-burned-in = Testo impresso
anon-masks = { $count ->
    [one] { $count } area mascherata
   *[other] { $count } aree mascherate
}
anon-masks-none = Nessuna maschera
anon-folder = Cartella
anon-folder-choose = Scegli…
anon-export = Esporta
anon-risk-present = Queste immagini dichiarano di mostrare testo del paziente: mascheralo prima dell’esportazione.
anon-risk-possible = Le immagini di questo tipo mostrano spesso testo del paziente: controlla prima dell’esportazione.
anon-progress = Esportazione di { $done } di { $total }…
anon-stopping = Arresto…
anon-mask-title = Maschera testo impresso
anon-mask-hint = Trascina sul testo per mascherarlo. Le aree mascherate diventano nere in ogni fotogramma.
anon-mask-suggest = Suggerisci
anon-mask-remove = Rimuovi
anon-mask-clear = Cancella
anon-mask-done = Fatto
anon-mask-area = Area { $n }, { $width } × { $height } pixel
anon-mask-suggested = { $count ->
    [0] Nessun testo trovato vicino ai bordi
    [one] { $count } area di testo suggerita
   *[other] { $count } aree di testo suggerite
}
anon-done = { $count ->
    [one] { $count } file anonimizzato scritto in { $folder }
   *[other] { $count } file anonimizzati scritti in { $folder }
}
anon-disc-done = { $count ->
    [one] Cartella disco paziente con { $count } immagine scritta in { $folder }
   *[other] Cartella disco paziente con { $count } immagini scritta in { $folder }
}
anon-skipped-burned-in = { $count ->
    [one] { $count } file con testo impresso e senza maschere è stato saltato
   *[other] { $count } file con testo impresso e senza maschere sono stati saltati
}
anon-skipped-failed = { $count ->
    [one] Impossibile esportare { $count } file
   *[other] Impossibile esportare { $count } file
}
anon-stopped = Esportazione interrotta
anon-failed = Impossibile esportare: { $reason }
anon-not-empty = La cartella contiene già un disco paziente
