# machine-drafted, needs native review
# Specular — die Galerie (Miniaturansicht, DESIGN §10) und Einstellungen → Galerie. Deutsch.

## Palette und Menüs

palette-gallery = Galerie
gallery-palette-view = Galerieansicht …
gallery-palette-layout = Layout
gallery-palette-sort = Sortieren nach
gallery-palette-group = Gruppieren nach
gallery-palette-filter = Filtern …
gallery-palette-quick-filters = Schnellfilter
gallery-palette-details = Detailansicht
gallery-palette-info-overlay = Infos auf Miniaturen
gallery-palette-stacks = RAW+JPEG und Serienbilder stapeln
gallery-palette-subfolders = Unterordner einbeziehen
gallery-palette-compare = Auswahl vergleichen
gallery-palette-rate = Bewerten …
gallery-palette-settings = Galerie-Einstellungen …
gallery-view-menu = Ansicht
gallery-more-menu = Mehr
gallery-open = Öffnen
gallery-new-window = In neuem Fenster öffnen
gallery-reveal = Im Ordner anzeigen
gallery-properties = Eigenschaften
gallery-copy-paths = Pfade kopieren
gallery-select-all = Alles auswählen
gallery-select-none = Nichts auswählen
gallery-select-invert = Auswahl umkehren
gallery-next-unrated = Nächstes unbewertetes
gallery-stack-toggle = Stapel auf- oder zuklappen
gallery-run-on = { $name } ({ $count } Dateien)
gallery-depth-off = Nur dieser Ordner
gallery-depth-all = Alle Unterordner
gallery-depth-levels = { $depth ->
    [one] Eine Ebene Unterordner
   *[other] { $depth } Ebenen Unterordner
}

## Status und leere Zustände

gallery-items = { $count ->
    [one] { $count } Element
   *[other] { $count } Elemente
}
gallery-selected = { $count } von { $total } ausgewählt
gallery-shown-of = { $count } von { $total } angezeigt
gallery-listing = Ordner wird gelesen … { $count }
gallery-empty = Dieser Ordner enthält keine Bilder.
gallery-empty-filter = Keine Bilder entsprechen dem Filter.
gallery-list-error = Dieser Ordner kann nicht gelesen werden: { $reason }
gallery-no-folder = Kein Ordner geöffnet
gallery-kind-folder = Ordner
gallery-drag-count = { $count ->
    [one] { $count } Datei
   *[other] { $count } Dateien
}
gallery-badge-pages = { $count } Seiten

## Filterleiste

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Name oder rating>=3 camera:X-T5 …
gallery-filter-clear = Filter löschen
gallery-filter-rating = { $stars ->
    [one] ★ 1 oder mehr
   *[other] ★ { $stars } oder mehr
}
gallery-filter-label = Farbmarkierung: { $label }
gallery-filter-picks = Ausgewählte
gallery-filter-hide-rejects = Abgelehnte ausblenden
gallery-filter-unrated = Unbewertet
gallery-filter-raw = RAW
gallery-filter-animated = Animiert
gallery-filter-hdr = HDR
gallery-filter-landscape = Querformat
gallery-filter-portrait = Hochformat

## Query errors and facets

gallery-query-unknown-field = Unbekanntes Feld „{ $field }“
gallery-query-unknown-field-suggest = Unbekanntes Feld „{ $field }“: meinten Sie „{ $suggestion }“?
gallery-query-missing-value = „{ $field }“ braucht einen Wert
gallery-query-bad-number = „{ $value }“ ist keine Zahl ({ $field })
gallery-query-bad-date = „{ $value }“ ist kein Datum ({ $field }): 2024, 2024-05 oder 2024-05-12
gallery-query-bad-word = „{ $value }“ passt nicht zu { $field }: { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = „{ $value }“ ist kein Exif-Tag-Name
gallery-query-no-compare = { $field } lässt sich hier nicht mit < oder > vergleichen
gallery-query-unterminated = Ein schließendes Anführungszeichen fehlt
gallery-query-unavailable = „{ $field }“ braucht die Bibliothek (hier nicht verfügbar)
gallery-facets = Facetten
gallery-facets-counting = Wird gezählt…
gallery-facets-empty = Nichts zu zählen
gallery-facets-search = Facetten durchsuchen
gallery-facets-no-match = Keine passenden Werte
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Startbildschirm
gallery-facet-format = Format
gallery-facet-camera = Kamera
gallery-facet-lens = Objektiv
gallery-facet-rating = Bewertung
gallery-facet-year = Jahr
gallery-facet-orientation = Ausrichtung
gallery-facet-label = Farbmarkierung
gallery-facet-properties = Eigenschaften
gallery-facet-square = Quadratisch
gallery-facet-alpha = Transparenz
gallery-facet-gps = Ort
gallery-facet-multipage = Mehrere Seiten
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } Datei
   *[other] { $count } Dateien
}

## Gruppen

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Dieser Ordner
gallery-group-unknown = Unbekannt
gallery-group-unrated = Unbewertet

## Aussortieren

gallery-stars = { $stars ->
    [one] 1 Stern
   *[other] { $stars } Sterne
}
gallery-rate-none = Keine Bewertung
gallery-pick = Auswählen
gallery-reject = Ablehnen
gallery-unflag = Keine Markierung
gallery-label-red = Rot
gallery-label-yellow = Gelb
gallery-label-green = Grün
gallery-label-blue = Blau
gallery-label-none = Keine Farbmarkierung
gallery-compare-hint = Wählen Sie 2 bis 4 Bilder aus, um sie zu vergleichen.

## Dateien

gallery-trash-count = { $count ->
    [one] { $count } Datei in den Papierkorb verschieben
   *[other] { $count } Dateien in den Papierkorb verschieben
}
gallery-moved-to = { $count ->
    [one] { $count } Datei nach { $folder } verschoben
   *[other] { $count } Dateien nach { $folder } verschoben
}
gallery-copied-to = { $count ->
    [one] { $count } Datei nach { $folder } kopiert
   *[other] { $count } Dateien nach { $folder } kopiert
}
gallery-move-failed = { $file } konnte nicht verschoben werden: { $reason }
gallery-drop-move = Nach { $folder } verschieben
gallery-drop-copy = Nach { $folder } kopieren
gallery-move-here = Auswahl hierher verschieben
gallery-copy-here = Auswahl hierher kopieren
gallery-not-an-image = { $file } ist kein Bild, das { -app-name } öffnen kann.
gallery-cache-cleared = Miniatur-Cache geleert
# machine-drafted, needs native review
gallery-thumb-failed = Keine Miniatur: { $reason }
gallery-cache-clear-failed = Der Miniatur-Cache konnte nicht geleert werden: { $reason }

## Eigenschaften

gallery-prop-name = Name
gallery-prop-folder = Ordner
gallery-prop-size = Dateigröße
gallery-prop-dimensions = Abmessungen
gallery-prop-format = Format
gallery-prop-date = Datum
gallery-prop-camera = Kamera
gallery-prop-lens = Objektiv
gallery-prop-rating = Bewertung
gallery-prop-label = Farbmarkierung
gallery-prop-items = Elemente

## Bildschirmleser

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } Element
   *[other] { $count } Elemente
}
gallery-a11y-dims = { $width } × { $height }
gallery-a11y-folder = Ordner
gallery-a11y-up = Hoch zu { $folder }
gallery-a11y-archive = Archiv
gallery-a11y-stack = Stapel mit { $count }
gallery-a11y-filter-bar = Filter
gallery-a11y-filter-field = Nach Name oder Feld filtern
gallery-a11y-chip = Filterbegriff { $term }
gallery-a11y-chip-remove = Zum Entfernen drücken
gallery-a11y-results = { $count ->
    [0] Keine passenden Elemente
    [one] { $count } passendes Element
   *[other] { $count } passende Elemente
}

## Einstellungen → Galerie

settings-section-gallery = Galerie
settings-gallery-layout = Layout
settings-gallery-layout-help = Quadrate schneiden Miniaturen auf ein Raster zu; Blocksatz-Zeilen behalten die Form jedes Bildes; unbeschnitten passt ganze Bilder in gleich große Zellen ein.
settings-gallery-layout-squares = Quadrate
settings-gallery-layout-justified = Blocksatz-Zeilen
settings-gallery-layout-uncropped = Unbeschnitten
settings-gallery-layout-details = Details
settings-gallery-thumbnail-size = Miniaturgröße
settings-gallery-thumbnail-size-help = In der Galerie auch mit Strg+Mausrad, Zwei-Finger-Zoom oder Strg+Plus und Minus.
settings-gallery-captions = Dateinamen anzeigen
settings-gallery-captions-help = Eine Beschriftung unter jeder Miniatur.
settings-gallery-badges = Kennzeichen anzeigen
settings-gallery-badges-help = Kleine Symbole für RAW, animiert, HDR, mehrseitig, Bewertung und Farbmarkierung.
settings-gallery-sort-order = Sortieren nach
settings-gallery-sort-order-help = Die Reihenfolge der Galerie. Ordner stehen immer am Anfang.
settings-gallery-sort-order-name = Name
settings-gallery-sort-order-date-taken = Aufnahmedatum
settings-gallery-sort-order-modified = Änderungsdatum
settings-gallery-sort-order-size = Dateigröße
settings-gallery-sort-order-type = Dateityp
settings-gallery-sort-order-dimensions = Abmessungen
settings-gallery-sort-descending = Umgekehrte Reihenfolge
settings-gallery-sort-descending-help = Vom letzten zum ersten sortieren.
settings-gallery-group-by = Gruppieren nach
settings-gallery-group-by-help = Abschnittsüberschriften, die beim Scrollen oben stehen bleiben.
settings-gallery-group-by-none = Nichts
settings-gallery-group-by-day = Tag
settings-gallery-group-by-month = Monat
settings-gallery-group-by-year = Jahr
settings-gallery-group-by-event = Ereignis
settings-gallery-group-by-folder = Ordner
settings-gallery-group-by-camera = Kamera
settings-gallery-group-by-format = Format
settings-gallery-group-by-rating = Bewertung
settings-gallery-group-by-target = Objekt
settings-gallery-group-by-filter = Filter
settings-gallery-per-folder-view = Ansicht für jeden Ordner merken
settings-gallery-per-folder-view-help = Layout, Größe, Sortierung und Gruppierung werden pro Ordner gespeichert; andernfalls gilt überall dieselbe Ansicht.
settings-gallery-open-folders-in-gallery = Ordner in der Galerie öffnen
settings-gallery-open-folders-in-gallery-help = Beim Öffnen eines Ordners mit seinen Miniaturen statt mit dem ersten Bild beginnen.
settings-gallery-escape-opens-gallery = Esc öffnet die Galerie
settings-gallery-escape-opens-gallery-help = Wenn es nichts anderes zu schließen gibt, wechselt Esc im Betrachter zur Galerie, statt nichts zu tun.
settings-gallery-wrap-around = Pfeiltasten springen ans andere Ende
settings-gallery-wrap-around-help = Am Ende der Liste springen die Pfeiltasten zum Anfang und vom Anfang zum Ende. Eine gehaltene Taste hält weiterhin am Ende an.
settings-gallery-show-hidden = Versteckte Dateien anzeigen
settings-gallery-show-hidden-help = Dateien und Ordner einbeziehen, deren Namen mit einem Punkt beginnen.
settings-gallery-show-other-files = Andere Dateien anzeigen
settings-gallery-show-other-files-help = Dateien einbeziehen, die { -app-name } nicht öffnen kann; sie werden mit einem allgemeinen Symbol angezeigt.
settings-gallery-show-archives = Archive anzeigen
settings-gallery-show-archives-help = Comics und andere Archive (zip, cbz, rar, 7z …) als Kacheln einbeziehen. Ein Archiv direkt zu öffnen, funktioniert weiterhin.
settings-gallery-subfolder-depth = Unterordner-Ebenen
settings-gallery-subfolder-depth-help = Auch Bilder aus Unterordnern anzeigen, bis zu dieser Anzahl von Ebenen (0: nur dieser Ordner).
settings-gallery-stacks = Zusammengehörige Dateien stapeln
settings-gallery-stacks-help = RAW+JPEG-Paare und Serienbilder als eine Kachel anzeigen. Mit S wird ein Stapel aufgeklappt.
settings-gallery-info-overlay = Infos auf Miniaturen
settings-gallery-info-overlay-help = Abmessungen und Belichtung über jeder Miniatur.
settings-gallery-prefer-embedded = Eingebettete Vorschauen bevorzugen
settings-gallery-prefer-embedded-help = Die in der Datei gespeicherte Vorschau verwenden, wenn sie groß genug ist. Viel schneller, etwas weniger genau.
settings-gallery-sharper-raw-thumbnails = Schärfere RAW-Miniaturen
settings-gallery-sharper-raw-thumbnails-help = Bearbeitete RAW-Fotos werden für ihre Miniaturen mit 1024 px entwickelt. Schärfer, etwas langsamer. Aus: 512 px.
settings-gallery-rating-sidecars = Bewertungen in XMP-Filialdateien schreiben
settings-gallery-rating-sidecars-help = Bewertungen, Farbmarkierungen und Markierungen werden auch in eine .xmp-Datei neben dem Bild geschrieben, die andere Fotoanwendungen lesen.
settings-gallery-cache-size = Größe des Miniatur-Cache
settings-gallery-cache-size-help = Wenn der Cache voll ist, werden die ältesten Miniaturen entfernt.
settings-gallery-cache-location = Speicherort des Miniatur-Cache
settings-gallery-cache-location-help = Ein Ordner auf einem schnellen Laufwerk ist am besten geeignet.
settings-gallery-clear-cache = Miniatur-Cache leeren
settings-gallery-clear-cache-help = Löscht die gespeicherten Miniaturen. Sie werden beim Durchsehen neu erstellt.

## Compare
# machine-drafted, needs native review

compare-side = Nebeneinander
compare-overlay = Überlagerung
compare-difference = Differenz
compare-flicker = Flackern
compare-blend = Überblenden
compare-kind-absolute = Absoluter Fehler
compare-kind-squared = Quadratischer Fehler
compare-kind-relative = Relativer Fehler
compare-scale-down = Weniger empfindlich
compare-scale-up = Empfindlicher
compare-scale = Vollausschlag { $value }
compare-space = Lineares Licht, wie angezeigt
compare-space-help = Fehler und Kennzahlen werden in linearem Licht gemessen, wie das Bild angezeigt wird, nach dem Tone-Mapping, je Kanal einschließlich Alpha.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = abgetastet mit { $width } × { $height }
compare-aligned = Oben links ausgerichtet, { $width } × { $height } verglichen
compare-vs = { $b } gegen { $a }
compare-overlay-label = { $b } über { $a } mit { PERCENT($percent) }
compare-pending = Wird gemessen…
# machine-drafted, needs native review
compare-diff-failed = Die Differenz kann nicht angezeigt werden: { $reason }
compare-no-overlap = Nichts zum Vergleichen im Bild
compare-zoom-fit = Einpassen
compare-zoom = { PERCENT($percent) }
compare-showing = Zeigt { $name }
compare-a11y-group = Vergleich
compare-a11y-blend = Zwischen den Bildern überblenden
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Nebeneinander vergleichen
gallery-palette-compare-overlay = Als Überlagerung vergleichen
gallery-palette-compare-difference = Differenz vergleichen: { $kind }
gallery-palette-compare-flicker = Zwischen verglichenen Bildern flackern
gallery-palette-compare-actual = Bei { PERCENT(100) } vergleichen

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Leertaste oder Strg+Eingabe wählt mehrere

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Farbprofil
gallery-facet-date = Aufnahmedatum
gallery-facet-picked = Ausgewählte
gallery-facet-rejected = Abgelehnte
gallery-facet-flagged = Ausgewählte oder abgelehnte
gallery-facet-rated = Bewertete
gallery-facet-labelled = Mit Farbmarkierung

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Bittiefe
gallery-facet-pages = Seiten
gallery-facet-width = Breite
gallery-facet-height = Höhe
gallery-facet-megapixels = Megapixel
gallery-picker-bits = { $bits } Bit
gallery-picker-pages = { $pages ->
    [one] { $pages } Seite
   *[other] { $pages } Seiten
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = Alle in einer Gruppe
gallery-picker-group-keep = Vorschlag: behalten
gallery-picker-group-discard = Vorschlag: verwerfen
gallery-picker-ops = Vergleichen
gallery-picker-op-eq = Gleich
gallery-picker-op-ge = Mindestens
gallery-picker-op-le = Höchstens
gallery-picker-ge = mindestens { $value }
gallery-picker-gt = mehr als { $value }
gallery-picker-le = höchstens { $value }
gallery-picker-lt = weniger als { $value }

# Map view.
gallery-facet-place = Ort
gallery-palette-map = Karte
map-title = Karte
map-summary = { $count ->
    [one] { $count } Foto mit Ort
   *[other] { $count } Fotos mit Ort
}
map-photos = { $count ->
    [one] { $count } Foto
   *[other] { $count } Fotos
}
map-zoom-in = Vergrößern
map-zoom-out = Verkleinern
map-show-all = Alle zeigen
map-done = Fertig
map-credit = Kartendaten: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Karte: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Online-Kartenkacheln
map-basemap-consent-server = Kartenkacheln kommen von { $host }.
map-basemap-consent-sees = Dieser Server sieht Ihre Internetadresse und die Gebiete, die Sie ansehen.
map-basemap-consent-private = Ihre Fotos, deren Namen und Orte werden nicht gesendet.
map-basemap-consent-cache = Kacheln werden auf diesem Computer gespeichert, bis zu { $size }.
map-basemap-allow = Online-Karte verwenden
map-basemap-decline = Offline-Karte behalten
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Grundkarte nicht angezeigt: { $reason }
map-basemap-bad-address = die Adresse des Kachelservers braucht {"{"}z{"}"}, {"{"}x{"}"} und {"{"}y{"}"}.
map-basemap-unknown-tiles = die Kacheln der Kartendatei sind von unbekannter Art.
map-loading = Fotoorte werden gelesen…
map-empty = Keines dieser Fotos hat einen Ort
map-hidden = Fotoorte sind ausgeblendet (Einstellungen › Metadaten und Datenschutz)
map-a11y-group = Karte der Fotoorte
map-a11y-status = { $count ->
    [one] { $count } Ort gezeigt, Zoomstufe { $zoom }
   *[other] { $count } Orte gezeigt, Zoomstufe { $zoom }
}
