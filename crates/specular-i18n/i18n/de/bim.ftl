# machine-drafted, needs native review
## 3D-Modelle prüfen: Gehen und Fliegen, Schnittebenen und Schnittbox, gespeicherte Ansichten,
## BCF-Themen (3D-Modus). Die genannten Tasten sind die Standardtasten.

cheatsheet-review = Prüfen

palette-mesh-walk = Gehen
palette-mesh-fly = Fliegen
palette-mesh-views = Gespeicherte Ansichten
palette-mesh-save-view = Ansicht speichern
palette-mesh-section-plane = Schnittebene hinzufügen
palette-mesh-section-box = Schnittbox
palette-mesh-section-flip = Schnittebene umkehren
palette-mesh-section-clear = Alle Schnitte entfernen
palette-mesh-measure-cut = Am Schnitt messen
palette-mesh-bcf-topics = BCF-Themen
# machine-drafted, needs native review
palette-mesh-model-checks = Modellprüfung
# machine-drafted, needs native review
palette-mesh-models = Modelle in der Szene
# machine-drafted, needs native review
palette-mesh-add-model = Modell hinzufügen…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Modelle aus einem Ordner hinzufügen…

# Gehen und Fliegen: Meldungen und Anzeigezeilen, nach $what. $eye: Augenhöhe über dem
# Boden, mit Einheit (nur walk-line).
mesh-nav = { $what ->
    [walk-toast] Gehen: W A S D bewegt, Ziehen schaut umher, Esc beendet
    [fly-toast] Fliegen: W A S D bewegt, Q E runter und hoch, Esc beendet
    [orbit] Orbit
    [walk-line] Gehen · Augenhöhe { $eye } über dem Boden
    [walk-keys] W A S D bewegt · Ziehen schaut · Shift schneller · Esc beendet
    [preparing] Wände noch nicht bereit: Sie gehen hindurch
    [fly-keys] W A S D bewegt · Q E runter, hoch · Ziehen schaut · Shift schneller · Esc beendet
   *[fly-line] Fliegen
}

# Kurztexte zu Schnitten, Messen und gespeicherten Ansichten, nach $what.
bim-note = { $what ->
    [plane-added] Schnittebene hinzugefügt: zum Verschieben am Griff ziehen
    [box-on] Schnittbox an: zum Verschieben am Griff einer Seite ziehen
    [box-off] Schnittbox aus
    [cleared] Schnitte entfernt
    [measure-on] Zwei Punkte auf dem Schnitt anklicken
    [measure-needs-cut] Zuerst eine Schnittebene oder die Schnittbox hinzufügen
    [measure-off-cut] Auf den Schnitt klicken
    [measure-hint] Messen: zwei Punkte auf dem Schnitt anklicken
    [ask-view-name] Name der Ansicht
    [ask-rename] Neuer Name
    [views-empty] Keine gespeicherten Ansichten. N speichert diese.
    [views-hints] Enter aufrufen · N speichern · F2 umbenennen · Delete entfernen · P als PNG speichern
    [delete-detail] Nur die gespeicherte Ansicht wird entfernt. Das Modell bleibt unverändert.
    [delete-ok] Entfernen
    [not-over-model] Andere Datei wählen: Diese würde das Modell ersetzen.
   *[view-name] Ansicht
}

# BCF (BIM Collaboration Format), Kurztexte, nach $what.
bcf-note = { $what ->
    [title-none] BCF-Themen
    [new-file] neu
    [empty] Keine Themen. O öffnet eine BCF-Datei, N fügt ein Thema hinzu.
    [hints] Enter aufrufen · ← → Ansichtspunkte · N Thema · C Kommentar · S Status · E Titel · O öffnen · Ctrl+S exportieren
    [ask-title] Titel des Themas
    [ask-comment] Kommentar
    [untitled] Ohne Titel
    [topic-added] Thema mit dieser Ansicht hinzugefügt
   *[comment-added] Kommentar mit dieser Ansicht hinzugefügt
}

# $planes: Anzahl der Schnittebenen.
bim-sections-line = { $planes ->
    [0] Keine Schnittebenen
    [one] 1 Schnittebene
   *[other] { $planes } Schnittebenen
}
# $size: Größe der Schnittbox mit Einheit.
bim-box-line = Box { $size }
# $count: die höchstmögliche Zahl von Ebenen.
bim-planes-full = Höchstens { $count } Schnittebenen
# Ein Ziehgriff, für Bildschirmleser. $n: seine Nummer.
bim-handle = { $kind ->
    [box] Schnittbox-Seite { $n }
   *[plane] Schnittebene { $n }
}
# $length: die Länge mit Einheit.
bim-measure-line = Länge am Schnitt: { $length }
bim-views-title = { $count ->
    [one] 1 gespeicherte Ansicht
   *[other] { $count } gespeicherte Ansichten
}
# $name: Name einer Ansicht.
bim-view-saved = „{ $name }“ gespeichert
bim-view-delete-title = Ansicht „{ $name }“ entfernen?
# $file: eine geschriebene Datei.
bim-saved-file = { $file } gespeichert
# $reason: warum es fehlschlug.
bim-failed = { $what ->
    [open] BCF-Datei konnte nicht geöffnet werden: { $reason }
    [views] Ansichten nur für diese Sitzung behalten: { $reason }
   *[save] Speichern fehlgeschlagen: { $reason }
}
# $count: Themen; $file: Dateiname des Archivs.
bcf-title = { $count ->
    [one] 1 Thema
   *[other] { $count } Themen
} · { $file }
bcf-comments = { $count ->
    [one] 1 Kommentar
   *[other] { $count } Kommentare
}
# $at: der gezeigte Ansichtspunkt; $count: Ansichtspunkte des Themas.
bcf-viewpoint-of = Ansichtspunkt { $at } von { $count }
bcf-opened = { $count ->
    [one] 1 Thema geöffnet
   *[other] { $count } Themen geöffnet
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modelle: 1
   *[other] Modelle: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Diese Datei ist kein IFC-Modell.
    [not-ifc-open] Öffnen Sie zuerst ein IFC-Modell. Nur IFC-Modelle lassen sich kombinieren.
    [this-model] Dieses Modell
    [hints] A Modell hinzufügen · Shift+A Ordner hinzufügen · Space zeigen oder ausblenden · C nach Modell einfärben · Enter aufrufen · Delete entfernen
    [origin-ok] Am Ursprung platzieren
    [first-stays] Das geöffnete Modell bleibt. Öffnen Sie eine andere Datei, um es zu wechseln.
    [remove-detail] Es verlässt die Szene. Die Datei auf dem Datenträger bleibt erhalten.
   *[remove-ok] Entfernen
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 Bauteil
   *[other] { $elements } Bauteile
} · { $shown ->
    [hidden] ausgeblendet
   *[shown] angezeigt
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] geöffnetes Modell
    [map-crs] nach Kartenkoordinaten ({ $crs })
    [map] nach Kartenkoordinaten
    [site] nach Breiten- und Längengrad des Grundstücks (ungefähr)
    [project] nach gemeinsamen Projektkoordinaten
   *[origin] am Ursprung (nicht ausgerichtet)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Die Modelle verwenden unterschiedliche Koordinatensysteme: { $first } und { $this }.
    [only-first] Das geöffnete Modell ist georeferenziert, dieses nicht.
    [only-this] Dieses Modell ist georeferenziert, das geöffnete nicht.
   *[broken] Die Kartenumrechnung dieses Modells ist unbrauchbar (ihr Maßstab ist null).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Um { $distance } verschoben, um { $angle }° gedreht
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] 1 Modell wird gelesen…
   *[other] { $count } Modelle werden gelesen…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } lässt sich nicht ausrichten. Am Ursprung platzieren?
# machine-drafted, needs native review
fed-no-files = Keine IFC-Dateien in { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } wurde nicht hinzugefügt: { $reason }
# machine-drafted, needs native review
fed-added = { $file } hinzugefügt. Modelle in der Szene: { $count }.
# machine-drafted, needs native review
fed-remove-title = { $file } aus der Szene entfernen?
# machine-drafted, needs native review
fed-removed = { $file } aus der Szene entfernt.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] ausgeblendet
   *[shown] angezeigt
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Einfärben nach Modell an
   *[off] Einfärben nach Modell aus
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI und gespeicherte Ansichten
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 Annotation
   *[other] { $count } Annotationen
}
# machine-drafted, needs native review
pmi-view = Gespeicherte Ansicht
# machine-drafted, needs native review
pmi-view-all = zeigt alle Annotationen
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] zeigt 1 Annotation
   *[other] zeigt { $count } Annotationen
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Ansicht „{ $name }“ wird gezeigt. A zeigt alle.
# machine-drafted, needs native review
pmi-has-faces = Flächen hervorgehoben
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Keine PMI in diesem Modell.
    [hints] ↑ ↓ auswählen · Enter aufrufen · A alle · D wie gezeichnet · Shift+A ausblenden
    [hidden] Annotationen ausgeblendet
    [shown] Annotationen eingeblendet
    [as-drawn] Annotationen so, wie die Datei sie zeichnet
    [as-labels] Annotationen als Beschriftungen
   *[all-shown] Alle Annotationen eingeblendet
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Maß
    [tolerance] Geometrische Toleranz
    [datum] Bezug
    [target] Bezugsstelle
   *[graphic] Gezeichnete Annotation
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Abstand
    [angular] Winkel
    [diameter] Durchmesser
    [radius] Radius
    [length] Länge
    [thickness] Dicke
   *[other] Maß
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Geradheit
    [flatness] Ebenheit
    [circularity] Rundheit
    [cylindricity] Zylinderform
    [line-profile] Linienform
    [surface-profile] Flächenform
    [angularity] Neigung
    [perpendicularity] Rechtwinkligkeit
    [parallelism] Parallelität
    [position] Position
    [concentricity] Konzentrizität
    [coaxiality] Koaxialität
    [symmetry] Symmetrie
    [circular-runout] Lauf
   *[total-runout] Gesamtlauf
}
