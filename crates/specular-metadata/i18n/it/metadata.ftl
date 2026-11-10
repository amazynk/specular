# machine-drafted, needs native review
# Specular — pannello dei metadati. Italiano.
# Vedi i18n/en/metadata.ftl per le convenzioni. Numeri e date arrivano già
# formattati da ICU4X per la lingua locale.

## Titoli dei gruppi del riepilogo

group-camera = Fotocamera
group-exposure = Esposizione
group-lens = Obiettivo
group-date = Data e ora
group-location = Luogo
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drone
label-gimbal = Gimbal
label-flight = Velivolo
label-relative-altitude = Sopra il punto di decollo
drone-attitude = imbardata { $yaw }, beccheggio { $pitch }, rollio { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperature
drone-thermal-refused = Non mostrate: la licenza dell’SDK di DJI non lo consente
drone-thermal-refused-detail = Leggere le temperature DJI richiede il Thermal SDK di DJI, la cui licenza ne consente l’uso solo ai programmi che lo includono. L’immagine e i suoi metadati sono mostrati.
group-image = Immagine
group-file = File
group-rights = Diritti e descrizione
group-credentials = Credenziali del contenuto
# machine-drafted, needs native review
group-medical = Immagine medica
# machine-drafted, needs native review
group-inspection = Componente e ispezione
group-nde-image = Immagine di ispezione (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Immagine di ispezione
group-science = Immagine scientifica
group-em = Microscopia elettronica
group-spm = Microscopia a scansione di sonda
group-slide = Vetrino
group-microscopy = Microscopia
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Diffrazione
group-surface = Topografia superficiale
group-ebsd = Diffrazione di elettroni retrodiffusi
group-seismic = Dati sismici
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = Immagine CASA
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Trattografia
# machine-drafted, needs native review
group-annotations = Annotazioni
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Imaging di spettrometria di massa
# machine-drafted, needs native review
group-thermal = Immagine termica
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Citometria a flusso
# machine-drafted, needs native review
group-spectroscopy = Spettroscopia
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Mesh non strutturata
group-drawing = Disegno
# machine-drafted, needs native review
group-volume = Volume

## Etichette delle voci del riepilogo

label-camera = Fotocamera
label-software = Software
label-application = Applicazione
label-shutter = Tempo di posa
label-aperture = Diaframma
label-iso = ISO
label-focal-length = Lunghezza focale
label-exposure-bias = Compensazione esposizione
label-program = Modalità
label-metering = Misurazione
label-white-balance = Bilanciamento del bianco
label-flash = Flash
label-lens = Obiettivo
label-lens-make = Produttore obiettivo
label-lens-range = Gamma obiettivo
# machine-drafted, needs native review
label-focus-mode = Modalità di messa a fuoco
label-stabilisation = Stabilizzazione dell’immagine
label-taken = Scattata
label-digitised = Digitalizzata
label-edited = Ultima modifica
label-place = Luogo
label-coordinates = Coordinate
label-altitude = Altitudine
label-direction = Direzione
label-accuracy = Precisione
label-gps-time = Ora GPS
label-dimensions = Dimensioni
label-encoding = Codifica
# machine-drafted, needs native review
label-motion = Movimento
motion-kind-motion-photo = Foto in movimento
motion-kind-micro-video = Foto in movimento (MicroVideo)
motion-kind-samsung = Foto in movimento Samsung
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Profilo colore
label-orientation = Orientamento
label-file-name = Nome
label-folder = Cartella
label-file-size = Dimensione
label-format = Formato
label-modified = Modificato
label-title = Titolo
label-description = Descrizione
label-creator = Autore
label-copyright = Copyright
label-keywords = Parole chiave
label-rating = Valutazione
label-colour-label = Etichetta
label-credentials = Credenziali del contenuto

## Titoli dei gruppi "Tutti i campi" (i nomi dei tag restano come negli standard)

fields-ifd0 = IFD0 (immagine principale)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperabilità
fields-ifd1 = IFD1 (miniatura)
fields-icc = Intestazione profilo ICC
# $namespace: da xmp-namespace; $prefix: ad es. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Base
    [xmpRights] Diritti
    [xmpMM] Gestione dei media
    [xmpDM] Media dinamici
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Accessori fotocamera
    [photoshop] Photoshop
    [crs] Impostazioni Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] Licenze PLUS
    [GPano] Foto sferica
    [GCamera] Fotocamera Google
    [hdrgm] Mappa di guadagno HDR
    [mwg-rs] Regioni
    [pdf] PDF
    [stEvt] Eventi della risorsa
    [stRef] Riferimenti della risorsa
   *[other] Altro
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Tag sconosciuto { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } byte)
   *[other] ({ $num } byte)
}
maker-note-bytes = ({ $num ->
        [one] { $num } byte
       *[other] { $num } byte
    }, nota del produttore, non decodificata)
more-values = { $shown }, … ({ $total ->
        [one] { $total } valore
       *[other] { $total } valori
    })

## Elementi di collegamento

# Separa le parti di un elenco costruito nel codice (descrizione del flash, aree del luogo).
list-separator = {", "}
# Separa tempo · diaframma · ISO · focale nel riepilogo su una riga.
one-line-separator = {" · "}

## Esposizione

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm ({ $equiv } mm equiv.)
focal-length-equiv-only = { $equiv } mm equiv.
# $ev è il valore assoluto; il segno viene dal selettore.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manuale
    [2] Programma (auto)
    [3] Priorità diaframma
    [4] Priorità tempi
    [5] Creativo (profondità di campo)
    [6] Azione (tempi rapidi)
    [7] Ritratto
    [8] Paesaggio
   *[other] Sconosciuto
}
metering-mode = { $value ->
    [1] Media
    [2] Media ponderata al centro
    [3] Spot
    [4] Multi-spot
    [5] Matrice (valutativa)
    [6] Parziale
   *[other] Altro
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] AF One-Shot
    [1] AF AI Servo
    [2] AF AI Focus
    [3] Messa a fuoco manuale
    [4] Singolo
    [5] Continuo
    [6] Messa a fuoco manuale
    [16] Pan Focus
    [256] AF One-Shot (Live View)
    [257] AF AI Servo (Live View)
    [258] AF AI Focus (Live View)
    [512] Messa a fuoco Movie Snap
    [519] AF Servo filmato
   *[other] Altro
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Disattivata
    [1] Attivata
    [2] Solo allo scatto
    [3] Panning
    [4] Dinamica
    [256] Disattivata
    [257] Attivata
    [258] Solo allo scatto
    [259] Panning
    [260] Dinamica
   *[other] Altro
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Auto
    [2] Messa a fuoco manuale
    [4] Auto (pulsante di messa a fuoco)
    [5] Auto (continua)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Altro
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Attivo (ottico)
    [3] Disattivo
    [4] Attivo (modo 2)
    [5] Attivo (ottico, panning)
    [6] Attivo (solo corpo)
    [7] Attivo (solo corpo, panning)
    [9] Dual IS
    [10] Dual IS (panning)
    [11] Dual IS 2
    [12] Dual IS 2 (panning)
   *[other] Altro
}
label-comment = Commento
fields-tags = Tag { $kind }
group-picture = Impostazioni immagine
fields-fujifilm = Dati del produttore Fujifilm
label-film-simulation = Simulazione pellicola
label-dynamic-range = Gamma dinamica
label-highlight-tone = Alte luci
label-shadow-tone = Ombre
label-colour-setting = Colore
label-sharpness = Nitidezza
label-noise-reduction = Riduzione del rumore
label-clarity = Chiarezza
label-grain-effect = Effetto grana
label-colour-chrome = Effetto colore cromo
label-colour-chrome-blue = Colore cromo blu
label-shutter-type = Tipo di otturatore
label-drive-mode = Modalità di scatto
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (standard)
    [256] Ritratto in studio
    [272] Ritratto in studio (più saturazione)
    [288] Astia (morbido)
    [304] Ritratto in studio (più nitido)
    [512] Velvia (vivace)
    [768] Ritratto in studio Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Altro
}
fuji-monochrome = { $value ->
    [768] Monocromatico
    [769] Monocromatico, filtro rosso
    [770] Monocromatico, filtro giallo
    [771] Monocromatico, filtro verde
    [784] Seppia
    [1280] Acros
    [1281] Acros, filtro rosso
    [1282] Acros, filtro giallo
    [1283] Acros, filtro verde
   *[other] Monocromatico
}
fuji-white-balance = { $value ->
    [0] Automatico
    [1] Automatico (priorità al bianco)
    [2] Automatico (priorità all’atmosfera)
    [256] Luce diurna
    [512] Nuvoloso
    [768] Fluorescente luce diurna
    [769] Fluorescente bianco diurno
    [770] Fluorescente bianco
    [771] Fluorescente bianco caldo
    [772] Fluorescente bianco caldo da interni
    [1024] Incandescenza
    [1280] Flash
    [1536] Subacqueo
    [3840] Personalizzato 1
    [3841] Personalizzato 2
    [3842] Personalizzato 3
    [3843] Personalizzato 4
    [3844] Personalizzato 5
    [4080] Temperatura colore
   *[other] Altro
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Automatico (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Automatico
    [256] Standard (DR100)
    [512] Ampio 1 (DR230)
    [513] Ampio 2 (DR400)
   *[other] Altro
}
fuji-effect = { $value ->
    [0] Disattivo
    [32] Debole
    [64] Forte
   *[other] Altro
}
fuji-grain-size = { $value ->
    [16] piccola
    [32] grande
   *[other] altro
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Meccanico
    [1] Elettronico
    [2] Elettronico (pose lunghe)
    [3] Prima tendina elettronica
   *[other] Altro
}
fuji-drive-mode = { $value ->
    [0] Scatto singolo
    [1] Continuo, bassa velocità
    [2] Continuo, alta velocità
   *[other] Altro
}
fuji-focus-mode = { $value ->
    [0] Manuale
    [1] AF-S
    [2] AF-C
    [100] Automatico
    [101] Manuale
   *[other] Altro
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Nessuna
    [1] Ottica
    [2] Sensore mobile
    [3] Obiettivo
    [258] Sensore mobile e digitale
    [512] Digitale
   *[other] Altro
}
fuji-stabilisation-mode = { $value ->
    [0] Disattivo
    [1] attiva, continua
    [2] attiva, solo allo scatto
   *[other] Altro
}
light-source = { $value ->
    [1] Luce diurna
    [2] Fluorescente
    [3] Tungsteno
    [4] Flash
    [9] Bel tempo
    [10] Nuvoloso
    [11] Ombra
    [12] Fluorescente luce diurna
    [13] Fluorescente bianco diurno
    [14] Fluorescente bianco freddo
    [15] Fluorescente bianco
    [16] Fluorescente bianco caldo
    [17] Luce standard A
    [18] Luce standard B
    [19] Luce standard C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Tungsteno da studio
   *[other] Altro
}
white-balance-auto = Automatico
white-balance-manual = Manuale
white-balance-manual-with-light = Manuale ({ $light })
# La descrizione del flash è composta da queste parti, unite con list-separator.
flash-none = Nessuna funzione flash
flash-fired = Flash scattato
flash-not-fired = Flash non scattato
flash-forced-on = forzato attivo
flash-forced-off = forzato disattivo
flash-auto = modalità automatica
flash-return-not-detected = luce di ritorno non rilevata
flash-return-detected = luce di ritorno rilevata
flash-red-eye = riduzione occhi rossi

## Obiettivo

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Data e ora

# $weekday: "martedì" (ICU); $date: "14 mag 2024" (ICU)
date-with-weekday = { $weekday } { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Fuso orario ricavato dall’orologio GPS

## Luogo

# $lat e $lon sono senza segno; l’emisfero viene dal selettore.
coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] O
   *[east] E
}
altitude = { $ref ->
    [above] { $m } m sul livello del mare
    [below] { $m } m sotto il livello del mare
   *[sea] Al livello del mare
}
compass-point = { $point ->
    [n] nord
    [nne] nord-nord-est
    [ne] nord-est
    [ene] est-nord-est
    [e] est
    [ese] est-sud-est
    [se] sud-est
    [sse] sud-sud-est
    [s] sud
    [ssw] sud-sud-ovest
    [sw] sud-ovest
    [wsw] ovest-sud-ovest
    [w] ovest
    [wnw] ovest-nord-ovest
    [nw] nord-ovest
   *[nnw] nord-nord-ovest
}
# $point: da compass-point; $degrees: "287"
facing = Rivolta a { $point } ({ $degrees }°)
detail-true-north = Rispetto al nord geografico
detail-magnetic-north = Rispetto al nord magnetico
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# A più di 30 km dalla località trovata.
place-near = vicino a { $place }
place-detail = { $areas } · { $distance } da { $city }
place-detail-distance = { $distance } da { $city }

## Immagine

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } bit
# $format: "YCbCr 4:2:0 a 8 bit"
encoding-progressive = { $format }, progressivo
encoding-baseline = { $format }, baseline
encoding-hierarchical = { $format }, gerarchico
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] con alfa premoltiplicato
    [transparency] con trasparenza
   *[alpha] con alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Scala di grigi
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Colore indicizzato
    [Multichannel] Multicanale
    [Duotone] Bicromia
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminanza/crominanza (YC)
    [Bayer] RAW Bayer
    [X-Trans] RAW X-Trans
    [CFA] RAW con matrice di filtri colore
    [LinearRaw] RAW lineare
    [RGBE] RGBE (esponente condiviso)
    [XYZE] XYZE (esponente condiviso)
    [R] Canale singolo (R)
    [RG] Due canali (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (da tag EXIF; nessun profilo incorporato)
orientation = { $value ->
    [1] Dritta (nessuna rotazione necessaria)
    [2] Riflessa orizzontalmente (applicato)
    [3] Ruotata di 180° (applicato)
    [4] Riflessa verticalmente (applicato)
    [5] Riflessa orizzontalmente e ruotata di 90° in senso antiorario (applicato)
    [6] Ruotata di 90° in senso orario (applicato)
    [7] Riflessa orizzontalmente e ruotata di 90° in senso orario (applicato)
    [8] Ruotata di 90° in senso antiorario (applicato)
   *[other] Sconosciuto ({ $value })
}
# Forma breve per gli avvisi.
orientation-short = { $value ->
    [1] Dritta
    [2] Riflessa orizzontalmente
    [3] Ruotata di 180°
    [4] Riflessa verticalmente
    [5] Riflessa orizzontalmente e ruotata di 90° in senso antiorario
    [6] Ruotata di 90° in senso orario
    [7] Riflessa orizzontalmente e ruotata di 90° in senso orario
    [8] Ruotata di 90° in senso antiorario
   *[other] valore { $value }
}

## File

file-size-bytes = { $num ->
    [one] { $num } byte
   *[other] { $num } byte
}
# Unità decimali (1 KB = 1000 byte).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Diritti

rating-rejected = Scartata
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n } su 5)

## Credenziali del contenuto

c2pa-present = Presenti (non verificate)
c2pa-detail = Questo file contiene un manifesto C2PA; la sua firma non è stata verificata.

## Valori dell’intestazione ICC

icc-class = { $class ->
    [scnr] Dispositivo di input
    [mntr] Dispositivo di visualizzazione
    [prtr] Dispositivo di output
    [link] Collegamento tra dispositivi
    [spac] Spazio colore
    [abst] Astratto
    [nmcl] Colore con nome
   *[other] Sconosciuto
}
icc-intent = { $intent ->
    [0] Percettivo
    [1] Colorimetrico relativo
    [2] Saturazione
    [3] Colorimetrico assoluto
   *[other] Sconosciuto
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Non specificata
   *[other] Sconosciuta
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalità
label-body-part = Regione anatomica
label-study = Studio
label-series = Serie
label-slice-thickness = Spessore della fetta
label-pixel-spacing = Spaziatura dei pixel
label-imager-pixel-spacing = Spaziatura dei pixel al rivelatore
label-frames = Fotogrammi
label-transfer-syntax = Sintassi di trasferimento
label-document = Documento
label-document-type = Tipo di documento
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (compressione con perdita)
detail-pixel-spacing = Distanza tra i centri dei pixel nel paziente (righe × colonne).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Distanza tra i centri dei pixel nell’oggetto (righe × colonne).
detail-imager-pixel-spacing = Misurata al rivelatore, non corretta per l’ingrandimento: le distanze nel paziente sono di solito minori.

## Ispezione industriale (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Forma del componente
label-nde-component-manufacturer = Produttore del componente
label-nde-material = Materiale
label-nde-thickness = Spessore del materiale
label-nde-pipe-diameter = Diametro del tubo
label-nde-source = Sorgente
label-nde-environment = Ambiente
label-nde-iqi = Indicatore di qualità dell’immagine
label-nde-indications = Indicazioni
label-nde-standard = Norma

## Volumi di ricerca (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Dimensione voxel
label-data-type = Tipo di dati
label-time-points = Punti temporali
label-orientation-source = Orientamento da
label-intent = Valori
label-compression = Compressione
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Intestazione Analyze (non affidabile)
volume-orient-space = Direzioni spaziali NRRD
volume-orient-transform = Trasformazione MetaImage
volume-orient-spacings = Solo spaziatura dei voxel
volume-orient-none = Non indicato
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = Predefinito FreeSurfer (non affidabile)
volume-orient-minc = Coseni direttori MINC
volume-orient-object = Coordinate dell’oggetto (nessun orientamento paziente)
volume-content-projections = Proiezioni (scansione grezza)
volume-content-reconstruction = Volume ricostruito
volume-coords-rectilinear = Rettilinea (spaziatura irregolare, mostrata uniforme)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Tempo di ripetizione
label-volume-te = Tempo di eco
label-volume-ti = Tempo di inversione
label-volume-flip = Angolo di flip
label-volume-fov = Campo di vista
label-volume-field = Intensità di campo
label-volume-pedir = Codifica di fase
label-volume-cmdline = Riga di comando
label-volume-xform = Trasformazione di Talairach
label-volume-content = Contenuto
label-volume-projections = Proiezioni
label-volume-angles = Intervallo di rotazione
label-volume-angle-step = Passo di rotazione
label-volume-kv = Tensione del tubo
label-volume-current = Corrente del tubo
label-volume-power = Potenza della sorgente
label-volume-exposure = Esposizione
label-volume-sod = Sorgente–oggetto
label-volume-odd = Oggetto–rivelatore
label-volume-sdd = Sorgente–rivelatore
label-volume-magnification = Ingrandimento ottico
label-volume-binning = Binning
label-volume-objective = Obiettivo
label-volume-filter = Filtro della sorgente
label-volume-camera = Rivelatore
label-volume-instrument = Strumento
label-volume-software = Software
label-volume-date = Acquisito
label-volume-ring = Correzione artefatti ad anello
label-volume-smoothing = Smussatura
label-volume-coords = Griglia
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Occhio
volume-eye-right = Destro (OD)
volume-eye-left = Sinistro (OS)
detail-volume-unreliable = Il file non indica dove puntano i suoi assi nel paziente: destra e sinistra potrebbero essere invertite, quindi non vengono mostrate etichette di orientamento.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Gli assi sono quelli dello scanner, non di un paziente, quindi non vengono mostrate etichette di orientamento.

## Immagini scientifiche (FITS, XISF)

label-object = Oggetto
label-telescope = Telescopio
label-instrument = Strumento
label-filter = Filtro
label-observed = Osservato
label-exposure-time = Esposizione
label-sample = Campioni
label-unit = Unità
label-hdu = Estensione
label-centre = Centro
label-centre-deg = Centro (gradi)
label-frame = Sistema di coordinate
# machine-drafted, needs native review
science-frame-galactic = Galattico
science-frame-ecliptic = Eclittico
label-projection = Proiezione
label-pixel-scale = Scala del pixel
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ per pixel
label-distortion = Distorsione
label-wcs = Coordinate celesti
science-wcs-unsupported = Non supportato: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Approssimate (solo puntamento, senza distorsione)
label-observer = Osservatore
label-bayer = Schema Bayer
label-capture-start = Inizio ripresa (UTC)
label-capture-end = Fine ripresa (UTC)
label-frame-rate = Frequenza fotogrammi
science-fps = { $fps } fotogrammi/s
science-frames-cut = { $frames } di { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Dataset
label-pixel-size = Dimensione pixel
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } per pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Microscopio
label-voltage = Tensione di accelerazione
label-magnification = Ingrandimento
label-imaging-mode = Modalità
label-detector = Rivelatore
label-acquired = Acquisito
label-field-of-view = Campo visivo
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Produttore
label-scanner = Scanner
label-objective = Obiettivo
label-pyramid-levels = Livelli
# machine-drafted, needs native review
label-emissivity = Emissività
label-object-distance = Distanza dell’oggetto
label-reflected-temperature = Temperatura riflessa
label-atmospheric-temperature = Temperatura atmosferica
label-humidity = Umidità relativa
label-raw-range = Intervallo grezzo
label-temperature-range = Intervallo di temperatura
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Lunghezza d’onda
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Energia dei fotoni
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Distanza del rivelatore
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Centro del fascio
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = 2θ del rivelatore
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Fi (φ)
label-omega = Omega (ω)
label-chi = Chi (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscillazione
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° attorno a { $axis }
# The count at which a pixel saturates.
label-overload = Soglia di saturazione
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Risoluzione
xtal-resolution = { $edge } Å al bordo, { $corner } Å negli angoli
label-geometry = Geometria
xtal-curved = Image plate curva: niente q, d né anelli di risoluzione
xtal-incomplete = Intestazione incompleta: niente q, d né anelli di risoluzione
# All-fields group title: "FITS header".
fields-science-header = Intestazione { $format }
# machine-drafted, needs native review
fields-asdf-tree = Albero ASDF
label-asdf-model = Modello dati
label-asdf-version = Versione del file
label-asdf-standard = Standard ASDF
label-asdf-library = Scritto da
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = Chiavi ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = Versione di ROOT
label-root-compression = Compressione
label-root-keys = Chiavi
label-root-class = Classe
label-root-name = Nome
label-root-title = Titolo
label-root-entries = Voci
label-root-sum-weights = Somma dei pesi
# A graph's number of points.
label-root-points = Punti
# $axis is "x", "y" or "z".
label-root-axis = Asse { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
       *[other] { $bins } bin
    }, da { $min } a { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } bin variabile
        [many] { $bins } bin variabili
       *[other] { $bins } bin variabili
    }, da { $min } a { $max }
label-root-mean-rms = Media e RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Alberi
root-tree = { $name } ({ $entries ->
        [one] { $entries } voce
        [many] { $entries } voci
       *[other] { $entries } voci
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Tipo di immagine
casa-type-image = Immagine restaurata
casa-type-residual = Residuo
casa-type-model = Modello
casa-type-pb = Fascio primario
casa-type-mask = Maschera di pulizia
casa-type-psf = Funzione di diffusione del punto
casa-type-sumwt = Somma dei pesi
casa-type-weight = Pesi
label-casa-beam = Fascio di restauro
casa-beam = { $major }″ × { $minor }″, AP { $pa }°
label-casa-plane-beams = Fasci per piano
casa-plane-beams = { $count ->
        [one] { $count } fascio
        [many] { $count } di fasci
       *[other] { $count } fasci
    }, asse maggiore da { $min }″ a { $max }″
label-casa-rest-frequency = Frequenza a riposo
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Sistema di riferimento spettrale
label-casa-velocity = Definizione della velocità
casa-velocity-radio = Radio
casa-velocity-optical = Ottica
casa-velocity-relativistic = Relativistica
label-casa-mask = Maschera predefinita
label-casa-storage = Archiviazione
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Mesh
label-ugrid-convention = Convenzione
label-ugrid-nodes = Nodi
label-ugrid-faces = Facce
label-ugrid-edges = Spigoli
label-ugrid-shapes = Forme delle facce
label-ugrid-coordinates = Coordinate
label-ugrid-values = Valori su
label-ugrid-split = Divise per il disegno
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } triangolo
    [many] { $count } di triangoli
   *[other] { $count } triangoli
}
ugrid-quads = { $count ->
    [one] { $count } quadrilatero
    [many] { $count } di quadrilateri
   *[other] { $count } quadrilateri
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } poligono di { $max } nodi
    [many] { $count } di poligoni fino a { $max } nodi
   *[other] { $count } poligoni fino a { $max } nodi
}
ugrid-lonlat = Longitudine e latitudine
ugrid-projected = x e y proiettate
ugrid-projected-unit = x e y proiettate ({ $unit })
ugrid-on-faces = Facce (disegnate in tinta unita)
ugrid-on-nodes = Nodi (interpolati sulle facce)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Nessuno: l’immagine mostra i nodi per faccia
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } faccia sull’antimeridiano o su un polo
    [many] { $count } di facce sull’antimeridiano o su un polo
   *[other] { $count } facce sull’antimeridiano o su un polo
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = File
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Forme
label-annotation-groups = Gruppi
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Omesse
annotation-polygon = Poligono
annotation-polyline = Linea
annotation-rectangle = Rettangolo
annotation-ellipse = Ellisse
annotation-point = Punto
annotation-points = Punti
annotation-ruler = Righello
annotation-arrow = Freccia
# The list of annotations is cut short: "and 1,200 more".
annotation-more = e altre { $count }
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Annotazioni in { $file }
label-drawing-version = Versione
label-drawing-units = Unità
label-drawing-extents = Estensione
label-drawing-sheet = Foglio
label-drawing-layouts = Layout
label-drawing-layers = Layer
label-drawing-entities = Entità
label-drawing-codepage = Tabella codici
label-drawing-missing-fonts = Font sostituiti
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (stili { $styles }), sostituito da { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (stili { $styles }), sostituito da font di riserva
# Survey data (LandXML).
label-survey-crs = Sistema di riferimento
survey-crs-none = Non indicato
label-survey-project = Nome del progetto
label-survey-application = Scritto da
label-survey-surface = Modello del terreno
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } punto
       *[other] { $points } punti
    }, { $triangles ->
        [one] { $triangles } triangolo
       *[other] { $triangles } triangoli
    }; area in pianta { $area } { $unit }², area reale { $area3d } { $unit }²; { $volume } { $unit }³ sopra il punto più basso; quote { $zmin }–{ $zmax } { $unit }; curve di livello ogni { $interval } { $unit }
label-survey-alignment = Asse
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, progressive da { $start } a { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = Font SHX
label-drawing-warnings = Problemi
label-plot-paper = Carta
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } di { $pages }
label-plot-page = Pagina
label-plot-pens = Penne usate
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Processo di stampa
# What a plot draws: strokes, fills and labels.
label-plot-items = Elementi disegnati
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Modello
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } disattivati o congelati
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Pollici
    [2] Piedi
    [3] Miglia
    [4] Millimetri
    [5] Centimetri
    [6] Metri
    [7] Chilometri
    [8] Micropollici
    [9] Millesimi di pollice
    [10] Iarde
    [11] Ångström
    [12] Nanometri
    [13] Micron
    [14] Decimetri
    [15] Decametri
    [16] Ettometri
    [17] Gigametri
    [18] Unità astronomiche
    [19] Anni luce
    [20] Parsec
    [21] Piedi topografici USA
   *[other] Senza unità
}
# All-fields group title: "DXF entities".
fields-drawing-entities = Entità { $format }

## Layout di circuiti integrati (GDSII)

# machine-drafted, needs native review
label-layout-library = Libreria
label-layout-units = Unità
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } unità di database, { $user } unità utente
# The top cell shown (each top cell is a page).
label-layout-cell = Cella
label-layout-cells = Celle
# Cells no other cell places.
label-layout-top-cells = Celle top
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } e altre { $more }
# Layer/datatype pairs.
label-layout-layers = Layer
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Forme
label-layout-texts = Etichette
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Riferimenti
layout-references = { $refs } ({ $placements } { $placements ->
    [one] istanza
    [many] di istanze
   *[other] istanze
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Forme appiattite
label-layout-bounds = Limiti
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } a { $x1 }, { $y1 } µm

## Circuiti stampati e schemi (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Circuito stampato
group-schematic = Schema elettrico
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Vista
label-pcb-project = Progetto
label-pcb-revision = Revisione
label-pcb-date = Data
# The CAD program that wrote the file.
label-pcb-generator = Creato con
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Dimensioni della scheda
label-pcb-thickness = Spessore
label-pcb-copper-layers = Layer in rame
label-pcb-sheets = Fogli
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Layer
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Footprint
label-pcb-symbols = Simboli
label-pcb-nets = Net
label-pcb-tracks = Piste
label-pcb-wires = Collegamenti
label-pcb-vias = Via
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Fori
pcb-holes = { $holes } in { $sizes } { $sizes ->
    [one] diametro
    [many] di diametri
   *[other] diametri
}
label-pcb-apertures = Aperture
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Rame interno { $n }
pcb-role-top-copper = Rame superiore
pcb-role-bottom-copper = Rame inferiore
pcb-role-top-solder-mask = Solder mask superiore
pcb-role-bottom-solder-mask = Solder mask inferiore
pcb-role-top-silkscreen = Serigrafia superiore
pcb-role-bottom-silkscreen = Serigrafia inferiore
pcb-role-top-paste = Pasta superiore
pcb-role-bottom-paste = Pasta inferiore
pcb-role-top-solder-paste = Pasta saldante superiore
pcb-role-bottom-solder-paste = Pasta saldante inferiore
pcb-role-top-adhesive = Adesivo superiore
pcb-role-bottom-adhesive = Adesivo inferiore
pcb-role-top-courtyard = Ingombro superiore
pcb-role-bottom-courtyard = Ingombro inferiore
pcb-role-top-fabrication = Fabbricazione superiore
pcb-role-bottom-fabrication = Fabbricazione inferiore
pcb-role-board-outline = Contorno della scheda
pcb-role-plated-drill = Foratura metallizzata
pcb-role-non-plated-drill = Foratura non metallizzata
pcb-role-drill = Foratura
pcb-role-drill-holes = Fori di foratura
pcb-role-mechanical = Meccanico
pcb-role-margin = Margine
pcb-role-drawings = Disegni
pcb-role-comments = Commenti
pcb-role-engineering-change-1 = Modifica tecnica 1
pcb-role-engineering-change-2 = Modifica tecnica 2
pcb-role-vias = Via
pcb-role-other = Altro
pcb-role-unknown = Sconosciuto
label-pcb-macros = Macro di apertura

## Spettri (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spettro
label-spectrum-format = Formato
label-spectrum-technique = Tecnica
label-spectrum-title = Titolo
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Pagina
spectrum-page-of = { $page } di { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = Asse X
label-spectrum-y = Asse Y
label-spectrum-nucleus = Nucleo
label-spectrum-frequency = Frequenza dello spettrometro
label-spectrum-solvent = Solvente
label-spectrum-pulse-program = Sequenza di impulsi
label-spectrum-spectra = Spettri
label-spectrum-points = Punti
label-spectrum-instrument = Strumento
# Who or what lab measured it.
label-spectrum-origin = Origine
label-spectrum-date = Misurato
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Elaborazione
spectrum-technique-ir = Infrarosso
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescenza
spectrum-technique-nmr = NMR
spectrum-technique-ms = Spettro di massa
spectrum-technique-chromatogram = Cromatogramma
spectrum-technique-other = Spettro

## Modelli 3D (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = Modello 3D
label-triangles = Triangoli
label-vertices = Vertici
label-materials = Materiali
label-textures = Texture
label-up-axis = Asse verticale
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } mancanti)

## Strutture molecolari (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Struttura
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Voce
label-structure-classification = Classificazione
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Metodo
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Risoluzione
structure-resolution = { $value } Å
label-structure-deposited = Depositata
label-structure-formula = Formula
label-structure-chains = Catene
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Residui
label-structure-atoms = Atomi
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } nell’unità asimmetrica)
label-structure-bonds = Legami
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modelli
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Record
structure-first-of = { $count } (viene mostrato il primo)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligandi
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } e altri { $more }
label-structure-cell = Cella elementare
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Gruppo spaziale
label-structure-secondary = Struttura secondaria
structure-ss-file = Dal file
structure-ss-computed = Calcolata dalla catena principale

## HDF4 e HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Libreria
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Dataset
label-hdf4-images = Immagini raster
label-hdf4-vgroups = Gruppi
label-hdf4-vdatas = Tabelle
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Versione
# The grids, swaths and point structures the file defines.
label-eos-structures = Strutture
eos-grids = { $count ->
    [one] { $count } griglia ({ $names })
    [many] { $count } griglie ({ $names })
   *[other] { $count } griglie ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } strisciata ({ $names })
    [many] { $count } strisciate ({ $names })
   *[other] { $count } strisciate ({ $names })
}
eos-points = { $count ->
    [one] { $count } struttura di punti
    [many] { $count } strutture di punti
   *[other] { $count } strutture di punti
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Struttura
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Proiezione
label-eos-grid-size = Dimensioni della griglia
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Angoli
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Prodotto
# When the data was acquired.
label-eos-time = Data
eos-time = da { $begin } a { $end }

## Immagini planetarie (VICAR, PDS3, PDS4: dati di missioni NASA ed ESA)

# machine-drafted, needs native review
group-planetary = Immagine planetaria
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Obiettivo
label-planetary-mission = Missione
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Veicolo spaziale
label-planetary-start = Ora di inizio
label-planetary-stop = Ora di fine
# The product's identifier in the archive.
label-planetary-product = ID prodotto
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Proiezione cartografica
label-planetary-resolution = Risoluzione
planetary-metres-per-pixel = { $metres } m per pixel
planetary-pixels-per-degree = { $pixels } pixel per grado
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Centro della proiezione
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Copertura
planetary-coverage = latitudine da { $south }° a { $north }°, longitudine da { $west }° a { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Raggio del corpo
planetary-km = { $km } km

## OMERO (immagini aperte da un server OMERO o dall’Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Server
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (come { $user })
omero-server-public = { $server } (pubblico)
label-omero-image = Immagine
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Progetto
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Screening
label-omero-owner = Proprietario
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Acquisizione
label-omero-dimensions = Dimensioni
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } piano Z
   *[other] { $count } piani Z
}
omero-c = { $count ->
    [one] { $count } canale
   *[other] { $count } canali
}
omero-t = { $count ->
    [one] { $count } punto temporale
   *[other] { $count } punti temporali
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Dimensione pixel
label-omero-channels = Canali
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (nascosto)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Ottenuto come
omero-fetched-tiles = Riquadri { $encoding } da { $width } × { $height }, { $levels ->
    [one] { $levels } livello
   *[other] { $levels } livelli
}
omero-fetched-whole = Piani interi in { $encoding }

## Immagini a trasformazione di riflettanza (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Trasformazione di riflettanza
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Base
label-rti-order = Ordine
# Coefficients per colour channel.
label-rti-terms = Coefficienti
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Disposizione del colore
# Coefficient images stored in the file.
label-rti-planes = Piani
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Scala
label-rti-bias = Scostamento

## Nuvole di punti

# machine-drafted, needs native review
group-points = Nuvola di punti
label-points = Punti
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } di { $total }
# The LAS point data record format number (0–10).
label-point-format = Formato punto
# The coordinate reference system.
label-crs = Sistema di coordinate
# What each point carries besides its position.
label-attributes = Attributi
points-channel-rgb = RGB
points-channel-intensity = Intensità
points-channel-classification = Classificazione
points-channel-returns = Ritorni
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Classi
# E57 scans, with point counts.
label-scans = Scansioni
point-class-0 = Mai classificato
point-class-1 = Non classificato
point-class-2 = Terreno
point-class-3 = Vegetazione bassa
point-class-4 = Vegetazione media
point-class-5 = Vegetazione alta
point-class-6 = Edificio
point-class-7 = Punto basso (rumore)
point-class-8 = Punto chiave del modello
point-class-9 = Acqua
point-class-10 = Ferrovia
point-class-11 = Superficie stradale
point-class-12 = Sovrapposizione
point-class-13 = Fune di guardia
point-class-14 = Conduttore
point-class-15 = Traliccio
point-class-16 = Connettore di cavi
point-class-17 = Impalcato del ponte
point-class-18 = Rumore alto
point-class-19 = Struttura sopraelevata
point-class-20 = Terreno ignorato
point-class-21 = Neve
point-class-22 = Esclusione temporale
# A class without a standard name, by number.
point-class-other = Classe { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Splat gaussiani
label-splats = Splat
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } di { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Colore dipendente dalla vista
splats-harmonics-none = Nessuno
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, addestrato con antialiasing

## Avvisi

# machine-drafted, needs native review
warn-not-diagnostic = Non per uso diagnostico. Specular è un visualizzatore di immagini generico, non un dispositivo medico: la resa non è calibrata per la diagnosi.

warn-gps = Questo file registra dove è stato scattato. Rimuovi la posizione prima di condividerlo se deve restare privata.
warn-null-island = La posizione GPS è esattamente 0°, 0°: probabilmente la fotocamera non aveva un segnale, quindi il luogo è quasi certamente errato.
warn-date-future = La data di scatto è nel futuro: probabilmente l’orologio della fotocamera era impostato male.
warn-icc-damaged = Il profilo colore incorporato è danneggiato e non è stato possibile leggerlo.
warn-metadata-failed = Non è stato possibile leggere i metadati a causa di un errore di Specular. L’immagine in sé è integra.
warn-orientation-conflict = EXIF e XMP non concordano sull’orientamento (EXIF: { $exif }, XMP: { $xmp }). Viene usato il valore EXIF.
warn-exif-damaged = Alcuni dati EXIF non sono leggibili ({ $error }); viene mostrato il resto.
warn-xmp-damaged = I metadati XMP sono malformati ({ $error }); è stato possibile leggerne solo una parte.

## Informazioni

geonames-attribution = Nomi dei luoghi: GeoNames (www.geonames.org), con licenza Creative Commons Attribution 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Ricerca offline tramite il crate reverse_geocoder di Grant Miner (MIT o Apache-2.0). I dati sono stati convertiti in un elenco di città; sono forniti così come sono, senza garanzia.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Immagine a piena risoluzione
    [1] Immagine a risoluzione ridotta
    [2] Una pagina di un’immagine multipagina
    [4] Maschera di trasparenza
   *[other] Altro
}
predictor = { $value ->
    [1] Nessuno
    [2] Differenza orizzontale
    [3] Differenza in virgola mobile
   *[other] Altro
}
extra-samples = { $value ->
    [0] Non specificato
    [1] Alfa premoltiplicato
    [2] Alfa non premoltiplicato
   *[other] Altro
}
sample-format = { $value ->
    [1] Intero senza segno
    [2] Intero con segno
    [3] Virgola mobile
    [4] Non definito
   *[other] Altro
}

## Dati geospaziali (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Geospaziale
label-geo-crs = Sistema di riferimento
label-geo-datum = Datum
label-geo-projection = Proiezione
label-geo-unit = Unità
label-geo-bounds = Estensione
label-geo-lonlat-bounds = Longitudine e latitudine
label-geo-pixel-size = Dimensione pixel
label-geo-overviews = Panoramiche
label-geo-nodata = Valore NoData
label-geo-raster-type = Tipo di raster
label-geo-vertical = Riferimento verticale
label-geo-citation = Citazione
label-geo-features = Elementi
label-geo-vertices = Vertici
label-geo-layer = Layer
label-geo-layers = Layer
label-geo-tiles = Tile
label-geo-encoding = Codifica del testo
label-geo-warnings = Problemi
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = File associati
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (coordinate non supportate)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Verso WGS 84
geo-to-wgs84-same = Non necessaria
geo-to-wgs84-helmert = Cambio di datum (Helmert)
geo-to-wgs84-by-area = Cambio di datum (Helmert, scelto per zona)
geo-to-wgs84-unknown = Sconosciuta: le coordinate sono nel datum del file
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Non indicato
# Map bounds: the lower-left and upper-right corners.
geo-bounds = da { $x0 }, { $y0 } a { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = da { $west }° a { $east }° di longitudine, da { $south }° a { $north }° di latitudine
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } panoramica: { $sizes }
   *[other] { $n } panoramiche: { $sizes }
}
geo-pixel-is-point = I pixel sono punti
geo-layer = { $features ->
    [one] { $features } elemento
   *[other] { $features } elementi
}, { $fields ->
    [one] { $fields } campo
   *[other] { $fields } campi
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoom { $zoom }, { $tiles ->
    [one] { $tiles } tile
   *[other] { $tiles } tile
}
fields-geo-geokeys = Chiavi GeoTIFF
fields-geo-gdal = Metadati GDAL
fields-geo-wkt = Sistema di riferimento (WKT)
fields-geo-projjson = Sistema di riferimento (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Set di riquadri
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Attribuzione
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Intestazione
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = Versione GIFTI
label-gifti-role = Contiene
gifti-role-surface = Superficie
gifti-role-labels = Etichette per vertice
gifti-role-data = Valori per vertice
label-gifti-arrays = Array di dati
label-gifti-encoding = Codifica
label-gifti-vertices = Vertici
label-gifti-triangles = Triangoli
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Struttura
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Tipo geometrico
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Coordinate
gifti-transform = { $from } (trasformazione in { $to } non necessaria)
gifti-transform-applied = { $from } → { $to } (applicata)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Mostrato su
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Array
label-gifti-intent = Intent
label-gifti-data-type = Tipo di dati
# Entries of the label table (names and colours of regions).
label-gifti-labels = Tabella delle etichette
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = Versione CIFTI
label-cifti-intent = Intent
label-cifti-matrix = Matrice
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Mappe
cifti-maps-scalars = { $count ->
        [one] { $count } mappa scalare
       *[other] { $count } mappe scalari
    }
cifti-maps-labels = { $count ->
        [one] { $count } mappa di etichette
       *[other] { $count } mappe di etichette
    }
cifti-maps-series = { $count ->
        [one] { $count } istante
       *[other] { $count } istanti
    }
cifti-maps-parcels = { $count ->
        [one] { $count } parcella
       *[other] { $count } parcelle
    }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Connettività
cifti-dense = { $seeds ->
        [one] { $seeds } seed
       *[other] { $seeds } seed
    }, { $size } letti per seed
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Serie
cifti-series = da { $start }, ogni { $step } ({ $unit })
label-cifti-models = Modelli cerebrali
label-cifti-parcels = Parcelle
label-cifti-structures = Strutture
label-cifti-voxels = Voxel sottocorticali
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Superficie
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } vertice
       *[other] { $vertices } vertici
    }, { $file }
cifti-surface-missing = { $structure }: serve una superficie con { $vertices ->
        [one] { $vertices } vertice
       *[other] { $vertices } vertici
    } (nessuna accanto al file)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Mostrato come
cifti-shown-surfaces = Superfici colorate
cifti-shown-voxels = Volume dei voxel sottocorticali
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Connettività del seed scelto
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Mappe escluse
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Formato
label-tract-streamlines = Streamline
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } mostrate)
label-tract-points = Punti
label-tract-mean-length = Lunghezza media
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Scalari per punto
label-tract-properties = Proprietà per streamline
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Griglia
label-tract-voxel-size = Dimensione voxel
label-tract-voxel-order = Ordine dei voxel
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Coordinate
tract-space-vox-to-ras = Dalla matrice voxel-RAS
tract-space-voxel-order = Dall’ordine dei voxel (nessuna matrice voxel-RAS)
tract-space-scanner = Spazio dello scanner
# MRtrix: the tracking step size.
label-tract-step = Passo

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Intestazione del file
label-header-created = Creato
# The program or person that wrote the file.
label-header-creator = Autore
label-header-copyright = Copyright
label-header-project = Progetto
label-header-comment = Commento
# The scanner, camera or other device the image came from.
label-header-device = Dispositivo
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Timecode
# A film frame's edge code (KeyKode).
label-header-edge-code = Codice di bordo
label-header-frame-rate = Frequenza fotogrammi
# Where the frame sits in its sequence.
label-header-frame = Fotogramma
# Film stock and slate.
label-header-film = Pellicola
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Funzione di trasferimento
# The colour primaries the file states.
label-header-colorimetric = Colorimetria
# The X window a screen dump was taken of.
label-header-window = Finestra
label-header-resolution = Risoluzione
label-header-compression = Compressione
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profilo
# machine-drafted, needs native review
label-header-level = Livello
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Livelli wavelet
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Trasformazione del colore
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Tavole da disegno
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Non disegnato
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Disegnato in modo approssimato

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Font
label-font-family = Famiglia
label-font-style = Stile
# The face's place in a font collection: "2 of 4".
label-font-face = Variante
font-face-of = { $index } di { $count }
label-font-version = Versione
label-font-designer = Designer
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Fonderia
label-font-glyphs = Glifi
label-font-outlines = Contorni
font-outlines-truetype = TrueType (quadratici)
font-outlines-cff = PostScript (CFF, cubici)
font-outlines-cff2 = PostScript (CFF2, cubici)
font-outlines-bitmap = Solo bitmap
font-outlines-none = Nessuno
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Incorporamento
font-embedding-installable = Si può incorporare e installare.
font-embedding-restricted = Non si può incorporare.
font-embedding-preview-print = Si può incorporare solo per vedere e stampare.
font-embedding-editable = Si può incorporare in documenti modificabili.
font-embedding-no-subsetting = Solo intero, non come sottoinsieme.
font-embedding-bitmap-only = Solo come bitmap.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Assi di variazione
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Stili con nome
label-font-colour = Glifi a colori
font-colour-colr0 = Livelli COLR
font-colour-colr1 = Sfumature COLR v1
font-colour-sbix = Bitmap (sbix)
font-colour-cbdt = Bitmap (CBDT)
font-colour-svg = SVG
label-font-weight = Peso
label-font-units-per-em = Unità per em
label-font-tables = Tabelle
label-font-licence = Licenza
label-font-licence-url = URL della licenza
label-font-copyright = Copyright
label-font-trademark = Marchio

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Documento
label-document-kind = Tipo
document-kind-word = Documento Word
document-kind-excel = Cartella di lavoro Excel
document-kind-powerpoint = Presentazione PowerPoint
document-kind-pages = Documento Pages
document-kind-numbers = Foglio di calcolo Numbers
document-kind-keynote = Presentazione Keynote
document-kind-odf-text = Testo OpenDocument
document-kind-odf-spreadsheet = Foglio di calcolo OpenDocument
document-kind-odf-presentation = Presentazione OpenDocument
document-kind-odf-graphics = Disegno OpenDocument
document-kind-odf-other = File OpenDocument
# machine-drafted, needs native review
document-kind-playlist = Playlist
# machine-drafted, needs native review
label-playlist-items = Elementi
# Appended to the kind: "Word document, template, with macros".
document-template = modello
document-macros = con macro
document-legacy = formato Office 97–2003
label-document-preview = Anteprima
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = Immagine { $format }
document-preview-none = Nessuna in questo file
label-document-subject = Oggetto
label-document-author = Autore
label-document-description = Descrizione
label-document-category = Categoria
label-document-last-saved-by = Ultimo salvataggio di
label-document-created = Creato
label-document-modified = Modificato
label-document-printed = Ultima stampa
label-document-pages = Pagine
label-document-slides = Diapositive
label-document-hidden-slides = Diapositive nascoste
label-document-notes = Pagine note
label-document-sheets = Fogli
label-document-words = Parole
label-document-characters = Caratteri
label-document-paragraphs = Paragrafi
label-document-lines = Righe
label-document-editing-time = Tempo di modifica
document-minutes = { $minutes } min
label-document-revision = Revisione
label-document-application = Salvato con
label-document-company = Società
label-document-template = Modello

## Books (EPUB)

# machine-drafted, needs native review
group-book = Libro
label-book-authors = Autori
label-book-publisher = Editore
label-book-date = Pubblicato
label-book-language = Lingua
label-book-pages = Pagine
# Entries in the book's table of contents.
label-book-contents = Voci dell’indice
label-book-layout = Impaginazione
book-layout-reflowable = Adattabile
book-layout-fixed = Impaginazione fissa
book-right-to-left = da destra a sinistra
label-book-version = Formato
book-version = EPUB { $version }
label-book-fonts = Font incorporati
book-fonts-obfuscated = { $count } ({ $obfuscated } offuscati)
label-book-subjects = Argomenti
label-book-identifier = Identificativo
label-book-rights = Diritti

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

# machine-drafted, needs native review
volume-orient-parrec = Angolazione e decentramento Philips
volume-orient-ecat = Dimensioni voxel e offset ECAT (non affidabili)
label-volume-technique = Tecnica
label-volume-version = Versione del formato
label-volume-isotope = Isotopo
label-volume-tracer = Tracciante
label-volume-units = Unità

# Text, tables and subtitles.
label-header-encoding = Set di caratteri
label-header-lines = Righe
label-header-line-ends = Fine riga
label-header-words = Parole
label-header-pages = Pagine
label-header-pictures = Immagini
label-header-pictures-not-shown = Immagini non mostrate
label-header-tables = Tabelle
label-header-rows = Righe della tabella
label-header-columns = Colonne
label-header-delimiter = Delimitatore
label-header-column-names = Nomi delle colonne
label-header-cues = Sottotitoli
label-header-cue-span = Sottotitoli da – a
