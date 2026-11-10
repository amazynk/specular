# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Porównaj serie (połączone widoki)
# machine-drafted, needs native review
palette-compare-layout = Następny układ widoków
# machine-drafted, needs native review
palette-next-viewport = Następny widok
# machine-drafted, needs native review
palette-prev-viewport = Poprzedni widok
# machine-drafted, needs native review
palette-reference-lines = Pokaż lub ukryj linie referencyjne
# machine-drafted, needs native review
palette-sync-window = Następna synchr. okna
# machine-drafted, needs native review
palette-sync-zoom = Wspólne powiększanie i przesuwanie według rozmiaru
# machine-drafted, needs native review
palette-link-series = Następne powiązanie serii
# machine-drafted, needs native review
palette-align-series = Wyrównaj tutaj pozostałe serie
# machine-drafted, needs native review
palette-compare-series-help = Pokaż kilka serii obok siebie, przewijanych razem.
# machine-drafted, needs native review
palette-compare-layout-help = Pokaż więcej lub mniej widoków.
# machine-drafted, needs native review
palette-next-viewport-help = Uczyń następny widok aktywnym.
# machine-drafted, needs native review
palette-prev-viewport-help = Uczyń poprzedni widok aktywnym.
# machine-drafted, needs native review
palette-reference-lines-help = Narysuj, gdzie aktywny przekrój przecina inne widoki.
# machine-drafted, needs native review
palette-sync-window-help = Wybierz, które widoki współdzielą zmianę okna/poziomu: wyłączona, ta sama modalność lub wszystkie.
# machine-drafted, needs native review
palette-sync-zoom-help = Pokaż wszystkie widoki w tym samym rozmiarze i przesuwaj je razem.
# machine-drafted, needs native review
palette-link-series-help = Wybierz, jak ta seria podąża za pozostałymi.
# machine-drafted, needs native review
palette-align-series-help = Wyrównaj wcześniejszą serię z tą na pokazanych przekrojach.
# machine-drafted, needs native review
disabled-not-comparing = Najpierw porównaj serie

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Układ: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Linie referencyjne
# machine-drafted, needs native review
linked-zoom = Synchronizacja powiększenia
# machine-drafted, needs native review
linked-window-sync = Synchronizacja okna: { $mode ->
    [off] wyłączona
    [modality] ta sama modalność
   *[all] wszystkie
}
# machine-drafted, needs native review
linked-link = Powiązanie: { $mode ->
    [frame] układ odniesienia
    [position] pozycja pacjenta
   *[off] wyłączone
}
# machine-drafted, needs native review
linked-align = Wyrównaj tutaj

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Widok { $n } z { $count }, aktywny
   *[no] Widok { $n } z { $count }
}
# machine-drafted, needs native review
linked-empty = Przeciągnij tutaj serię
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Przekrój { $n } z { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = W przerwie: najbliższy przekrój w odległości { $distance } mm
# machine-drafted, needs native review
linked-outside = Poza tą serią ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Niepowiązana
# machine-drafted, needs native review
linked-aligned = Wyrównana ręcznie ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = Kursor 3D w { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, obrazów: { $count }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Tylko jedna seria: przeciągnij kolejną z przeglądarki badań
    [few] Porównywanie { $count } serii
    [many] Porównywanie { $count } serii
   *[other] Porównywanie { $count } serii
}
# machine-drafted, needs native review
linked-no-geometry = Ta seria nie ma pozycji przekrojów: nie można jej powiązać
# machine-drafted, needs native review
linked-active = Widok { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Nic do wyrównania: pozostałe serie mają ten sam układ odniesienia
    [one] Wyrównano 1 serię z aktywną, na czas tej sesji
    [few] Wyrównano { $count } serie z aktywną, na czas tej sesji
    [many] Wyrównano { $count } serii z aktywną, na czas tej sesji
   *[other] Wyrównano { $count } serii z aktywną, na czas tej sesji
}
