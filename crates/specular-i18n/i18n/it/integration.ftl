# machine-drafted, needs native review
# Integrazione: apri con, comandi utente, incolla (Stage 2). Italiano.
palette-open-with = Apri con…
palette-paste = Incolla
open-with-placeholder = Apri con…
open-with-loading = Ricerca delle applicazioni…
open-with-default = { $name } (predefinita)
open-with-choose-other = Scegli un’altra applicazione…
open-with-choose-title = Scegli un’applicazione
open-with-failed = Impossibile aprire con { $name }: { $reason }
command-missing = { $program } non trovato
command-failed = Impossibile avviare { $name }: { $reason }
command-exited = { $name } è terminato con un errore: { $status }
paste-nothing = Niente da aprire negli appunti
paste-failed = Impossibile incollare l’immagine: { $reason }
drag-failed = Impossibile trascinare il file: { $reason }
about-storage = Impostazioni: { $settings } · Cache: { $cache }
about-storage-portable = Modalità portatile: impostazioni e cache in { $dir }

## Scripting and recipes

palette-copy-view-command = Copia vista come comando shell
palette-copy-view-python = Copia vista come Python
toast-copied-view-command = Comando copiato
script-copy-no-image = Nessuna immagine da descrivere
recipe-confirm-title = Eseguire la ricetta «{ $name }» per la prima volta? Esegue: { $command }
recipe-confirm-run = Esegui
recipe-no-file = La ricetta richiede un file immagine
recipe-running = { $name } in esecuzione…
recipe-done = { $name }: { $shapes ->
    [one] 1 forma aggiunta
   *[other] { $shapes } forme aggiunte
}
recipe-failed = { $name } non riuscita: { $reason }
recipe-timeout = fermato dopo { $seconds } s
recipe-no-output = { $name } non ha scritto { $file }
