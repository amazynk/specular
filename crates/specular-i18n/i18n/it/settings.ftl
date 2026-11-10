# machine-drafted, needs native review
# Specular — finestra Impostazioni. Italiano.

## Elementi della finestra

settings-title = Impostazioni
settings-search-placeholder = Cerca impostazioni
settings-search-no-results = Nessuna impostazione corrisponde a { $query }
settings-reset = Ripristina
settings-reset-section = Ripristina sezione
settings-modified = Modificata
settings-default-value = Predefinito: { $value }
settings-restart-required = Ha effetto dopo il riavvio
settings-restart-now = Riavvia ora
settings-undo = Annulla
settings-redo = Ripeti
settings-value-on = Attivo
settings-value-off = Disattivo
settings-value-auto = Automatico
settings-value-system = Segui il sistema
settings-value-system-language = Predefinita di sistema
settings-value-follow-language = Segui la lingua dell’interfaccia
settings-value-default-location = Posizione predefinita
settings-choose = Scegli…
settings-clear = Cancella
settings-file-reloaded = settings.toml è stato modificato al di fuori di { -app-name } ed è stato ricaricato.
settings-warning-unknown-key = Impostazione sconosciuta { $key } in settings.toml. Resta nel file ma viene ignorata.
settings-warning-invalid-value = Valore non valido per { $key }: { $reason } Viene usato il valore predefinito.
settings-warning-newer-version = settings.toml è stato scritto da una versione più recente di { -app-name }. Le impostazioni sconosciute a questa versione vengono conservate.
settings-warning-syntax = Impossibile leggere settings.toml (riga { $line }): { $reason } Vengono usati i valori predefiniti finché non viene corretto.

## Errori di convalida (stampati anche da `specular --set`)

settings-error-unknown-key = Non esiste un’impostazione chiamata { $key }.
settings-error-unknown-key-suggestion = Non esiste un’impostazione chiamata { $key }. Forse intendevi { $suggestion }?
settings-error-not-bool = Atteso on o off (true o false).
settings-error-not-integer = Atteso un numero intero.
settings-error-not-number = Atteso un numero.
settings-error-out-of-range = Deve essere compreso tra { $min } e { $max }.
settings-error-out-of-range-auto = Deve essere 0 (automatico) o compreso tra { $min } e { $max }.
settings-error-unknown-option = Atteso uno tra: { $options }.
settings-error-bad-colour = Atteso un colore come #1E1E1E o #1E1E1E80.
settings-error-bad-language = Atteso un codice lingua come de, pt-BR o zh-Hans, oppure «system».
settings-error-bad-path = Atteso il percorso di un file o di una cartella.
settings-error-not-settable = Questa voce è un comando e non ha un valore.
settings-error-syntax = Atteso chiave=valore, ad esempio viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Non può contenere tabulazioni, a capo o altri caratteri di controllo.
settings-error-invalid = { $reason }
settings-error-tile-url = Serve un indirizzo http o https con {"{"}z{"}"}, {"{"}x{"}"} e {"{"}y{"}"}.

## Unità

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } immagine
       *[other] { $value } immagini
    }
unit-threads =
    { $value ->
        [one] { $value } thread
       *[other] { $value } thread
    }
unit-bytes =
    { $value ->
        [one] { $value } byte
       *[other] { $value } byte
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Sezioni

settings-section-general = Generali
settings-section-viewing = Visualizzazione
settings-section-browsing = Navigazione
settings-section-colour = Colore
settings-section-metadata = Metadati e privacy
# machine-drafted, needs native review
settings-section-privacy = Privacy
settings-section-performance = Prestazioni
settings-section-accessibility = Accessibilità
settings-section-keys = Tasti
settings-section-advanced = Avanzate

## Generali

settings-general-language = Lingua dell’interfaccia
settings-general-language-help = Le modifiche hanno effetto subito. «Predefinita di sistema» segue la lingua del sistema operativo.
settings-general-region-formats = Formati regionali
settings-general-region-formats-help = Come vengono scritti date, orari, numeri e dimensioni. Per impostazione predefinita seguono la lingua dell’interfaccia.
settings-general-theme = Tema
settings-general-theme-help = Aspetto chiaro o scuro per finestre e controlli. Le immagini non vengono mai modificate.
settings-general-theme-system = Segui il sistema
settings-general-theme-light = Chiaro
settings-general-theme-dark = Scuro
# machine-drafted, needs native review
settings-general-on-start = Quando { -app-name } si apre senza un file
settings-general-on-start-help = Cosa mostrare quando { -app-name } viene aperto da solo. Un file aperto dal file manager o dalla riga di comando appare sempre nel visualizzatore.
settings-general-on-start-start-screen = La schermata iniziale
settings-general-on-start-gallery = Galleria dell’ultima cartella
settings-general-on-start-library = Libreria
settings-general-start-tips = Mostra «Da provare» nella schermata iniziale
settings-general-start-tips-help = Una fila di schede con funzioni utili e i loro tasti. «Mostra benvenuto» nella tavolozza dei comandi la riporta.
settings-general-tips = Mostra suggerimenti
settings-general-tips-help = Un breve suggerimento, una sola volta, la prima volta che una funzione può aiutare, come il tasto che mostra una cartella come galleria. Mai durante una presentazione.
settings-general-tips-shown = Suggerimenti già mostrati
settings-general-tips-shown-help = I suggerimenti unici già dati, per non ripeterli.
# machine-drafted, needs native review
settings-general-your-name = Il tuo nome
settings-general-your-name-help = Compare sui timbri e sulle annotazioni che aggiungi. Salvato solo nei file di annotazioni che crei.
settings-general-single-instance = Apri i file nella finestra esistente
settings-general-single-instance-help = Se { -app-name } è già in esecuzione, un’altra immagine aperta viene mostrata in quella finestra invece di avviarne una nuova.
settings-general-file-associations = Associazioni dei file…
settings-general-file-associations-help = Imposta { -app-name } come visualizzatore predefinito per i formati di immagine supportati.

## Visualizzazione

settings-viewing-initial-zoom = Zoom iniziale
settings-viewing-initial-zoom-help = Con quale grandezza viene mostrata un’immagine appena aperta.
settings-viewing-initial-zoom-fit = Adatta alla finestra
settings-viewing-initial-zoom-fit-if-larger = Adatta le immagini grandi, le piccole al { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Dimensioni reali ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Riempi la finestra
settings-viewing-wheel-behaviour = Rotella del mouse
settings-viewing-wheel-behaviour-help = Cosa fa la rotella. Tieni premuto Ctrl per l’altra azione.
settings-viewing-wheel-behaviour-help-macos = Cosa fa la rotella. Tieni premuto ⌘ per l’altra azione.
settings-viewing-wheel-behaviour-zoom = Zoom
settings-viewing-wheel-behaviour-navigate = Immagine successiva e precedente
settings-viewing-keep-zoom = Mantieni lo zoom tra le immagini
settings-viewing-keep-zoom-help = Con lo zoom attivo, l’immagine successiva e la precedente si aprono con lo stesso zoom e mostrano la stessa parte dell’inquadratura: utile per controllare la messa a fuoco di una raffica.
settings-viewing-zoom-animation = Anima lo zoom
settings-viewing-zoom-animation-help = Zoom fluido invece che a scatti. Disattivato automaticamente quando «Riduci movimento» è attivo.
settings-viewing-zoom-animation-speed = Velocità animazione zoom
settings-viewing-zoom-animation-speed-help = Quanto velocemente si assestano gli zoom animati.
settings-viewing-zoom-animation-speed-relaxed = Rilassata
settings-viewing-zoom-animation-speed-normal = Normale
settings-viewing-zoom-animation-speed-fast = Veloce
settings-viewing-background = Sfondo
settings-viewing-background-help = Il colore dietro l’immagine in una finestra.
settings-viewing-background-theme = Come il tema
settings-viewing-background-black = Nero
settings-viewing-background-dark-grey = Grigio scuro
settings-viewing-background-light-grey = Grigio chiaro
settings-viewing-background-white = Bianco
settings-viewing-background-custom = Colore personalizzato
settings-viewing-background-color = Colore di sfondo personalizzato
settings-viewing-background-color-help = Usato quando Sfondo è impostato su «Colore personalizzato».
settings-viewing-fullscreen-background = Sfondo a schermo intero
settings-viewing-fullscreen-background-help = Il colore dietro l’immagine a schermo intero.
settings-viewing-transparency = Aree trasparenti
settings-viewing-transparency-help = Come vengono disegnate le parti trasparenti di un’immagine.
settings-viewing-transparency-checkerboard = Scacchiera
settings-viewing-transparency-background = Colore di sfondo
settings-viewing-transparency-black = Nero
settings-viewing-transparency-white = Bianco
settings-viewing-pixel-grid-threshold = Mostra griglia dei pixel da
settings-viewing-pixel-grid-threshold-help = Disegna una griglia tra i pixel quando lo zoom è almeno a questo livello.
settings-viewing-smoothing-high-zoom = Pixel ingranditi
settings-viewing-smoothing-high-zoom-help = Come appaiono i pixel quando l’immagine è mostrata oltre il { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Sfumati, poi nitidi dal { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Sempre sfumati
settings-viewing-smoothing-high-zoom-pixelated = Sempre quadrati nitidi
settings-viewing-double-click = Doppio clic sull’immagine
settings-viewing-double-click-help = Il doppio clic sullo sfondo vuoto attiva o disattiva sempre lo schermo intero.
settings-viewing-double-click-toggle-zoom = Alterna tra adatta e { PERCENT(100) }
settings-viewing-double-click-fullscreen = Attiva/disattiva schermo intero
settings-viewing-double-click-nothing = Nessuna azione
settings-viewing-fullscreen-on-open = Apri le immagini a schermo intero
settings-viewing-fullscreen-on-open-help = Avvia a schermo intero quando un’immagine viene aperta dall’esterno di { -app-name }.
settings-viewing-fullscreen-mode = Stile dello schermo intero
settings-viewing-fullscreen-mode-help = Lo schermo intero nativo usa uno Space proprio; quello istantaneo copre lo Space attuale senza animazione.
settings-viewing-fullscreen-mode-native = Nativo (Space proprio)
settings-viewing-fullscreen-mode-borderless = Istantaneo
settings-viewing-toolbar-autohide = Nascondi automaticamente la barra degli strumenti
settings-viewing-toolbar-autohide-help = La barra degli strumenti e le frecce ai bordi scompaiono quando il puntatore è fermo e ricompaiono quando si muove.
settings-viewing-toolbar-autohide-delay = Nascondi la barra degli strumenti dopo
settings-viewing-toolbar-autohide-delay-help = Per quanto tempo il puntatore deve restare fermo prima che la barra scompaia.
settings-viewing-toolbar-items = Pulsanti della barra degli strumenti
settings-viewing-toolbar-items-help = Quali pulsanti mostra la barra degli strumenti, e in che ordine.
settings-viewing-toolbar-drag = Riordina la barra degli strumenti trascinando
settings-viewing-toolbar-drag-help = Trascina un pulsante lungo la barra degli strumenti per spostarlo. Un clic breve lo preme comunque; Esc riporta al suo posto un pulsante trascinato.
settings-viewing-toolbar-items-open = Apri
settings-viewing-toolbar-items-prev = Immagine precedente
settings-viewing-toolbar-items-next = Immagine successiva
settings-viewing-toolbar-items-zoom-out = Riduci
settings-viewing-toolbar-items-zoom-level = Livello di zoom
settings-viewing-toolbar-items-zoom-in = Ingrandisci
settings-viewing-toolbar-items-fit = Adatta alla finestra
settings-viewing-toolbar-items-rotate-left = Ruota in senso antiorario
settings-viewing-toolbar-items-rotate-right = Ruota in senso orario
settings-viewing-toolbar-items-info = Informazioni
settings-viewing-toolbar-items-copy = Copia immagine
settings-viewing-toolbar-items-delete = Sposta nel cestino
settings-viewing-toolbar-items-palette = Tavolozza dei comandi
settings-viewing-toolbar-items-settings = Impostazioni
settings-viewing-toolbar-items-fullscreen = Schermo intero
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Spazio flessibile
settings-viewing-toolbar-items-separator = Separatore
settings-toolbar-preview = Anteprima della barra
settings-toolbar-hidden = Pulsanti nascosti
settings-toolbar-add-spacer = Aggiungi spazio flessibile
settings-toolbar-add-separator = Aggiungi separatore
settings-toolbar-reset = Ripristina predefiniti
settings-toolbar-remove = Rimuovi { $item }
settings-toolbar-hint = Trascina una riga per spostarla, oppure selezionala e premi Alt+↑ o Alt+↓. Tab passa tra il campo di ricerca e l’elenco.
settings-toolbar-move-up = Sposta su
settings-toolbar-move-down = Sposta giù
settings-toolbar-move-start = Sposta all’inizio
settings-toolbar-move-end = Sposta alla fine
settings-toolbar-menu-hide = Nascondi
settings-toolbar-menu-show = Mostra
settings-toolbar-menu-remove = Rimuovi
settings-toolbar-row-menu = Altre azioni per { $item }
settings-toolbar-position = Posizione { $index } di { $count }
settings-toolbar-hidden-state = Nascosto

## Navigazione

settings-browsing-sort-order = Ordina le immagini per
settings-browsing-sort-order-help = L’ordine usato per scorrere una cartella. L’ordine per nome mette «img2» prima di «img10» e segue l’alfabeto della tua lingua.
settings-browsing-sort-order-natural = Nome
settings-browsing-sort-order-date-taken = Data di scatto
settings-browsing-sort-order-modified = Data di modifica
settings-browsing-sort-order-size = Dimensione file
settings-browsing-sort-order-type = Tipo di file
settings-browsing-sort-descending = Ordine inverso
settings-browsing-sort-descending-help = Ordina dall’ultimo al primo.
settings-browsing-loop-folder = Ricomincia alla fine della cartella
settings-browsing-loop-folder-help = Dopo l’ultima immagine prosegui con la prima, e viceversa.
settings-browsing-include-subfolders = Includi sottocartelle
settings-browsing-include-subfolders-help = Scorri anche le immagini nelle sottocartelle.
settings-browsing-hold-acceleration = Accelera tenendo premuto avanti o indietro
settings-browsing-hold-acceleration-help = Quanto accelera lo scorrimento mentre un tasto o un pulsante è tenuto premuto.
settings-browsing-hold-acceleration-off = Nessuna accelerazione
settings-browsing-hold-acceleration-gentle = Leggera
settings-browsing-hold-acceleration-normal = Normale
settings-browsing-hold-acceleration-fast = Veloce
settings-browsing-confirm-trash = Chiedi prima di spostare nel cestino
settings-browsing-confirm-trash-help = Lo spostamento nel cestino si può sempre annullare, quindi per impostazione predefinita { -app-name } non chiede conferma.
settings-browsing-arrows-follow-reading-direction = Frecce secondo la direzione di lettura
settings-browsing-arrows-follow-reading-direction-help = Nelle lingue da destra a sinistra, il tasto freccia sinistra passa all’immagine successiva.

## Colore

settings-colour-management = Gestione del colore
settings-colour-management-help = Mostra i colori come dovrebbero apparire, usando il profilo colore dell’immagine e quello dello schermo.
settings-colour-display-profile = Profilo dello schermo
settings-colour-display-profile-help = Il profilo colore dello schermo. Automatico usa quello indicato dal sistema operativo.
settings-colour-rendering-intent = Intento di rendering
settings-colour-rendering-intent-help = Come i colori che lo schermo non può mostrare vengono riportati nella sua gamma.
settings-colour-rendering-intent-perceptual = Percettivo
settings-colour-rendering-intent-relative-colorimetric = Colorimetrico relativo
settings-colour-rendering-intent-saturation = Saturazione
settings-colour-rendering-intent-absolute-colorimetric = Colorimetrico assoluto
settings-colour-black-point-compensation = Compensazione del punto nero
settings-colour-black-point-compensation-help = Conserva i dettagli nelle ombre quando il nero dello schermo è più chiaro di quello dell’immagine.
settings-colour-untagged-images = Immagini senza profilo colore
settings-colour-untagged-images-help = Come mostrare le immagini che non indicano quali colori usano.
settings-colour-untagged-images-srgb = Tratta come sRGB
settings-colour-untagged-images-display = Mostra senza modifiche
settings-colour-dithering = Dithering
settings-colour-dithering-help = Aggiunge un leggero rumore blu quando si riduce la profondità del colore, evitando le bande in cieli e sfumature.
settings-colour-hdr-output = Uscita HDR
settings-colour-hdr-output-help = Mostra le immagini HDR con la loro piena luminosità sugli schermi che lo supportano. Automatico usa l’HDR quando lo schermo segnala luminosità extra; altrimenti le aree luminose vengono compresse delicatamente.
settings-colour-hdr-output-auto = Automatico
settings-colour-hdr-output-on = Attivo
settings-colour-hdr-output-off = Disattivo
# machine-drafted, needs native review
settings-colour-ocio-config = Configurazione OCIO
settings-colour-ocio-config-help = Una configurazione OpenColorIO (.ocio) per immagini EXR, Radiance HDR e PFM. Se vuoto, usa quella indicata da $OCIO.
settings-colour-ocio-display = Display OCIO
settings-colour-ocio-display-help = Il display della configurazione OCIO che corrisponde al tuo schermo.
settings-colour-ocio-display-default = Il primo della configurazione
settings-colour-ocio-view = Vista OCIO
settings-colour-ocio-view-help = Come le immagini scene-linear vengono mostrate su quel display, ad esempio con una vista con tone mapping.
settings-colour-ocio-view-default = La prima del display
settings-colour-ocio-input = Spazio colore di ingresso OCIO
settings-colour-ocio-input-help = Lo spazio colore dei valori delle immagini.
settings-colour-ocio-input-default = Il ruolo scene-linear della configurazione
# machine-drafted, needs native review
settings-colour-proof-profile = Profilo della prova a video
settings-colour-proof-profile-help = Il profilo stampante usato per ultimo dalla prova a video di Sviluppo.
settings-colour-proof-intent = Intento della prova a video
settings-colour-proof-intent-help = Come vengono adattati i colori che la stampante non può stampare.
settings-colour-proof-intent-perceptual = Percettivo
settings-colour-proof-intent-relative = Colorimetrico relativo
settings-colour-proof-bpc = Compensazione del punto nero nella prova
settings-colour-proof-bpc-help = Porta il nero al nero della stampante.
settings-colour-proof-paper = Bianco carta nella prova
settings-colour-proof-paper-help = Mostra il colore proprio della carta.
settings-colour-proof-ink = Nero inchiostro nella prova
settings-colour-proof-ink-help = Mostra quanto è scuro il nero dell’inchiostro.

## Metadati e privacy

settings-metadata-show-info-on-open = Mostra il pannello informazioni all’apertura
settings-metadata-show-info-on-open-help = Apri il pannello con fotocamera, data e luogo ogni volta che si apre un’immagine.
settings-metadata-info-detail = Il pannello informazioni mostra
settings-metadata-info-detail-help = Il riepilogo spiega i dettagli più utili in linguaggio semplice; tutti i campi elenca tutto ciò che è salvato nel file.
settings-metadata-info-detail-summary = Riepilogo
settings-metadata-info-detail-all = Tutti i campi
settings-metadata-reverse-geocoding = Mostra i nomi dei luoghi per le posizioni GPS
settings-metadata-reverse-geocoding-help = Cerca la località più vicina nei dati integrati. Funziona offline: le posizioni non lasciano mai il computer.
settings-metadata-hide-gps = Nascondi le posizioni GPS sullo schermo
settings-metadata-hide-gps-help = Non mostrare coordinate o luoghi, ad esempio durante la condivisione dello schermo. I file non vengono modificati.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Salva le modifiche di Sviluppo accanto alla foto
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Scrive IMG_1234.CR2.specular.xmp accanto al file, così le modifiche viaggiano con esso. Disattivato: le modifiche restano solo nella libreria.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importa impostazioni Lightroom in Sviluppo
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Una foto con impostazioni Lightroom e senza modifiche si apre con esse, in modo approssimativo. Disattivato: Sviluppo offre un pulsante.
settings-metadata-units = Unità
settings-metadata-units-help = Unità per altitudine, distanze e velocità.
settings-metadata-units-auto = Segui i formati regionali
settings-metadata-units-metric = Metriche
settings-metadata-units-imperial = Imperiali
# machine-drafted, needs native review
settings-metadata-basemap = Mappa di base
settings-metadata-basemap-help = La mappa sotto le posizioni delle foto. Le mappe online scaricano i riquadri della zona visualizzata.
settings-metadata-basemap-off = Solo mappa integrata (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Altro server di riquadri (online)
settings-metadata-basemap-file = Il mio file di mappa
settings-metadata-basemap-url = Indirizzo del server di riquadri
settings-metadata-basemap-url-help = Un indirizzo con {"{"}z{"}"}, {"{"}x{"}"} e {"{"}y{"}"}, ad esempio https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = File di mappa
settings-metadata-basemap-path-help = Un file MBTiles o PMTiles. Funziona offline.
settings-metadata-basemap-cache = Cache dei riquadri di mappa
settings-metadata-basemap-cache-help = Spazio su disco per i riquadri scaricati. I più vecchi vengono rimossi per primi.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Server di riquadri consentiti
settings-metadata-basemap-allowed-help = Server di mappe da cui hai accettato di scaricare riquadri.

## Prestazioni

settings-performance-ram-cache = Cache in memoria
settings-performance-ram-cache-help = Memoria usata per tenere pronte le immagini decodificate. Automatico sceglie in base alla memoria del computer.
settings-performance-vram-cache = Cache nella memoria GPU
settings-performance-vram-cache-help = Memoria GPU usata per immagini e riquadri. Automatico sceglie in base alla GPU.
settings-performance-prefetch-depth = Immagini da preparare in anticipo
settings-performance-prefetch-depth-help = Quante immagini nella direzione di scorrimento vengono decodificate in anticipo.
settings-performance-decoder-threads = Thread di decodifica
settings-performance-decoder-threads-help = Thread della CPU usati per decodificare le immagini. Automatico usa tutti i core ad alte prestazioni e si adatta al file.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Quale GPU usare quando il computer ne ha più di una.
settings-performance-gpu-adapter-high-performance = Prestazioni massime
settings-performance-gpu-adapter-low-power = Consumo minimo
settings-performance-processing = Dove vengono elaborate le immagini
settings-performance-processing-help = Alcune operazioni possono essere eseguite dalla CPU, dalla GPU o da un decodificatore hardware. Automatico misura questo computer e sceglie il modo più veloce per l’immagine che stai guardando e quello più economico per il lavoro in background. Ciò che ha scelto su questo computer, e perché, è indicato qui sotto.
settings-performance-processing-automatic = Automatico
settings-performance-processing-prefer-gpu = Preferisci la GPU
settings-performance-processing-cpu-only = Solo CPU
settings-performance-drag-preview = Anteprima durante il trascinamento
settings-performance-drag-preview-help = Come Sviluppo mostra la foto mentre trascini un cursore. Automatico usa un’immagine più piccola solo quando il computer non tiene il passo.
settings-performance-drag-preview-automatic = Automatico
settings-performance-drag-preview-sharp = Sempre nitida
settings-performance-drag-preview-fast = Sempre veloce

## Impostazioni → Prestazioni → Come funziona { -app-name } su questo computer
settings-paths-title = Come funziona { -app-name } su questo computer
settings-paths-intro = { -app-name } misura i tempi di ogni modo di svolgere queste operazioni su questo computer e usa il migliore. I tempi si riferiscono a una foto da { $mpx } megapixel.
settings-paths-intro-measuring = Misurazioni non ancora completate: { -app-name } misura quando il computer è inattivo.
settings-paths-intro-battery = A batteria: le scelte privilegiano un minor consumo di energia.
settings-paths-keywords = GPU CPU NPU scheda grafica accelerazione hardware decodificatore benchmark velocità prestazioni
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, tutti i core
paths-engine-gpu = GPU
paths-engine-hw-decoder = Decodificatore hardware
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (la tua impostazione)
settings-paths-measured-faster = Misurato su questo computer: { $ratio } volte più veloce che tramite { $other }.
settings-paths-measured-similar = Misurato su questo computer: veloce circa quanto tramite { $other }.
settings-paths-measured-power = A batteria consuma meno energia (misurato su questo computer).
settings-paths-not-measured = Non ancora misurato: la scelta abituale per questo tipo di computer.
settings-paths-set-by = Scelto dalla tua impostazione «{ $setting }: { $option }».
settings-paths-blocked-check = L'elaborazione tramite { $engine } è stata disattivata il { $date }: il risultato non corrispondeva a quello della CPU.
settings-paths-blocked-error = L'elaborazione tramite { $engine } è stata disattivata il { $date } dopo un errore.
settings-paths-blocked-lost = L'elaborazione tramite { $engine } è stata disattivata il { $date } perché ha smesso di rispondere.
settings-paths-blocked-undated = L'elaborazione tramite { $engine } è stata disattivata dopo un errore.
settings-paths-checking = L'elaborazione tramite { $engine } viene confrontata con la CPU prima di essere usata per l’immagine che stai guardando.
settings-paths-exploring = Prova dell'elaborazione tramite { $engine } in background in corso.
settings-paths-unavailable = Questo computer non dispone di { $engine } per questa operazione.
settings-paths-background-cheaper = In background: elaborazione tramite { $engine }, che secondo le misurazioni usa il { $percent } di tempo CPU in meno.
settings-paths-background-power = In background: elaborazione tramite { $engine }, per risparmiare energia.
settings-paths-background = In background: elaborazione tramite { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms di tempo CPU, { $runs ->
    [one] { $runs } misurazione
   *[other] { $runs } misurazioni
} ({ $confidence })
settings-paths-detail-guess = { $engine }: circa { $time } ms (stima)
settings-paths-confidence-low = affidabilità bassa
settings-paths-confidence-medium = affidabilità media
settings-paths-confidence-high = affidabilità alta
settings-paths-machine = Misurato per { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Riprova
settings-paths-measure-again = Misura di nuovo
settings-paths-measure-again-question = Dimenticare le misurazioni fatte su questo computer, compresi i percorsi disattivati, e misurare di nuovo quando il computer è inattivo?
settings-paths-measure-again-confirm = Dimentica le misurazioni
settings-performance-display-cache = Mantieni una cache di visualizzazione su disco
settings-performance-display-cache-help = Salva copie delle immagini a dimensione schermo, così le cartelle già viste si aprono all’istante.
settings-performance-display-cache-size = Dimensione cache di visualizzazione
settings-performance-display-cache-size-help = Le voci più vecchie vengono rimosse quando la cache è piena.
settings-performance-display-cache-location = Posizione cache di visualizzazione
settings-performance-display-cache-location-help = È consigliata una cartella su un’unità veloce.
settings-performance-fast-simd = Usa le istruzioni SIMD
settings-performance-fast-simd-help = Usa le istruzioni vettoriali della CPU per la decodifica. Disattivare solo per diagnosticare un problema.
settings-performance-fast-dct-scaling = Decodifica alla dimensione dello schermo
settings-performance-fast-dct-scaling-help = Decodifica i JPEG grandi direttamente alla dimensione in cui sono mostrati. Disattivare solo per diagnosticare un problema.
settings-performance-fast-parallel-decode = Decodifica su più core
settings-performance-fast-parallel-decode-help = Suddivide le immagini grandi tra i core della CPU. Disattivare solo per diagnosticare un problema.
settings-performance-fast-restart-index = Indicizza i JPEG grandi per lo zoom
settings-performance-fast-restart-index-help = Ricorda dove inizia ogni parte di un JPEG, così le aree ingrandite si decodificano rapidamente. Disattivare solo per diagnosticare un problema.
settings-performance-fast-yuv-upload = Converti i colori sulla GPU
settings-performance-fast-yuv-upload-help = Converte i colori dei JPEG sulla GPU o sulla CPU. Automatico sceglie il più veloce su questo computer. Disattivare solo per diagnosticare un problema.
settings-performance-fast-yuv-upload-auto = Automatico
settings-performance-fast-yuv-upload-on = Attivo
settings-performance-fast-yuv-upload-off = Disattivo
settings-performance-fast-async-io = Lettura asincrona dei file
settings-performance-fast-async-io-help = Legge in anticipo con l’interfaccia file più veloce del sistema operativo. Disattivare solo per diagnosticare un problema.
settings-performance-fast-mmap = File mappati in memoria
settings-performance-fast-mmap-help = Legge i file senza copiarli. Disattivare solo per diagnosticare un problema.
settings-performance-hardware-decode = Decodifica hardware
settings-performance-hardware-decode-help = Lascia che il decodificatore integrato nella GPU apra le foto JPEG. Automatico lo usa dove aiuta, di solito per le immagini preparate in background. Viene prima verificato con il decodificatore normale, e ciò che non sa gestire si apre normalmente.
settings-performance-hardware-decode-auto = Automatico
settings-performance-hardware-decode-on = Attivo
settings-performance-hardware-decode-off = Disattivo
settings-performance-frame-time-hud = HUD dei tempi di frame
settings-performance-frame-time-hud-help = Mostra i tempi di decodifica e di frame e quali percorsi rapidi sono in uso.
settings-performance-stall-warning = Segnala caricamenti bloccati dopo
settings-performance-stall-warning-help = Se un’immagine è ancora in caricamento dopo questo tempo, scrivi il suo stato di caricamento e le operazioni in sospeso nel log (standard error o SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Suggerimenti sulle prestazioni
settings-performance-insights-help = Quando le immagini continuano ad aprirsi lentamente per lo stesso motivo, mostra «Perché è lento?» nell’angolo con ciò che aiuterebbe. Ciò che impara resta su questo computer.
settings-performance-insights-storage = Suggerimenti sull’archiviazione lenta
settings-performance-insights-storage-help = Mostra suggerimenti quando la lettura dei file (da una condivisione di rete, un’unità lenta o archivi) continua a rallentare l’apertura delle immagini.
settings-performance-insights-decoder = Suggerimenti sulla decodifica
settings-performance-insights-decoder-help = Mostra suggerimenti quando le immagini continuano a essere decodificate sulla CPU, anche se un decodificatore hardware potrebbe aiutare.
# machine-drafted, needs native review
settings-performance-insights-restore = Mostra di nuovo i suggerimenti nascosti
settings-performance-insights-restore-help = Ripristina i suggerimenti nascosti con × su «Perché è lento?», sia per l’apertura delle immagini sia per le miniature. Non disponibile finché non ce ne sono di nascosti.

## Accessibilità

settings-accessibility-reduce-motion = Riduci movimento
settings-accessibility-reduce-motion-help = Sostituisce le animazioni di zoom, scorrimento e dissolvenza con cambi istantanei.
settings-accessibility-reduce-motion-system = Segui il sistema
settings-accessibility-reduce-motion-on = Attivo
settings-accessibility-reduce-motion-off = Disattivo
settings-accessibility-increase-contrast = Aumenta contrasto
settings-accessibility-increase-contrast-help = Bordi e testo più marcati e meno trasparenze.
settings-accessibility-increase-contrast-system = Segui il sistema
settings-accessibility-increase-contrast-on = Attivo
settings-accessibility-increase-contrast-off = Disattivo
settings-accessibility-text-scale = Dimensione del testo
settings-accessibility-text-scale-help = Dimensione del testo rispetto a quella standard.
settings-accessibility-ui-scale = Scala dell’interfaccia
settings-accessibility-ui-scale-help = Dimensione dell’intera interfaccia, in aggiunta al ridimensionamento dello schermo impostato nel sistema operativo.
settings-accessibility-focus-ring-always = Mostra sempre l’indicatore di focus
settings-accessibility-focus-ring-always-help = Mostra quale controllo ha il focus della tastiera anche quando si usa il mouse.
settings-accessibility-screen-reader-verbosity = Dettaglio dello screen reader
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Quanto annuncia uno screen reader quando si passa da un’immagine all’altra e quanto legge dei valori sotto il cursore.
settings-accessibility-screen-reader-verbosity-concise = Breve
settings-accessibility-screen-reader-verbosity-normal = Normale
settings-accessibility-screen-reader-verbosity-verbose = Dettagliato
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Leggi il valore sotto il cursore
settings-accessibility-announce-cursor-help = Uno screen reader legge il valore del pixel, le HU, la temperatura o la posizione sotto il cursore o il mirino da tastiera quando smette di muoversi.

## Privacy

# machine-drafted, needs native review
settings-privacy-no-network = Nessuna rete
# machine-drafted, needs native review
settings-privacy-no-network-help = Disattivato: si connettono solo questi, ciascuno dopo che lo attivi o vi apri qualcosa: controllo degli aggiornamenti, download dei modelli di IA locale, mappe online, immagini da un indirizzo web o da un archivio cloud e i server che aggiungi (DICOMweb, OMERO, mappe). Attivato: non si connette nulla.

## Avanzate

settings-advanced-portable-mode = Modalità portatile
settings-advanced-portable-mode-help = Conserva impostazioni e cache nella cartella del programma invece che nel profilo utente.
settings-advanced-log-level = Dettaglio del log
settings-advanced-log-level-help = Quanto { -app-name } scrive nel file di log.
settings-advanced-log-level-error = Solo errori
settings-advanced-log-level-warn = Avvisi
settings-advanced-log-level-info = Informazioni
settings-advanced-log-level-debug = Debug
settings-advanced-log-level-trace = Tutto
settings-advanced-developer-mode = Modalità sviluppatore
settings-advanced-developer-mode-help = Mostra strumenti per sviluppare e testare { -app-name }, come le pseudo-lingue nell’elenco delle lingue.
# machine-drafted, needs native review
settings-advanced-audit-log = Registro di audit locale
# machine-drafted, needs native review
settings-advanced-audit-log-help = Registra in audit.jsonl, nella cartella dei dati, le immagini mediche aperte, le esportazioni, le anonimizzazioni, le stampe e gli invii a un PACS. Resta su questo computer.
settings-advanced-hw-report = Rapporto hardware…
settings-advanced-hw-report-help = Mostra quali funzioni della CPU e della GPU { -app-name } ha rilevato e quali percorsi rapidi usa.
# machine-drafted, needs native review
settings-advanced-renderer-info = Informazioni sul renderer…
settings-advanced-renderer-info-help = Mostra la scheda grafica, il driver, i formati di visualizzazione e i limiti di memoria che { -app-name } usa, pronti da copiare in una segnalazione di bug.
settings-advanced-edit-settings-file = Modifica settings.toml
settings-advanced-edit-settings-file-help = Modifica il file delle impostazioni nell’editor integrato di { -app-name }. Controlla il file mentre scrivi e lo salva solo quando non contiene errori.
settings-advanced-import = Importa impostazioni…
settings-advanced-import-help = Carica le impostazioni da un file esportato da { -app-name }.
settings-advanced-export = Esporta impostazioni…
settings-advanced-export-help = Salva tutte le impostazioni in un file, come backup o per un altro computer.
settings-advanced-reset-all = Ripristina tutte le impostazioni…
settings-advanced-reset-all-help = Riporta ogni impostazione al valore predefinito. Si può annullare.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Salva
config-editor-revert = Ripristina
config-editor-close = Chiudi
config-editor-modified = Modificato
config-editor-text = File delle impostazioni
config-editor-position = Riga { $line }, colonna { $column }
config-editor-saved = settings.toml salvato e applicato.
config-editor-cant-save =
    { $count ->
        [one] Impossibile salvare: 1 problema. Correggilo prima.
       *[other] Impossibile salvare: { $count } problemi. Correggili prima.
    }
config-editor-save-failed = Impossibile salvare settings.toml: { $reason }
config-editor-conflict = settings.toml è stato modificato altrove.
config-editor-reload = Ricarica
config-editor-keep-mine = Mantieni i miei
config-editor-discard-question = Scartare le modifiche non salvate a settings.toml?
config-editor-discard = Scarta
config-editor-keep-editing = Continua a modificare
config-editor-find-placeholder = Cerca nel file
config-editor-find-count = { $current } di { $total }
config-editor-find-none = Nessuna corrispondenza
config-editor-find-previous = Corrispondenza precedente
config-editor-find-next = Corrispondenza successiva
config-editor-find-close = Chiudi ricerca
config-editor-problems = Problemi
config-editor-problems-count =
    { $count ->
        [0] Nessun problema
        [one] 1 problema
       *[other] { $count } problemi
    }
config-editor-no-problems = Nessun problema. Il file può essere salvato.
config-editor-problem-at = Riga { $line }: { $message }
config-editor-severity-error = Errore
config-editor-severity-warning = Avviso
config-editor-severity-info = Nota
config-editor-problem-syntax = TOML non valido: { $reason }
config-editor-problem-duplicate = { $key } è impostato due volte nella stessa tabella. Rimuovine uno.
config-editor-problem-unknown-section = Non esiste una sezione delle impostazioni [{ $key }].
config-editor-problem-unknown-section-suggestion = Non esiste una sezione delle impostazioni [{ $key }]. Intendevi [{ $suggestion }]?
config-editor-problem-unknown-key = Non esiste un’impostazione { $key }.
config-editor-problem-unknown-key-suggestion = Non esiste un’impostazione { $key }. Intendevi { $suggestion }?
config-editor-problem-invalid = Valore non valido per { $key }: { $reason }
config-editor-problem-migrated = { $key } è scritto come lo scriveva una versione precedente di { -app-name }. Viene convertito in lettura.
config-editor-problem-newer-version = Questo file è stato scritto da una versione più recente di { -app-name } (formato { $found }; questa versione legge { $supported }). Le impostazioni sconosciute vengono conservate.
config-editor-problem-command = Comando { $index } in [[commands]]: { $reason }
config-editor-problem-hook = Hook n. { $index } in [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = File delle scorciatoie da tastiera
config-editor-keymap-saved = keymap.toml salvato e applicato.
config-editor-keymap-save-failed = Impossibile salvare keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml è stato modificato altrove.
config-editor-keymap-discard-question = Scartare le modifiche non salvate a keymap.toml?
config-editor-new-file = nuovo file, creato al salvataggio
config-editor-no-folder = non c’è una cartella delle impostazioni in cui salvarlo.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Ripristinare tutte le impostazioni ai valori predefiniti?
settings-reset-all-confirm-detail = Riguarda tutte le sezioni delle Impostazioni. Si può annullare in seguito.
settings-reset-all-confirm-ok = Ripristina tutto
settings-reset-all-done = Tutte le impostazioni sono state ripristinate ai valori predefiniti.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Codificatore hardware

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = L'elaborazione tramite { $engine } viene confrontata con la CPU prima di essere usata.
settings-paths-unit-thumbnail = Tempi per una miniatura da { $size }
settings-paths-unit-frame = Tempi per fotogramma video da { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Cartella fotogrammi di calibrazione
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flat e dark per Sviluppo. I nomi di file o cartella contengono «flat» o «dark».

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Bianco
settings-colour-light-grey = Grigio chiaro
settings-colour-grey = Grigio
settings-colour-dark-grey = Grigio scuro
settings-colour-black = Nero
settings-colour-yellow = Giallo
settings-colour-orange = Arancione
settings-colour-red = Rosso
settings-colour-magenta = Magenta
settings-colour-blue = Blu
settings-colour-cyan = Ciano
settings-colour-green = Verde
