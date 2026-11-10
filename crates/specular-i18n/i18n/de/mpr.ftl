# Multiplanar reformatting (MPR).

palette-mpr = Multiplanare Rekonstruktion (MPR)
palette-mpr-slab = Nächster MPR-Slab (dünn, MIP, MinIP, Mittelwert)
disabled-no-volume = Das ist kein Volumen: Öffnen Sie eine Serie, eine Mehrbilddatei, ein 3D-Volumen oder einen Z-Stapel
disabled-mpr-closed = Öffnen Sie zuerst die multiplanaren Ansichten
mpr-axial = Axial
mpr-coronal = Koronal
mpr-sagittal = Sagittal
mpr-oblique = Schräg
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D-Ansicht
mpr-thin = Dünn
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Mittelwert
mpr-slab = Slab: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Slab: { $mode }, { $thickness } Å
mpr-slab-unit = Slab: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Slab: { $mode }, { $thickness } Voxel
mpr-thicker = Dickerer Slab
mpr-thinner = Dünnerer Slab
mpr-reset = Ansichten zurücksetzen
mpr-close = MPR schließen
mpr-maximise = Ansicht maximieren
mpr-export-planes = Ebenen exportieren
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Streamlines aus { $files }: Slab { $thickness } mm
mpr-tracts-off = Streamlines aus { $files }: ausgeblendet
mpr-tracts-none = Keine TrackVis- oder MRtrix-Streamlines neben diesem Volumen
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } Voxel
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } von { $count }
mpr-axis-channel = Kanal
mpr-axis-unnamed = Achse { $axis }
mpr-one-channel = MPR zeigt jeweils einen Kanal
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } Kanäle in Farbe, jeder in seinem Anzeigebereich
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Ein Kanal angezeigt: { $reason }
mpr-isotropic = Keine Voxelgröße in der Datei: Voxel als Würfel angezeigt
mpr-isotropic-z = Kein Schichtabstand in der Datei: Schichten im Abstand von einem Pixel angezeigt
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = { $loaded } von { $count ->
        [one] { $count } Schicht
       *[other] { $count } Schichten
    } geladen
mpr-loaded = { $count ->
        [one] { $count } Schicht
       *[other] { $count } Schichten
    }, { $width } × { $height }
mpr-downsampled = In 1/{ $factor } der Auflösung angezeigt (Speicher- oder Grafikgrenzen)
mpr-converted = Werte als 32-Bit-Gleitkommazahlen angezeigt
mpr-opening = Volumen wird geöffnet…
mpr-gpu-memory = { $size } auf der Grafikkarte
mpr-refused = Kein Volumen möglich: { $reason }
mpr-failed = Das Volumen wurde nicht geladen: { $reason }
mpr-no-gpu = Die Grafikkarte kann dieses Volumen nicht aufnehmen: { $reason }
mpr-planes-saved = { $count ->
        [one] { $count } Ebene
       *[other] { $count } Ebenen
    } in { $folder } gespeichert
mpr-planes-failed = Die Ebenen konnten nicht gespeichert werden: { $reason }
mpr-view-a11y = Ansicht { $view }, { $position } mm
mpr-crosshair-a11y = Fadenkreuz bei { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Fadenkreuz bei { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Fadenkreuz bei { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Fadenkreuz bei Voxel { $x }, { $y }, { $z }
mpr-controls = MPR-Steuerung

# 3D view palette entries.
palette-mpr-preset = Nächstes 3D-Preset (CT Knochen, Weichteil, Lunge, Angio; MR)
palette-mpr-render = Nächste 3D-Darstellung (schattiert, Cinematic, MIP, MinIP)
palette-mpr-clip = Nächstes 3D-Clipping (keines, Box der 2D-Ansichten, Ecke ausgeschnitten)
palette-mpr-export-stl = 3D-Oberfläche als STL exportieren…
# machine-drafted, needs native review
palette-mpr-tract-slab = Nächster Streamline-Slab in der MPR
palette-mpr-maximise = MPR-Ansicht maximieren oder alle Ansichten zeigen
palette-mpr-export-planes = MPR-Ebenen als Bilder exportieren…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] CT Knochen
    [soft] CT Weichteil
    [lung] CT Lunge
    [angio] CT Angio
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Schattiert
}
vr-clip = { $clip ->
    [box] Clipping: Box der Ansichten
    [cut] Clipping: Ecke ausgeschnitten
   *[off] Kein Clipping
}
vr-smooth = Oberfläche glätten
vr-export = STL exportieren
vr-threshold = Oberfläche bei { $value }
vr-stl-exporting = Oberfläche wird erstellt…
vr-stl-saved = { $count ->
    [0] Nichts bei diesem Schwellenwert: keine Oberfläche gespeichert
   *[other] { $count } Dreiecke in { $name } gespeichert
}
vr-stl-failed = Die Oberfläche konnte nicht gespeichert werden: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, Oberfläche bei { $threshold }. Umschalt+Pfeiltasten drehen, Plus und Minus zoomen.

## Rohvolumen: wie eine .raw- / .vol-Datei ohne Header aufgebaut ist

# machine-drafted, needs native review
raw-volume-title = Rohvolumen öffnen
raw-volume-file = { $file } hat keinen Header. Geben Sie an, wie die Werte angeordnet sind.
raw-volume-guess-label = Vorgeschlagene Anordnung
raw-volume-guess = { $index } von { $count }
raw-volume-custom = Benutzerdefiniert
raw-volume-width = Breite (Voxel)
raw-volume-height = Höhe (Voxel)
raw-volume-slices = Schichten
raw-volume-type = Werttyp
raw-volume-type-unsigned = Vorzeichenlos, { $bits } Bit
raw-volume-type-signed = Vorzeichenbehaftet, { $bits } Bit
raw-volume-type-float = Gleitkomma, { $bits } Bit
raw-volume-order = Bytereihenfolge
raw-volume-little = Little-Endian
raw-volume-big = Big-Endian
raw-volume-offset = Header-Versatz
raw-volume-bytes = { $count ->
    [one] 1 Byte
   *[other] { $count } Byte
}
raw-volume-pixel = Pixelgröße
raw-volume-spacing = Schichtabstand
raw-volume-mm = { $value } mm
raw-volume-exact = Füllt die Datei genau ({ $size })
raw-volume-fits = Nutzt { $size } von { $file }; { $left } bleiben übrig
raw-volume-too-big = Benötigt { $size }, die Datei hat aber nur { $file }
raw-volume-cancel = Abbrechen
raw-volume-open = Öffnen
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Konnektivität von { $seed }; Fadenkreuz auf einen anderen Seed bewegen
# machine-drafted, needs native review
mpr-measure-hint-edit = Ende ziehen, um es zu verschieben; auf das andere Ende, um es zu entfernen
# machine-drafted, needs native review
mpr-measure-removed = Messung entfernt
mpr-measure-lengths-only = MPR misst nur Längen
mpr-more = Mehr…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Gekrümmte MPR entlang einer geklickten Kurve
palette-mpr-curve-layout = Gekrümmte MPR: begradigt oder gestreckt
palette-mpr-curve-save = Kurve speichern…
palette-mpr-curve-load = Kurve laden…
palette-mpr-export-segments = Jedes Segment als STL exportieren…
palette-mpr-dental = Dentallayout (Panorama und Querschnitte)
palette-mpr-nerve = Mandibularkanal nachzeichnen
cpr-kind = { $kind ->
    [stretched] Gestreckt
   *[straightened] Begradigt
}
cpr-button = Gekrümmt
cpr-controls = Steuerung der gekrümmten MPR
cpr-at = Bei { $position }
cpr-section = Querschnitt
cpr-hint-start = Klicken Sie in der Ansicht { $view } Punkte entlang der Struktur
cpr-hint-more = Klicken Sie noch einen Punkt
cpr-add-points = Punkte hinzufügen
cpr-undo-point = Punkt rückgängig
# machine-drafted, needs native review
cpr-mark = Hier markieren
cpr-mark-set = Markierung { $n } gesetzt.
cpr-mark-removed = Markierung entfernt.
cpr-mark-here = Markierung { $n }
cpr-diameter = Durchmesser { $value }
cpr-diameters = Durchmesser { $min } bis { $max }
cpr-mark-span = Markierung { $from } bis { $to }: { $length } entlang der Mittellinie
cpr-mark-total = Alle Markierungen: { $length }
cpr-clear = Kurve löschen
cpr-save = Kurve speichern
cpr-load = Kurve laden
cpr-done = Fertig
cpr-a11y = { $name } über { $length }, Cursor bei { $position }. Das Mausrad bewegt den Cursor.
cpr-none = Noch keine Kurve: Klicken Sie zuerst Punkte
cpr-cleared = Kurve gelöscht
cpr-saved = Kurve gespeichert: { $name }
cpr-save-failed = Die Kurve konnte nicht gespeichert werden: { $reason }
cpr-loaded = Kurve geladen
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 Mittellinie geladen
   *[other] { $count } Mittellinien geladen
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] 1 gespeicherte Mittellinie gefunden
   *[other] { $count } gespeicherte Mittellinien gefunden
}
# A centreline without a name, by its place in the list.
cpr-centreline = Mittellinie { $number }
cpr-new = Neue Mittellinie
cpr-rename = Umbenennen
cpr-name = Name der Mittellinie
cpr-delete = Löschen
# $name: the centreline's name.
cpr-delete-title = Die Mittellinie „{ $name }“ löschen?
cpr-delete-detail = Ihre Punkte werden entfernt. Gespeicherte Dateien bleiben auf dem Datenträger.
cpr-deleted = „{ $name }“ gelöscht
cpr-load-failed = Die Kurve konnte nicht geladen werden: { $reason }
cpr-load-not-json = die Datei ist kein JSON
cpr-load-not-curves = die Datei ist keine Specular-Kurve
cpr-load-empty = die Datei enthält keine Kurve aus zwei oder mehr Punkten
cpr-panoramic = Panorama
cpr-panoramic-slab = Panorama, Slab { $thickness } mm
cpr-panoramic-thin = Panorama, dünn

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Dental
dental-not-for-diagnosis = Nicht für Diagnose oder Behandlungsplanung
dental-nerve = Mandibularkanal
dental-nerve-hint = Klicken Sie im Panorama oder in einem Querschnitt Punkte entlang des Mandibularkanals
dental-nerve-done = Nachzeichnen des Mandibularkanals aus
dental-spacing = Alle { $spacing } mm
dental-section = Schnitt { $number }
dental-off = Öffnen Sie zuerst das Dentallayout
dental-suggesting = Zahnbogen wird gesucht…
dental-suggested = Zahnbogen vorgeschlagen: Ziehen Sie seine Punkte zum Anpassen
dental-no-arch = Kein Zahnbogen gefunden: Klicken Sie in der axialen Ansicht Punkte entlang des Bogens
dental-vendor = Dental-DVT von { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Segmente exportieren
seg-stl-detail = Segmente: { $count ->
        [one] { $count } Dreieck
       *[other] { $count } Dreiecke
    }
seg-stl-detail-full = Segmente: volle Detailtreue
seg-stl-none = Keine Segmente zum Exportieren
seg-stl-not-labels = dieses Volumen ist keine Label-Map
seg-stl-exporting = Oberflächen der Segmente werden erstellt…
seg-stl-saved = { $count ->
    [one] { $count } Segment in { $folder } gespeichert
   *[other] { $count } Segmente in { $folder } gespeichert
}
seg-stl-open = Nicht geschlossen: { $count }.
seg-stl-empty = Die Segmente sind leer: nichts gespeichert
seg-stl-failed = Die Segmente konnten nicht gespeichert werden: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Drei Ebenen (MPR)
toolbar-mpr-close = Drei Ebenen schließen (MPR)
settings-viewing-toolbar-items-mpr = Drei Ebenen (MPR)
