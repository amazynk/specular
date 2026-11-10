# machine-drafted, needs native review
# Specular — la galleria (modalità miniature, DESIGN §10) e Impostazioni → Galleria. Italiano.

## Tavolozza e menu

palette-gallery = Galleria
gallery-palette-view = Vista galleria…
gallery-palette-layout = Disposizione
gallery-palette-sort = Ordina per
gallery-palette-group = Raggruppa per
gallery-palette-filter = Filtra…
gallery-palette-quick-filters = Filtri rapidi
gallery-palette-details = Vista dettagli
gallery-palette-info-overlay = Info sulle miniature
gallery-palette-stacks = Raggruppa RAW+JPEG e raffiche
gallery-palette-subfolders = Includi sottocartelle
gallery-palette-compare = Confronta selezionate
gallery-palette-rate = Valuta…
gallery-palette-settings = Impostazioni galleria…
gallery-view-menu = Vista
gallery-more-menu = Altro
gallery-open = Apri
gallery-new-window = Apri in una nuova finestra
gallery-reveal = Mostra nella cartella
gallery-properties = Proprietà
gallery-copy-paths = Copia percorsi
gallery-select-all = Seleziona tutto
gallery-select-none = Deseleziona tutto
gallery-select-invert = Inverti selezione
gallery-next-unrated = Successiva non valutata
gallery-stack-toggle = Espandi o comprimi pila
gallery-run-on = { $name } ({ $count } file)
gallery-depth-off = Solo questa cartella
gallery-depth-all = Tutte le sottocartelle
gallery-depth-levels = { $depth ->
    [one] { $depth } livello di sottocartelle
   *[other] { $depth } livelli di sottocartelle
}

## Stato e stati vuoti

gallery-items = { $count ->
    [one] { $count } elemento
   *[other] { $count } elementi
}
gallery-selected = { $count } di { $total } selezionati
gallery-shown-of = { $count } di { $total } mostrati
gallery-listing = Lettura della cartella… { $count }
gallery-empty = Questa cartella non contiene immagini.
gallery-empty-filter = Nessuna immagine corrisponde al filtro.
gallery-list-error = Impossibile leggere questa cartella: { $reason }
gallery-no-folder = Nessuna cartella aperta
gallery-kind-folder = Cartella
gallery-drag-count = { $count ->
    [one] { $count } file
   *[other] { $count } file
}
gallery-badge-pages = { $count } pagine

## Barra dei filtri

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Nome, oppure rating>=3 camera:X-T5 …
gallery-filter-clear = Cancella filtro
gallery-filter-rating = { $stars ->
    [one] ★ { $stars } o più
   *[other] ★ { $stars } o più
}
gallery-filter-label = Etichetta: { $label }
gallery-filter-picks = Scelte
gallery-filter-hide-rejects = Nascondi scartate
gallery-filter-unrated = Non valutate
gallery-filter-raw = RAW
gallery-filter-animated = Animate
gallery-filter-hdr = HDR
gallery-filter-landscape = Orizzontali
gallery-filter-portrait = Verticali

## Query errors and facets

gallery-query-unknown-field = Campo sconosciuto «{ $field }»
gallery-query-unknown-field-suggest = Campo sconosciuto «{ $field }»: forse intendevi «{ $suggestion }»?
gallery-query-missing-value = «{ $field }» richiede un valore
gallery-query-bad-number = «{ $value }» non è un numero ({ $field })
gallery-query-bad-date = «{ $value }» non è una data ({ $field }): usa 2024, 2024-05 o 2024-05-12
gallery-query-bad-word = «{ $value }» non va bene per { $field }: usa { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = «{ $value }» non è un nome di tag Exif
gallery-query-no-compare = { $field } non si può confrontare con < o > qui
gallery-query-unterminated = Manca una virgoletta di chiusura
gallery-query-unavailable = «{ $field }» richiede la Libreria (non disponibile qui)
gallery-facets = Facet
gallery-facets-counting = Conteggio…
gallery-facets-empty = Niente da contare
gallery-facets-search = Cerca nei facet
gallery-facets-no-match = Nessun valore corrisponde
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Schermata iniziale
gallery-facet-format = Formato
gallery-facet-camera = Fotocamera
gallery-facet-lens = Obiettivo
gallery-facet-rating = Valutazione
gallery-facet-year = Anno
gallery-facet-orientation = Orientamento
gallery-facet-label = Etichetta
gallery-facet-properties = Proprietà
gallery-facet-square = Quadrate
gallery-facet-alpha = Trasparenza
gallery-facet-gps = Posizione
gallery-facet-multipage = Più pagine
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } file
   *[other] { $count } file
}

## Gruppi

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Questa cartella
gallery-group-unknown = Sconosciuto
gallery-group-unrated = Non valutate

## Selezione

gallery-stars = { $stars ->
    [one] { $stars } stella
   *[other] { $stars } stelle
}
gallery-rate-none = Nessuna valutazione
gallery-pick = Scegli
gallery-reject = Scarta
gallery-unflag = Nessun contrassegno
gallery-label-red = Rosso
gallery-label-yellow = Giallo
gallery-label-green = Verde
gallery-label-blue = Blu
gallery-label-none = Nessuna etichetta
gallery-compare-hint = Seleziona da 2 a 4 immagini per confrontarle.

## File

gallery-trash-count = { $count ->
    [one] Sposta { $count } file nel cestino
   *[other] Sposta { $count } file nel cestino
}
gallery-moved-to = { $count ->
    [one] { $count } file spostato in { $folder }
   *[other] { $count } file spostati in { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } file copiato in { $folder }
   *[other] { $count } file copiati in { $folder }
}
gallery-move-failed = Impossibile spostare { $file }: { $reason }
gallery-drop-move = Sposta in { $folder }
gallery-drop-copy = Copia in { $folder }
gallery-move-here = Sposta qui la selezione
gallery-copy-here = Copia qui la selezione
gallery-not-an-image = { $file } non è un’immagine che { -app-name } può aprire.
gallery-cache-cleared = Cache delle miniature svuotata
# machine-drafted, needs native review
gallery-thumb-failed = Nessuna miniatura: { $reason }
gallery-cache-clear-failed = Impossibile svuotare la cache delle miniature: { $reason }

## Proprietà

gallery-prop-name = Nome
gallery-prop-folder = Cartella
gallery-prop-size = Dimensione file
gallery-prop-dimensions = Dimensioni
gallery-prop-format = Formato
gallery-prop-date = Data
gallery-prop-camera = Fotocamera
gallery-prop-lens = Obiettivo
gallery-prop-rating = Valutazione
gallery-prop-label = Etichetta
gallery-prop-items = Elementi

## Screen reader

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } elemento
   *[other] { $count } elementi
}
gallery-a11y-dims = { $width } per { $height }
gallery-a11y-folder = cartella
gallery-a11y-up = Su a { $folder }
gallery-a11y-archive = archivio
gallery-a11y-stack = pila di { $count }
gallery-a11y-filter-bar = Filtro
gallery-a11y-filter-field = Filtra per nome o campo
gallery-a11y-chip = Criterio di filtro { $term }
gallery-a11y-chip-remove = Premi per rimuovere
gallery-a11y-results = { $count ->
    [0] Nessun elemento corrisponde
    [one] { $count } elemento corrisponde
   *[other] { $count } elementi corrispondono
}

## Impostazioni → Galleria

settings-section-gallery = Galleria
settings-gallery-layout = Disposizione
settings-gallery-layout-help = I quadrati ritagliano le miniature in una griglia; le righe giustificate mantengono la forma di ogni immagine; senza ritaglio adatta le immagini intere in celle uguali.
settings-gallery-layout-squares = Quadrati
settings-gallery-layout-justified = Righe giustificate
settings-gallery-layout-uncropped = Senza ritaglio
settings-gallery-layout-details = Dettagli
settings-gallery-thumbnail-size = Dimensione miniature
settings-gallery-thumbnail-size-help = Anche con Ctrl+rotella del mouse, pizzico o Ctrl+più e meno nella galleria.
settings-gallery-captions = Mostra nomi dei file
settings-gallery-captions-help = Una didascalia sotto ogni miniatura.
settings-gallery-badges = Mostra indicatori
settings-gallery-badges-help = Piccoli simboli per RAW, animazione, HDR, più pagine, valutazione ed etichetta.
settings-gallery-sort-order = Ordina per
settings-gallery-sort-order-help = L’ordine della galleria. Le cartelle vengono sempre per prime.
settings-gallery-sort-order-name = Nome
settings-gallery-sort-order-date-taken = Data di scatto
settings-gallery-sort-order-modified = Data di modifica
settings-gallery-sort-order-size = Dimensione file
settings-gallery-sort-order-type = Tipo di file
settings-gallery-sort-order-dimensions = Dimensioni
settings-gallery-sort-descending = Ordine inverso
settings-gallery-sort-descending-help = Ordina dall’ultimo al primo.
settings-gallery-group-by = Raggruppa per
settings-gallery-group-by-help = Intestazioni di sezione che restano in alto durante lo scorrimento.
settings-gallery-group-by-none = Nessuno
settings-gallery-group-by-day = Giorno
settings-gallery-group-by-month = Mese
settings-gallery-group-by-year = Anno
settings-gallery-group-by-event = Evento
settings-gallery-group-by-folder = Cartella
settings-gallery-group-by-camera = Fotocamera
settings-gallery-group-by-format = Formato
settings-gallery-group-by-rating = Valutazione
settings-gallery-group-by-target = Oggetto
settings-gallery-group-by-filter = Filtro
settings-gallery-per-folder-view = Ricorda la vista per ogni cartella
settings-gallery-per-folder-view-help = Disposizione, dimensione, ordinamento e raggruppamento sono salvati per cartella; altrimenti si usa un’unica vista ovunque.
settings-gallery-open-folders-in-gallery = Apri le cartelle nella galleria
settings-gallery-open-folders-in-gallery-help = All’apertura di una cartella, mostra prima le miniature invece della prima immagine.
settings-gallery-escape-opens-gallery = Esc apre la galleria
settings-gallery-escape-opens-gallery-help = Quando non c’è nient’altro da chiudere, Esc nel visualizzatore passa alla galleria invece di non fare nulla.
settings-gallery-wrap-around = Le frecce ricominciano dall’altro capo
settings-gallery-wrap-around-help = Alla fine dell’elenco le frecce tornano all’inizio, e dall’inizio vanno alla fine. Tenendo premuto un tasto ci si ferma comunque alla fine.
settings-gallery-show-hidden = Mostra file nascosti
settings-gallery-show-hidden-help = Includi file e cartelle il cui nome inizia con un punto.
settings-gallery-show-other-files = Mostra altri file
settings-gallery-show-other-files-help = Includi i file che { -app-name } non può aprire, mostrati con un’icona generica.
settings-gallery-show-archives = Mostra archivi
settings-gallery-show-archives-help = Includi fumetti e altri archivi (zip, cbz, rar, 7z…) come riquadri. L’apertura diretta di un archivio funziona comunque.
settings-gallery-subfolder-depth = Livelli di sottocartelle
settings-gallery-subfolder-depth-help = Mostra anche le immagini delle sottocartelle, fino a questo numero di livelli (0: solo questa cartella).
settings-gallery-stacks = Raggruppa file correlati
settings-gallery-stacks-help = Mostra coppie RAW+JPEG e raffiche come un unico riquadro. Premi S per espandere una pila.
settings-gallery-info-overlay = Info sulle miniature
settings-gallery-info-overlay-help = Dimensioni ed esposizione sopra ogni miniatura.
settings-gallery-prefer-embedded = Preferisci anteprime incorporate
settings-gallery-prefer-embedded-help = Usa l’anteprima salvata nel file quando è abbastanza grande. Molto più veloce, un po’ meno precisa.
settings-gallery-sharper-raw-thumbnails = Miniature RAW più nitide
settings-gallery-sharper-raw-thumbnails-help = Le foto RAW modificate vengono sviluppate a 1024 px per le miniature. Più nitide, un po’ più lente. Disattivato: 512 px.
settings-gallery-rating-sidecars = Scrivi valutazioni in file XMP collaterali
settings-gallery-rating-sidecars-help = Valutazioni, etichette e contrassegni vanno anche in un file .xmp accanto all’immagine, letto da altre applicazioni fotografiche.
settings-gallery-cache-size = Dimensione cache miniature
settings-gallery-cache-size-help = Le miniature più vecchie vengono rimosse quando la cache è piena.
settings-gallery-cache-location = Posizione cache miniature
settings-gallery-cache-location-help = È consigliata una cartella su un’unità veloce.
settings-gallery-clear-cache = Svuota cache miniature
settings-gallery-clear-cache-help = Elimina le miniature salvate. Vengono ricreate durante la navigazione.

## Compare
# machine-drafted, needs native review

compare-side = Affiancate
compare-overlay = Sovrapposizione
compare-difference = Differenza
compare-flicker = Alternanza
compare-blend = Dissolvenza
compare-kind-absolute = Errore assoluto
compare-kind-squared = Errore quadratico
compare-kind-relative = Errore relativo
compare-scale-down = Meno sensibile
compare-scale-up = Più sensibile
compare-scale = Fondo scala { $value }
compare-space = Luce lineare, come visualizzata
compare-space-help = Errori e metriche sono misurati in luce lineare come l’immagine viene visualizzata, dopo la mappatura dei toni, per canale, alfa compreso.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = campionato a { $width } × { $height }
compare-aligned = Allineate in alto a sinistra, { $width } × { $height } confrontati
compare-vs = { $b } rispetto a { $a }
compare-overlay-label = { $b } su { $a } al { PERCENT($percent) }
compare-pending = Misurazione…
# machine-drafted, needs native review
compare-diff-failed = Impossibile mostrare la differenza: { $reason }
compare-no-overlap = Niente da confrontare nella vista
compare-zoom-fit = Adatta
compare-zoom = { PERCENT($percent) }
compare-showing = Mostra { $name }
compare-a11y-group = Confronto
compare-a11y-blend = Dissolvenza tra le immagini
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Confronta affiancate
gallery-palette-compare-overlay = Confronta in sovrapposizione
gallery-palette-compare-difference = Confronta differenza: { $kind }
gallery-palette-compare-flicker = Alterna le immagini confrontate
gallery-palette-compare-actual = Confronta al { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Spazio o Ctrl+Invio ne seleziona più di uno

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Profilo colore
gallery-facet-date = Data di scatto
gallery-facet-picked = Scelte
gallery-facet-rejected = Scartate
gallery-facet-flagged = Scelte o scartate
gallery-facet-rated = Valutate
gallery-facet-labelled = Con etichetta

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Profondità di bit
gallery-facet-pages = Pagine
gallery-facet-width = Larghezza
gallery-facet-height = Altezza
gallery-facet-megapixels = Megapixel
gallery-picker-bits = { $bits } bit
gallery-picker-pages = { $pages ->
    [one] { $pages } pagina
   *[other] { $pages } pagine
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = Qualsiasi in un gruppo
gallery-picker-group-keep = Suggerito: tieni
gallery-picker-group-discard = Suggerito: scarta
gallery-picker-ops = Confronta
gallery-picker-op-eq = Uguale a
gallery-picker-op-ge = Almeno
gallery-picker-op-le = Al massimo
gallery-picker-ge = almeno { $value }
gallery-picker-gt = più di { $value }
gallery-picker-le = al massimo { $value }
gallery-picker-lt = meno di { $value }

# Map view.
gallery-facet-place = Luogo
gallery-palette-map = Mappa
map-title = Mappa
map-summary = { $count ->
    [one] { $count } foto con posizione
   *[other] { $count } foto con posizione
}
map-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } foto
}
map-zoom-in = Ingrandisci
map-zoom-out = Riduci
map-show-all = Mostra tutto
map-done = Fine
map-credit = Dati: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Mappa: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Riquadri di mappa online
map-basemap-consent-server = I riquadri della mappa provengono da { $host }.
map-basemap-consent-sees = Quel server vede il tuo indirizzo internet e le zone che guardi.
map-basemap-consent-private = Le tue foto, i loro nomi e le loro posizioni non vengono inviati.
map-basemap-consent-cache = I riquadri restano su questo computer, fino a { $size }.
map-basemap-allow = Usa la mappa online
map-basemap-decline = Mantieni la mappa offline
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Mappa di base non mostrata: { $reason }
map-basemap-bad-address = l’indirizzo del server di riquadri richiede {"{"}z{"}"}, {"{"}x{"}"} e {"{"}y{"}"}.
map-basemap-unknown-tiles = i riquadri del file di mappa sono di un tipo sconosciuto.
map-loading = Lettura delle posizioni delle foto…
map-empty = Nessuna di queste foto ha una posizione
map-hidden = Le posizioni delle foto sono nascoste (Impostazioni › Metadati e privacy)
map-a11y-group = Mappa delle posizioni delle foto
map-a11y-status = { $count ->
    [one] { $count } luogo mostrato, livello di zoom { $zoom }
   *[other] { $count } luoghi mostrati, livello di zoom { $zoom }
}
