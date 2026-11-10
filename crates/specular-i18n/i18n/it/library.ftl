# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Libreria

## Top bar
library-title = Libreria
library-locations-button = Posizioni

## Locations sidebar
library-locations = Posizioni
library-all = Tutte le posizioni
library-add = Aggiungi posizione…
library-remove = Rimuovi { $location } dalla libreria
library-offline = Non in linea
library-location-pending = { $count } da indicizzare
library-a11y-location = { $location }, { $count ->
    [one] { $count } immagine
   *[other] { $count } immagini
}
library-a11y-location-offline = { $location }, Non in linea, { $count ->
    [one] { $count } immagine
   *[other] { $count } immagini
}

## Indexing progress
library-walking = Immagini trovate: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Immagini trovate: { $count }
library-reading = Indicizzazione… { $done } di { $total }
library-held = L’indicizzazione si ferma mentre navighi
library-up-to-date = Aggiornata
# machine-drafted, needs native review
library-stopped = Indicizzazione interrotta
library-stopped-pending = Indicizzazione interrotta · { $count } da indicizzare
library-resume = Riprendi
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indicizzazione interrotta: { $reason }
library-reason-stalled = nulla si muove da { $minutes ->
    [one] un minuto
   *[other] { $minutes } minuti
}. Un file o un’unità potrebbe non rispondere.
library-retry = Riprova
library-retry-description = Riavvia l’indicizzazione. Ciò che è già stato indicizzato resta ricercabile.
library-save-diagnostics = Salva pacchetto diagnostico…
library-stop-description = Interrompe l’indicizzazione. Ciò che è già stato indicizzato resta ricercabile; Riprendi o Ripeti scansione (F5) prosegue.
library-resume-description = Ripercorre tutte le posizioni e indicizza ciò che manca.
library-opening = Apertura della libreria…
library-searching = Ricerca…

## Empty and error states
library-empty = Ancora nessuna posizione. Aggiungi una cartella: le sue immagini e quelle delle sottocartelle appaiono qui in un’unica griglia.
library-empty-offline = Queste posizioni non sono in linea. Le immagini riappariranno quando torneranno disponibili.
library-indexing = Ricerca di immagini…
# machine-drafted, needs native review
library-remove-confirm = Rimuovere «{ $location }» dalla libreria?
library-remove-confirm-detail = Le tue foto restano sul disco, con valutazioni, etichette e parole chiave. La libreria dimentica ciò che vi aveva indicizzato; se aggiungi di nuovo la cartella, viene indicizzata di nuovo.
library-remove-confirm-ok = Rimuovi
library-indexing-in = Ricerca di immagini in { $location }… { $count } trovate finora
# machine-drafted, needs native review
library-empty-stopped = L’indicizzazione è stata interrotta prima che queste immagini fossero lette. Riprendi prosegue da lì.
library-unavailable = Impossibile aprire la libreria: { $reason }
library-read-only = Questa libreria è stata scritta da una versione più recente di Specular: si può cercare ma non aggiornare.

## Screen readers
library-a11y-list = Libreria, { $count ->
    [one] { $count } immagine
   *[other] { $count } immagini
}
library-a11y-added = { $location } aggiunta alla libreria
library-a11y-removed = { $location } rimossa dalla libreria

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Aggiungere tutto «{ $folder }»?
library-home-confirm-detail = Contiene file di programmi, download e cache, non solo foto. L’indicizzazione richiede più tempo e la libreria si riempie di icone e altre immagini.
library-home-confirm-pictures = La cartella Immagini contiene solo le tue immagini.
library-home-confirm-use-pictures = Aggiungi Immagini
library-home-confirm-keep = Aggiungi cartella home

## Folder watches
# machine-drafted, needs native review
library-unwatched = Alcune cartelle non sono monitorate
library-unwatched-detail = { $minutes ->
    [one] Le modifiche compaiono entro un minuto
   *[other] Le modifiche compaiono entro { $minutes } min
}
library-unwatched-description = Specular ha esaurito i monitoraggi di cartelle. Apre la guida su come alzare il limite.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Libreria
settings-library-skip-folders = Salta le cartelle chiamate
settings-library-skip-folders-help = La libreria non cerca nelle cartelle con questi nomi, separati da virgole. Le cartelle nascoste vengono sempre saltate.
settings-library-skip-marked = Salta le cartelle che contengono
settings-library-skip-marked-help = Né nelle cartelle che ne contengono una di queste: .git per i checkout di codice, pyvenv.cfg per gli ambienti Python, CACHEDIR.TAG per le cache. La cartella di una posizione non viene mai saltata.
settings-library-show-timeline = Linea del tempo
settings-library-show-timeline-help = Mostra quante foto ha ogni anno, mese o giorno, sopra la griglia della libreria.
