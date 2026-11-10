# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = Karten
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Layer
cheatsheet-map-coordinates = Koordinaten
cheatsheet-map-measure = Messen

## Palette entries and their one-line descriptions.
palette-map-layers = Layer-Bereich ein- oder ausblenden
palette-map-layers-help = Die Layer der Karte: Reihenfolge, Deckkraft, Sichtbarkeit und Stil.
palette-map-add-layer = Layer aus Datei hinzufügen…
palette-map-add-layer-help = Eine weitere Kartendatei über diese legen, etwa einen GPX-Track über ein Shapefile.
palette-map-zoom-to-layer = Auf den Layer zoomen
palette-map-zoom-to-layer-help = Den im Layer-Bereich gewählten Layer ganz zeigen.
palette-map-attribute-table = Attributtabelle ein- oder ausblenden
palette-map-attribute-table-help = Die Objekte des gewählten Layers als Tabelle: sortieren, filtern, suchen und auswählen.
palette-map-coordinate-format = Nächstes Koordinatenformat
palette-map-coordinate-format-help = Dezimalgrad, Grad Minuten Sekunden, UTM oder MGRS.
palette-map-units = Metrische oder angloamerikanische Einheiten
palette-map-units-help = Längen und Flächen in Metern oder in Fuß und Meilen.
palette-map-copy-coordinate = Koordinate kopieren
palette-map-copy-coordinate-help = Die Position unter dem Zeiger kopieren oder die der Kartenmitte.
palette-map-go-to-coordinate = Zu Koordinate gehen…
palette-map-go-to-coordinate-help = Eine Position in beliebigem Format eingeben, um die Karte dort zu zentrieren.
palette-map-measure-distance = Strecke messen
palette-map-measure-distance-help = Punkte auf der Karte anklicken: Länge auf der Erdoberfläche und Azimut.
palette-map-measure-area = Fläche messen
palette-map-measure-area-help = Die Ecken einer Fläche anklicken: Größe und Umfang auf der Erdoberfläche.
palette-map-elevation-profile = Höhenprofil
palette-map-elevation-profile-help = Eine Linie über ein Höhenmodell ziehen, um seine Höhen darzustellen.
palette-map-overview = Übersichtskarte ein- oder ausblenden
palette-map-overview-help = Eine kleine Karte des ganzen Gebiets, mit markiertem sichtbarem Ausschnitt.

## Why a map command is unavailable.
disabled-not-map = Nur für Karten, bei eingeschalteten Karten-Werkzeugen
disabled-no-lonlat = Die Karte hat kein bekanntes Koordinatensystem
disabled-no-features = Die Karte hat keine Objekte
disabled-no-heights = Die Karte hat keine Höhen

## Settings › Maps.
settings-section-maps = Karten
settings-maps-coordinate-format = Koordinatenformat
settings-maps-coordinate-format-help = Wie Positionen angezeigt und kopiert werden. Zu Koordinate gehen liest jedes Format.
settings-maps-coordinate-format-dd = Dezimalgrad
settings-maps-coordinate-format-dms = Grad, Minuten, Sekunden
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Einheiten
settings-maps-units-help = Längen und Flächen auf Karten: Maßstabsleiste und Messungen. Automatisch folgt Metadaten › Einheiten.
settings-maps-units-auto = Automatisch
settings-maps-units-metric = Metrisch
settings-maps-units-imperial = Angloamerikanisch

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Breite, Länge { $position }
}
map-height = Höhe { $height }
map-height-label = Höhe
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Maßstabsleiste: { $length }
map-north-label = Nordpfeil: Norden liegt { $degrees }° im Uhrzeigersinn von oben
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] Grad, Minuten, Sekunden
    [utm] UTM
    [mgrs] MGRS
   *[dd] Dezimalgrad
}
map-format-toast = Koordinaten in { $format }
map-units-metric = Einheiten: Meter und Kilometer
map-units-imperial = Einheiten: Fuß und Meilen
toast-copied-coordinate = Koordinate kopiert

## Go to coordinate.
map-goto-hint = Dezimalgrad, DMS, UTM oder MGRS. Eingabe springt dorthin.
map-goto-empty = Eine Position eingeben.
map-goto-unrecognised = Keine Position in einem bekannten Format.
map-goto-range = Breite oder Länge außerhalb des Bereichs.
map-goto-no-way = Diese Karte kann keine Position verorten.
map-goto-outside = Diese Position liegt außerhalb der Karte.
map-goto-toast = Zentriert auf { $position }

## The layer panel.
map-layers-title = Layer: { $shown } von { $total } angezeigt
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } Objekt
   *[other] { $features } Objekte
}, { PERCENT($opacity) } deckend
map-kind-area = Flächen
map-kind-line = Linien
map-kind-point = Punkte
map-kind-empty = Keine Objekte
map-layer-visible = Layer ein- oder ausblenden
map-layer-opacity-down = Weniger deckend
map-layer-opacity-up = Stärker deckend
map-layer-colour = Nächste Farbe
map-layer-width = Nächste Linienbreite
map-layer-point = Nächste Punktgröße
map-layer-up = Nach oben
map-layer-down = Nach unten
map-layer-remove = Entfernen
map-layer-keep = Behalten
map-layer-remove-ask = { $name } von der Karte entfernen? Die Datei bleibt auf dem Datenträger.
map-layer-adding = Layer wird hinzugefügt…
map-removed = { $name } von der Karte entfernt
map-added = { $layers ->
    [one] { $name } hinzugefügt
   *[other] { $name } hinzugefügt: { $layers } Layer
}
map-add-failed = { $name } kann nicht hinzugefügt werden: { $reason }
map-add-filter = Kartendateien
map-add-not-map = es ist keine Karte.
map-add-not-vector = es hat keine Objekte.
map-add-no-crs = es hat keine Projektionsdatei (.prj).

## Identify.
map-identify-more = und { $count } weitere in der Attributtabelle
map-cell = Zelle { $x }, { $y }
map-band = Band { $n }
map-no-data = Keine Daten

## The attribute table.
map-table-title = { $layer }: { $shown } von { $total } Objekten, { $selected } ausgewählt
map-table-search = Suchen, oder Feld = Wert
map-table-selected-only = Nur ausgewählte
map-table-empty = Keine passenden Objekte.
map-table-many = viele
map-table-stats-number = { $field }: { $count ->
        [one] { $count } Wert
       *[other] { $count } Werte
    }, { $empty } leer · Min. { $min } · Max. { $max } · Mittel { $mean } · Summe { $sum } · Std.-Abw. { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } Wert
       *[other] { $count } Werte
    }, { $empty } leer · { $distinct } verschiedene
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } Wert
       *[other] { $count } Werte
    }, { $empty } leer · { $distinct } verschiedene · am häufigsten { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Punkte entlang des Wegs anklicken.
map-measure-hint-area = Die Ecken der Fläche anklicken.
map-measure-hint-profile = Punkte entlang der Linie anklicken.
map-measure-keys = Eingabe beendet. Rücktaste entfernt den letzten Punkt. Esc schließt.
map-measure-distance = Strecke { $length }
map-measure-bearing = Azimut { $bearing }
map-measure-segment = Letzter Abschnitt { $length }, Azimut { $bearing }
map-measure-area = Fläche { $area }
map-measure-perimeter = Umfang { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Länge { $length } · tiefste { $min } · höchste { $max } · Anstieg { $climb } · Abstieg { $descent }
map-profile-no-heights = Länge { $length }. Keine Höhen entlang der Linie.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: keine Höhe
