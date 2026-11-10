# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Sammenlign serier (sammenkædede visninger)
# machine-drafted, needs native review
palette-compare-layout = Næste visningslayout
# machine-drafted, needs native review
palette-next-viewport = Næste visning
# machine-drafted, needs native review
palette-prev-viewport = Forrige visning
# machine-drafted, needs native review
palette-reference-lines = Vis eller skjul referencelinjer
# machine-drafted, needs native review
palette-sync-window = Næste vinduessynk
# machine-drafted, needs native review
palette-sync-zoom = Zoom og panorér sammen efter størrelse
# machine-drafted, needs native review
palette-link-series = Næste seriekæde
# machine-drafted, needs native review
palette-align-series = Juster de andre serier hertil
# machine-drafted, needs native review
palette-compare-series-help = Vis flere serier side om side, der ruller sammen.
# machine-drafted, needs native review
palette-compare-layout-help = Vis flere eller færre visninger.
# machine-drafted, needs native review
palette-next-viewport-help = Gør den næste visning aktiv.
# machine-drafted, needs native review
palette-prev-viewport-help = Gør den forrige visning aktiv.
# machine-drafted, needs native review
palette-reference-lines-help = Tegn, hvor det aktive snit skærer de andre visninger.
# machine-drafted, needs native review
palette-sync-window-help = Vælg, hvilke visninger der deler en vindue/niveau-ændring: fra, samme modalitet eller alle.
# machine-drafted, needs native review
palette-sync-zoom-help = Vis alle visninger i samme størrelse, og panorér dem sammen.
# machine-drafted, needs native review
palette-link-series-help = Vælg, hvordan denne serie følger de andre.
# machine-drafted, needs native review
palette-align-series-help = Juster en tidligere serie efter denne ved de viste snit.
# machine-drafted, needs native review
disabled-not-comparing = Sammenlign serier først

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Layout: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Referencelinjer
# machine-drafted, needs native review
linked-zoom = Zoomsynk
# machine-drafted, needs native review
linked-window-sync = Vinduessynk: { $mode ->
    [off] fra
    [modality] samme modalitet
   *[all] alle
}
# machine-drafted, needs native review
linked-link = Kæde: { $mode ->
    [frame] referencesystem
    [position] patientposition
   *[off] fra
}
# machine-drafted, needs native review
linked-align = Juster hertil

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Visning { $n } af { $count }, aktiv
   *[no] Visning { $n } af { $count }
}
# machine-drafted, needs native review
linked-empty = Træk en serie hertil
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Snit { $n } af { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = I et hul: nærmeste snit { $distance } mm væk
# machine-drafted, needs native review
linked-outside = Uden for denne serie ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Ikke sammenkædet
# machine-drafted, needs native review
linked-aligned = Justeret manuelt ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = 3D-markør ved { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } billede
       *[other] { $count } billeder
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Kun én serie her: træk en til fra undersøgelsesbrowseren
   *[other] Sammenligner { $count } serier
}
# machine-drafted, needs native review
linked-no-geometry = Denne serie har ingen snitpositioner: den kan ikke sammenkædes
# machine-drafted, needs native review
linked-active = Visning { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Intet at justere: de andre serier deler dette referencesystem
    [one] 1 serie justeret efter den aktive, for denne session
   *[other] { $count } serier justeret efter den aktive, for denne session
}
