# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Dansk.

keys-mode-drawing = Tegninger
cheatsheet-drawing = Tegninger
palette-drawing-layers = Vis eller skjul lagpanelet
palette-drawing-all-layers = Vis alle lag, eller lagene som gemt
palette-drawing-paper = Byt mørkt og lyst papir
palette-drawing-lineweights = Vis eller skjul linjetykkelser
disabled-not-drawing = Kun for tegninger (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Model
# machine-drafted, needs native review
cad-pen-name = Pen { $n }
cad-page-name = Side { $n }
cad-layer-name = Lag { $n }
cad-map-name = Kort
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Lag
cad-board-top-name = Overside
cad-board-bottom-name = Underside
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Ramme
cad-trace-name = Kurve { $n }
cad-contours-positive-name = Positive konturer
cad-contours-negative-name = Negative konturer
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D-model
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Lag · { $shown } af { $total } vist
# Toasts.
drawing-layers-all-toast = Alle lag vist
drawing-layers-saved-toast = Lag som gemt i tegningen
drawing-paper-dark-toast = Mørkt papir
drawing-paper-light-toast = Lyst papir
drawing-lineweights-on = Linjetykkelser vist
drawing-lineweights-off = Linjetykkelser fra: tynde linjer

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Markér
palette-markup-help = Skyer, tekst, pile, stempler, mål og fotonåle, gemt ved siden af filen.
palette-compare-sheets = Sammenlign tegningsark
palette-compare-sheets-help = Læg en anden revision af dette ark ovenpå, justeret, med ændringerne markeret.
markup-title = Markeringer
markup-list = Markeringer på denne side
markup-count = { $count ->
    [0] Ingen markeringer på denne side
    [one] 1 markering på denne side
   *[other] { $count } markeringer på denne side
}
markup-empty = Vælg et værktøj, og klik på arket.
markup-item = { $n }. { $kind }
markup-tool-select = Vælg
markup-tool-cloud = Sky
markup-tool-text = Tekst
markup-tool-arrow = Pil
markup-tool-stamp = Stempel
markup-tool-length = Længde
markup-tool-area = Areal
markup-tool-pin = Fotonål
markup-hint-select = Klik på et element for at vælge det. Træk for at flytte det. Enter redigerer teksten.
markup-hint-cloud = Træk et rektangel, eller klik på hjørnerne, og tryk på Enter.
markup-hint-text = Klik, hvor teksten skal stå.
markup-hint-arrow = Træk fra etiketten til punktet, eller klik på begge.
markup-hint-stamp = Klik, hvor stemplet skal sidde.
markup-hint-length = Træk, eller klik på begge ender.
markup-hint-area = Klik på hjørnerne, og tryk derefter på Enter.
markup-hint-pin = Klik på et punkt, og vælg derefter et foto.
markup-kind-cloud = Sky
markup-kind-text = Tekst
markup-kind-arrow = Pil
markup-kind-stamp = Stempel
markup-kind-length = Længde
markup-kind-area = Areal
markup-kind-pin = Foto
markup-colour = Farve: { $colour }
markup-colour-red = Rød
markup-colour-blue = Blå
markup-colour-green = Grøn
markup-colour-orange = Orange
markup-colour-magenta = Magenta
markup-colour-black = Sort
markup-width = Linje: { $width }
markup-width-thin = Tynd
markup-width-medium = Mellem
markup-width-thick = Tyk
markup-stamp = Stempel: { $words }
markup-stamp-approved = Godkendt
markup-stamp-rejected = Afvist
markup-stamp-reviewed = Gennemgået
markup-stamp-custom = Egen tekst
markup-undo = Fortryd
markup-redo = Annuller fortryd
markup-delete = Slet
markup-open-photo = Åbn foto
markup-from-measurements = Tilføj målinger
markup-from-measurements-done = { $count ->
    [one] 1 måling tilføjet som markering
   *[other] { $count } målinger tilføjet som markeringer
}
markup-export-pdf = Eksportér PDF…
markup-export-png = Eksportér PNG…
markup-done = Færdig
markup-ask-text = Tekst (Enter placerer, Esc annullerer)
markup-ask-label = Etiket (valgfri; Enter placerer)
markup-ask-stamp = Stempeltekst (Enter placerer)
markup-ask-edit = Ny tekst (Enter gemmer)
markup-added = Tilføjet: { $kind }
markup-moved = Flyttet
markup-pin-hint = Klik på nålen igen, eller tryk på Enter for at åbne fotoet.
markup-deleted = Slettet
markup-undone = Fortrudt
markup-redone = Gendannet
markup-delete-ask = Slet { $name }?
markup-delete-detail = Det fjernes fra markeringsfilen. Selve tegningen ændres ikke.
markup-delete-ok = Slet
markup-photo-missing = { $name } findes ikke længere.
markup-load-failed = Markeringer ikke læst: { $reason }
markup-save-failed = Markeringer ikke gemt: { $reason }
markup-sidecar-too-large = markeringsfilen er for stor
markup-pdf-unreadable = PDF-filen kunne ikke læses
markup-exported = Gemte { $name }
markup-export-failed = Ikke eksporteret: { $reason }
markup-export-same-file = vælg et andet navn; originalen overskrives aldrig
markup-export-encrypted = PDF-filen er krypteret. Eksportér PNG i stedet.
markup-export-damaged = PDF-filens struktur er beskadiget. Eksportér PNG i stedet.
sheets-title = Sammenlign tegningsark
sheets-other = Andet ark…
sheets-other-named = Andet: { $name }
sheets-page = Side { $page } af { $count }
sheets-auto = Justér
sheets-pick = Vælg 2 punkter
sheets-overlay = Overlejring
sheets-difference = Forskel
sheets-blend = Overtoning
sheets-blink = Blink
sheets-slider = Andet ark { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Ingen ændringer
    [one] Markér 1 ændring med sky
   *[other] Markér { $count } ændringer med skyer
}
sheets-done = Færdig
sheets-choose = Vælg det andet ark.
sheets-reading = Læser arket…
sheets-aligning = Justerer…
sheets-aligned = Justeret: skala { $scale }, drejet { $angle }°, afvigelse inden for { $error } px. { $changes ->
    [0] Ingen ændringer fundet.
    [one] 1 ændring.
   *[other] { $changes } ændringer.
}
sheets-align-failed = Kunne ikke justere automatisk. Vælg 2 punkter.
sheets-pick-this = Punkt { $n }: klik på det på dette ark.
sheets-pick-other = Punkt { $n }: klik på samme punkt på det andet ark.
sheets-pick-same = De to punkter er ens. Vælg 2 punkter igen.
sheets-read-failed = Ark ikke læst: { $reason }
sheets-failed = Det trin mislykkedes. Prøv igen, eller vælg et andet ark.
sheets-legend-this = Kun dette ark
sheets-legend-other = Kun det andet ark
sheets-legend-both = Begge
sheets-cloud-text = Ændret
sheets-clouds-added = { $count ->
    [0] Ingen skyer tilføjet
    [one] 1 sky tilføjet til markeringerne
   *[other] { $count } skyer tilføjet til markeringerne
}
sheets-opened = Sammenlign tegningsark
sheets-closed = Sammenligning lukket
markup-ask-new-line = Shift+Enter: ny linje

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Tegninger
settings-drawings-shx-folders = Mapper med SHX-skrifttyper
settings-drawings-shx-folders-help = Mapper med SHX-skrifttyper til tekst i DXF og DWG, adskilt af ;. Tegningens egen mappe gennemsøges først.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Ikke en mappe: { $folder }
