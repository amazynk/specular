# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

palette-compare-series = Compare series (linked views)
palette-compare-layout = Next viewport layout
palette-next-viewport = Next viewport
palette-prev-viewport = Previous viewport
palette-reference-lines = Show or hide reference lines
palette-sync-window = Next window sync
palette-sync-zoom = Zoom and pan together by size
palette-link-series = Next series link
palette-align-series = Align the other series here
palette-compare-series-help = Show several series side by side, scrolling together.
palette-compare-layout-help = Show more or fewer viewports.
palette-next-viewport-help = Make the next viewport the active one.
palette-prev-viewport-help = Make the previous viewport the active one.
palette-reference-lines-help = Draw where the active slice cuts the other viewports.
palette-sync-window-help = Choose which viewports share a window/level change: off, same modality or all.
palette-sync-zoom-help = Show every viewport at the same size, and pan them together.
palette-link-series-help = Choose how this series follows the others.
palette-align-series-help = Line up a prior with this series at the slices shown.
disabled-not-comparing = Compare series first

# The bar above the viewports. Toggles show their state by colour; the others name it.
linked-layout = Layout: { $cols } × { $rows }
linked-lines = Reference lines
linked-zoom = Zoom sync
linked-window-sync = Window sync: { $mode ->
    [off] off
    [modality] same modality
   *[all] all
}
linked-link = Link: { $mode ->
    [frame] frame of reference
    [position] patient position
   *[off] off
}
linked-align = Align here

# One viewport. $active is "yes" for the active one.
linked-viewport = { $active ->
    [yes] Viewport { $n } of { $count }, active
   *[no] Viewport { $n } of { $count }
}
linked-empty = Drag a series here
linked-series = { $number } · { $modality } · { $description }
linked-slice = Slice { $n } of { $count } · { $position } mm
linked-in-gap = In a gap: nearest slice { $distance } mm away
linked-outside = Outside this series ({ $distance } mm)
linked-not-linked = Not linked
linked-aligned = Aligned by hand ({ $offset } mm)
linked-cursor = 3D cursor at { $x }, { $y }, { $z } mm
linked-chip = { $label }, { $count ->
        [one] { $count } image
       *[other] { $count } images
    }

# Toasts and announcements.
linked-opened = { $count ->
    [one] Only one series here: drag another from the Studies browser
   *[other] Comparing { $count } series
}
linked-no-geometry = This series has no slice positions: it can’t be linked
linked-active = Viewport { $n }: { $series }
linked-aligned-count = { $count ->
    [0] Nothing to align: the other series share this frame of reference
    [one] Aligned 1 series with the active one, for this session
   *[other] Aligned { $count } series with the active one, for this session
}
