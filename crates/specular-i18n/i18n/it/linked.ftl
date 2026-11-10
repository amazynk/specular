# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Confronta serie (viste collegate)
# machine-drafted, needs native review
palette-compare-layout = Disposizione viste successiva
# machine-drafted, needs native review
palette-next-viewport = Vista successiva
# machine-drafted, needs native review
palette-prev-viewport = Vista precedente
# machine-drafted, needs native review
palette-reference-lines = Mostra o nascondi le linee di riferimento
# machine-drafted, needs native review
palette-sync-window = Sincro finestra successiva
# machine-drafted, needs native review
palette-sync-zoom = Zoom e spostamento insieme per dimensione
# machine-drafted, needs native review
palette-link-series = Prossimo collegamento serie
# machine-drafted, needs native review
palette-align-series = Allinea qui le altre serie
# machine-drafted, needs native review
palette-compare-series-help = Mostra più serie affiancate, che scorrono insieme.
# machine-drafted, needs native review
palette-compare-layout-help = Mostra più o meno viste.
# machine-drafted, needs native review
palette-next-viewport-help = Rendi attiva la vista successiva.
# machine-drafted, needs native review
palette-prev-viewport-help = Rendi attiva la vista precedente.
# machine-drafted, needs native review
palette-reference-lines-help = Disegna dove la sezione attiva interseca le altre viste.
# machine-drafted, needs native review
palette-sync-window-help = Scegli quali viste condividono una modifica di finestra/livello: off, stessa modalità o tutte.
# machine-drafted, needs native review
palette-sync-zoom-help = Mostra tutte le viste alla stessa dimensione e spostale insieme.
# machine-drafted, needs native review
palette-link-series-help = Scegli come questa serie segue le altre.
# machine-drafted, needs native review
palette-align-series-help = Allinea una serie precedente a questa nelle sezioni mostrate.
# machine-drafted, needs native review
disabled-not-comparing = Prima confronta le serie

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Disposizione: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Linee di riferimento
# machine-drafted, needs native review
linked-zoom = Zoom sincronizzato
# machine-drafted, needs native review
linked-window-sync = Finestra sincronizzata: { $mode ->
    [off] off
    [modality] stessa modalità
   *[all] tutte
}
# machine-drafted, needs native review
linked-link = Collegamento: { $mode ->
    [frame] sistema di riferimento
    [position] posizione del paziente
   *[off] off
}
# machine-drafted, needs native review
linked-align = Allinea qui

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Vista { $n } di { $count }, attiva
   *[no] Vista { $n } di { $count }
}
# machine-drafted, needs native review
linked-empty = Trascina qui una serie
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Sezione { $n } di { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = In un intervallo: sezione più vicina a { $distance } mm
# machine-drafted, needs native review
linked-outside = Fuori da questa serie ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Non collegata
# machine-drafted, needs native review
linked-aligned = Allineata a mano ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = Cursore 3D in { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } immagine
       *[other] { $count } immagini
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Una sola serie qui: trascinane un’altra dal browser degli studi
   *[other] Confronto di { $count } serie
}
# machine-drafted, needs native review
linked-no-geometry = Questa serie non ha posizioni delle sezioni: non può essere collegata
# machine-drafted, needs native review
linked-active = Vista { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Niente da allineare: le altre serie condividono questo sistema di riferimento
    [one] 1 serie allineata a quella attiva, per questa sessione
   *[other] { $count } serie allineate a quella attiva, per questa sessione
}
