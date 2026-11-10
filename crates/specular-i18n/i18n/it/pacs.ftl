# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Tipo
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Host
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Porta
dicomweb-hint-port = 104 o 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE title del server
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Il nostro AE title
# machine-drafted, needs native review
dicomweb-field-tls = Connessione sicura (TLS)
dicomweb-tls-on = Connessione sicura attiva
dicomweb-tls-off = Connessione sicura disattivata
dicomweb-problem-host = Inserisci il nome host o l’indirizzo del server.
dicomweb-problem-port = La porta è un numero da 1 a 65535.
dicomweb-problem-ae = Un AE title ha da 1 a 16 caratteri, senza barra rovesciata.
# machine-drafted, needs native review
dicomweb-dimse-note-open = I server DICOM (DIMSE) consentono la ricerca, l’apertura delle serie e l’invio.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Recupera con
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Ricevi sull’indirizzo
# machine-drafted, needs native review
pacs-hint-receive-address = Indirizzo di questo computer, ad es. 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Ricevi sulla porta
# machine-drafted, needs native review
pacs-hint-receive-port = Ad es. 11113
# machine-drafted, needs native review
pacs-move-note = Per C-MOVE il server deve conoscere il nostro AE title { $ae } con questo indirizzo e questa porta.
    Specular resta in ascolto lì solo mentre arriva una serie.
# machine-drafted, needs native review
pacs-problem-receive-address = Inserisci l’indirizzo di questo computer (ad es. 192.168.1.20) su cui ricevere.
# machine-drafted, needs native review
pacs-problem-receive-port = Inserisci una porta su cui ricevere. C-MOVE ne ha bisogno.
dicomweb-verify = Verifica
dicomweb-verifying = Verifica del server…
dicomweb-verify-ok = { $server } ha risposto.

## The Studies browser and the palette.

studies-send = Invia a PACS…
studies-send-description = Invia a un server questa serie, il suo studio o tutto il contenuto del disco.
palette-send-to-pacs = Invia a PACS…
palette-send-to-pacs-help = Invia la serie o lo studio corrente a un server PACS.

## The Send page.

pacs-title = Invia a PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } studio
    [many] { $studies } studi
   *[other] { $studies } studi
} · { $files ->
    [one] { $files } file
    [many] { $files } file
   *[other] { $files } file
}
pacs-what = Invia
pacs-scope-series = Questa serie ({ $files ->
    [one] { $files } file
    [many] { $files } file
   *[other] { $files } file
})
pacs-scope-study = Questo studio ({ $files ->
    [one] { $files } file
    [many] { $files } file
   *[other] { $files } file
})
pacs-scope-all = Tutto ciò che è stato letto ({ $studies ->
    [one] { $studies } studio
    [many] { $studies } studi
   *[other] { $studies } studi
}, { $files ->
    [one] { $files } file
    [many] { $files } file
   *[other] { $files } file
})
pacs-anonymise = Anonimizza
pacs-anonymise-note = I dati del paziente vengono rimossi dalle copie inviate. I tuoi file non vengono modificati.
pacs-identified-note = I file vengono inviati così come sono, con i dati del paziente.
# The list of servers to choose from.
pacs-to = A
pacs-no-servers = Ancora nessun server. Aggiungine uno in Server DICOMweb.
pacs-send = Invia…
pacs-go-back = Indietro
pacs-confirm-send = Invia
pacs-confirm = Inviare { $studies ->
    [one] { $studies } studio
    [many] { $studies } studi
   *[other] { $studies } studi
} ({ $files ->
    [one] { $files } file
    [many] { $files } file
   *[other] { $files } file
}) a { $server }?
pacs-busy = Un invio è ancora in corso.
pacs-started = Invio in corso.
pacs-stopping = Interruzione…
pacs-sending = Invio di { $done } di { $total } a { $server }…
pacs-sent-all = { $total ->
    [one] { $total } file è stato inviato a { $server }.
    [many] Tutti i { $total } file sono stati inviati a { $server }.
   *[other] Tutti i { $total } file sono stati inviati a { $server }.
}
pacs-sent-some = { $sent } file su { $total } sono stati inviati a { $server }.
pacs-stopped = Interrotto dopo { $sent } su { $total ->
        [one] { $total } file
       *[other] { $total } file
    }.
pacs-stopped-because = Interrotto dopo { $sent } su { $total ->
        [one] { $total } file
       *[other] { $total } file
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } file è stato archiviato con un avviso.
    [many] { $count } file sono stati archiviati con un avviso.
   *[other] { $count } file sono stati archiviati con un avviso.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = E altri { $count }.

## Why one file was not sent.

pacs-reason-unreadable = Impossibile leggerlo: { $reason }
pacs-reason-not-dicom = Non è un file DICOM
pacs-reason-burned-in = Ha dati del paziente incorporati nell’immagine, quindi non viene inviato anonimizzato
pacs-reason-document = Un documento che non può essere anonimizzato
pacs-reason-anonymise = Impossibile anonimizzarlo: { $reason }
pacs-reason-not-accepted = Il server non accetta { $class }
pacs-reason-convert = Impossibile convertirlo per il server: { $reason }
pacs-reason-resources = Il server ha esaurito lo spazio o le risorse
pacs-reason-mismatch = Il server dice che i dati non corrispondono al loro tipo
pacs-reason-understand = Il server non è riuscito a leggerlo
pacs-reason-class = Il server non supporta questo tipo di oggetto
pacs-reason-not-authorised = Il server non lo consente
pacs-reason-status = Il server lo ha rifiutato (stato { $code })
pacs-reason-refused = Il server lo ha rifiutato
pacs-reason-too-large = Troppo grande per il server
pacs-reason-http = Il server lo ha rifiutato (errore { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = La rete è disattivata.
pacs-fatal-unknown-host = { $host } non è un nome host conosciuto.
pacs-fatal-connect = Nessuno risponde su { $host }, porta { $port }.
pacs-fatal-unreachable = { $host } non è raggiungibile.
pacs-fatal-timeout = { $host } non ha risposto in tempo.
pacs-fatal-cancelled = Interrotto.
pacs-fatal-called-ae = { $host } non risponde all’AE title { $ae }. Controlla l’AE title del server.
pacs-fatal-calling-ae = { $host } non conosce il nostro AE title { $ae }. Aggiungilo sul server.
pacs-fatal-busy = { $host } è occupato. Riprova più tardi.
pacs-fatal-rejected = { $host } ha rifiutato la connessione.
pacs-fatal-aborted = { $host } ha chiuso la connessione.
pacs-fatal-nothing-accepted = { $host } non accetta nessuno di questi tipi di oggetto.
pacs-fatal-protocol = { $host } ha risposto in un modo che Specular non comprende.
pacs-fatal-auth = { $host } non ha accettato l’accesso.
pacs-fatal-http = { $host } ha risposto con l’errore { $status }.

## Teaching quiz.

palette-teaching-quiz = Quiz didattico
palette-teaching-quiz-help = Scorri le immagini chiave o un caso didattico, con i reperti nascosti finché non li mostri.
# "Case 2 of 5".
quiz-case = Caso { $n } di { $total }
quiz-hint-hidden = Spazio mostra i reperti · → caso successivo · Esc termina
quiz-hint-revealed = Spazio li nasconde di nuovo · → caso successivo · Esc termina
quiz-hidden = Reperti nascosti.
quiz-no-findings = Nessun reperto registrato.
quiz-notes = Note del caso: { $notes }
quiz-ended = Quiz terminato.
quiz-empty = Non ci sono casi da mostrare. Segna le immagini chiave con K o scegli una cartella di caso didattico.
quiz-not-a-case = Questa cartella non è un caso didattico esportato da Specular.
quiz-pick-title = Scegli una cartella di caso didattico

## Teaching case folder (Export key images).

figure-keys-case = Cartella di caso didattico
figure-case-notes = Note del caso
figure-case-findings = Reperti
figure-case-none = Nessuno registrato
figure-case-readme = Caso didattico esportato da Specular. I dati del paziente sono stati rimossi.
    images: le immagini chiave, con il testo incorporato oscurato.
    DICOM: copie anonimizzate e un manifest Key Object Selection «For Teaching».
    case.txt: le note del caso e i reperti di ogni immagine. case.json: lo stesso per gli script.
    Per Radiopaedia: carica la cartella DICOM o le immagini e copia le note da case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } non risponde alle ricerche (C-FIND).
pacs-find-status = { $host } ha rifiutato la ricerca (stato { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } non invia serie con C-GET.
    Scegli C-MOVE nelle impostazioni del server.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } non accetta richieste C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } non conosce il nostro AE title { $ae }.
    Aggiungilo sul server con l’indirizzo di questo computer e la porta { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } ha esaurito le risorse. Riprova più tardi.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } ha rifiutato di inviare la serie (stato { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } non ha immagini per questa serie.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } ha rifiutato di inviare immagini su questa connessione.
    Scegli C-MOVE nelle impostazioni del server.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } non è riuscito a inviare le immagini a questo computer.
    Controlla l’indirizzo, la porta e il firewall, e che il server conosca il nostro AE title { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } non è riuscito a inviare { $count ->
    [one] l’immagine
    [many] nessuna delle { $count } immagini
   *[other] nessuna delle { $count } immagini
} in un formato leggibile da Specular.
# machine-drafted, needs native review
pacs-retrieve-listen = Impossibile ricevere su { $address }, porta { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Il certificato di { $host } non è attendibile.
    Il suo emittente deve essere tra i certificati attendibili del sistema.
pacs-fatal-tls = Nessuna connessione sicura con { $host }.
    Verifica che la porta { $port } usi DICOM su TLS.
