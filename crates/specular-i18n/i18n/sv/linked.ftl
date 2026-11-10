# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Jämför serier (länkade vyer)
# machine-drafted, needs native review
palette-compare-layout = Nästa vylayout
# machine-drafted, needs native review
palette-next-viewport = Nästa vy
# machine-drafted, needs native review
palette-prev-viewport = Föregående vy
# machine-drafted, needs native review
palette-reference-lines = Visa eller dölj referenslinjer
# machine-drafted, needs native review
palette-sync-window = Nästa fönstersynk
# machine-drafted, needs native review
palette-sync-zoom = Zooma och panorera tillsammans efter storlek
# machine-drafted, needs native review
palette-link-series = Nästa serielänk
# machine-drafted, needs native review
palette-align-series = Justera de andra serierna hit
# machine-drafted, needs native review
palette-compare-series-help = Visa flera serier sida vid sida som rullar tillsammans.
# machine-drafted, needs native review
palette-compare-layout-help = Visa fler eller färre vyer.
# machine-drafted, needs native review
palette-next-viewport-help = Gör nästa vy till den aktiva.
# machine-drafted, needs native review
palette-prev-viewport-help = Gör föregående vy till den aktiva.
# machine-drafted, needs native review
palette-reference-lines-help = Rita var det aktiva snittet skär de andra vyerna.
# machine-drafted, needs native review
palette-sync-window-help = Välj vilka vyer som delar en fönster/nivå-ändring: av, samma modalitet eller alla.
# machine-drafted, needs native review
palette-sync-zoom-help = Visa alla vyer i samma storlek och panorera dem tillsammans.
# machine-drafted, needs native review
palette-link-series-help = Välj hur den här serien följer de andra.
# machine-drafted, needs native review
palette-align-series-help = Justera en tidigare serie mot den här vid de visade snitten.
# machine-drafted, needs native review
disabled-not-comparing = Jämför serier först

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Layout: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Referenslinjer
# machine-drafted, needs native review
linked-zoom = Zoomsynk
# machine-drafted, needs native review
linked-window-sync = Fönstersynk: { $mode ->
    [off] av
    [modality] samma modalitet
   *[all] alla
}
# machine-drafted, needs native review
linked-link = Länk: { $mode ->
    [frame] referensram
    [position] patientposition
   *[off] av
}
# machine-drafted, needs native review
linked-align = Justera hit

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Vy { $n } av { $count }, aktiv
   *[no] Vy { $n } av { $count }
}
# machine-drafted, needs native review
linked-empty = Dra en serie hit
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Snitt { $n } av { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = I ett gap: närmaste snitt { $distance } mm bort
# machine-drafted, needs native review
linked-outside = Utanför den här serien ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Inte länkad
# machine-drafted, needs native review
linked-aligned = Justerad för hand ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = 3D-markör vid { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } bild
       *[other] { $count } bilder
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Bara en serie här: dra hit en till från undersökningsbläddraren
   *[other] Jämför { $count } serier
}
# machine-drafted, needs native review
linked-no-geometry = Den här serien saknar snittpositioner: den kan inte länkas
# machine-drafted, needs native review
linked-active = Vy { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Inget att justera: de andra serierna delar den här referensramen
    [one] 1 serie justerad mot den aktiva, under den här sessionen
   *[other] { $count } serier justerade mot den aktiva, under den här sessionen
}
