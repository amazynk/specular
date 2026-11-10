# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Serien vergleichen (verknüpfte Ansichten)
# machine-drafted, needs native review
palette-compare-layout = Nächstes Ansichtslayout
# machine-drafted, needs native review
palette-next-viewport = Nächste Ansicht
# machine-drafted, needs native review
palette-prev-viewport = Vorherige Ansicht
# machine-drafted, needs native review
palette-reference-lines = Referenzlinien ein- oder ausblenden
# machine-drafted, needs native review
palette-sync-window = Nächster Fenster-Sync
# machine-drafted, needs native review
palette-sync-zoom = Zoom und Verschieben gemeinsam nach Größe
# machine-drafted, needs native review
palette-link-series = Nächste Serienverknüpfung
# machine-drafted, needs native review
palette-align-series = Die anderen Serien hier ausrichten
# machine-drafted, needs native review
palette-compare-series-help = Mehrere Serien nebeneinander zeigen, die gemeinsam scrollen.
# machine-drafted, needs native review
palette-compare-layout-help = Mehr oder weniger Ansichten zeigen.
# machine-drafted, needs native review
palette-next-viewport-help = Die nächste Ansicht zur aktiven machen.
# machine-drafted, needs native review
palette-prev-viewport-help = Die vorherige Ansicht zur aktiven machen.
# machine-drafted, needs native review
palette-reference-lines-help = Zeichnen, wo die aktive Schicht die anderen Ansichten schneidet.
# machine-drafted, needs native review
palette-sync-window-help = Wählen, welche Ansichten eine Fenster/Level-Änderung teilen: aus, gleiche Modalität oder alle.
# machine-drafted, needs native review
palette-sync-zoom-help = Alle Ansichten in gleicher Größe zeigen und gemeinsam verschieben.
# machine-drafted, needs native review
palette-link-series-help = Wählen, wie diese Serie den anderen folgt.
# machine-drafted, needs native review
palette-align-series-help = Eine frühere Untersuchung an dieser Serie bei den gezeigten Schichten ausrichten.
# machine-drafted, needs native review
disabled-not-comparing = Zuerst Serien vergleichen

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Layout: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Referenzlinien
# machine-drafted, needs native review
linked-zoom = Zoom-Sync
# machine-drafted, needs native review
linked-window-sync = Fenster-Sync: { $mode ->
    [off] aus
    [modality] gleiche Modalität
   *[all] alle
}
# machine-drafted, needs native review
linked-link = Verknüpfung: { $mode ->
    [frame] Bezugssystem
    [position] Patientenposition
   *[off] aus
}
# machine-drafted, needs native review
linked-align = Hier ausrichten

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Ansicht { $n } von { $count }, aktiv
   *[no] Ansicht { $n } von { $count }
}
# machine-drafted, needs native review
linked-empty = Serie hierher ziehen
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Schicht { $n } von { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = In einer Lücke: nächste Schicht { $distance } mm entfernt
# machine-drafted, needs native review
linked-outside = Außerhalb dieser Serie ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Nicht verknüpft
# machine-drafted, needs native review
linked-aligned = Von Hand ausgerichtet ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = 3D-Cursor bei { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } Bild
       *[other] { $count } Bilder
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Nur eine Serie hier: eine weitere aus dem Studienbrowser ziehen
   *[other] { $count } Serien werden verglichen
}
# machine-drafted, needs native review
linked-no-geometry = Diese Serie hat keine Schichtpositionen: Sie kann nicht verknüpft werden
# machine-drafted, needs native review
linked-active = Ansicht { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Nichts auszurichten: Die anderen Serien teilen dieses Bezugssystem
    [one] 1 Serie an der aktiven ausgerichtet, für diese Sitzung
   *[other] { $count } Serien an der aktiven ausgerichtet, für diese Sitzung
}
