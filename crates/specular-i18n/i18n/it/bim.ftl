# machine-drafted, needs native review
## Revisione di modelli 3D: cammina e vola, piani di sezione e box di sezione, viste salvate,
## topic BCF (modalità 3D). I tasti citati sono quelli predefiniti.

cheatsheet-review = Revisione

palette-mesh-walk = Cammina
palette-mesh-fly = Vola
palette-mesh-views = Viste salvate
palette-mesh-save-view = Salva la vista
palette-mesh-section-plane = Aggiungi un piano di sezione
palette-mesh-section-box = Box di sezione
palette-mesh-section-flip = Inverti il piano di sezione
palette-mesh-section-clear = Rimuovi tutti i tagli
palette-mesh-measure-cut = Misura sul taglio
palette-mesh-bcf-topics = Topic BCF
# machine-drafted, needs native review
palette-mesh-model-checks = Verifiche del modello
# machine-drafted, needs native review
palette-mesh-models = Modelli nella scena
# machine-drafted, needs native review
palette-mesh-add-model = Aggiungi modello…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Aggiungi modelli da una cartella…

# Camminare e volare: avvisi e righe di lettura, secondo $what. $eye: altezza dell’occhio dal
# pavimento, con l’unità (solo walk-line).
mesh-nav = { $what ->
    [walk-toast] Camminata: W A S D per muoversi, trascina per guardare, Esc per fermarsi
    [fly-toast] Volo: W A S D per muoversi, Q E giù e su, Esc per fermarsi
    [orbit] Orbita
    [walk-line] Camminata · occhio a { $eye } dal pavimento
    [walk-keys] W A S D muovi · trascina guarda · Shift più veloce · Esc ferma
    [preparing] Pareti non ancora pronte: le attraversi
    [fly-keys] W A S D muovi · Q E giù, su · trascina guarda · Shift più veloce · Esc ferma
   *[fly-line] Volo
}

# Testi brevi di tagli, misure e viste salvate, secondo $what.
bim-note = { $what ->
    [plane-added] Piano di sezione aggiunto: trascina la maniglia per spostarlo
    [box-on] Box di sezione attivo: trascina la maniglia di una faccia per spostarla
    [box-off] Box di sezione disattivato
    [cleared] Tagli rimossi
    [measure-on] Fai clic su due punti del taglio
    [measure-needs-cut] Aggiungi prima un piano di sezione o il box
    [measure-off-cut] Fai clic sul taglio
    [measure-hint] Misura: fai clic su due punti del taglio
    [ask-view-name] Nome della vista
    [ask-rename] Nuovo nome
    [views-empty] Nessuna vista salvata. Premi N per salvare questa.
    [views-hints] Enter vai · N salva · F2 rinomina · Delete rimuovi · P salva come PNG
    [delete-detail] Viene rimossa solo la vista salvata. Il modello non cambia.
    [delete-ok] Rimuovi
    [not-over-model] Scegli un altro file: questo sostituirebbe il modello.
   *[view-name] Vista
}

# Testi brevi BCF (BIM Collaboration Format), secondo $what.
bcf-note = { $what ->
    [title-none] Topic BCF
    [new-file] nuovo
    [empty] Nessun topic. Premi O per aprire un file BCF o N per aggiungere un topic.
    [hints] Enter vai · ← → punti di vista · N topic · C commento · S stato · E titolo · O apri · Ctrl+S esporta
    [ask-title] Titolo del topic
    [ask-comment] Commento
    [untitled] Senza titolo
    [topic-added] Topic aggiunto con questa vista
   *[comment-added] Commento aggiunto con questa vista
}

# $planes: quanti piani di sezione.
bim-sections-line = { $planes ->
    [0] Nessun piano di sezione
    [one] 1 piano di sezione
   *[other] { $planes } piani di sezione
}
# $size: le dimensioni del box di sezione, con l’unità.
bim-box-line = box di sezione { $size }
# $count: il numero massimo di piani.
bim-planes-full = Al massimo { $count } piani di sezione
# Una maniglia di trascinamento, per gli screen reader. $n: il suo numero.
bim-handle = { $kind ->
    [box] Faccia { $n } del box di sezione
   *[plane] Piano di sezione { $n }
}
# $length: la lunghezza con l’unità.
bim-measure-line = Lunghezza sul taglio: { $length }
bim-views-title = { $count ->
    [one] 1 vista salvata
   *[other] { $count } viste salvate
}
# $name: il nome di una vista.
bim-view-saved = «{ $name }» salvata
bim-view-delete-title = Rimuovere la vista «{ $name }»?
# $file: un file scritto.
bim-saved-file = { $file } salvato
# $reason: perché non è riuscito.
bim-failed = { $what ->
    [open] Impossibile aprire il file BCF: { $reason }
    [views] Viste tenute solo per questa sessione: { $reason }
   *[save] Impossibile salvare: { $reason }
}
# $count: i topic; $file: il nome del file dell’archivio.
bcf-title = { $count ->
    [one] 1 topic
   *[other] { $count } topic
} · { $file }
bcf-comments = { $count ->
    [one] 1 commento
   *[other] { $count } commenti
}
# $at: il punto di vista mostrato; $count: i punti di vista del topic.
bcf-viewpoint-of = Punto di vista { $at } di { $count }
bcf-opened = { $count ->
    [one] 1 topic aperto
   *[other] { $count } topic aperti
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modelli: 1
   *[other] Modelli: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Questo file non è un modello IFC.
    [not-ifc-open] Apri prima un modello IFC. Si possono combinare solo modelli IFC.
    [this-model] Questo modello
    [hints] A aggiungi modello · Shift+A aggiungi cartella · Space mostra o nascondi · C colore per modello · Enter vai · Delete rimuovi
    [origin-ok] Posiziona nell’origine
    [first-stays] Il modello aperto resta. Apri un altro file per cambiarlo.
    [remove-detail] Esce dalla scena. Il file sul disco resta.
   *[remove-ok] Rimuovi
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 elemento
   *[other] { $elements } elementi
} · { $shown ->
    [hidden] nascosto
   *[shown] visibile
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] modello aperto
    [map-crs] per coordinate cartografiche ({ $crs })
    [map] per coordinate cartografiche
    [site] per latitudine e longitudine del sito (approssimativo)
    [project] per coordinate di progetto condivise
   *[origin] nell’origine (non allineato)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] I modelli usano sistemi di coordinate diversi: { $first } e { $this }.
    [only-first] Il modello aperto è georeferenziato, questo no.
    [only-this] Questo modello è georeferenziato, quello aperto no.
   *[broken] La conversione cartografica di questo modello non è utilizzabile (la sua scala è zero).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Spostato di { $distance }, ruotato di { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Lettura di 1 modello…
   *[other] Lettura di { $count } modelli…
}
# machine-drafted, needs native review
fed-ask-origin = Impossibile allineare { $file }. Posizionarlo nell’origine?
# machine-drafted, needs native review
fed-no-files = Nessun file IFC in { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } non è stato aggiunto: { $reason }
# machine-drafted, needs native review
fed-added = { $file } aggiunto. Modelli nella scena: { $count }.
# machine-drafted, needs native review
fed-remove-title = Rimuovere { $file } dalla scena?
# machine-drafted, needs native review
fed-removed = { $file } rimosso dalla scena.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] nascosto
   *[shown] visibile
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Colore per modello attivo
   *[off] Colore per modello disattivato
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI e viste salvate
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 annotazione
   *[other] { $count } annotazioni
}
# machine-drafted, needs native review
pmi-view = Vista salvata
# machine-drafted, needs native review
pmi-view-all = mostra tutte le annotazioni
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] mostra 1 annotazione
   *[other] mostra { $count } annotazioni
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Vista «{ $name }» mostrata. A mostra tutto.
# machine-drafted, needs native review
pmi-has-faces = facce evidenziate
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Nessuna PMI in questo modello.
    [hints] ↑ ↓ scegli · Enter vai · A tutto · D come disegnato · Shift+A nascondi
    [hidden] Annotazioni nascoste
    [shown] Annotazioni visibili
    [as-drawn] Annotazioni come le disegna il file
    [as-labels] Annotazioni come etichette
   *[all-shown] Tutte le annotazioni visibili
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Quota
    [tolerance] Tolleranza geometrica
    [datum] Riferimento
    [target] Riferimento parziale
   *[graphic] Annotazione disegnata
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Distanza
    [angular] Angolo
    [diameter] Diametro
    [radius] Raggio
    [length] Lunghezza
    [thickness] Spessore
   *[other] Quota
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Rettilineità
    [flatness] Planarità
    [circularity] Circolarità
    [cylindricity] Cilindricità
    [line-profile] Forma di una linea
    [surface-profile] Forma di una superficie
    [angularity] Inclinazione
    [perpendicularity] Perpendicolarità
    [parallelism] Parallelismo
    [position] Localizzazione
    [concentricity] Concentricità
    [coaxiality] Coassialità
    [symmetry] Simmetria
    [circular-runout] Oscillazione circolare
   *[total-runout] Oscillazione totale
}
