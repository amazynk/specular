# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Italiano.

keys-mode-drawing = Disegni
cheatsheet-drawing = Disegni
palette-drawing-layers = Mostra o nascondi il pannello dei layer
palette-drawing-all-layers = Mostra tutti i layer, o i layer come salvati
palette-drawing-paper = Scambia carta scura e chiara
palette-drawing-lineweights = Mostra o nascondi gli spessori di linea
disabled-not-drawing = Solo per i disegni (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Modello
# machine-drafted, needs native review
cad-pen-name = Penna { $n }
cad-page-name = Pagina { $n }
cad-layer-name = Layer { $n }
cad-map-name = Mappa
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Layer
cad-board-top-name = Lato superiore
cad-board-bottom-name = Lato inferiore
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Cornice
cad-trace-name = Traccia { $n }
cad-contours-positive-name = Contorni positivi
cad-contours-negative-name = Contorni negativi
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = Modello 3D
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Layer · { $shown } di { $total } visibili
# Toasts.
drawing-layers-all-toast = Tutti i layer visibili
drawing-layers-saved-toast = Layer come salvati nel disegno
drawing-paper-dark-toast = Carta scura
drawing-paper-light-toast = Carta chiara
drawing-lineweights-on = Spessori di linea visibili
drawing-lineweights-off = Spessori di linea disattivati: linee sottili

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Annota
palette-markup-help = Nuvole, testo, frecce, timbri, misure e segnaposto foto, salvati accanto al file.
palette-compare-sheets = Confronta tavole
palette-compare-sheets-help = Sovrapponi un’altra revisione di questa tavola, allineata, con le modifiche evidenziate.
markup-title = Annotazioni
markup-list = Annotazioni in questa pagina
markup-count = { $count ->
    [0] Nessuna annotazione in questa pagina
    [one] 1 annotazione in questa pagina
   *[other] { $count } annotazioni in questa pagina
}
markup-empty = Scegli uno strumento e fai clic sulla tavola.
markup-item = { $n }. { $kind }
markup-tool-select = Seleziona
markup-tool-cloud = Nuvola
markup-tool-text = Testo
markup-tool-arrow = Freccia
markup-tool-stamp = Timbro
markup-tool-length = Lunghezza
markup-tool-area = Area
markup-tool-pin = Segnaposto foto
markup-hint-select = Fai clic su un elemento per selezionarlo. Trascinalo per spostarlo. Enter ne modifica il testo.
markup-hint-cloud = Trascina un rettangolo, oppure fai clic sugli angoli e premi Enter.
markup-hint-text = Fai clic dove va il testo.
markup-hint-arrow = Trascina dall’etichetta al punto, oppure fai clic su entrambi.
markup-hint-stamp = Fai clic dove va il timbro.
markup-hint-length = Trascina, oppure fai clic su entrambe le estremità.
markup-hint-area = Fai clic sugli angoli, poi premi Enter.
markup-hint-pin = Fai clic su un punto, poi scegli una foto.
markup-kind-cloud = Nuvola
markup-kind-text = Testo
markup-kind-arrow = Freccia
markup-kind-stamp = Timbro
markup-kind-length = Lunghezza
markup-kind-area = Area
markup-kind-pin = Foto
markup-colour = Colore: { $colour }
markup-colour-red = Rosso
markup-colour-blue = Blu
markup-colour-green = Verde
markup-colour-orange = Arancione
markup-colour-magenta = Magenta
markup-colour-black = Nero
markup-width = Linea: { $width }
markup-width-thin = Sottile
markup-width-medium = Media
markup-width-thick = Spessa
markup-stamp = Timbro: { $words }
markup-stamp-approved = Approvato
markup-stamp-rejected = Respinto
markup-stamp-reviewed = Verificato
markup-stamp-custom = Testo libero
markup-undo = Annulla
markup-redo = Ripeti
markup-delete = Elimina
markup-open-photo = Apri foto
markup-from-measurements = Aggiungi misure
markup-from-measurements-done = { $count ->
    [one] 1 misura aggiunta come annotazione
   *[other] { $count } misure aggiunte come annotazioni
}
markup-export-pdf = Esporta PDF…
markup-export-png = Esporta PNG…
markup-done = Fine
markup-ask-text = Testo (Enter per posizionare, Esc per annullare)
markup-ask-label = Etichetta (facoltativa; Enter per posizionare)
markup-ask-stamp = Testo del timbro (Enter per posizionare)
markup-ask-edit = Nuovo testo (Enter per salvare)
markup-added = Aggiunto: { $kind }
markup-moved = Spostato
markup-pin-hint = Fai di nuovo clic sul segnaposto o premi Enter per aprirne la foto.
markup-deleted = Eliminato
markup-undone = Annullato
markup-redone = Ripetuto
markup-delete-ask = Eliminare { $name }?
markup-delete-detail = Viene rimosso dal file delle annotazioni. Il disegno stesso non viene modificato.
markup-delete-ok = Elimina
markup-photo-missing = { $name } non c’è più.
markup-load-failed = Annotazioni non lette: { $reason }
markup-save-failed = Annotazioni non salvate: { $reason }
markup-sidecar-too-large = il file delle annotazioni è troppo grande
markup-pdf-unreadable = impossibile leggere il PDF
markup-exported = Salvato { $name }
markup-export-failed = Non esportato: { $reason }
markup-export-same-file = scegli un altro nome; l’originale non viene mai sovrascritto
markup-export-encrypted = il PDF è cifrato. Esporta invece in PNG.
markup-export-damaged = la struttura del PDF è danneggiata. Esporta invece in PNG.
sheets-title = Confronta tavole
sheets-other = Altra tavola…
sheets-other-named = Altra: { $name }
sheets-page = Pagina { $page } di { $count }
sheets-auto = Allinea
sheets-pick = Scegli 2 punti
sheets-overlay = Sovrapposizione
sheets-difference = Differenza
sheets-blend = Dissolvenza
sheets-blink = Lampeggio
sheets-slider = Altra tavola { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Nessuna modifica
    [one] Racchiudi 1 modifica in una nuvola
   *[other] Racchiudi { $count } modifiche in nuvole
}
sheets-done = Fine
sheets-choose = Scegli l’altra tavola.
sheets-reading = Lettura della tavola…
sheets-aligning = Allineamento…
sheets-aligned = Allineata: scala { $scale }, rotazione { $angle }°, scarto entro { $error } px. { $changes ->
    [0] Nessuna modifica trovata.
    [one] 1 modifica.
   *[other] { $changes } modifiche.
}
sheets-align-failed = Impossibile allineare automaticamente. Scegli 2 punti.
sheets-pick-this = Punto { $n }: fai clic su di esso in questa tavola.
sheets-pick-other = Punto { $n }: fai clic sullo stesso punto nell’altra tavola.
sheets-pick-same = I due punti coincidono. Scegli di nuovo 2 punti.
sheets-read-failed = Tavola non letta: { $reason }
sheets-failed = Il passaggio non è riuscito. Riprova o scegli un’altra tavola.
sheets-legend-this = Solo questa tavola
sheets-legend-other = Solo l’altra tavola
sheets-legend-both = Entrambe
sheets-cloud-text = Modificato
sheets-clouds-added = { $count ->
    [0] Nessuna nuvola aggiunta
    [one] 1 nuvola aggiunta alle annotazioni
   *[other] { $count } nuvole aggiunte alle annotazioni
}
sheets-opened = Confronta tavole
sheets-closed = Confronto chiuso
markup-ask-new-line = Shift+Enter: nuova riga

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Disegni
settings-drawings-shx-folders = Cartelle dei font SHX
settings-drawings-shx-folders-help = Cartelle con font SHX per il testo di DXF e DWG, separate da ;. La cartella del disegno stesso viene cercata per prima.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Non è una cartella: { $folder }
