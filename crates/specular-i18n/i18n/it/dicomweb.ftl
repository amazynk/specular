# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Italiano.

palette-dicom-servers = Server DICOMweb…

dicomweb-title = Server DICOMweb
dicomweb-servers-intro = Gli studi su un server vengono cercati e aperti solo quando lo chiedi. Prima non viene inviato nulla.
dicomweb-servers = Server
dicomweb-servers-empty = Ancora nessun server. Aggiungine uno con il suo indirizzo DICOMweb.
dicomweb-session-only = Password e token vengono conservati solo fino alla chiusura di Specular.
dicomweb-add = Aggiungi…
dicomweb-edit = Modifica…
dicomweb-remove = Rimuovi
dicomweb-close = Chiudi
dicomweb-browse = Sfoglia
dicomweb-back = Indietro
dicomweb-save = Salva
dicomweb-search = Cerca
dicomweb-stop = Interrompi
dicomweb-removed = { $server } rimosso
# machine-drafted, needs native review
dicomweb-remove-ask = Rimuovere “{ $server }” dai server salvati?
dicomweb-remove-detail = Non si perde nulla sul disco. La sua password o il suo token per questa sessione vengono dimenticati.
# Server raggiunti senza cifratura (http://).
dicomweb-not-encrypted = Non cifrato

# Pagina per aggiungere o modificare un server.
dicomweb-add-title = Aggiungi un server
dicomweb-edit-title = Impostazioni del server
dicomweb-field-name = Nome
dicomweb-hint-name = Facoltativo
dicomweb-field-url = Indirizzo
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Accesso
dicomweb-auth-none = Nessun accesso
dicomweb-auth-basic = Nome utente e password
dicomweb-auth-bearer = Token di accesso
dicomweb-field-user = Nome utente
dicomweb-field-password = Password
dicomweb-field-token = Token di accesso
dicomweb-hint-secret-kept = Lascia vuoto per mantenere quello attuale
dicomweb-secret-session = La password o il token viene conservato solo fino alla chiusura di Specular, mai su disco.
dicomweb-secret-needed = Inserisci di nuovo la password o il token: viene conservato solo fino alla chiusura di Specular.
dicomweb-http-warning = Questo indirizzo non è cifrato: la password o il token verrebbe inviato in chiaro.
dicomweb-url-empty = Inserisci l’indirizzo DICOMweb del server.
dicomweb-url-scheme = L’indirizzo deve iniziare con https:// o http://.
dicomweb-url-host = L’indirizzo non contiene il nome del server.
dicomweb-url-userinfo = Inserisci nome utente e password nei campi di accesso, non nell’indirizzo.
dicomweb-save-failed = Impossibile salvare l’elenco dei server: { $reason }

# Pagina di ricerca.
dicomweb-search-title = Trova studi
dicomweb-field-patient-name = Nome del paziente
dicomweb-field-patient-id = ID paziente
dicomweb-field-date-from = Dal
dicomweb-field-date-to = Al
dicomweb-hint-date = AAAAMMGG
dicomweb-field-modality = Modalità
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Numero di accettazione
dicomweb-show-patient = Mostra dati del paziente
dicomweb-switch-on = Attivo
dicomweb-switch-off = Disattivo
dicomweb-patient-shown = Dati del paziente visibili
dicomweb-patient-hidden = Dati del paziente nascosti
dicomweb-studies = Studi
dicomweb-search-hint = Compila uno qualsiasi dei campi e avvia la ricerca.
dicomweb-no-studies = Nessuno studio corrispondente.
dicomweb-searching = Ricerca in corso…
dicomweb-stopped = Interrotto
dicomweb-load-more = Carica altri
dicomweb-study-untitled = Studio senza descrizione
dicomweb-series-untitled = Serie senza descrizione
dicomweb-study-counts = { $series ->
    [one] Una serie
   *[other] { $series } serie
}, { $images ->
    [one] un’immagine
   *[other] { $images } immagini
}
dicomweb-series-images = { $count ->
    [one] Un’immagine
   *[other] { $count } immagini
}
dicomweb-patient-id = ID { $id }
dicomweb-accession = Accettazione { $number }
dicomweb-found = { $count ->
    [one] Trovato uno studio
   *[other] Trovati { $count } studi
}
dicomweb-series-found = { $count ->
    [one] Una serie
   *[other] { $count } serie
}

# Errori di comunicazione con un server.
dicomweb-error-offline = { $server } non è raggiungibile. Controlla la rete e l’indirizzo.
dicomweb-error-auth = Il server ha rifiutato nome utente, password o token.
dicomweb-error-not-found = A questo indirizzo non c’è alcun servizio DICOMweb.
dicomweb-error-other = Il server non ha potuto rispondere: { $reason }

# Serie trasmessa nel visualizzatore.
dicomweb-loading = Caricamento di { $series }…
dicomweb-loading-progress = Caricamento di { $series }: { $done } di { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] un’immagine caricata
   *[other] tutte le { $total } immagini caricate
}
dicomweb-loaded-failed = Impossibile caricare { $failed } immagini su { $total }.
dicomweb-offline-cached = { $server } non è raggiungibile: vengono mostrate le immagini salvate in precedenza.
dicomweb-cache-failed = Impossibile aprire la cartella dei download: { $reason }
