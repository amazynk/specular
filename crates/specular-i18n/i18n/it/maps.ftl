# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = Mappe
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Layer
cheatsheet-map-coordinates = Coordinate
cheatsheet-map-measure = Misura

## Palette entries and their one-line descriptions.
palette-map-layers = Mostra o nascondi il pannello dei layer
palette-map-layers-help = I layer della mappa: ordine, opacità, visibilità e stile.
palette-map-add-layer = Aggiungi un layer da file…
palette-map-add-layer-help = Sovrapponi un altro file di mappa, come una traccia GPX su uno shapefile.
palette-map-zoom-to-layer = Zoom sul layer
palette-map-zoom-to-layer-help = Mostra per intero il layer scelto nel pannello dei layer.
palette-map-attribute-table = Mostra o nascondi la tabella degli attributi
palette-map-attribute-table-help = Gli elementi del layer scelto come tabella: ordina, filtra, cerca e seleziona.
palette-map-coordinate-format = Formato di coordinate successivo
palette-map-coordinate-format-help = Gradi decimali, gradi minuti secondi, UTM o MGRS.
palette-map-units = Unità metriche o imperiali
palette-map-units-help = Lunghezze e aree in metri, o in piedi e miglia.
palette-map-copy-coordinate = Copia coordinata
palette-map-copy-coordinate-help = Copia la posizione sotto il puntatore, o quella del centro della mappa.
palette-map-go-to-coordinate = Vai a coordinata…
palette-map-go-to-coordinate-help = Scrivi una posizione in qualsiasi formato per centrarvi la mappa.
palette-map-measure-distance = Misura una distanza
palette-map-measure-distance-help = Fai clic su punti della mappa: la lunghezza sulla superficie terrestre e l’azimut.
palette-map-measure-area = Misura un’area
palette-map-measure-area-help = Fai clic sugli angoli di un’area: la sua superficie e il perimetro sulla superficie terrestre.
palette-map-elevation-profile = Profilo altimetrico
palette-map-elevation-profile-help = Traccia una linea su un modello di elevazione per rappresentarne le quote.
palette-map-overview = Mostra o nascondi la mappa d’insieme
palette-map-overview-help = Una piccola mappa dell’intera estensione, con la parte sullo schermo evidenziata.

## Why a map command is unavailable.
disabled-not-map = Solo per le mappe, con gli strumenti per mappe attivi
disabled-no-lonlat = La mappa non ha un sistema di coordinate noto
disabled-no-features = La mappa non ha elementi
disabled-no-heights = La mappa non ha quote

## Settings › Maps.
settings-section-maps = Mappe
settings-maps-coordinate-format = Formato delle coordinate
settings-maps-coordinate-format-help = Come le posizioni vengono mostrate e copiate. Vai a coordinata legge ogni formato.
settings-maps-coordinate-format-dd = Gradi decimali
settings-maps-coordinate-format-dms = Gradi, minuti, secondi
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Unità
settings-maps-units-help = Lunghezze e aree sulle mappe: la barra di scala e le misure. Automatiche segue Metadati › Unità.
settings-maps-units-auto = Automatiche
settings-maps-units-metric = Metriche
settings-maps-units-imperial = Imperiali

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Lat, lon { $position }
}
map-height = Quota { $height }
map-height-label = Quota
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Barra di scala: { $length }
map-north-label = Freccia del nord: il nord è a { $degrees }° in senso orario dall’alto
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] gradi, minuti, secondi
    [utm] UTM
    [mgrs] MGRS
   *[dd] gradi decimali
}
map-format-toast = Coordinate in { $format }
map-units-metric = Unità: metri e chilometri
map-units-imperial = Unità: piedi e miglia
toast-copied-coordinate = Coordinata copiata

## Go to coordinate.
map-goto-hint = Gradi decimali, DMS, UTM o MGRS. Invio per andarci.
map-goto-empty = Scrivi una posizione.
map-goto-unrecognised = Non è una posizione in un formato noto.
map-goto-range = Latitudine o longitudine fuori intervallo.
map-goto-no-way = Questa mappa non può collocare una posizione.
map-goto-outside = Quella posizione è fuori dalla mappa.
map-goto-toast = Centrata su { $position }

## The layer panel.
map-layers-title = Layer: { $shown } di { $total } visibili
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } elemento
   *[other] { $features } elementi
}, opacità { PERCENT($opacity) }
map-kind-area = Aree
map-kind-line = Linee
map-kind-point = Punti
map-kind-empty = Nessun elemento
map-layer-visible = Mostra o nascondi il layer
map-layer-opacity-down = Meno opaco
map-layer-opacity-up = Più opaco
map-layer-colour = Colore successivo
map-layer-width = Spessore di linea successivo
map-layer-point = Dimensione del punto successiva
map-layer-up = Sposta su
map-layer-down = Sposta giù
map-layer-remove = Rimuovi
map-layer-keep = Mantieni
map-layer-remove-ask = Rimuovere { $name } dalla mappa? Il file resta sul disco.
map-layer-adding = Aggiunta di un layer…
map-removed = { $name } rimosso dalla mappa
map-added = { $layers ->
    [one] { $name } aggiunto
   *[other] { $name } aggiunto: { $layers } layer
}
map-add-failed = Impossibile aggiungere { $name }: { $reason }
map-add-filter = File di mappa
map-add-not-map = non è una mappa.
map-add-not-vector = non ha elementi.
map-add-no-crs = non ha un file di proiezione (.prj).

## Identify.
map-identify-more = e altri { $count } nella tabella degli attributi
map-cell = Cella { $x }, { $y }
map-band = Banda { $n }
map-no-data = Nessun dato

## The attribute table.
map-table-title = { $layer }: { $shown } di { $total } elementi, { $selected } selezionati
map-table-search = Cerca, o campo = valore
map-table-selected-only = Solo selezionati
map-table-empty = Nessun elemento corrisponde.
map-table-many = molti
map-table-stats-number = { $field }: { $count ->
        [one] { $count } valore
       *[other] { $count } valori
    }, { $empty } vuoti · min { $min } · max { $max } · media { $mean } · somma { $sum } · dev. std. { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } valore
       *[other] { $count } valori
    }, { $empty } vuoti · { $distinct } distinti
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } valore
       *[other] { $count } valori
    }, { $empty } vuoti · { $distinct } distinti · più frequente { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Fai clic su punti lungo il percorso.
map-measure-hint-area = Fai clic sugli angoli dell’area.
map-measure-hint-profile = Fai clic su punti lungo la linea.
map-measure-keys = Invio termina. Backspace rimuove l’ultimo punto. Esc chiude.
map-measure-distance = Distanza { $length }
map-measure-bearing = Azimut { $bearing }
map-measure-segment = Ultimo tratto { $length }, azimut { $bearing }
map-measure-area = Area { $area }
map-measure-perimeter = Perimetro { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Lunghezza { $length } · minima { $min } · massima { $max } · salita { $climb } · discesa { $descent }
map-profile-no-heights = Lunghezza { $length }. Nessuna quota lungo la linea.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: nessuna quota
