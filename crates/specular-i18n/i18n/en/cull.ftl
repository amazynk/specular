# Specular — Cull mode (DESIGN.md §6.3). English.
# The strip and the mark confirmations reuse the gallery's mark names (gallery-pick, …).

palette-cull = Enter or leave cull mode
cheatsheet-cull = Cull mode

## Settings → Cull mode
settings-section-cull = Cull mode
settings-cull-auto-enter = Cull RAW folders
settings-cull-auto-enter-help = Open a folder that is mostly RAW files straight into cull mode.
settings-cull-auto-advance = Next image after marking
settings-cull-auto-advance-help = Move on after a pick, a reject or a star rating. Labels and unflagging stay on the image.
settings-cull-show = Show for RAW+JPEG pairs
settings-cull-show-help = A RAW and a JPEG with the same name are culled as one image, and marks apply to both. The JPEG opens faster.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW
settings-cull-bursts = Stack bursts
settings-cull-bursts-help = Shots taken in quick succession by one camera are shown as one stack. S opens or closes a stack, B keeps the image on screen and rejects the rest of its stack.
settings-cull-burst-gap = Burst gap
settings-cull-burst-gap-help = The longest pause between two shots of one burst.
settings-cull-watch = Show new files
settings-cull-watch-help = Files that appear in the folder while culling (a tethered camera, an import) are added at the end once they are completely written.
settings-cull-follow-newest = Follow the newest file
settings-cull-follow-newest-help = Jump to each new file as it arrives. T switches this while culling.
settings-cull-slot-1 = Folder 1
settings-cull-slot-1-help = Alt+1 copies the image (with its RAW or JPEG and its sidecars) to this folder, Alt+Shift+1 moves it there.
settings-cull-slot-1-unset = Not set
settings-cull-slot-2 = Folder 2
settings-cull-slot-2-help = Alt+2 copies the image (with its RAW or JPEG and its sidecars) to this folder, Alt+Shift+2 moves it there.
settings-cull-slot-2-unset = Not set
settings-cull-slot-3 = Folder 3
settings-cull-slot-3-help = Alt+3 copies the image (with its RAW or JPEG and its sidecars) to this folder, Alt+Shift+3 moves it there.
settings-cull-slot-3-unset = Not set
settings-cull-collisions = When a name is taken
settings-cull-collisions-help = Files in the folders are never replaced.
settings-cull-collisions-suffix = Add a number
settings-cull-collisions-skip = Skip the image

## Copy and move to folders
# Cull mode chosen with no folder or gallery images open.
cull-nothing = Nothing to cull — open a folder of photos first
cull-slot-unset = No folder { $slot } set — choose one in Settings → Cull mode
cull-slot-missing = Folder { $slot } is not available: { $reason }
cull-copied = Copied to { $folder }
cull-moved = Moved to { $folder }
cull-slot-renamed = Saved as { $name } in { $folder }
cull-slot-skipped = { $folder } already has { $name } — skipped
cull-slot-same = The image is already in { $folder }
cull-slot-failed = Could not { $op ->
    [move] move
   *[copy] copy
} { $file }: { $reason }
cull-copy-kept = Kept { $count ->
    [one] a copy that was changed since
   *[other] { $count } copies that were changed since
}
cull-undo-failed = Could not undo for { $file }: { $reason }

## Bursts
cull-burst-open = Burst { $index } of { $count }
cull-burst-closed = Burst of { $count }
cull-burst-toggle = Open or close the burst
cull-burst-best = Kept 1, rejected { $count }
cull-burst-none = Not part of a burst

## Hot folder
cull-follow-on = Following new files
cull-follow-off = Not following new files
cull-new-files = { $count ->
    [one] 1 new file
   *[other] { $count } new files
}

## Cheat sheet
cull-key-copy = Copy to folder 1–3
cull-key-move = Move to folder 1–3
cull-key-burst = Open or close a burst
cull-key-best = Keep this, reject the rest of the burst
cull-key-follow = Follow new files
cull-key-subframes = Show or hide the subframe table
cull-key-sort = Sort by the next column
cull-key-sort-reverse = Reverse the order
cull-key-metric = Chart the next metric
cull-key-sigma-down = Auto-reject: lower k
cull-key-sigma-up = Auto-reject: raise k
cull-key-auto-reject = Reject the suggested frames
cull-key-move-rejects = Move rejected frames to a subfolder
cull-key-same-stretch = Same stretch for every frame

## Strip tooltips (the key follows in brackets, see tooltip-with-shortcut)
cull-tip-burst-open = Open the burst
cull-tip-burst-close = Close the burst
cull-tip-exit = Leave cull mode
# $label is a colour label's name (gallery-label-*).
cull-tip-label = Label: { $label }

## Astronomy subframes
cull-astro-panel = Subframes
cull-astro-title = { $count ->
    [one] 1 subframe
   *[other] { $count } subframes
}
cull-astro-pending = measuring { $count }
cull-astro-col-name = Frame
cull-astro-col-filter = Filter
cull-astro-col-exposure = Exp.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Ecc.
cull-astro-col-stars = Stars
cull-astro-col-bg = Background
cull-astro-col-snr = SNR
cull-astro-col-noise = Noise
cull-astro-same-stretch = Same stretch
cull-astro-own-stretch = Own stretch
# $k is the number of standard deviations from the median.
cull-astro-suggest-high = { $metric } above { $value } (median + { $k }σ): { $count ->
    [one] 1 frame
   *[other] { $count } frames
}
cull-astro-suggest-low = { $metric } below { $value } (median − { $k }σ): { $count ->
    [one] 1 frame
   *[other] { $count } frames
}
cull-astro-suggest-none = No frame beyond { $k }σ
cull-astro-suggest-few = Auto-reject needs 3 measured frames
cull-astro-suggested = suggested for rejection
cull-astro-k = k { $k }
cull-astro-reject = Reject { $count }
cull-astro-move = Move rejected…
cull-astro-chart = { $metric } over { $count ->
        [one] { $count } frame
       *[other] { $count } frames
    }, median { $median }.
cull-astro-sort-by = Sort by { $column }
cull-astro-ascending = ascending
cull-astro-descending = descending
cull-astro-sorted = Sorted by { $column }
cull-astro-reversed = Order reversed
cull-astro-metric = Chart: { $metric }
cull-astro-k-set = Auto-reject at { $k }σ
cull-astro-none = No subframes here
cull-astro-same-on = Same stretch for every frame
cull-astro-same-off = Each frame stretched on its own
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = eccentricity { $value }
cull-astro-line-stars = { $count ->
    [one] 1 star
   *[other] { $count } stars
}
cull-astro-measuring = measuring stars…
cull-astro-reject-confirm = { $count ->
    [one] Reject the suggested frame?
   *[other] Reject the { $count } suggested frames?
}
cull-astro-reject-detail = { $rule }. Undo brings the marks back.
cull-astro-reject-ok = Reject
cull-astro-nothing-to-reject = No frame to reject
cull-astro-rejected = { $count ->
    [one] Rejected 1 frame
   *[other] Rejected { $count } frames
}
cull-move-rejects-confirm = { $count ->
    [one] Move the rejected frame to the folder “{ $folder }”?
   *[other] Move the { $count } rejected frames to the folder “{ $folder }”?
}
cull-move-rejects-detail = The files are moved, not deleted. Undo puts them back.
cull-move-rejects-ok = Move
cull-move-rejects-none = No rejected frames to move
cull-moved-rejects = { $count ->
    [one] Moved 1 frame to “{ $folder }”
   *[other] Moved { $count } frames to “{ $folder }”
}
