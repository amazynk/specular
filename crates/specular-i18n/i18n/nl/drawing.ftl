# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Nederlands.

keys-mode-drawing = Tekeningen
cheatsheet-drawing = Tekeningen
palette-drawing-layers = Lagenpaneel tonen of verbergen
palette-drawing-all-layers = Alle lagen tonen, of de lagen zoals opgeslagen
palette-drawing-paper = Donker en licht papier wisselen
palette-drawing-lineweights = Lijndiktes tonen of verbergen
disabled-not-drawing = Alleen voor tekeningen (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Model
# machine-drafted, needs native review
cad-pen-name = Pen { $n }
cad-page-name = Pagina { $n }
cad-layer-name = Laag { $n }
cad-map-name = Kaart
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Lagen
cad-board-top-name = Bovenkant
cad-board-bottom-name = Onderkant
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Kader
cad-trace-name = Curve { $n }
cad-contours-positive-name = Positieve contouren
cad-contours-negative-name = Negatieve contouren
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D-model
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Lagen · { $shown } van { $total } zichtbaar
# Toasts.
drawing-layers-all-toast = Alle lagen zichtbaar
drawing-layers-saved-toast = Lagen zoals opgeslagen in de tekening
drawing-paper-dark-toast = Donker papier
drawing-paper-light-toast = Licht papier
drawing-lineweights-on = Lijndiktes zichtbaar
drawing-lineweights-off = Lijndiktes uit: dunne lijnen

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Markeren
palette-markup-help = Wolken, tekst, pijlen, stempels, maten en fotopins, opgeslagen naast het bestand.
palette-compare-sheets = Bladen vergelijken
palette-compare-sheets-help = Een andere revisie van dit blad uitgelijnd eroverheen leggen, met de wijzigingen omlijnd.
markup-title = Markeringen
markup-list = Markeringen op deze pagina
markup-count = { $count ->
    [0] Geen markeringen op deze pagina
    [one] 1 markering op deze pagina
   *[other] { $count } markeringen op deze pagina
}
markup-empty = Kies een gereedschap en klik op het blad.
markup-item = { $n }. { $kind }
markup-tool-select = Selecteren
markup-tool-cloud = Wolk
markup-tool-text = Tekst
markup-tool-arrow = Pijl
markup-tool-stamp = Stempel
markup-tool-length = Lengte
markup-tool-area = Oppervlakte
markup-tool-pin = Fotopin
markup-hint-select = Klik op een item om het te selecteren. Sleep om het te verplaatsen. Enter bewerkt de tekst.
markup-hint-cloud = Sleep een rechthoek, of klik op de hoeken en druk op Enter.
markup-hint-text = Klik waar de tekst moet komen.
markup-hint-arrow = Sleep van het label naar het punt, of klik op beide.
markup-hint-stamp = Klik waar de stempel moet komen.
markup-hint-length = Sleep, of klik op beide uiteinden.
markup-hint-area = Klik op de hoeken en druk dan op Enter.
markup-hint-pin = Klik op een punt en kies dan een foto.
markup-kind-cloud = Wolk
markup-kind-text = Tekst
markup-kind-arrow = Pijl
markup-kind-stamp = Stempel
markup-kind-length = Lengte
markup-kind-area = Oppervlakte
markup-kind-pin = Foto
markup-colour = Kleur: { $colour }
markup-colour-red = Rood
markup-colour-blue = Blauw
markup-colour-green = Groen
markup-colour-orange = Oranje
markup-colour-magenta = Magenta
markup-colour-black = Zwart
markup-width = Lijn: { $width }
markup-width-thin = Dun
markup-width-medium = Middel
markup-width-thick = Dik
markup-stamp = Stempel: { $words }
markup-stamp-approved = Goedgekeurd
markup-stamp-rejected = Afgekeurd
markup-stamp-reviewed = Gecontroleerd
markup-stamp-custom = Eigen tekst
markup-undo = Ongedaan maken
markup-redo = Opnieuw
markup-delete = Verwijderen
markup-open-photo = Foto openen
markup-from-measurements = Metingen toevoegen
markup-from-measurements-done = { $count ->
    [one] 1 meting als markering toegevoegd
   *[other] { $count } metingen als markeringen toegevoegd
}
markup-export-pdf = Exporteren als PDF…
markup-export-png = Exporteren als PNG…
markup-done = Klaar
markup-ask-text = Tekst (Enter om te plaatsen, Esc om te annuleren)
markup-ask-label = Label (optioneel; Enter om te plaatsen)
markup-ask-stamp = Stempeltekst (Enter om te plaatsen)
markup-ask-edit = Nieuwe tekst (Enter om op te slaan)
markup-added = Toegevoegd: { $kind }
markup-moved = Verplaatst
markup-pin-hint = Klik nogmaals op de pin of druk op Enter om de foto te openen.
markup-deleted = Verwijderd
markup-undone = Ongedaan gemaakt
markup-redone = Opnieuw uitgevoerd
markup-delete-ask = { $name } verwijderen?
markup-delete-detail = Het wordt uit het markeringsbestand verwijderd. De tekening zelf blijft ongewijzigd.
markup-delete-ok = Verwijderen
markup-photo-missing = { $name } is er niet meer.
markup-load-failed = Markeringen niet gelezen: { $reason }
markup-save-failed = Markeringen niet opgeslagen: { $reason }
markup-sidecar-too-large = het markeringsbestand is te groot
markup-pdf-unreadable = de pdf kon niet worden gelezen
markup-exported = { $name } opgeslagen
markup-export-failed = Niet geëxporteerd: { $reason }
markup-export-same-file = kies een andere naam; het origineel wordt nooit overschreven
markup-export-encrypted = de pdf is versleuteld. Exporteer in plaats daarvan als PNG.
markup-export-damaged = de structuur van de pdf is beschadigd. Exporteer in plaats daarvan als PNG.
sheets-title = Bladen vergelijken
sheets-other = Ander blad…
sheets-other-named = Ander: { $name }
sheets-page = Pagina { $page } van { $count }
sheets-auto = Uitlijnen
sheets-pick = 2 punten kiezen
sheets-overlay = Overlay
sheets-difference = Verschil
sheets-blend = Overvloeien
sheets-blink = Knipperen
sheets-slider = Ander blad { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Geen wijzigingen
    [one] 1 wijziging omwolken
   *[other] { $count } wijzigingen omwolken
}
sheets-done = Klaar
sheets-choose = Kies het andere blad.
sheets-reading = Blad lezen…
sheets-aligning = Uitlijnen…
sheets-aligned = Uitgelijnd: schaal { $scale }, gedraaid { $angle }°, passend binnen { $error } px. { $changes ->
    [0] Geen wijzigingen gevonden.
    [one] 1 wijziging.
   *[other] { $changes } wijzigingen.
}
sheets-align-failed = Automatisch uitlijnen lukte niet. Kies 2 punten.
sheets-pick-this = Punt { $n }: klik erop op dit blad.
sheets-pick-other = Punt { $n }: klik op hetzelfde punt op het andere blad.
sheets-pick-same = De twee punten zijn gelijk. Kies opnieuw 2 punten.
sheets-read-failed = Blad niet gelezen: { $reason }
sheets-failed = Die stap is mislukt. Probeer het opnieuw of kies een ander blad.
sheets-legend-this = Alleen dit blad
sheets-legend-other = Alleen ander blad
sheets-legend-both = Beide
sheets-cloud-text = Gewijzigd
sheets-clouds-added = { $count ->
    [0] Geen wolken toegevoegd
    [one] 1 wolk aan de markeringen toegevoegd
   *[other] { $count } wolken aan de markeringen toegevoegd
}
sheets-opened = Bladen vergelijken
sheets-closed = Vergelijking gesloten
markup-ask-new-line = Shift+Enter: nieuwe regel

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Tekeningen
settings-drawings-shx-folders = Mappen met SHX-lettertypen
settings-drawings-shx-folders-help = Mappen met SHX-lettertypen voor tekst in DXF en DWG, gescheiden door ;. De map van de tekening zelf wordt eerst doorzocht.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Geen map: { $folder }
