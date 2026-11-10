# machine-drafted, needs native review
# Specular — Rapporto di apertura: perché un'immagine ha impiegato quel tempo ad aprirsi (pannello
# informazioni, sezione «Prestazioni»; DESIGN.md §12.7).

open-report-title = Prestazioni

## Headlines
open-report-fast = Aperta in { $ms } ms
open-report-display-cache = Mostrata dalla cache di visualizzazione
open-report-slow-read = Archiviazione lenta: il { $percent } del tempo è servito a leggere il file ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Archiviazione lenta: il { $percent } del tempo è servito a leggere il file (file piccolo: soprattutto attesa della risposta dell’archiviazione)
open-report-archive-read = Il { $percent } del tempo è servito a estrarre l'archivio
open-report-startup = Il { $percent } del tempo è servito ad avviare { -app-name }
open-report-checked = Decodificata due volte: il primo risultato del decodificatore hardware è stato confrontato con quello della CPU ({ $percent } del tempo)
open-report-hardware = Decodificata dal decodificatore hardware, che ha impiegato il { $percent } del tempo
open-report-decode = Immagine grande: decodificare { $mpx } megapixel ha richiesto il { $percent } del tempo
open-report-upload = Il { $percent } del tempo è servito a inviare l'immagine alla GPU
open-report-waiting = Il { $percent } del tempo è servito ad attendere altre operazioni
open-report-stage = { $stage } ha richiesto il { $percent } del tempo
open-report-memory-short = La memoria scarseggiava, il sistema potrebbe aver dovuto usare lo swap

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Decodificata via software: il decodificatore hardware era ancora in fase di rilevamento
open-report-sw-no-device = Decodificata via software: { $api } non è disponibile su questo computer
open-report-sw-no-codec = Decodificata via software: { $api } non ha un decodificatore { $codec } su questa GPU
open-report-sw-blocked-mismatch = Decodificata via software: il decodificatore hardware è stato disattivato perché il suo risultato non corrispondeva a quello della CPU
open-report-sw-blocked-error = Decodificata via software: il decodificatore hardware è stato disattivato dopo un errore
open-report-sw-blocked-lost = Decodificata via software: il decodificatore hardware è stato disattivato perché non rispondeva più
open-report-sw-failed = Decodificata via software: il decodificatore hardware non è riuscito con questa immagine
open-report-sw-busy = Decodificata via software: il decodificatore hardware era occupato
open-report-sw-unsupported = Decodificata via software: il decodificatore hardware non supporta questa immagine
open-report-sw-unavailable = Decodificata via software: la decodifica hardware è disattivata fino al riavvio di { -app-name }
open-report-sw-cpu-only = Decodificata via software: l'elaborazione è impostata su «Solo CPU»
open-report-sw-setting-off = Decodificata via software: la decodifica hardware è disattivata nelle impostazioni
open-report-sw-unchecked = Decodificata via software: il decodificatore hardware non è ancora verificato (viene provato prima su un'immagine preparata in background)
open-report-sw-lower-power = Decodificata via software: a batteria, la CPU consuma meno per questo
open-report-sw-faster = Decodificata via software: più veloce su questo computer (circa { $cpu } ms, contro { $hw } ms con il decodificatore hardware)
open-report-sw-scaled = Decodificata via software: il decodificatore hardware decodifica le immagini solo a dimensione piena

## Hints
open-report-hint-copy-local = Copia il file su un disco locale per aprirlo più velocemente.
open-report-hint-extract = Estrai l'archivio in una cartella per aprirne le pagine più velocemente.
open-report-hint-try-again = Per riprovare il decodificatore hardware: Impostazioni → Prestazioni → Riprova.
open-report-hint-automatic = Imposta «Dove vengono elaborate le immagini» su Automatico in Impostazioni → Prestazioni.
open-report-hint-turn-on = Imposta «Decodifica hardware» su Automatico in Impostazioni → Prestazioni.
open-report-hint-free-memory = Chiudi altri programmi per liberare memoria.
open-report-hint-once-only = Solo la prima immagine viene controllata così.

## Rows
open-report-total = Fino alla nitidezza
open-report-first-pixels = Primi pixel
# machine-drafted, needs native review
open-report-whole = Immagine intera visibile
open-report-stage-startup = Avvio
open-report-stage-read = Lettura del file
open-report-stage-parse = Lettura dell'intestazione
open-report-stage-colour = Preparazione della gestione colore
open-report-stage-decode = Decodifica
open-report-stage-upload = Invio alla GPU
open-report-stage-waiting = Attesa di altre operazioni
open-report-decoder = Decodificata da
open-report-decoder-checked = { $engine }, verificato con la CPU
open-report-decoder-cache = Cache di visualizzazione
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, a 1/{ $scale } della dimensione
open-report-decoder-scaled-full = { $engine }, a dimensione piena, poi ridotta a 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, a 1/{ $scale } della dimensione, senza l'indice di avvio rapido
open-report-colour-conversion = Colori convertiti da

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Ricorrente
open-report-advice-change = Sarebbe utile
open-report-advice-gain = Guadagno stimato
open-report-advice-software = { $count } delle ultime { $total } immagini { $format } lente { $count ->
    [one] è stata decodificata
   *[other] sono state decodificate
} via software per questo motivo
open-report-advice-slow-storage = { $count } delle ultime { $total } immagini lente { $count ->
    [one] è stata rallentata
   *[other] sono state rallentate
} dalla lettura del file (circa { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } delle ultime { $total } immagini lente { $count ->
    [one] è stata rallentata
   *[other] sono state rallentate
} dalla lettura del file (file piccoli: soprattutto attesa della risposta dell’archiviazione)
open-report-advice-archive = { $count } delle ultime { $total } immagini lente { $count ->
    [one] è stata rallentata
   *[other] sono state rallentate
} dall'estrazione dell'archivio
open-report-advice-hardware-decoder = Una GPU con decodificatore hardware { $codec }
open-report-advice-gain-range = Circa da { $low } a { $high } volte più veloce
open-report-advice-gain-one = Circa { $low } volte più veloce

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Suggerimenti sulle prestazioni
open-report-insights-indicator = Perché è lento?
open-report-insights-announce = Suggerimento sulle prestazioni: le immagini continuano ad aprirsi lentamente per lo stesso motivo. Apri «Perché è lento?» per vedere cosa aiuterebbe.
open-report-insights-hide = Nascondi
open-report-insights-last = Ultima immagine lenta: { $total } fino alla nitidezza
open-report-insights-now = Cosa puoi fare ora
open-report-insights-hardware = Hardware che aiuterebbe
open-report-insights-none = Nessun rallentamento ricorrente
open-report-insights-none-detail = I suggerimenti compaiono quando più immagini si aprono lentamente per lo stesso motivo. Per trovarli, { -app-name } conserva una settimana di aperture lente su questo computer; nulla viene inviato altrove.
open-report-insights-software = Le immagini { $format } vengono decodificate sulla CPU
open-report-insights-storage = La lettura dei file rallenta le immagini
open-report-insights-archive = L'estrazione degli archivi rallenta le immagini
open-report-insights-nothing-now = Nessuna impostazione di { -app-name } cambia questa situazione.
open-report-insights-display-cache = Attiva la cache di visualizzazione: le immagini già aperte si apriranno da una copia veloce su questo computer.
open-report-insights-prefetch = Tieni pronte più immagini vicine, così le successive vengono preparate mentre guardi.
open-report-insights-hw-ssd = Un'unità interna veloce (un SSD NVMe) per questi file
open-report-insights-action-turn-on = Attiva
open-report-insights-action-try-again = Riprova
open-report-insights-action-automatic = Imposta su Automatico
open-report-insights-dismiss-storage = Non mostrare più per l'archiviazione
open-report-insights-dismiss-decoder = Non mostrare più per la decodifica
open-report-insights-off = Disattiva i suggerimenti
open-report-insights-on = Attiva i suggerimenti
open-report-insights-restore = Mostra di nuovo i suggerimenti nascosti
open-report-insights-power = Questo computer sta risparmiando energia (a batteria o in una modalità di risparmio energetico), il che rallenta la decodifica sulla CPU. Con l'alimentazione collegata, le immagini potrebbero aprirsi più velocemente.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Suggerimento sulle prestazioni: le miniature continuano a comparire lentamente per lo stesso motivo. Apri «Perché è lento?» per vedere cosa aiuterebbe.
open-report-insights-last-thumbnail = Ultima miniatura lenta: { $total } per crearla
open-report-insights-thumbnails-software = Le miniature delle immagini { $format } vengono decodificate sulla CPU
open-report-insights-thumbnails-storage = La lettura dei file rallenta le miniature
open-report-insights-thumbnails-archive = L'estrazione degli archivi rallenta le miniature
open-report-insights-thumbnail-cache = Le miniature non possono essere conservate su questo computer (la cache delle miniature non è disponibile), quindi vengono ricreate ogni volta che si mostra la cartella.
open-report-advice-thumbnails-software = { $count } delle ultime { $total } miniature { $format } lente { $count ->
    [one] è stata decodificata
   *[other] sono state decodificate
} via software per questo motivo
open-report-advice-thumbnails-slow-storage = { $count } delle ultime { $total } miniature lente { $count ->
    [one] è stata rallentata
   *[other] sono state rallentate
} dalla lettura del file (circa { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } delle ultime { $total } miniature lente { $count ->
    [one] è stata rallentata
   *[other] sono state rallentate
} dalla lettura del file (file piccoli: soprattutto attesa della risposta dell’archiviazione)
open-report-advice-thumbnails-archive = { $count } delle ultime { $total } miniature lente { $count ->
    [one] è stata rallentata
   *[other] sono state rallentate
} dall'estrazione dell'archivio
