# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Italiano. machine-drafted, needs native review

palette-omero-servers = Server OMERO…

omero-title = Server OMERO
omero-servers-intro = Progetti, screening e immagini di un server vengono elencati solo quando lo si esplora. Prima non viene inviato nulla.
omero-servers = Server
omero-servers-empty = Ancora nessun server. Aggiungine uno con il suo indirizzo OMERO.web.
omero-session-only = Le password vengono conservate solo fino alla chiusura di Specular.
omero-add = Aggiungi…
omero-edit = Modifica…
omero-remove = Rimuovi
omero-close = Chiudi
omero-browse = Esplora
omero-back = Indietro
omero-save = Salva
omero-stop = Interrompi
omero-open = Apri
# $server a server's name.
omero-removed = { $server } rimosso
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Non cifrato
# A server browsed without signing in.
omero-public = Pubblico
# $user the user name a server is signed in with.
omero-signed-in-as = Come { $user }

# The page that adds or edits a server.
omero-add-title = Aggiungi un server
omero-edit-title = Impostazioni del server
omero-field-name = Nome
omero-hint-name = Facoltativo
omero-field-url = Indirizzo
omero-hint-url = https://omero.example.org
omero-field-user = Nome utente
omero-hint-user = Vuoto per i dati pubblici
omero-field-password = Password
omero-hint-password-kept = Lascia vuoto per mantenere quella già indicata
omero-password-session = La password viene conservata solo fino alla chiusura di Specular, mai su disco.
omero-password-needed = Inserisci la password: viene conservata solo fino alla chiusura di Specular.
omero-http-warning = Questo indirizzo non è cifrato: la password verrebbe inviata in chiaro.
omero-url-invalid = Inserisci l’indirizzo OMERO.web, che inizi con https:// o http://.
# $reason why the list could not be saved.
omero-save-failed = Impossibile salvare l’elenco dei server: { $reason }

# The browse page.
omero-browse-title = Esplora
omero-top = Progetti e screening
omero-datasets = Dataset
omero-plates = Piastre
omero-wells = Pozzetti
omero-fields = Campi
omero-images = Immagini
omero-kind-project = Progetto
omero-kind-dataset = Dataset
omero-kind-image = Immagine
omero-kind-screen = Screening
omero-kind-plate = Piastra
omero-kind-well = Pozzetto
# $well a well's row and column, as B7.
omero-well = Pozzetto { $well }
omero-well-fields = { $count ->
    [one] Un campo
   *[other] { $count } campi
}
omero-children = { $count ->
    [one] Un elemento
   *[other] { $count } elementi
}
omero-load-more = Carica altro
omero-loading = Caricamento…
omero-empty = Qui non c’è niente
omero-stopped = Interrotto
omero-found = { $count ->
    [one] Un elemento
   *[other] { $count } elementi
}
# $reason why the image could not be opened.
omero-open-failed = Impossibile aprire l’immagine: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Almeno un canale resta visibile
# $server a server's name.
omero-session-ended = La sessione su { $server } è terminata. Inserisci la password per continuare.

# Errors.
# $server a server's name.
omero-error-offline = Impossibile raggiungere { $server }.
omero-error-sign-in = Questo server richiede l’accesso.
omero-error-expired = La sessione è terminata.
omero-error-login = Nome utente o password rifiutati.
omero-error-forbidden = Il tuo account potrebbe non avere accesso a questo.
omero-error-not-found = Non trovato sul server (potrebbe essere stato rimosso).
# $code an HTTP status code.
omero-error-status = Il server ha risposto con l’errore { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Impossibile leggere la risposta del server: { $reason }
