# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Svenska.

keys-mode-drawing = Ritningar
cheatsheet-drawing = Ritningar
palette-drawing-layers = Visa eller dölj lagerpanelen
palette-drawing-all-layers = Visa alla lager, eller lagren som sparade
palette-drawing-paper = Växla mellan mörkt och ljust papper
palette-drawing-lineweights = Visa eller dölj linjetjocklekar
disabled-not-drawing = Endast för ritningar (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Modell
# machine-drafted, needs native review
cad-pen-name = Penna { $n }
cad-page-name = Sida { $n }
cad-layer-name = Lager { $n }
cad-map-name = Karta
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Lager
cad-board-top-name = Ovansida
cad-board-bottom-name = Undersida
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Ram
cad-trace-name = Kurva { $n }
cad-contours-positive-name = Positiva konturer
cad-contours-negative-name = Negativa konturer
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D-modell
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Lager · { $shown } av { $total } visas
# Toasts.
drawing-layers-all-toast = Alla lager visas
drawing-layers-saved-toast = Lager som sparade i ritningen
drawing-paper-dark-toast = Mörkt papper
drawing-paper-light-toast = Ljust papper
drawing-lineweights-on = Linjetjocklekar visas
drawing-lineweights-off = Linjetjocklekar av: tunna linjer

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Markera
palette-markup-help = Moln, text, pilar, stämplar, mått och fotonålar, sparade bredvid filen.
palette-compare-sheets = Jämför ritningsblad
palette-compare-sheets-help = Lägg en annan revision av det här bladet ovanpå, justerad, med ändringarna inringade.
markup-title = Markeringar
markup-list = Markeringar på den här sidan
markup-count = { $count ->
    [0] Inga markeringar på den här sidan
    [one] 1 markering på den här sidan
   *[other] { $count } markeringar på den här sidan
}
markup-empty = Välj ett verktyg och klicka på bladet.
markup-item = { $n }. { $kind }
markup-tool-select = Välj
markup-tool-cloud = Moln
markup-tool-text = Text
markup-tool-arrow = Pil
markup-tool-stamp = Stämpel
markup-tool-length = Längd
markup-tool-area = Area
markup-tool-pin = Fotonål
markup-hint-select = Klicka på ett objekt för att välja det. Dra för att flytta det. Enter redigerar texten.
markup-hint-cloud = Dra en rektangel, eller klicka på hörnen och tryck på Enter.
markup-hint-text = Klicka där texten ska stå.
markup-hint-arrow = Dra från etiketten till punkten, eller klicka på båda.
markup-hint-stamp = Klicka där stämpeln ska sitta.
markup-hint-length = Dra, eller klicka på båda ändarna.
markup-hint-area = Klicka på hörnen och tryck sedan på Enter.
markup-hint-pin = Klicka på en punkt och välj sedan ett foto.
markup-kind-cloud = Moln
markup-kind-text = Text
markup-kind-arrow = Pil
markup-kind-stamp = Stämpel
markup-kind-length = Längd
markup-kind-area = Area
markup-kind-pin = Foto
markup-colour = Färg: { $colour }
markup-colour-red = Röd
markup-colour-blue = Blå
markup-colour-green = Grön
markup-colour-orange = Orange
markup-colour-magenta = Magenta
markup-colour-black = Svart
markup-width = Linje: { $width }
markup-width-thin = Tunn
markup-width-medium = Mellan
markup-width-thick = Tjock
markup-stamp = Stämpel: { $words }
markup-stamp-approved = Godkänd
markup-stamp-rejected = Avvisad
markup-stamp-reviewed = Granskad
markup-stamp-custom = Egen text
markup-undo = Ångra
markup-redo = Gör om
markup-delete = Ta bort
markup-open-photo = Öppna foto
markup-from-measurements = Lägg till mätningar
markup-from-measurements-done = { $count ->
    [one] 1 mätning tillagd som markering
   *[other] { $count } mätningar tillagda som markeringar
}
markup-export-pdf = Exportera PDF…
markup-export-png = Exportera PNG…
markup-done = Klar
markup-ask-text = Text (Enter placerar, Esc avbryter)
markup-ask-label = Etikett (valfri; Enter placerar)
markup-ask-stamp = Stämpeltext (Enter placerar)
markup-ask-edit = Ny text (Enter sparar)
markup-added = Tillagt: { $kind }
markup-moved = Flyttat
markup-pin-hint = Klicka på nålen igen eller tryck på Enter för att öppna fotot.
markup-deleted = Borttaget
markup-undone = Ångrat
markup-redone = Gjort om
markup-delete-ask = Ta bort { $name }?
markup-delete-detail = Det tas bort ur markeringsfilen. Själva ritningen ändras inte.
markup-delete-ok = Ta bort
markup-photo-missing = { $name } finns inte längre.
markup-load-failed = Markeringar inte lästa: { $reason }
markup-save-failed = Markeringar inte sparade: { $reason }
markup-sidecar-too-large = markeringsfilen är för stor
markup-pdf-unreadable = PDF-filen kunde inte läsas
markup-exported = Sparade { $name }
markup-export-failed = Inte exporterat: { $reason }
markup-export-same-file = välj ett annat namn; originalet skrivs aldrig över
markup-export-encrypted = PDF-filen är krypterad. Exportera PNG i stället.
markup-export-damaged = PDF-filens struktur är skadad. Exportera PNG i stället.
sheets-title = Jämför ritningsblad
sheets-other = Annat blad…
sheets-other-named = Annat: { $name }
sheets-page = Sida { $page } av { $count }
sheets-auto = Justera
sheets-pick = Välj 2 punkter
sheets-overlay = Överlägg
sheets-difference = Skillnad
sheets-blend = Tona
sheets-blink = Blinka
sheets-slider = Annat blad { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Inga ändringar
    [one] Ringa in 1 ändring med moln
   *[other] Ringa in { $count } ändringar med moln
}
sheets-done = Klar
sheets-choose = Välj det andra bladet.
sheets-reading = Läser bladet…
sheets-aligning = Justerar…
sheets-aligned = Justerat: skala { $scale }, vridet { $angle }°, avvikelse inom { $error } px. { $changes ->
    [0] Inga ändringar hittades.
    [one] 1 ändring.
   *[other] { $changes } ändringar.
}
sheets-align-failed = Kunde inte justera automatiskt. Välj 2 punkter.
sheets-pick-this = Punkt { $n }: klicka på den på det här bladet.
sheets-pick-other = Punkt { $n }: klicka på samma punkt på det andra bladet.
sheets-pick-same = De två punkterna är samma. Välj 2 punkter igen.
sheets-read-failed = Bladet inte läst: { $reason }
sheets-failed = Det steget misslyckades. Försök igen eller välj ett annat blad.
sheets-legend-this = Bara det här bladet
sheets-legend-other = Bara det andra bladet
sheets-legend-both = Båda
sheets-cloud-text = Ändrat
sheets-clouds-added = { $count ->
    [0] Inga moln tillagda
    [one] 1 moln tillagt i markeringarna
   *[other] { $count } moln tillagda i markeringarna
}
sheets-opened = Jämför ritningsblad
sheets-closed = Jämförelsen stängd
markup-ask-new-line = Shift+Enter: ny rad

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Ritningar
settings-drawings-shx-folders = Mappar med SHX-typsnitt
settings-drawings-shx-folders-help = Mappar med SHX-typsnitt för text i DXF och DWG, avgränsade med ;. Ritningens egen mapp genomsöks först.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Inte en mapp: { $folder }
