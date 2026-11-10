# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Deutsch.

keys-mode-drawing = Zeichnungen
cheatsheet-drawing = Zeichnungen
palette-drawing-layers = Layer-Bereich ein- oder ausblenden
palette-drawing-all-layers = Alle Layer zeigen oder die Layer wie gespeichert
palette-drawing-paper = Dunkles und helles Papier tauschen
palette-drawing-lineweights = Linienstärken ein- oder ausblenden
disabled-not-drawing = Nur für Zeichnungen (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Modell
# machine-drafted, needs native review
cad-pen-name = Stift { $n }
cad-page-name = Seite { $n }
cad-layer-name = Layer { $n }
cad-map-name = Karte
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Lagen
cad-board-top-name = Oben
cad-board-bottom-name = Unten
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Rahmen
cad-trace-name = Kurve { $n }
cad-contours-positive-name = Positive Konturen
cad-contours-negative-name = Negative Konturen
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D-Modell
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Layer · { $shown } von { $total } sichtbar
# Toasts.
drawing-layers-all-toast = Alle Layer sichtbar
drawing-layers-saved-toast = Layer wie in der Zeichnung gespeichert
drawing-paper-dark-toast = Dunkles Papier
drawing-paper-light-toast = Helles Papier
drawing-lineweights-on = Linienstärken sichtbar
drawing-lineweights-off = Linienstärken aus: dünne Linien

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Markieren
palette-markup-help = Wolken, Text, Pfeile, Stempel, Maße und Foto-Pins, neben der Datei gespeichert.
palette-compare-sheets = Pläne vergleichen
palette-compare-sheets-help = Einen anderen Planstand dieses Blatts ausgerichtet überlagern, Änderungen umrandet.
markup-title = Markierungen
markup-list = Markierungen auf dieser Seite
markup-count = { $count ->
    [0] Keine Markierungen auf dieser Seite
    [one] Eine Markierung auf dieser Seite
   *[other] { $count } Markierungen auf dieser Seite
}
markup-empty = Werkzeug wählen und auf das Blatt klicken.
markup-item = { $n }. { $kind }
markup-tool-select = Auswählen
markup-tool-cloud = Wolke
markup-tool-text = Text
markup-tool-arrow = Pfeil
markup-tool-stamp = Stempel
markup-tool-length = Länge
markup-tool-area = Fläche
markup-tool-pin = Foto-Pin
markup-hint-select = Element anklicken, um es auszuwählen. Ziehen verschiebt es. Enter bearbeitet seinen Text.
markup-hint-cloud = Rechteck aufziehen oder Ecken anklicken und Enter drücken.
markup-hint-text = Dort klicken, wo der Text hin soll.
markup-hint-arrow = Von der Beschriftung zum Punkt ziehen oder beide anklicken.
markup-hint-stamp = Dort klicken, wo der Stempel hin soll.
markup-hint-length = Ziehen oder beide Enden anklicken.
markup-hint-area = Ecken anklicken, dann Enter drücken.
markup-hint-pin = Punkt anklicken, dann ein Foto wählen.
markup-kind-cloud = Wolke
markup-kind-text = Text
markup-kind-arrow = Pfeil
markup-kind-stamp = Stempel
markup-kind-length = Länge
markup-kind-area = Fläche
markup-kind-pin = Foto
markup-colour = Farbe: { $colour }
markup-colour-red = Rot
markup-colour-blue = Blau
markup-colour-green = Grün
markup-colour-orange = Orange
markup-colour-magenta = Magenta
markup-colour-black = Schwarz
markup-width = Linie: { $width }
markup-width-thin = Dünn
markup-width-medium = Mittel
markup-width-thick = Dick
markup-stamp = Stempel: { $words }
markup-stamp-approved = Freigegeben
markup-stamp-rejected = Abgelehnt
markup-stamp-reviewed = Geprüft
markup-stamp-custom = Eigener Text
markup-undo = Rückgängig
markup-redo = Wiederholen
markup-delete = Löschen
markup-open-photo = Foto öffnen
markup-from-measurements = Messungen übernehmen
markup-from-measurements-done = { $count ->
    [one] Eine Messung als Markierung übernommen
   *[other] { $count } Messungen als Markierungen übernommen
}
markup-export-pdf = Als PDF exportieren…
markup-export-png = Als PNG exportieren…
markup-done = Fertig
markup-ask-text = Text (Enter platziert, Esc bricht ab)
markup-ask-label = Beschriftung (optional; Enter platziert)
markup-ask-stamp = Stempeltext (Enter platziert)
markup-ask-edit = Neuer Text (Enter speichert)
markup-added = Hinzugefügt: { $kind }
markup-moved = Verschoben
markup-pin-hint = Pin erneut anklicken oder Enter drücken, um sein Foto zu öffnen.
markup-deleted = Gelöscht
markup-undone = Rückgängig gemacht
markup-redone = Wiederholt
markup-delete-ask = { $name } löschen?
markup-delete-detail = Es wird aus der Markierungsdatei entfernt. Die Zeichnung selbst bleibt unverändert.
markup-delete-ok = Löschen
markup-photo-missing = { $name } ist nicht mehr vorhanden.
markup-load-failed = Markierungen nicht gelesen: { $reason }
markup-save-failed = Markierungen nicht gespeichert: { $reason }
markup-sidecar-too-large = die Markierungsdatei ist zu groß
markup-pdf-unreadable = das PDF konnte nicht gelesen werden
markup-exported = { $name } gespeichert
markup-export-failed = Nicht exportiert: { $reason }
markup-export-same-file = einen anderen Namen wählen; das Original wird nie überschrieben
markup-export-encrypted = das PDF ist verschlüsselt. Stattdessen als PNG exportieren.
markup-export-damaged = die Struktur des PDFs ist beschädigt. Stattdessen als PNG exportieren.
sheets-title = Pläne vergleichen
sheets-other = Anderes Blatt…
sheets-other-named = Anderes: { $name }
sheets-page = Seite { $page } von { $count }
sheets-auto = Ausrichten
sheets-pick = 2 Punkte wählen
sheets-overlay = Überlagerung
sheets-difference = Differenz
sheets-blend = Überblenden
sheets-blink = Blinken
sheets-slider = Anderes Blatt { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Keine Änderungen
    [one] Eine Änderung einwolken
   *[other] { $count } Änderungen einwolken
}
sheets-done = Fertig
sheets-choose = Das andere Blatt wählen.
sheets-reading = Blatt wird gelesen…
sheets-aligning = Wird ausgerichtet…
sheets-aligned = Ausgerichtet: Maßstab { $scale }, gedreht { $angle }°, Abweichung bis { $error } px. { $changes ->
    [0] Keine Änderungen gefunden.
    [one] Eine Änderung.
   *[other] { $changes } Änderungen.
}
sheets-align-failed = Automatisches Ausrichten nicht möglich. 2 Punkte wählen.
sheets-pick-this = Punkt { $n }: auf diesem Blatt anklicken.
sheets-pick-other = Punkt { $n }: denselben Punkt auf dem anderen Blatt anklicken.
sheets-pick-same = Die beiden Punkte sind gleich. Erneut 2 Punkte wählen.
sheets-read-failed = Blatt nicht gelesen: { $reason }
sheets-failed = Dieser Schritt ist fehlgeschlagen. Erneut versuchen oder ein anderes Blatt wählen.
sheets-legend-this = Nur dieses Blatt
sheets-legend-other = Nur anderes Blatt
sheets-legend-both = Beide
sheets-cloud-text = Geändert
sheets-clouds-added = { $count ->
    [0] Keine Wolken hinzugefügt
    [one] Eine Wolke zu den Markierungen hinzugefügt
   *[other] { $count } Wolken zu den Markierungen hinzugefügt
}
sheets-opened = Pläne vergleichen
sheets-closed = Vergleich geschlossen
markup-ask-new-line = Shift+Enter: neue Zeile

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Zeichnungen
settings-drawings-shx-folders = Ordner für SHX-Schriften
settings-drawings-shx-folders-help = Ordner mit SHX-Schriften für Text in DXF und DWG, getrennt durch ;. Der Ordner der Zeichnung selbst wird zuerst durchsucht.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Kein Ordner: { $folder }
