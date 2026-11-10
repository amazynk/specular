# machine-drafted, needs native review
# Specular — interfaccia del visualizzatore, tavolozza dei comandi, errori, Informazioni. Italiano.

-app-name = Specular

## Descrizioni comandi e etichette di accessibilità della barra degli strumenti

toolbar-open = Apri immagine…
toolbar-prev = Immagine precedente
toolbar-next = Immagine successiva
toolbar-back-gallery = Torna alla galleria
toolbar-zoom-out = Riduci
toolbar-zoom-in = Ingrandisci
toolbar-zoom-level = Livello di zoom e preimpostazioni
toolbar-zoom-fit = Adatta alla finestra
toolbar-zoom-100 = Dimensioni reali ({ PERCENT(100) })
toolbar-rotate-left = Ruota in senso antiorario
toolbar-rotate-right = Ruota in senso orario
toolbar-fullscreen-enter = Schermo intero
toolbar-fullscreen-exit = Esci da schermo intero
toolbar-info = Informazioni immagine
toolbar-settings = Impostazioni
toolbar-delete = Sposta nel cestino
toolbar-copy = Copia immagine
toolbar-palette = Tavolozza dei comandi
toolbar-go-to = Vai all’immagine…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Personalizza barra degli strumenti…
toolbar-more = Altro
toolbar-announce-moved = { $item }: posizione { $index } di { $count }
toolbar-announce-added = { $item } aggiunto: posizione { $index } di { $count }
toolbar-announce-removed = { $item } rimosso dalla barra degli strumenti
toolbar-announce-reset = Barra degli strumenti ripristinata
# Un’etichetta seguita dalla sua scorciatoia da tastiera.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Descrizione di un controllo non utilizzabile ora; $reason è uno dei messaggi disabled-*.
tooltip-unavailable = { $label } (non disponibile: { $reason })
# Perché un controllo o un comando non è disponibile.
disabled-no-image = Nessuna immagine aperta
disabled-still-loading = L’immagine è ancora in caricamento
disabled-decode-failed = Impossibile decodificare questa immagine
disabled-no-other-images = Nessun’altra immagine in questa cartella

## Indicatori

# Posizione nella cartella, ad es. "12 / 348".
readout-position = { $index } / { $count }
# Letto dagli screen reader per l’indicatore di posizione.
readout-position-a11y = Immagine { $index } di { $count }
# $zoom è una percentuale già formattata, ad es. "150%".
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Adatta ({ $zoom })
zoom-preset-fit = Adatta alla finestra
zoom-preset-fill = Riempi la finestra
zoom-preset-actual = Dimensioni reali
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Schermo intero
# $zoom è una percentuale già formattata.
zoom-preset-percent = { $zoom }
# Dimensioni dell’immagine in pixel, ad es. "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Numero di immagini nella cartella corrente.
status-image-count =
    { $count ->
        [one] { $count } immagine
       *[other] { $count } immagini
    }

## Tavolozza dei comandi

palette-placeholder = Digita un comando o un’impostazione…
palette-no-results = Nessun comando corrispondente
# Modifica di un’impostazione dalla tavolozza, ad es. "Imposta: Rotella del mouse → Immagine successiva".
palette-set-setting = Imposta: { $setting } → { $value }
palette-next = Immagine successiva
palette-prev = Immagine precedente
palette-first = Prima immagine
palette-last = Ultima immagine
palette-go-to = Vai all’immagine…
palette-zoom-in = Ingrandisci
palette-zoom-out = Riduci
palette-zoom-fit = Adatta alla finestra
palette-zoom-fill = Riempi la finestra
# machine-drafted, needs native review
palette-zoom-fit-width = Adatta alla larghezza
palette-zoom-fit-height = Adatta all’altezza
palette-zoom-100 = Zoom al { PERCENT(100) }
palette-zoom-200 = Zoom al { PERCENT(200) }
palette-zoom-300 = Zoom al { PERCENT(300) }
palette-zoom-400 = Zoom al { PERCENT(400) }
palette-zoom-rectangle = Zoom sulla selezione
palette-rotate-left = Ruota in senso antiorario
palette-rotate-right = Ruota in senso orario
palette-flip-horizontal = Rifletti orizzontalmente
palette-flip-vertical = Rifletti verticalmente
palette-toggle-fullscreen = Attiva/disattiva schermo intero
palette-navigator = Mostra o nascondi navigatore
palette-loupe = Lente
palette-pixel-grid = Mostra o nascondi griglia dei pixel
palette-info-panel = Mostra o nascondi pannello informazioni
palette-copy-image = Copia immagine
# machine-drafted, needs native review
palette-read-codes = Leggi codici
# machine-drafted, needs native review
palette-find-text = Trova testo
# machine-drafted, needs native review
palette-face-boxes = Mostra volti
palette-copy-path = Copia percorso del file
palette-move-to-trash = Sposta nel cestino
palette-undo-trash = Annulla spostamento nel cestino
palette-open = Apri immagine…
# machine-drafted, needs native review
palette-open-address = Apri indirizzo…
palette-settings = Impostazioni
# machine-drafted, needs native review
palette-customise-toolbar = Personalizza barra degli strumenti…
palette-cheat-sheet = Scorciatoie da tastiera
palette-hud = Mostra o nascondi HUD dei tempi di frame
palette-language = Lingua
palette-theme = Tema
# machine-drafted, needs native review
palette-interface-larger = Interfaccia più grande
palette-interface-smaller = Interfaccia più piccola
palette-interface-reset = Interfaccia al { PERCENT(100) }
palette-quit = Esci da { -app-name }

## Scorciatoie

cheatsheet-title = Scorciatoie da tastiera
cheatsheet-navigation = Navigazione
cheatsheet-zoom-pan = Zoom e spostamento
cheatsheet-view = Vista
cheatsheet-file = File
cheatsheet-window = Finestra
cheatsheet-help = Aiuto
cheatsheet-hold = Tieni premuto { $key }
cheatsheet-close-hint = Premi { $key } o Esc per chiudere

## Errori

# machine-drafted, needs native review
viewer-no-preview = Nessuna anteprima in questo file.
error-cannot-open = Impossibile aprire { $file }
error-cannot-open-reason = Impossibile aprire { $file }: { $reason }
error-unsupported-format = { $file } è in un formato che { -app-name } non sa ancora leggere.
error-file-truncated = { $file } è incompleto. Viene mostrata la parte leggibile.
error-file-too-large = { $file } è troppo grande da aprire ({ $megapixels } megapixel; il limite è { $limit } megapixel).
error-not-found = { $file } non esiste più.
error-decoder-stopped = il decodificatore si è fermato senza produrre un’immagine
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = serve una superficie cerebrale (un file .surf.gii con { $vertices } vertici) nella stessa cartella
error-permission-denied = Non hai il permesso di leggere { $file }.
error-details = Dettagli
crash-last-time = L’ultima volta { -app-name } si è chiuso in modo imprevisto; è stato salvato un rapporto in { $path }
error-trash-failed = Impossibile spostare { $file } nel cestino: { $reason }
# machine-drafted, needs native review
# Motivi, mostrati dopo «Impossibile aprire FILE:» e altri messaggi «Impossibile …:», quindi
# iniziano in minuscolo. Dicono cosa è successo e, se utile, cosa fare.
error-reason-not-found = non c’è più. Potrebbe essere stato spostato, rinominato o eliminato.
error-reason-permission = non hai il permesso di leggerlo. Controlla il proprietario e i permessi.
error-reason-disk-full = il disco è pieno. Libera spazio e riprova.
error-reason-read-only = il disco è di sola lettura.
error-reason-truncated = il file è incompleto. Potrebbe essere ancora in copia o in download.
error-reason-is-folder = è una cartella, non un file.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = il server ha negato l’accesso. I dati potrebbero essere privati; per un bucket S3, configura prima le credenziali AWS.
error-reason-remote-missing = non ci sono dati Zarr a quell’indirizzo.
# $status is an HTTP status code ("500").
error-reason-remote-status = il server ha risposto con l’errore { $status }.
error-reason-offline = il server non è raggiungibile. Controlla l’indirizzo e la connessione.
# machine-drafted, needs native review
error-reason-network-off = la rete è disattivata in Impostazioni › Privacy.
error-reason-timed-out = l’unità o il server non ha risposto in tempo. Controlla la connessione e riprova.
error-reason-memory = la memoria non è sufficiente. Chiudi altre immagini o applicazioni e riprova.
error-reason-unsupported = è in un formato che { -app-name } non sa ancora leggere.
# $detail è il testo del decoder stesso, tecnico e in inglese («12-bit lossless JPEG»).
error-reason-unsupported-detail = usa una funzionalità che { -app-name } non sa ancora leggere ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = non è nessuno dei file .{ $ext } che { -app-name } legge ({ $formats }).
error-reason-damaged = il file è danneggiato o non è ciò che dice il suo nome ({ $detail }).
error-reason-too-large = è troppo grande per essere aperto.
# machine-drafted, needs native review
error-reason-needs-memory = richiede { $needed } di RAM libera, ma al momento ne sono liberi solo { $free }. Chiudi alcune altre app e riprova, oppure aprilo su un computer con più memoria.
error-needs-memory = Questa immagine richiede { $needed } di RAM libera, ma al momento ne sono liberi solo { $free }. Chiudi alcune altre app e riprova, oppure aprila su un computer con più memoria.
error-needs-memory-short = Richiede { $needed } di RAM libera
error-reason-cancelled = l’apertura è stata interrotta.
error-reason-trash-access = non c’è più, oppure non hai il permesso di modificarlo.
error-reason-restore-collision = nella sua cartella c’è già un file con lo stesso nome.
error-reason-trash-root = non è possibile spostare un’intera unità nel cestino.
error-reason-not-in-trash = non è più nel cestino.
error-reason-program-missing = il programma non è installato o è stato spostato. Sceglilo di nuovo o reinstallalo.
error-reason-program-not-allowed = il programma non ha il permesso di essere eseguito. Controlla che sia eseguibile.
# machine-drafted, needs native review
error-reason-permission-write = non hai il permesso di modificarlo. Controlla il proprietario e i permessi.
error-reason-file-has-errors = contiene errori. Correggili prima, così non si perde nulla.
error-reason-not-dicom = non è un file DICOM.
error-reason-no-image = non contiene alcuna immagine.
error-reason-needs-layout = è un volume senza intestazione. Per aprirlo servono dimensioni, tipo di campione e ordine dei byte.
error-reason-not-archive = non è un archivio che { -app-name } sa leggere.
error-reason-needs-sandbox = questo tipo di archivio viene aperto solo dal lettore protetto di { -app-name }, che non è disponibile.
error-reason-encrypted = è cifrato e richiede la password.
error-reason-wrong-password = la password è errata.
error-reason-checksum = il file è danneggiato: il contenuto non corrisponde al suo checksum.
error-reason-missing-entry = manca un file al suo interno.
error-reason-busy = è in uso da un altro programma. Chiudilo lì e riprova.
error-reason-catalogue-damaged = il file del catalogo è danneggiato.
error-reason-cannot-open = non è possibile aprire il suo file. Controlla che la cartella esista e che tu possa scriverci.
error-reason-newer-version = è stato scritto da una versione più recente di { -app-name }. Aggiorna { -app-name } per modificarlo.
error-reason-internal = si è verificato un problema interno in { -app-name }. Il log contiene i dettagli.
error-reason-volume-too-few = { $count ->
    [one] una sola sezione non basta per costruire un volume.
    [many] { $count } sezioni non bastano per costruire un volume.
   *[other] { $count } sezioni non bastano per costruire un volume.
}
error-reason-volume-no-geometry = una sezione non indica dove si trova o in quale direzione è orientata.
error-reason-volume-not-parallel = le sezioni non sono parallele.
error-reason-volume-shared-position = più immagini condividono { $count ->
        [one] { $count } posizione
       *[other] ciascuna delle { $count } posizioni
    } (fasi, echi o una serie temporale).
error-reason-volume-missing = { $count ->
        [one] manca circa { $count } sezione.
       *[other] mancano circa { $count } sezioni.
    }
error-reason-volume-mismatch = le sezioni differiscono per dimensioni o spaziatura dei pixel.
error-reason-volume-not-grey = le sezioni sono a colori, non in scala di grigi.
error-reason-volume-skewed = gli assi del volume non sono ortogonali.
error-reason-volume-too-large = richiede { $need } MiB e ne sono consentiti solo { $budget }.
error-reason-url-not-specular = non è un link di { -app-name }.
error-reason-url-too-long = è troppo lungo.
error-reason-url-damaged = è danneggiato. Potrebbe essere stato troncato o modificato durante la copia.
error-reason-url-unknown = richiede qualcosa che questa versione di { -app-name } non conosce. Aggiornare { -app-name } potrebbe aiutare.
error-reason-url-not-absolute = i percorsi dei file devono essere completi.
error-reason-url-no-paths = non indica alcun file.
error-reason-url-too-many = indica troppi file.

## Notifiche

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Impossibile completare: { $reason }
toast-moved-to-trash = Spostato nel cestino
toast-moved-to-trash-named = { $file } spostato nel cestino
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } immagine spostata nel cestino
       *[other] { $count } immagini spostate nel cestino
    }
toast-undo = Annulla
toast-restored = { $file } ripristinato
toast-copied-image = Immagine copiata
toast-copied-path = Percorso copiato
toast-copied-report = Rapporto copiato
toast-copied-value = Valore copiato
toast-loop-start = Di nuovo alla prima immagine
toast-loop-end = Passato all’ultima immagine
# machine-drafted, needs native review
toast-interface-size = Dimensione dell’interfaccia { PERCENT($percent) }
toast-language-changed = Lingua cambiata in { $language }

## Stati vuoti

empty-drop = Trascina qui un’immagine o premi { $shortcut }
empty-open-button = Apri immagine…
empty-drop-release = Rilascia per aprire
empty-folder = Questa cartella non contiene immagini che { -app-name } può aprire.
# machine-drafted, needs native review
# Sotto «Apri immagine…» nella finestra vuota: dove trovare tutto il resto.
empty-hint-keys = Premi { $cheatsheet } per le scorciatoie da tastiera, o { $palette } per trovare qualsiasi comando.
empty-hint-cheatsheet = Premi { $cheatsheet } per le scorciatoie da tastiera.
empty-hint-palette = Premi { $palette } per trovare qualsiasi comando.
# Sotto l’indicatore di caricamento quando un file impiega un po’ ad aprirsi.
viewer-opening = Apertura di { $file }…
fullscreen-exit-hint = Premi Esc per uscire da schermo intero
# Mostrato entrando in schermo intero; se ingrandita, Esc prima reimposta lo zoom.
fullscreen-back-hint = Premi Esc per reimpostare lo zoom o uscire da schermo intero
# Scorciatoie e tavolozza: cosa fa Esc, un passo per pressione.
action-back = Chiudi il pannello (in una finestra di dialogo, prima torna indietro di un passo), reimposta lo zoom, esci dallo schermo intero; nella galleria, sali di una cartella

## Selettore della lingua

language-picker-title = Lingua
# machine-drafted, needs native review
language-picker-search = Cerca lingue
# $percent è una percentuale già formattata, ad es. "87%".
language-completeness = Tradotto al { $percent }
language-system-default = Predefinita di sistema ({ $language })
language-pseudo-accented = Pseudo-lingua: accentata e più lunga
language-pseudo-rtl = Pseudo-lingua: speculare da destra a sinistra

## Informazioni

about-title = Informazioni su { -app-name }
about-version = Versione { $version }
about-tagline = Un visualizzatore di immagini veloce e fedele.
about-copyright = © { $year } Gli autori di { -app-name }
about-license = Distribuito con la licenza MIT License / Apache License 2.0.
about-geonames = Nomi dei luoghi da GeoNames (geonames.org), con licenza Creative Commons Attribution 4.0.
about-unicode = Dati locali dal Unicode CLDR tramite ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Profili degli obiettivi dal database Lensfun (lensfun.github.io), con licenza Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Licenze di terze parti
about-third-party-file = { -app-name } include software e dati di terze parti soggetti alle rispettive licenze; vedi THIRD-PARTY.md, installato con { -app-name }.
about-copy-info = Copia informazioni di sistema
about-close = Chiudi

## Interfaccia del visualizzatore (app Stage 1)

app-close = Chiudi
# Etichetta mostrata su un’immagine il cui file termina in anticipo.
app-truncated-badge = File incompleto
app-info-summary = Riepilogo
app-info-all-fields = Tutti i campi
app-info-copy-value = Copia valore
app-info-copy-field = Copia il valore di { $field }
app-info-loading = Lettura dei metadati…
# machine-drafted, needs native review
# Tutti i campi, per un file che non contiene metadati oltre ai pixel.
app-info-no-fields = Questo file non ha campi di metadati.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = Mappa di guadagno HDR
app-info-gain-map-headroom = Fino a { $headroom }× la luminosità SDR
app-copy-failed = Impossibile copiare: { $reason }
app-restore-failed = Impossibile ripristinare { $file }: { $reason }
app-open-dialog-title = Apri immagine
app-open-dialog-filter = File supportati
app-open-dialog-all = Tutti i file
app-open-group-images = Immagini
app-open-group-raw = RAW della fotocamera
app-open-group-video = Video
app-open-group-sound = Audio
app-open-group-documents = Documenti
app-open-group-drawings = Disegni e CAD
app-open-group-models = Modelli 3D
app-open-group-maps = Mappe e tracce
app-open-group-science = Scienza e medicina
app-open-group-fonts = Caratteri
app-open-group-other = Altro

## Informazioni sul renderer (Informazioni, Impostazioni › Avanzate)

# machine-drafted, needs native review
renderer-title = Informazioni sul renderer
renderer-section-adapter = Scheda grafica
renderer-section-limits = Limiti e funzionalità
renderer-section-display = Schermo
renderer-section-memory = Memoria
renderer-section-decoders = Decoder opzionali
renderer-unknown = Sconosciuto
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ID fornitore e dispositivo
# Wayland, X11, Windows or macOS.
renderer-display-server = Server grafico
renderer-features = Funzionalità usate
renderer-surface-format = Formato della finestra
renderer-surface-formats = Formati offerti
renderer-present-mode = Modalità di presentazione
renderer-present-modes = Modalità offerte
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Attivo (margine { $headroom }×)
renderer-hdr-off = Disattivato (lo schermo può mostrare l’HDR)
renderer-hdr-unavailable = Non disponibile su questo schermo
renderer-decode-budget = La decodifica può usare
renderer-decode-budget-detail = L’{ PERCENT(80) } della memoria libera, condiviso tra tutte le immagini in apertura.
renderer-display-cache = Cache di visualizzazione

## HUD delle prestazioni (F12)

app-hud-scale = Scala
app-hud-level = Livello
app-hud-level-thumbnail = Miniatura incorporata
app-hud-level-full = Risoluzione piena
# $denominator è 2, 4 o 8: l’immagine decodificata a 1/2, 1/4 o 1/8 della dimensione.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Apertura
app-hud-decode = Decodifica
app-hud-upload = Caricamento
app-hud-frame = Frame
app-hud-threads = Thread
app-hud-threads-value = { $threads } ({ $queued } in coda)
app-hud-tiles = Riquadri
app-hud-tiles-value = { $tiles } di { $capacity }
app-hud-ram = Memoria
app-hud-vram = Memoria GPU
app-hud-gpu = GPU

# Conferma dello spostamento nel cestino
app-confirm-trash-title = Spostare { $file } nel cestino?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Anche i file collaterali vanno nel cestino: { $files }
app-cancel = Annulla
# Avviso mostrato prima di decodificare un’immagine molto grande. $need e $available sono
# quantità di memoria ("3,2 GiB").
toast-low-memory = Questa immagine potrebbe richiedere circa { $need } di memoria, ma ne sono disponibili solo { $available }. Potrebbe caricarsi lentamente o non riuscire.

# Il rapporto hardware (Impostazioni → Avanzate). I valori sono inseriti dall’app.
hw-report-title = Rapporto hardware
hw-report-copy = Copia rapporto
hw-report-version = Versione
hw-report-cpu = CPU
hw-report-cores = Core
hw-report-cores-value = { $logical } logici, { $physical } fisici
hw-report-performance-cores = Core ad alte prestazioni
hw-report-cpu-features = Percorsi rapidi della CPU
hw-report-none = Nessuno
hw-report-memory = Memoria
hw-report-memory-value = { $total } in totale, { $available } disponibili
hw-report-graphics-api = API grafica
hw-report-driver = Driver
hw-report-max-texture = Texture più grande
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Thread di decodifica
hw-report-ram-cache = Cache delle immagini in memoria
hw-report-vram-cache = Cache della memoria GPU
hw-report-processing = Elaborazione
hw-report-path-jpeg-decode = Decodifica JPEG
hw-report-path-ycbcr-to-rgb = Conversione del colore
hw-report-path-png-unfilter = Decodifica PNG
hw-report-path-heic-decode = Decodifica HEIC
hw-report-path-inference = Analisi delle immagini
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, in parallelo
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = decodificatore hardware
hw-report-engine-npu = NPU
hw-report-build-debug = build di debug
hw-report-build-release = build di release

## Accessibilità (screen reader)
# L’immagine come la annuncia uno screen reader: nome del file e dimensioni in pixel.
a11y-image = { $name }, { $width } × { $height } pixel
# L’area in cui compaiono le notifiche brevi.
a11y-notifications = Notifiche
# machine-drafted, needs native review
a11y-transport-playback = Riproduzione
a11y-transport-pages = Pagine
a11y-settings-sections = Sezioni

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Codici in questa immagine
codes-looking = Ricerca dei codici…
codes-none = Nessun codice trovato in questa immagine.
codes-failed = Impossibile leggere l’immagine: { $reason }
codes-copy = Copia «{ $text }»
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Apri link su { $host }…
codes-network-off = Nessuna rete è attiva, quindi i link non si aprono.
codes-open-title = Aprire un link a { $host }?
codes-open-detail = Si apre nel browser web. Apri solo link di cui ti fidi.
codes-open = Apri nel browser
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Testo in questa immagine
# machine-drafted, needs native review
ocr-looking = Lettura del testo…
# machine-drafted, needs native review
ocr-none = Nessun testo trovato in questa immagine.
# machine-drafted, needs native review
ocr-failed = Impossibile leggere il testo: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Trova testo richiede i modelli di testo del pacchetto IA locale (Impostazioni › IA locale).
# machine-drafted, needs native review
ocr-loading = Caricamento dei modelli di testo…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Copia «{ $text }»
# machine-drafted, needs native review
ocr-copy-all = Copia tutto il testo
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Affidabilità: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Sistema
hw-report-section-gpu = GPU
hw-report-section-decoding = Decodifica
hw-report-operating-system = Sistema operativo
hw-report-adapter = Adattatore
hw-report-gpu-type = Tipo
hw-report-gpu-init = Tempo di avvio
hw-report-max-layers = Livelli di texture
hw-report-compression = Compressione delle texture
hw-report-unified-memory = Memoria condivisa con la CPU
hw-report-display = Schermo
hw-report-hw-decode = Decodifica hardware
hw-report-core-cluster = Gruppo di core
hw-report-power = Alimentazione
hw-report-calibration = Misurazioni di velocità
hw-report-codec-others = Altri
hw-report-performance-cores-value = { $count } su { $total }
hw-report-cluster-value = { $name }: { $physical } fisici, { $logical } logici
hw-report-thermal-nominal = Temperatura normale
hw-report-thermal-fair = Caldo
hw-report-thermal-serious = Molto caldo, rallentato
hw-report-thermal-critical = Surriscaldato
hw-report-power-battery = A batteria ({ $percent })
hw-report-power-battery-unknown = A batteria
hw-report-power-mains = Collegato alla corrente
hw-report-power-mains-percent = Collegato alla corrente ({ $percent })
hw-report-power-saving = Risparmio energetico attivo
hw-report-gpu-discrete = Dedicato
hw-report-gpu-integrated = Integrato
hw-report-gpu-virtual = Virtuale
hw-report-gpu-software = Software, sulla CPU
hw-report-gpu-other = Altro
hw-report-gpu-unavailable = Non disponibile
hw-report-ms = { $ms } ms
hw-report-yes = Sì
hw-report-no = No
hw-report-unified-platform-differs = Il sistema indica diversamente
hw-report-display-value = { $fps } Hz, margine HDR { $now } ora, fino a { $max }
hw-report-hw-decode-value = { $api } su { $device }
hw-report-hw-decode-checking = Verifica in corso…
hw-report-codec-up-to = Fino a { $width } × { $height }
hw-report-codec-none = Non disponibile
hw-report-codec-h264-encode = Codifica H.264
hw-report-supported = supportati: { $list }
hw-report-unsupported = non supportati: { $list }
hw-report-calibration-done = Eseguite su questo computer
hw-report-calibration-pending = Non ancora eseguite
hw-report-paths-power-saving = Risparmio energetico: si preferiscono i percorsi che consumano meno energia
hw-report-paths-caption = Percorsi per un’immagine da { $mpx } megapixel
hw-report-col-task = Operazione
hw-report-col-path = Percorso
hw-report-col-time = Tempo
hw-report-col-cpu = Tempo CPU (tutti i thread)
hw-report-col-runs = Misurazioni
hw-report-col-use = Usato per
hw-report-runs-untested = Non testato
hw-report-use-viewing = Visualizzazione ({ $reason })
hw-report-use-background = In background ({ $reason })
hw-report-reason-only = l’unica opzione
hw-report-reason-faster = il più veloce
hw-report-reason-cheaper = meno tempo CPU
hw-report-reason-lower-power = meno energia
hw-report-reason-explore = in prova
hw-report-reason-forced = scelto nelle Impostazioni
hw-report-reason-prefer-gpu = Preferisci la GPU
hw-report-reason-cpu-only = Solo CPU
hw-report-reason-calibrate = in misurazione
hw-report-reason-scaled = dimensioni ridotte
hw-report-trust-unverified = Non ancora verificato
hw-report-trust-blocked = Disattivato dopo un errore
hw-report-trust-absent = Non presente su questo computer

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galleria
cheatsheet-mode-compare = Confronta
cheatsheet-gallery-browse = Sfoglia
cheatsheet-gallery-select = Seleziona
cheatsheet-gallery-mark = Valuta e contrassegna
cheatsheet-gallery-move = Spostati tra le immagini
cheatsheet-gallery-page = Pagina su o giù
cheatsheet-gallery-ends = Prima o ultima immagine
cheatsheet-gallery-up = Sali alla cartella superiore
cheatsheet-gallery-history-back = Torna alla cartella precedente
cheatsheet-gallery-history-forward = Avanti alla cartella successiva
cheatsheet-gallery-back = Chiudi la vista in primo piano, poi torna indietro
cheatsheet-gallery-viewer = Torna al visualizzatore
cheatsheet-gallery-album-add = Aggiungi all’ultimo album
cheatsheet-gallery-select-one = Seleziona o deseleziona
cheatsheet-gallery-quick-look = Anteprima rapida
cheatsheet-gallery-filter = Filtra
cheatsheet-gallery-bigger = Miniature più grandi
cheatsheet-gallery-smaller = Miniature più piccole
cheatsheet-gallery-default-size = Dimensione standard delle miniature
cheatsheet-gallery-refresh = Aggiorna la cartella
cheatsheet-gallery-menu = Menu contestuale
cheatsheet-compare-views = Confronta
cheatsheet-compare-overlay = Sovrapposizione o affiancate
cheatsheet-compare-difference = Differenza (di nuovo: tipo successivo)
cheatsheet-compare-step-down = Meno dissolvenza o guadagno della differenza
cheatsheet-compare-step-up = Più dissolvenza o guadagno della differenza
cheatsheet-compare-pick = Immagine su cui agiscono i tasti
cheatsheet-compare-close = Chiudi il confronto
cheatsheet-channels = Canali
cheatsheet-medical-window = Finestra e livello
cheatsheet-medical-measure = Misura e annota
cheatsheet-medical-frames = Fotogrammi e serie
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D e ricostruzione
cheatsheet-medical-analysis = Segmentazione, fusione e PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Studi e condivisione
cheatsheet-palette-only-title = Nella tavolozza dei comandi ({ $key })
cheatsheet-palette-only-title-nokey = Nella tavolozza dei comandi
cheatsheet-palette-only-tag = Tavolozza
cheatsheet-palette-only-line = { $action }: nessun tasto; nella tavolozza dei comandi ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: nessun tasto; nella tavolozza dei comandi
# Esc nel promemoria scorciatoie (la descrizione lunga della tavolozza non ci sta).
cheatsheet-back = Chiudi o torna indietro

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Esportazione video
hw-report-engine-hw-encoder = codificatore hardware

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (per miniatura da { $size })
hw-report-task-frame = { $task } (per fotogramma da { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Solo software

about-natural-earth = Dati cartografici di Natural Earth (naturalearthdata.com), pubblico dominio.
app-info-show-on-map = Mostra sulla mappa
