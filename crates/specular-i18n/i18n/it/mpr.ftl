# Multiplanar reformatting (MPR).

palette-mpr = Ricostruzione multiplanare (MPR)
palette-mpr-slab = Spessore MPR successivo (sottile, MIP, MinIP, media)
disabled-no-volume = Questo non è un volume: apri una serie, un file multiframe, un volume 3D o uno z-stack
disabled-mpr-closed = Apri prima le viste multiplanari
mpr-axial = Assiale
mpr-coronal = Coronale
mpr-sagittal = Sagittale
mpr-oblique = Obliquo
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = Vista 3D
mpr-thin = Sottile
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Media
mpr-slab = Spessore: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Spessore: { $mode }, { $thickness } Å
mpr-slab-unit = Spessore: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Spessore: { $mode }, { $thickness } voxel
mpr-thicker = Spessore maggiore
mpr-thinner = Spessore minore
mpr-reset = Reimposta viste
mpr-close = Chiudi MPR
mpr-maximise = Ingrandisci vista
mpr-export-planes = Esporta piani
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Streamline da { $files }: spessore { $thickness } mm
mpr-tracts-off = Streamline da { $files }: nascoste
mpr-tracts-none = Nessuna streamline TrackVis o MRtrix accanto a questo volume
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } voxel
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } di { $count }
mpr-axis-channel = Canale
mpr-axis-unnamed = Asse { $axis }
mpr-one-channel = La MPR mostra un canale alla volta
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } canali a colori, ciascuno nel suo intervallo di visualizzazione
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Mostrato un solo canale: { $reason }
mpr-isotropic = Nessuna dimensione del voxel nel file: voxel mostrati come cubi
mpr-isotropic-z = Nessuna spaziatura delle sezioni nel file: sezioni mostrate a un pixel di distanza
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Caricamento di { $loaded } di { $count ->
        [one] { $count } sezione
       *[other] { $count } sezioni
    }
mpr-loaded = { $count ->
        [one] { $count } sezione
       *[other] { $count } sezioni
    }, { $width } × { $height }
mpr-downsampled = Mostrato a 1/{ $factor } della risoluzione (limiti di memoria o grafica)
mpr-converted = Valori mostrati come float a 32 bit
mpr-opening = Apertura del volume…
mpr-gpu-memory = { $size } sulla scheda grafica
mpr-refused = Impossibile creare un volume: { $reason }
mpr-failed = Il volume non è stato caricato: { $reason }
mpr-no-gpu = La scheda grafica non può contenere questo volume: { $reason }
mpr-planes-saved = { $count ->
        [one] { $count } piano salvato
       *[other] { $count } piani salvati
    } in { $folder }
mpr-planes-failed = Impossibile salvare i piani: { $reason }
mpr-view-a11y = Vista { $view }, { $position } mm
mpr-crosshair-a11y = Mirino a { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Mirino a { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Mirino a { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Mirino sul voxel { $x }, { $y }, { $z }
mpr-controls = Controlli MPR

# 3D view palette entries.
palette-mpr-preset = Preimpostazione 3D successiva (TC osso, tessuti molli, polmone, angio; RM)
palette-mpr-render = Rendering 3D successivo (ombreggiato, Cinematic, MIP, MinIP)
palette-mpr-clip = Taglio 3D successivo (nessuno, riquadro delle viste 2D, angolo rimosso)
palette-mpr-export-stl = Esporta la superficie 3D come STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Spessore delle streamline successivo in MPR
palette-mpr-maximise = Ingrandisci la vista MPR o mostra tutte le viste
palette-mpr-export-planes = Esporta i piani MPR come immagini…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] TC osso
    [soft] TC tessuti molli
    [lung] TC polmone
    [angio] Angio-TC
   *[mr] RM
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Ombreggiato
}
vr-clip = { $clip ->
    [box] Taglio: riquadro delle viste
    [cut] Taglio: angolo rimosso
   *[off] Nessun taglio
}
vr-smooth = Leviga la superficie
vr-export = Esporta STL
vr-threshold = Superficie a { $value }
vr-stl-exporting = Creazione della superficie…
vr-stl-saved = { $count ->
    [0] Niente a questa soglia: nessuna superficie salvata
   *[other] { $count } triangoli salvati in { $name }
}
vr-stl-failed = Impossibile salvare la superficie: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, superficie a { $threshold }. Maiusc+frecce per orbitare, più e meno per lo zoom.

## Volume grezzo: come è organizzato un file .raw / .vol senza intestazione

# machine-drafted, needs native review
raw-volume-title = Apri volume grezzo
raw-volume-file = { $file } non ha intestazione. Indica come sono disposti i campioni.
raw-volume-guess-label = Disposizione suggerita
raw-volume-guess = { $index } di { $count }
raw-volume-custom = Personalizzata
raw-volume-width = Larghezza (voxel)
raw-volume-height = Altezza (voxel)
raw-volume-slices = Sezioni
raw-volume-type = Tipo di campione
raw-volume-type-unsigned = Senza segno a { $bits } bit
raw-volume-type-signed = Con segno a { $bits } bit
raw-volume-type-float = Virgola mobile a { $bits } bit
raw-volume-order = Ordine dei byte
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Offset dell’intestazione
raw-volume-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } byte
}
raw-volume-pixel = Dimensione pixel
raw-volume-spacing = Spaziatura delle sezioni
raw-volume-mm = { $value } mm
raw-volume-exact = Riempie esattamente il file ({ $size })
raw-volume-fits = Usa { $size } di { $file }; ne avanzano { $left }
raw-volume-too-big = Servono { $size }, ma il file è di { $file }
raw-volume-cancel = Annulla
raw-volume-open = Apri
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Connettività di { $seed }; sposta il mirino su un altro seed
# machine-drafted, needs native review
mpr-measure-hint-edit = Trascina un estremo per spostarlo; sull’altro estremo per rimuovere la misura
# machine-drafted, needs native review
mpr-measure-removed = Misura rimossa
mpr-measure-lengths-only = La MPR misura solo lunghezze
mpr-more = Altro…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = MPR curvilinea lungo una curva cliccata
palette-mpr-curve-layout = MPR curvilinea: raddrizzata o stirata
palette-mpr-curve-save = Salva la curva…
palette-mpr-curve-load = Carica una curva…
palette-mpr-export-segments = Esporta ogni segmento come STL…
palette-mpr-dental = Layout dentale (panoramica e sezioni trasversali)
palette-mpr-nerve = Traccia il canale mandibolare
cpr-kind = { $kind ->
    [stretched] Stirata
   *[straightened] Raddrizzata
}
cpr-button = Curvilinea
cpr-controls = Controlli della MPR curvilinea
cpr-at = A { $position }
cpr-section = Sezione trasversale
cpr-hint-start = Fai clic su punti lungo la struttura nella vista { $view }
cpr-hint-more = Fai clic su un altro punto
cpr-add-points = Aggiungi punti
cpr-undo-point = Annulla punto
# machine-drafted, needs native review
cpr-mark = Segna qui
cpr-mark-set = Marcatore { $n } posizionato.
cpr-mark-removed = Marcatore rimosso.
cpr-mark-here = Marcatore { $n }
cpr-diameter = Diametro { $value }
cpr-diameters = Diametro da { $min } a { $max }
cpr-mark-span = Marcatori da { $from } a { $to }: { $length } lungo la linea centrale
cpr-mark-total = Tutti i marcatori: { $length }
cpr-clear = Cancella curva
cpr-save = Salva curva
cpr-load = Carica curva
cpr-done = Fine
cpr-a11y = { $name } lungo { $length }, cursore a { $position }. La rotella sposta il cursore.
cpr-none = Ancora nessuna curva: prima fai clic su dei punti
cpr-cleared = Curva cancellata
cpr-saved = Curva salvata: { $name }
cpr-save-failed = Impossibile salvare la curva: { $reason }
cpr-loaded = Curva caricata
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 linea centrale caricata
    [many] { $count } di linee centrali caricate
   *[other] { $count } linee centrali caricate
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] Trovata 1 linea centrale salvata
    [many] Trovate { $count } di linee centrali salvate
   *[other] Trovate { $count } linee centrali salvate
}
# A centreline without a name, by its place in the list.
cpr-centreline = Linea centrale { $number }
cpr-new = Nuova linea centrale
cpr-rename = Rinomina
cpr-name = Nome della linea centrale
cpr-delete = Elimina
# $name: the centreline's name.
cpr-delete-title = Eliminare la linea centrale “{ $name }”?
cpr-delete-detail = I suoi punti vengono rimossi. I file salvati sul disco restano.
cpr-deleted = “{ $name }” eliminata
cpr-load-failed = Impossibile caricare la curva: { $reason }
cpr-load-not-json = il file non è JSON
cpr-load-not-curves = il file non è una curva di Specular
cpr-load-empty = il file non contiene alcuna curva di due o più punti
cpr-panoramic = Panoramica
cpr-panoramic-slab = Panoramica, spessore { $thickness } mm
cpr-panoramic-thin = Panoramica, sottile

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Dentale
dental-not-for-diagnosis = Non destinato alla diagnosi né alla pianificazione del trattamento
dental-nerve = Canale mandibolare
dental-nerve-hint = Fai clic su punti lungo il canale mandibolare nella panoramica o in una sezione trasversale
dental-nerve-done = Tracciamento del canale mandibolare disattivato
dental-spacing = Ogni { $spacing } mm
dental-section = Sezione { $number }
dental-off = Apri prima il layout dentale
dental-suggesting = Ricerca dell’arcata dentale…
dental-suggested = Arcata proposta: trascina i suoi punti per regolarla
dental-no-arch = Nessuna arcata trovata: fai clic su punti lungo l’arcata nella vista assiale
dental-vendor = CBCT dentale { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Esporta segmenti
seg-stl-detail = Segmenti: { $count ->
        [one] { $count } triangolo
       *[other] { $count } triangoli
    }
seg-stl-detail-full = Segmenti: dettaglio completo
seg-stl-none = Nessun segmento da esportare
seg-stl-not-labels = questo volume non è una mappa di etichette
seg-stl-exporting = Creazione delle superfici dei segmenti…
seg-stl-saved = { $count ->
    [one] { $count } segmento salvato in { $folder }
    [many] { $count } segmenti salvati in { $folder }
   *[other] { $count } segmenti salvati in { $folder }
}
seg-stl-open = Non chiusi: { $count }.
seg-stl-empty = I segmenti sono vuoti: nulla è stato salvato
seg-stl-failed = Impossibile salvare i segmenti: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Tre piani (MPR)
toolbar-mpr-close = Chiudi tre piani (MPR)
settings-viewing-toolbar-items-mpr = Tre piani (MPR)
