# Specular — Gallerie per i clienti: una cartella HTML statica di foto, e la rilettura della scelta del cliente.

# machine-drafted, needs native review

palette-client-gallery = Galleria per il cliente…
palette-client-gallery-help = Salvare le foto come cartella di pagina web per un cliente, con una scelta che può rimandare.
palette-import-client-picks = Importa scelta del cliente…
palette-import-client-picks-help = Contrassegnare le foto che un cliente ha scelto nella sua galleria.

webgallery-heading = Galleria per il cliente
webgallery-subtitle = { $count ->
    [one] Una foto
   *[other] { $count } foto
}
webgallery-title-label = Titolo
webgallery-title-default = Provini
webgallery-watermark-label = Testo della filigrana (vuoto: nessuna)
webgallery-source = Foto
webgallery-source-current = Questa foto
webgallery-source-selection = Selezione
webgallery-source-album = Album «{ $name }»
webgallery-source-search = Risultati della ricerca
webgallery-source-folder = Questa cartella
webgallery-source-picks = Scelte
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Tema
webgallery-theme-light = Chiaro
webgallery-theme-dark = Scuro
webgallery-theme-minimal = Essenziale
webgallery-size = Lato lungo
webgallery-size-value = { $px } px
webgallery-format = Formato
webgallery-captions = Sotto ogni foto
webgallery-captions-off = Niente
webgallery-captions-name = Nome del file
webgallery-captions-title = Titolo
webgallery-captions-all = Nome del file, titolo e didascalia
webgallery-exif = Mostra fotocamera ed esposizione
webgallery-location = Mantieni la posizione (GPS) nelle foto
webgallery-note-private = Una galleria statica non può essere protetta da una password.
webgallery-note-share = Chiunque abbia la cartella o il suo indirizzo vede tutte le foto. Condividi il link solo con il tuo cliente, o usa il controllo di accesso del tuo hosting.
webgallery-note-picks = I clienti scelgono con un cuore. La scelta resta nel loro browser finché non ti mandano l’elenco.
webgallery-write = Crea galleria…
webgallery-busy = Scrittura di { $done } di { $total }…
webgallery-stop = Ferma
webgallery-done = { $count ->
    [one] Galleria creata: una foto in { $folder }
   *[other] Galleria creata: { $count } foto in { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Non è stato possibile scrivere una foto.
   *[other] Non è stato possibile scrivere { $failed } foto.
}
webgallery-stopped = Galleria fermata. Non è stato tenuto nulla.
webgallery-failed = Galleria non creata: { $reason }
webgallery-nothing = Non è stato possibile scrivere nessuna foto.

webgallery-page-count = { $count ->
    [one] Una foto
   *[other] { $count } foto
}
webgallery-page-hint = Tocca il cuore sulle foto che vuoi. La tua scelta resta in questo browser finché non la invii.
webgallery-page-picked = Scelte: {"{"}n{"}"}
webgallery-page-pick = Scegli
webgallery-page-copy = Copia la mia scelta
webgallery-page-copied = Copiata. Incolla l’elenco in un messaggio.
webgallery-page-copy-by-hand = Copia l’elenco selezionato e incollalo in un messaggio.
webgallery-page-download-txt = Scarica picks.txt
webgallery-page-download-json = Scarica picks.json
webgallery-page-clear = Cancella la scelta
webgallery-page-clear-ask = Cancellare tutta la tua scelta in questo browser?
webgallery-page-no-storage = Questo browser non conserverà la tua scelta. Inviala prima di chiudere la pagina.
webgallery-page-noscript = Per scegliere serve JavaScript. Le foto si aprono comunque.
webgallery-page-previous = Precedente
webgallery-page-next = Successiva
webgallery-page-close = Chiudi
webgallery-page-position = {"{"}i{"}"} di {"{"}n{"}"}
webgallery-page-viewer = Visualizzatore di foto
webgallery-page-picks = La tua scelta

clientpicks-heading = Importa scelta del cliente
clientpicks-subtitle = { $count ->
    [one] Confrontata con una foto mostrata
   *[other] Confrontata con le { $count } foto mostrate
}
clientpicks-paste-label = Nomi (incolla l’elenco del cliente)
clientpicks-choose = Scegli il file della scelta…
clientpicks-from-file = Da { $name }
clientpicks-apply-as = Contrassegnale
clientpicks-as-pick = Come scelte
clientpicks-as-stars = { $stars ->
    [one] Una stella
   *[other] { $stars } stelle
}
clientpicks-as-label = Etichetta { $label }
clientpicks-empty = Incolla l’elenco inviato dal cliente, o scegli il suo picks.txt o picks.json.
clientpicks-matched = { $matched } nomi su { $total } corrispondono a foto qui.
clientpicks-missing = Non trovati: { $names }
clientpicks-ambiguous = In più di una cartella, lasciati invariati: { $names }
clientpicks-more = { $names } e altri { $more }
clientpicks-apply = Applica
clientpicks-applied = { $count ->
    [one] Una foto contrassegnata.
   *[other] { $count } foto contrassegnate.
}
clientpicks-unchanged = Quelle foto erano già contrassegnate.
clientpicks-read-failed = Impossibile leggere { $name }: { $reason }
