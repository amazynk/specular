# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Series vergelijken (gekoppelde weergaven)
# machine-drafted, needs native review
palette-compare-layout = Volgende weergave-indeling
# machine-drafted, needs native review
palette-next-viewport = Volgende weergave
# machine-drafted, needs native review
palette-prev-viewport = Vorige weergave
# machine-drafted, needs native review
palette-reference-lines = Referentielijnen tonen of verbergen
# machine-drafted, needs native review
palette-sync-window = Volgende venstersync
# machine-drafted, needs native review
palette-sync-zoom = Samen zoomen en verschuiven op grootte
# machine-drafted, needs native review
palette-link-series = Volgende seriekoppeling
# machine-drafted, needs native review
palette-align-series = De andere series hier uitlijnen
# machine-drafted, needs native review
palette-compare-series-help = Meerdere series naast elkaar tonen, die samen scrollen.
# machine-drafted, needs native review
palette-compare-layout-help = Meer of minder weergaven tonen.
# machine-drafted, needs native review
palette-next-viewport-help = De volgende weergave actief maken.
# machine-drafted, needs native review
palette-prev-viewport-help = De vorige weergave actief maken.
# machine-drafted, needs native review
palette-reference-lines-help = Tekenen waar de actieve plak de andere weergaven snijdt.
# machine-drafted, needs native review
palette-sync-window-help = Kiezen welke weergaven een venster/niveau-wijziging delen: uit, zelfde modaliteit of alle.
# machine-drafted, needs native review
palette-sync-zoom-help = Alle weergaven even groot tonen en samen verschuiven.
# machine-drafted, needs native review
palette-link-series-help = Kiezen hoe deze serie de andere volgt.
# machine-drafted, needs native review
palette-align-series-help = Een eerdere serie met deze uitlijnen op de getoonde plakken.
# machine-drafted, needs native review
disabled-not-comparing = Vergelijk eerst series

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Indeling: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Referentielijnen
# machine-drafted, needs native review
linked-zoom = Zoomsynchronisatie
# machine-drafted, needs native review
linked-window-sync = Venstersynchronisatie: { $mode ->
    [off] uit
    [modality] zelfde modaliteit
   *[all] alle
}
# machine-drafted, needs native review
linked-link = Koppeling: { $mode ->
    [frame] referentiekader
    [position] patiëntpositie
   *[off] uit
}
# machine-drafted, needs native review
linked-align = Hier uitlijnen

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Weergave { $n } van { $count }, actief
   *[no] Weergave { $n } van { $count }
}
# machine-drafted, needs native review
linked-empty = Sleep hier een serie naartoe
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Plak { $n } van { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = In een opening: dichtstbijzijnde plak op { $distance } mm
# machine-drafted, needs native review
linked-outside = Buiten deze serie ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Niet gekoppeld
# machine-drafted, needs native review
linked-aligned = Handmatig uitgelijnd ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = 3D-cursor op { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } afbeelding
       *[other] { $count } afbeeldingen
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Maar één serie hier: sleep er nog een uit de onderzoeksbrowser
   *[other] { $count } series worden vergeleken
}
# machine-drafted, needs native review
linked-no-geometry = Deze serie heeft geen plakposities: ze kan niet worden gekoppeld
# machine-drafted, needs native review
linked-active = Weergave { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Niets uit te lijnen: de andere series delen dit referentiekader
    [one] 1 serie uitgelijnd met de actieve, voor deze sessie
   *[other] { $count } series uitgelijnd met de actieve, voor deze sessie
}
