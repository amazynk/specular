# machine-drafted, needs native review
# Specular — Modalità selezione (DESIGN.md §6.3).

palette-cull = Entra o esci dalla modalità selezione
cheatsheet-cull = Modalità selezione
settings-section-cull = Modalità selezione
settings-cull-auto-enter = Seleziona le cartelle RAW
settings-cull-auto-enter-help = Apri una cartella composta soprattutto da file RAW direttamente in modalità selezione.
settings-cull-auto-advance = Immagine successiva dopo la marcatura
settings-cull-auto-advance-help = Passa oltre dopo una scelta, uno scarto o una valutazione a stelle. Le etichette e la rimozione del flag restano sull’immagine.
settings-cull-show = Mostra per le coppie RAW+JPEG
settings-cull-show-help = Un RAW e un JPEG con lo stesso nome vengono selezionati come un’unica immagine e le marcature valgono per entrambi. Il JPEG si apre più in fretta.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Raggruppa le raffiche
settings-cull-bursts-help = Gli scatti fatti in rapida successione da una fotocamera appaiono come una pila. S apre o chiude una pila, B tiene l’immagine visualizzata e scarta il resto della pila.
settings-cull-burst-gap = Pausa tra scatti
settings-cull-burst-gap-help = La pausa più lunga tra due scatti di una raffica.
settings-cull-watch = Mostra i nuovi file
settings-cull-watch-help = I file che compaiono nella cartella durante la selezione (fotocamera collegata, importazione) vengono aggiunti in fondo una volta scritti completamente.
settings-cull-follow-newest = Segui il file più recente
settings-cull-follow-newest-help = Passa a ogni nuovo file appena arriva. T attiva o disattiva durante la selezione.
settings-cull-slot-1 = Cartella 1
settings-cull-slot-1-help = Alt+1 copia l’immagine (con il suo RAW o JPEG e i file collaterali) in questa cartella, Alt+Maiusc+1 la sposta lì.
settings-cull-slot-1-unset = Non impostata
settings-cull-slot-2 = Cartella 2
settings-cull-slot-2-help = Alt+2 copia l’immagine (con il suo RAW o JPEG e i file collaterali) in questa cartella, Alt+Maiusc+2 la sposta lì.
settings-cull-slot-2-unset = Non impostata
settings-cull-slot-3 = Cartella 3
settings-cull-slot-3-help = Alt+3 copia l’immagine (con il suo RAW o JPEG e i file collaterali) in questa cartella, Alt+Maiusc+3 la sposta lì.
settings-cull-slot-3-unset = Non impostata
settings-cull-collisions = Se un nome è già usato
settings-cull-collisions-help = I file nelle cartelle non vengono mai sostituiti.
settings-cull-collisions-suffix = Aggiungi un numero
settings-cull-collisions-skip = Salta l’immagine
# machine-drafted, needs native review
# Modalità selezione scelta senza cartelle o immagini della galleria aperte.
cull-nothing = Niente da selezionare: apri prima una cartella di foto
cull-slot-unset = Nessuna cartella { $slot } impostata: scegline una in Impostazioni → Modalità selezione
cull-slot-missing = La cartella { $slot } non è disponibile: { $reason }
cull-copied = Copiato in { $folder }
cull-moved = Spostato in { $folder }
cull-slot-renamed = Salvato come { $name } in { $folder }
cull-slot-skipped = { $folder } contiene già { $name }: saltato
cull-slot-same = L’immagine è già in { $folder }
cull-slot-failed = { $op ->
    [move] Impossibile spostare { $file }
   *[copy] Impossibile copiare { $file }
}: { $reason }
cull-copy-kept = { $count ->
    [one] Tenuta una copia modificata nel frattempo
   *[other] Tenute { $count } copie modificate nel frattempo
}
cull-undo-failed = Impossibile annullare per { $file }: { $reason }
cull-burst-open = Raffica { $index } di { $count }
cull-burst-closed = Raffica di { $count }
cull-burst-toggle = Apri o chiudi la raffica
cull-burst-best = 1 tenuta, { $count } scartate
cull-burst-none = Non fa parte di una raffica
cull-follow-on = Segue i nuovi file
cull-follow-off = Non segue i nuovi file
cull-new-files = { $count ->
    [one] 1 nuovo file
   *[other] { $count } nuovi file
}
cull-key-copy = Copia nella cartella 1–3
cull-key-move = Sposta nella cartella 1–3
cull-key-burst = Apri o chiudi una raffica
cull-key-best = Tieni questa, scarta il resto della raffica
cull-key-follow = Segui i nuovi file
cull-key-subframes = Mostra o nascondi la tabella delle pose
cull-key-sort = Ordina per la colonna successiva
cull-key-sort-reverse = Inverti l’ordine
cull-key-metric = Grafico della misura successiva
cull-key-sigma-down = Scarto automatico: abbassa k
cull-key-sigma-up = Scarto automatico: alza k
cull-key-auto-reject = Scarta le pose suggerite
cull-key-move-rejects = Sposta le pose scartate in una sottocartella
cull-key-same-stretch = Stesso stretching per tutte le pose

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Apri la raffica
cull-tip-burst-close = Chiudi la raffica
cull-tip-exit = Esci dalla modalità selezione
cull-tip-label = Etichetta: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Pose
cull-astro-title = { $count ->
    [one] 1 posa
   *[other] { $count } pose
}
cull-astro-pending = misurazione di { $count }
cull-astro-col-name = Posa
cull-astro-col-filter = Filtro
cull-astro-col-exposure = Esp.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Ecc.
cull-astro-col-stars = Stelle
cull-astro-col-bg = Fondo
cull-astro-col-snr = SNR
cull-astro-col-noise = Rumore
cull-astro-same-stretch = Stesso stretching
cull-astro-own-stretch = Stretching proprio
cull-astro-suggest-high = { $metric } sopra { $value } (mediana + { $k }σ): { $count ->
    [one] 1 posa
   *[other] { $count } pose
}
cull-astro-suggest-low = { $metric } sotto { $value } (mediana − { $k }σ): { $count ->
    [one] 1 posa
   *[other] { $count } pose
}
cull-astro-suggest-none = Nessuna posa oltre { $k }σ
cull-astro-suggest-few = Lo scarto automatico richiede 3 pose misurate
cull-astro-suggested = suggerita da scartare
cull-astro-k = k { $k }
cull-astro-reject = Scarta { $count }
cull-astro-move = Sposta le scartate…
cull-astro-chart = { $metric } su { $count ->
        [one] { $count } posa
       *[other] { $count } pose
    }, mediana { $median }.
cull-astro-sort-by = Ordina per { $column }
cull-astro-ascending = crescente
cull-astro-descending = decrescente
cull-astro-sorted = Ordinato per { $column }
cull-astro-reversed = Ordine invertito
cull-astro-metric = Grafico: { $metric }
cull-astro-k-set = Scarto automatico a { $k }σ
cull-astro-none = Nessuna posa qui
cull-astro-same-on = Stesso stretching per tutte le pose
cull-astro-same-off = Ogni posa con il proprio stretching
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = eccentricità { $value }
cull-astro-line-stars = { $count ->
    [one] 1 stella
   *[other] { $count } stelle
}
cull-astro-measuring = misurazione delle stelle…
cull-astro-reject-confirm = { $count ->
    [one] Scartare la posa suggerita?
   *[other] Scartare le { $count } pose suggerite?
}
cull-astro-reject-detail = { $rule }. Annulla ripristina i contrassegni.
cull-astro-reject-ok = Scarta
cull-astro-nothing-to-reject = Nessuna posa da scartare
cull-astro-rejected = { $count ->
    [one] 1 posa scartata
   *[other] { $count } pose scartate
}
cull-move-rejects-confirm = { $count ->
    [one] Spostare la posa scartata nella cartella «{ $folder }»?
   *[other] Spostare le { $count } pose scartate nella cartella «{ $folder }»?
}
cull-move-rejects-detail = I file vengono spostati, non eliminati. Annulla li rimette a posto.
cull-move-rejects-ok = Sposta
cull-move-rejects-none = Nessuna posa scartata da spostare
cull-moved-rejects = { $count ->
    [one] 1 posa spostata in «{ $folder }»
   *[other] { $count } pose spostate in «{ $folder }»
}
