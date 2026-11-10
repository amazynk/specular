# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Porovnat série (propojená zobrazení)
# machine-drafted, needs native review
palette-compare-layout = Další rozvržení zobrazení
# machine-drafted, needs native review
palette-next-viewport = Další zobrazení
# machine-drafted, needs native review
palette-prev-viewport = Předchozí zobrazení
# machine-drafted, needs native review
palette-reference-lines = Zobrazit nebo skrýt referenční čáry
# machine-drafted, needs native review
palette-sync-window = Další synchronizace okna
# machine-drafted, needs native review
palette-sync-zoom = Společné přiblížení a posun podle velikosti
# machine-drafted, needs native review
palette-link-series = Další propojení série
# machine-drafted, needs native review
palette-align-series = Zde zarovnat ostatní série
# machine-drafted, needs native review
palette-compare-series-help = Zobrazit několik sérií vedle sebe, posouvaných společně.
# machine-drafted, needs native review
palette-compare-layout-help = Zobrazit více nebo méně zobrazení.
# machine-drafted, needs native review
palette-next-viewport-help = Nastavit další zobrazení jako aktivní.
# machine-drafted, needs native review
palette-prev-viewport-help = Nastavit předchozí zobrazení jako aktivní.
# machine-drafted, needs native review
palette-reference-lines-help = Vykreslit, kde aktivní řez protíná ostatní zobrazení.
# machine-drafted, needs native review
palette-sync-window-help = Zvolit, která zobrazení sdílejí změnu okna/úrovně: vypnuto, stejná modalita nebo všechna.
# machine-drafted, needs native review
palette-sync-zoom-help = Zobrazit všechna zobrazení ve stejné velikosti a posouvat je společně.
# machine-drafted, needs native review
palette-link-series-help = Zvolit, jak tato série sleduje ostatní.
# machine-drafted, needs native review
palette-align-series-help = Zarovnat dřívější sérii s touto na zobrazených řezech.
# machine-drafted, needs native review
disabled-not-comparing = Nejprve porovnejte série

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Rozvržení: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Referenční čáry
# machine-drafted, needs native review
linked-zoom = Synchronizace přiblížení
# machine-drafted, needs native review
linked-window-sync = Synchronizace okna: { $mode ->
    [off] vypnuto
    [modality] stejná modalita
   *[all] všechny
}
# machine-drafted, needs native review
linked-link = Propojení: { $mode ->
    [frame] referenční systém
    [position] poloha pacienta
   *[off] vypnuto
}
# machine-drafted, needs native review
linked-align = Zarovnat zde

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Zobrazení { $n } z { $count }, aktivní
   *[no] Zobrazení { $n } z { $count }
}
# machine-drafted, needs native review
linked-empty = Přetáhněte sem sérii
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Řez { $n } z { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = V mezeře: nejbližší řez je { $distance } mm daleko
# machine-drafted, needs native review
linked-outside = Mimo tuto sérii ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Nepropojeno
# machine-drafted, needs native review
linked-aligned = Zarovnáno ručně ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = 3D kurzor na { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, obrázků: { $count }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Zde je jen jedna série: přetáhněte další z prohlížeče studií
    [few] Porovnávají se { $count } série
    [many] Porovnává se { $count } série
   *[other] Porovnává se { $count } sérií
}
# machine-drafted, needs native review
linked-no-geometry = Tato série nemá polohy řezů: nelze ji propojit
# machine-drafted, needs native review
linked-active = Zobrazení { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Není co zarovnat: ostatní série sdílejí tento referenční systém
    [one] Zarovnána 1 série s aktivní, pro tuto relaci
    [few] Zarovnány { $count } série s aktivní, pro tuto relaci
    [many] Zarovnáno { $count } série s aktivní, pro tuto relaci
   *[other] Zarovnáno { $count } sérií s aktivní, pro tuto relaci
}
