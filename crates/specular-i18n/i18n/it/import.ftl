# machine-drafted, needs native review
# Specular — Importazione da fotocamere, telefoni e schede di memoria. Italiano.
# La scheda viene solo letta; i file vengono eliminati solo su richiesta, dopo una copia verificata.

palette-import = Importa foto…
disabled-no-import-source = Nessuna scheda di memoria, fotocamera o telefono collegato

## Impostazioni → Importazione
settings-section-import = Importazione
settings-import-on-card = All’inserimento di una scheda
settings-import-on-card-help = Cosa succede quando si collega una scheda di memoria, una fotocamera o un telefono con foto.
settings-import-on-card-ask = Chiedi
settings-import-on-card-open = Apri Importa
settings-import-on-card-nothing = Non fare nulla
settings-import-destination = Importa in
settings-import-destination-help = La cartella in cui vanno foto e video importati. Se non è impostata, vanno nella cartella Immagini.
settings-import-destination-unset = Cartella Immagini
settings-import-folder-pattern = Cartelle
settings-import-folder-pattern-help = Le cartelle create al suo interno: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} e {"{"}event{"}"}, separate da /.
settings-import-rename-pattern = Rinomina
settings-import-rename-pattern-help = Nuovi nomi dei file, come in Converti: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Vuoto mantiene i nomi della fotocamera.
settings-import-keep-pairs = Tieni insieme RAW e JPEG
settings-import-keep-pairs-help = Un file RAW e il JPEG scattato con esso vanno in una stessa cartella con lo stesso nome, anche se rinominati.
settings-import-low-res-videos = Importa le copie video a bassa risoluzione
settings-import-low-res-videos-help = Alcune fotocamere registrano una piccola copia di anteprima di ogni video (.LRV). Se disattivato, restano sulla scheda.
settings-import-eject = Espelli al termine
settings-import-eject-help = Espelle la scheda quando tutto è copiato, così si può estrarre.

## La proposta all’inserimento di una scheda
import-offer = Importare da { $name }?
import-offer-button = Importa

## La vista di importazione
import-title = Importa
import-from = da { $name }
import-source-next = Origine successiva
import-close = Chiudi
import-scanning = Ricerca di nuove foto… { $done } di { $total }
import-scanning-start = Ricerca di nuove foto…
import-none-new = Tutto il contenuto di { $name } è già stato importato.
import-empty = Non ci sono foto né video su { $name }.
import-no-source = Nessuna scheda di memoria, fotocamera o telefono collegato.
import-summary = { $count ->
    [one] { $count } nuovo elemento
    [many] { $count } nuovi elementi
   *[other] { $count } nuovi elementi
} · { $size }
import-already = { $count } già importati
import-unreadable = { $count ->
    [one] Impossibile leggere { $count } file
    [many] Impossibile leggere { $count } file
   *[other] Impossibile leggere { $count } file
}
import-selected = { $count } di { $total } selezionati
import-select-all = Seleziona tutto
import-select-none = Deseleziona tutto
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Importa in
import-choose = Scegli…
import-folder-pattern = Cartelle
import-event = Evento
import-event-hint = Facoltativo, per {"{"}event{"}"}
import-rename = Rinomina
import-rename-hint = Vuoto mantiene i nomi della fotocamera
import-preview = Primo file: { $path }
import-pattern-error = { $error }
import-keep-pairs = Tieni insieme RAW e JPEG
import-eject = Espelli al termine
import-delete = Elimina dalla scheda dopo l’importazione
import-free = { $size } da copiare · { $free } di spazio libero
import-free-unknown = { $size } da copiare
import-no-space = Spazio libero insufficiente in { $folder }: questa importazione richiede { $need } e sono liberi solo { $free }. Libera spazio o scegli un’altra cartella.
import-start = { $count ->
    [one] Importa { $count } elemento
    [many] Importa { $count } elementi
   *[other] Importa { $count } elementi
}
import-nothing-selected = Seleziona le foto da importare

import-confirm-delete = { $count ->
    [one] Dopo la copia e la verifica, { $count } file verrà eliminato da { $name }. L’operazione non si può annullare.
    [many] Dopo la copia e la verifica, { $count } file verranno eliminati da { $name }. L’operazione non si può annullare.
   *[other] Dopo la copia e la verifica, { $count } file verranno eliminati da { $name }. L’operazione non si può annullare.
}
import-confirm-yes = Importa ed elimina
import-confirm-no = Indietro

import-progress = Importazione di { $done } di { $total } · { $size }
import-progress-eta = Importazione di { $done } di { $total } · { $size } · { $time }
import-paused = In pausa · { $done } di { $total }
import-pause = Pausa
import-resume = Riprendi
import-cancel = Annulla
import-hide = Nascondi
import-stopping = Interruzione…
import-running = È già in corso un’importazione.

import-done = { $count ->
    [one] { $count } file importato
    [many] { $count } file importati
   *[other] { $count } file importati
} in { $folder }
import-done-existed = { $count ->
        [one] { $count } era già presente
       *[other] { $count } erano già presenti
    }
import-done-deleted = { $count } eliminati dalla scheda
import-done-failed = { $count ->
    [one] Impossibile importare { $count } file
    [many] Impossibile importare { $count } file
   *[other] Impossibile importare { $count } file
}
import-done-cancelled = Importazione annullata dopo { $count ->
        [one] { $count } file
       *[other] { $count } file
    }. Importando di nuovo si riprende da dove si era fermata.
import-done-nothing = Non è stato importato nulla di nuovo.
import-ejected = { $name } può essere estratto.
import-eject-failed = Impossibile espellere { $name }: { $error }
import-failed-destination = Impossibile usare la cartella { $folder }: { $error }
import-ledger-failed = Impossibile leggere l’elenco dei file importati ({ $error }); file già importati potrebbero essere proposti di nuovo.

## Lettore di schermo
import-a11y-group = Importa
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, selezionato
