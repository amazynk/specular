# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Rilievo
cheatsheet-map-time = Tempo e meteo
cheatsheet-map-online = Servizi online
cheatsheet-map-photos = Foto e GPS
cheatsheet-map-output = Output

## Palette entries and their one-line descriptions.
palette-map-slope = Colori di pendenza
palette-map-slope-help = Colora un modello di elevazione in base alla sua pendenza.
palette-map-aspect = Colori di esposizione
palette-map-aspect-help = Colora un modello di elevazione in base alla direzione verso cui guardano i versanti.
palette-map-contours = Mostra o nascondi le isoipse
palette-map-contours-help = Isoipse su un modello di elevazione, etichettate ogni poche linee.
palette-map-terrain-3d = Rilievo in 3D
palette-map-terrain-3d-help = Mostra il modello di elevazione come superficie 3D con i suoi colori sovrapposti.
palette-map-time = Mostra o nascondi il cursore temporale
palette-map-time-help = Scorri i tempi di una griglia o lungo i tempi di una traccia.
palette-map-play = Riproduci o metti in pausa il tempo
palette-map-play-help = Fai avanzare il cursore temporale da solo, passo dopo passo.
palette-map-wind = Mostra o nascondi le frecce del vento
palette-map-wind-help = Frecce di velocità e direzione del vento dalle componenti u e v della griglia.
palette-map-weather-colours = Mappa di colori meteo successiva
palette-map-weather-colours-help = Mappe di colori per temperatura, pioggia, vento e pressione.
palette-map-services = Servizi di mappe online…
palette-map-services-help = Aggiungi layer da server WMS, WMTS, WFS, OGC API o STAC.
palette-map-photos-on-track = Mostra o nascondi le foto sulla traccia
palette-map-photos-on-track-help = Colloca le foto della cartella lungo la traccia GPS in base all’ora di scatto.
palette-map-geotag = Geotagga le foto dalla traccia…
palette-map-geotag-help = Scrivi le posizioni della traccia in file XMP collaterali delle foto della cartella.
palette-map-export = Esporta mappa…
palette-map-export-help = Salva la mappa come PNG o PDF in scala, con legenda, barra di scala e freccia del nord.
palette-map-print = Stampa mappa in scala…
palette-map-print-help = Stampa la mappa a una scala scelta, ad esempio 1:{ NUMBER(25000) }.
palette-map-copy-extent = Copia estensione
palette-map-copy-extent-help = Copia l’area sullo schermo come riquadro di delimitazione, o come WKT.

## Why a map command is unavailable.
disabled-no-time = La mappa non ha tempi
disabled-no-wind = La mappa non ha componenti del vento (u e v)
disabled-no-track = Nessuna traccia GPS con orari
disabled-not-grid = La mappa non è una griglia di valori

## Settings › Maps and Metadata.
settings-maps-services = Servizi di mappe online
settings-maps-services-help = Aggiungi layer da server WMS, WMTS, WFS, OGC API e STAC. Disattivato: non viene scaricato nulla.
settings-maps-services-allowed = Server di mappe consentiti
settings-maps-services-allowed-help = Server che hai accettato dopo la spiegazione di cosa vedono.
settings-metadata-basemap-service = Un layer di un servizio di mappe
settings-metadata-basemap-service-help = Il layer scelto nel pannello dei servizi online della mappa.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NE
    [e] E
    [se] SE
    [s] S
    [sw] SO
    [w] O
    [nw] NO
   *[n] N
}
map-slope-aspect = Pendenza { $slope }° · esposto a { $direction } ({ $aspect }°)
map-slope-flat = Pendenza { $slope }° · piano
map-slope-on = Colori di pendenza
map-aspect-on = Colori di esposizione
map-relief-off = Colori di quota
map-contours-on = Isoipse attive
map-contours-off = Isoipse disattivate
map-legend-slope = Pendenza
map-legend-aspect = Esposizione: verso dove guardano i versanti
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° e oltre
map-terrain-3d-on = Rilievo in 3D. Esc torna alla mappa.
map-terrain-3d-on-exaggerated = Rilievo in 3D, quote × { $times }. Esc torna alla mappa.
map-terrain-exaggeration = Quote esagerate { $times } volte.
map-terrain-drape = Colori della mappa

## Time, weather and wind.
map-time-grid = { $when } · { $step } di { $steps }
map-time-track = { $when } · dopo { $elapsed }
map-time-speed = × { $times }
map-time-play = Riproduci
map-time-pause = Pausa
map-weather-map = Colori: { $name }
map-weather-file = Colori dal file
map-wind-on = Frecce del vento attive
map-wind-off = Frecce del vento disattivate
map-wind-at = Vento { $speed } { $unit } da { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Riquadro di delimitazione copiato. Copia di nuovo per il WKT.
map-extent-copied-wkt = Estensione copiata come WKT

## Online map services.
map-services-title = Servizi di mappe online
map-service-off = I servizi di mappe online sono disattivati.
map-service-off-why = Attivali per aggiungere layer da server su internet.
map-service-turn-on = Attiva
map-service-close = Chiudi
map-service-back = Indietro
map-service-cancel = Annulla
map-service-connect = Connetti
map-service-allow = Consenti
map-service-remove = Rimuovi
map-service-download = Scarica
map-service-list-hint = Invio apre un server. Canc lo rimuove.
map-service-add = Aggiungi un server…
map-service-add-sub = WMS, WMTS, WFS, OGC API o STAC
map-service-add-hint = L’indirizzo del server. Invio si connette.
map-service-bad-address = Non è un indirizzo web (http o https, senza nome utente).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Servizio di mappe online
map-service-consent-server = Specular contatterà { $host }.
map-service-consent-sees = Quel server vede il tuo indirizzo internet e l’area della mappa che chiedi.
map-service-consent-private = I tuoi file e i loro nomi non vengono inviati.
map-service-asking = Interrogazione di { $host }…
map-service-downloading = Download da { $host }…
map-service-layers-hint = Layer { $kind }. Invio ne usa uno.
map-service-no-layers = Il server non elenca alcun layer.
map-service-offer-tiles = Riquadri: diventa la mappa di base della mappa delle foto
map-service-offer-features = Elementi: aggiunti a questa mappa per l’area sullo schermo
map-service-offer-search = Immagini: cerca nell’area sullo schermo
map-service-refused-mercator = Non offerto in Web Mercator (EPSG:3857), necessario alla mappa di base.
map-service-refused-format = Non offerto come immagini PNG o JPEG.
map-service-refused-geojson = Non offerto come GeoJSON, il formato letto qui.
map-service-items-hint = { $count ->
        [one] { $count } immagine trovata. Invio la scarica.
       *[other] { $count } immagini trovate. Invio ne scarica una.
    }
map-service-no-items = Nessuna immagine trovata qui.
map-service-item-cloud = { $when } · { PERCENT($cloud) } di nuvole
map-service-item-no-cog = Nessun Cloud-Optimized GeoTIFF
map-service-download-ask = Scaricare { $size } da { $host }?
map-service-download-ask-unknown = Scaricare questa immagine da { $host }? La dimensione non è nota.
map-service-download-cache = Viene tenuta nella cache delle mappe e aperta.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = L’immagine è di { $size }, troppo grande da scaricare.
map-service-opened = Apertura dell’immagine scaricata
map-service-basemap = { $layer } è ora la mappa di base della mappa delle foto. La mappa delle foto chiede prima del primo download.
map-service-status = Il server ha risposto { $status }.
map-service-unknown = Non è un servizio di mappe che questa versione legge.
map-service-exception = Il server dice: { $message }
map-service-malformed = La risposta del server è danneggiata: { $reason }
map-service-remove-ask = Rimuovere “{ $server }” dai server salvati?
map-service-remove-detail = Non si perde nulla sul disco. I layer già sulla mappa restano finché non viene chiusa.
map-service-removed = { $server } rimosso
map-service-saved = Server di mappe salvati
map-service-saved-none = Nessuno per ora. Aggiungine uno nel pannello dei servizi online della mappa.
map-service-keywords = WMS WMTS WFS OGC API STAC server servizi mappe

## Photos and geotagging.
map-geotag-title = Geotagga le foto dalla traccia
map-photos-on = Foto sulla traccia
map-photos-off = Foto sulla traccia nascoste
map-geotag-offset = Orologio della fotocamera: { $offset } rispetto all’ora GPS. ← → lo cambiano di un minuto, con Maiusc di un’ora, con Alt di un secondo.
map-geotag-reading = Lettura delle foto…
map-geotag-summary = { $count } di { $total } foto ricevono una posizione.
map-geotag-sidecars = Le posizioni vanno in file XMP collaterali accanto alle foto. I file delle foto non vengono modificati.
map-geotag-writing = Scrittura…
map-geotag-write = Scrivi file collaterali…
map-geotag-guess = Stima scarto
map-geotag-will-write = Riceve { $position }
map-geotag-has-position = Ha già una posizione: lasciata com’è
map-geotag-no-time = Nessun orario di scatto: lasciata com’è
map-geotag-outside = Scattata { $offset } fuori dalla traccia: lasciata com’è
map-geotag-gap = In un buco della traccia (a { $seconds } s da un fix): lasciata com’è
map-geotag-nothing = Con questo scarto nessuna foto riceve una posizione.
map-geotag-confirm = Scrivere le posizioni in { $count ->
    [one] { $count } file XMP collaterale
   *[other] { $count } file XMP collaterali
}? Le foto stesse non vengono modificate.
map-geotag-confirm-ok = Scrivi file collaterali
map-geotag-done = { $count ->
    [one] Scritto { $count } file collaterale
   *[other] Scritti { $count } file collaterali
}
map-geotag-done-failed = { $count ->
        [one] Scritto { $count } file collaterale
       *[other] Scritti { $count } file collaterali
    }; { $failed } non riusciti: { $reason }
map-geotag-not-sidecar = Non è un nome di file collaterale.

## Export and printing.
map-export-title = Esporta mappa
map-print-title = Stampa mappa in scala
map-export-keys = ↑ ↓ scegli, ← → cambia, Invio avvia.
map-export-format = Formato
map-export-scale = Scala
map-export-scale-value = 1:{ $n }
map-export-resolution = Risoluzione
map-export-dpi = { $dpi } dpi
map-export-paper = Carta
map-export-size = { $width } × { $height } px · { $across } × { $down } km sul terreno
map-export-save = Salva…
map-export-print = Stampa…
map-export-too-large = Troppo grande a questa scala: { $width } × { $height } px. Scegli una scala o una risoluzione minore.
map-export-no-scale = Questa mappa non ha una scala sul terreno.
map-export-no-gpu = L’esportazione richiede il processore grafico.
map-export-subtitle = Scala { $scale } · { $crs }
map-export-data = Dati: { $sources }
map-export-saved = { $name } salvato
map-export-printing = Stampa in corso…
map-export-printed = Inviato alla stampante
map-export-print-failed = Non stampato: { $reason }
