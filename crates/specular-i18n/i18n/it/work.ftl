# machine-drafted, needs native review
# Specular — Attività in background: l’indicatore mostrato mentre qualcosa è in esecuzione in background e il pannello che apre. Italiano.

## Il pannello

work-title = Attività in background
work-show = Mostra attività in background
work-empty = Nessuna attività in esecuzione in background.

## Attività

work-kind-library = Libreria foto
work-kind-ai-search = Ricerca IA
work-kind-faces = Persone
work-kind-text = Testo nelle foto
work-kind-culling = Punteggi di selezione
work-kind-download = Download del modello IA
work-kind-similar = Scatti simili

## Passaggi

work-step-finding = Ricerca delle foto
work-step-reading = Lettura dei dettagli delle foto
work-step-signatures = Confronto delle foto
work-step-grouping = Raggruppamento degli scatti simili
work-step-text-index = Creazione dell’indice di ricerca
work-step-embedding = Indicizzazione delle foto per la ricerca IA
work-step-switching = Nuova indicizzazione per la nuova dimensione del modello
work-step-faces = Ricerca dei volti
work-step-ocr = Lettura del testo nelle foto
work-step-scoring = Valutazione di nitidezza ed esposizione
work-step-downloading = Download in corso
work-step-focus = Verifica della nitidezza
work-step-item = { $step } · { $item }

## Avanzamento

work-count-photos = { $done } di { $total } { $count ->
    [one] foto
   *[other] foto
}
work-count = { $done } di { $total }
work-count-left = { $count } · ne mancano { $left }
work-so-far-photos = { $done } { $count ->
    [one] foto
   *[other] foto
} finora
work-so-far = { $done } finora

## Tempo rimanente

work-eta-estimating = Stima in corso…
work-eta-seconds = { $n ->
    [one] Manca circa { $n } secondo
   *[other] Mancano circa { $n } secondi
}
work-eta-minutes = { $n ->
    [one] Manca circa { $n } minuto
   *[other] Mancano circa { $n } minuti
}
work-eta-hours = { $h ->
    [one] Manca circa { $h } ora
   *[other] Mancano circa { $h } ore
}
work-eta-hours-minutes = Mancano circa { $h ->
    [one] { $h } ora
   *[other] { $h } ore
} e { $m ->
    [one] { $m } minuto
   *[other] { $m } minuti
}

## Perché un’attività attende

work-paused-browsing = In pausa mentre sfogli le foto
work-paused-battery = In pausa durante l’alimentazione a batteria
work-paused-low-power = In pausa in modalità risparmio energetico
work-paused-heat = In pausa finché il computer è caldo
work-paused-memory = In pausa finché la memoria è scarsa
work-paused-user = In pausa

## Cosa segue

work-then = Poi: { $steps }

## Pulsanti

work-pause = Pausa
work-resume = Riprendi
work-settings = Impostazioni…

## L’indicatore

work-chip = { $job } · { $done } di { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Attività in background: { $status }

## Annunci

work-started = { $job }: avviato in background
work-finished = { $job }: completato
