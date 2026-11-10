# machine-drafted, needs native review
# Strumenti di ispezione: avvisi di clipping, focus peaking, istogramma, ispettore pixel. Italiano.

## Tavolozza dei comandi
palette-clipping = Mostra o nascondi avvisi di clipping
palette-focus-peaking = Mostra o nascondi focus peaking
palette-histogram = Mostra o nascondi istogramma
palette-pixel-inspector = Mostra o nascondi ispettore pixel
# machine-drafted, needs native review
palette-crosshair = Mirino da tastiera
palette-announce-cursor = Leggi il valore sotto il cursore

## Pannello istogramma
histogram-title = Istogramma
# Quota di pixel visibili ai livelli di clipping o oltre; i valori sono percentuali.
histogram-clipped = Clipping: luci { $high }, ombre { $low }
histogram-empty = Nessuna immagine visibile
# Riepilogo per screen reader. $mean è la luminosità media (0–255), $high/$low sono percentuali.
histogram-a11y = Istogramma dell’immagine visibile: luminosità media { $mean }, luci bruciate { $high }, ombre chiuse { $low }

## Pannello ispettore pixel
inspector-title = Ispettore pixel
inspector-position = Posizione
inspector-source-rgb = RGB sorgente
inspector-source-rgba = RGBA sorgente
inspector-source-ycc = Y′CbCr sorgente
inspector-source-gray = Grigio sorgente
inspector-source-gray16 = Grigio sorgente (16 bit)
inspector-source-gray-alpha16 = Grigio sorgente + alfa (16 bit)
inspector-source-rgb16 = RGB sorgente (16 bit)
inspector-source-rgba16 = RGBA sorgente (16 bit)
inspector-source-pending = Sorgente
inspector-srgb = sRGB
inspector-hex = Esadecimale
inspector-linear = Lineare
# Mostrato quando i valori provengono da un’anteprima ridotta e non dai pixel a piena risoluzione.
inspector-approximate = Da un’anteprima ridotta; ingrandisci per i valori esatti
inspector-outside = Punta sull’immagine per ispezionare un pixel
# Riepilogo per screen reader: posizione del pixel e suo valore sRGB.
inspector-a11y = Pixel in { $x }, { $y }: sRGB { $value }

## Impostazioni
settings-section-inspection = Ispezione
settings-inspection-clip-highlight = Livello di clipping delle luci
settings-inspection-clip-highlight-help = I pixel con un canale pari o superiore a questo valore a 8 bit sono segnati come luci bruciate.
settings-inspection-clip-shadow = Livello di clipping delle ombre
settings-inspection-clip-shadow-help = I pixel con un canale pari o inferiore a questo valore a 8 bit sono segnati come ombre chiuse.
settings-inspection-highlight-color = Colore avviso luci
settings-inspection-highlight-color-help = Colore che segna le luci bruciate.
settings-inspection-shadow-color = Colore avviso ombre
settings-inspection-shadow-color-help = Colore che segna le ombre chiuse.
settings-inspection-peaking-sensitivity = Sensibilità del focus peaking
settings-inspection-peaking-sensitivity-help = Valori più alti segnano come a fuoco anche i bordi più morbidi.
settings-inspection-peaking-color = Colore del focus peaking
settings-inspection-peaking-color-help = Colore che segna i bordi nitidi.
settings-inspection-histogram-channels = Canali dell’istogramma
settings-inspection-histogram-channels-help = Quali curve mostra l’istogramma.
settings-inspection-histogram-channels-colour = Rosso, verde e blu
settings-inspection-histogram-channels-luminance = Luminanza
settings-inspection-histogram-channels-both = Colore e luminanza
settings-inspection-histogram-log = Istogramma logaritmico
settings-inspection-histogram-log-help = Scala l’istogramma in modo logaritmico, così anche i valori piccoli restano visibili.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Mostra il canale rosso
palette-channel-green = Mostra il canale verde
palette-channel-blue = Mostra il canale blu
palette-channel-alpha = Mostra il canale alfa (di nuovo: su scacchiera, poi su un colore)
palette-channel-luma = Mostra la luminanza
channel-normal = Tutti i canali
channel-red = Canale rosso
channel-green = Canale verde
channel-blue = Canale blu
channel-alpha = Canale alfa
channel-alpha-checker = Trasparenza su scacchiera
channel-alpha-matte = Trasparenza su colore
channel-luma = Luminanza
disabled-no-alpha = Questa immagine non ha trasparenza
settings-inspection-alpha-matte-color = Colore di sfondo per l’alfa
settings-inspection-alpha-matte-color-help = Colore uniforme dietro l’immagine nella vista «Trasparenza su colore».

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Mostra o nascondi falsi colori
palette-next-colour-map = Mappa colori successiva
palette-channel-colours = Colori dei canali successivi (rosso–verde, magenta–verde, magenta–giallo–ciano)
palette-hilo = Mostra o nascondi HiLo (pixel al massimo e al minimo della profondità di bit)
palette-colour-vision = Simulazione della visione dei colori successiva
palette-colour-vision-off = Interrompi la simulazione della visione dei colori
palette-colour-vision-protanopia = Simula la protanopia (senza coni rossi)
palette-colour-vision-deuteranopia = Simula la deuteranopia (senza coni verdi)
palette-colour-vision-tritanopia = Simula la tritanopia (senza coni blu)
palette-colour-vision-achromatopsia = Simula l’acromatopsia (senza colori)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (non adatta ai daltonici)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = valori grezzi
# Screen reader summary of the colour bar.
colour-bar-a11y = Barra dei colori { $map }: da { $low } a { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Falsi colori: { $map }
badge-hilo = HiLo: pixel al massimo e al minimo evidenziati
badge-colour-vision-protanopia = Simulazione della protanopia
badge-colour-vision-deuteranopia = Simulazione della deuteranopia
badge-colour-vision-tritanopia = Simulazione della tritanopia
badge-colour-vision-achromatopsia = Simulazione dell’acromatopsia
channel-colours-red-green = Canali: rosso, verde, blu
channel-colours-magenta-green = Canali: magenta, verde
channel-colours-magenta-yellow-cyan = Canali: magenta, giallo, ciano
settings-inspection-false-colour-map = Mappa dei falsi colori
settings-inspection-false-colour-map-help = Mappa colori per i falsi colori e la relativa barra. Tutte tranne jet e phase restano leggibili con qualsiasi visione dei colori.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergente)
settings-inspection-false-colour-map-vik = Vik (divergente)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (divergente)
settings-inspection-false-colour-map-curl = Curl (divergente)
settings-inspection-false-colour-map-phase = Phase (ciclica)
settings-inspection-false-colour-map-jet = Jet (non adatta ai daltonici)
settings-inspection-colour-bar = Barra dei colori
settings-inspection-colour-bar-help = Mostra una barra dei colori con i valori nelle unità dell’immagine quando i falsi colori sono attivi.
settings-inspection-channel-colours = Colori dei canali
settings-inspection-channel-colours-help = Come vengono mostrati i canali di colore. Magenta–verde e magenta–giallo–ciano restano distinti con il daltonismo rosso–verde.
settings-inspection-channel-colours-red-green = Rosso, verde, blu
settings-inspection-channel-colours-magenta-green = Magenta, verde
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, giallo, ciano
settings-inspection-colour-vision = Simulazione della visione dei colori
settings-inspection-colour-vision-help = Mostra l’intera finestra come la vede chi ha un deficit della visione dei colori (Machado et al. 2009). Le esportazioni non sono mai interessate.
settings-inspection-colour-vision-off = Disattivata
settings-inspection-colour-vision-protanopia = Protanopia
settings-inspection-colour-vision-deuteranopia = Deuteranopia
settings-inspection-colour-vision-tritanopia = Tritanopia
settings-inspection-colour-vision-achromatopsia = Acromatopsia
settings-inspection-auto-range = Intervallo automatico per grigi ad alta profondità
settings-inspection-auto-range-help = Estende le immagini in scala di grigi a 16 bit (TIFF, PNG, FITS) dal valore più scuro al più chiaro. Le immagini DICOM mantengono la propria finestra.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Mirino attivo. Le frecce lo spostano, Maiusc per passi più grandi. Invio fa clic. Esc termina.
crosshair-off = Mirino disattivato
announce-cursor-nothing = Niente sotto il cursore
