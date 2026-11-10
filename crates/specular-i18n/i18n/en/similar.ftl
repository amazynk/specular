# Specular — Duplicates and similar shots (gallery categories). English.

similar-duplicates = Duplicates
similar-similar = Similar shots

## View menu and palette entries
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } group
   *[other] { $groups } groups
}, { $size } to free
similar-menu-scanning = { $name } — scanning ({ PERCENT($percent) })
similar-menu-none = { $name } — none found

## Header
similar-summary = { $groups ->
    [one] { $groups } group to review
   *[other] { $groups } groups to review
}, { $files ->
    [one] { $files } file
   *[other] { $files } files
} to discard, { $size } to free
similar-progress = { $resolved } of { $total } resolved
similar-skipped = { $count } skipped
similar-scanning = Looking for duplicates and similar shots… { $done } of { $total }
# After the groups are shown: best-pick measures each shot's sharpness at full resolution.
similar-checking-sharpness = Checking sharpness… { $done } of { $total }
# The list while the first scan runs (its progress is in the header above).
similar-waiting = Groups appear here when the scan is done.
# The user stopped the scan (the header's Cancel, or Esc) before anything was grouped.
similar-stopped = Scan stopped: nothing grouped
# Stopped while checking sharpness: the groups stay, their best picks judged from thumbnails.
similar-stopped-sharpness = Sharpness check stopped: best picks from thumbnails
similar-stopped-empty = The scan was stopped. What it measured is kept: Scan again goes on from there.
# Starts a stopped scan again.
similar-scan-again = Scan again
similar-empty-duplicates = No duplicates in this folder.
similar-empty-similar = No similar shots in this folder.
similar-all-done = All groups reviewed.

## Similar shots: how alike photos must be to group
similar-sensitivity = Sensitivity
similar-sensitivity-strict = Strict
similar-sensitivity-loose = Loose

## Footer of the group list
similar-accept-all = Accept all suggestions
similar-show-skipped = Show skipped groups
similar-discard-with = Discard: { $action }
similar-action-trash = Move to trash
similar-action-move = Move to a folder…
similar-action-move-to = Move to “{ $folder }”
similar-action-reject = Flag as rejected only
similar-choose-folder = Choose folder for discarded files…

## Bulk confirmation
similar-confirm = Accept the suggestions of { $groups ->
    [one] { $groups } group
   *[other] { $groups } groups
}? { $files ->
    [one] { $files } file
   *[other] { $files } files
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Accept and discard { $files } file
   *[other] Accept and discard { $files } files
}
similar-cancel = Cancel

## A group
similar-group-title = Group { $index } of { $total }
similar-group-photos = { $count ->
    [one] { $count } photo
   *[other] { $count } photos
}
similar-to-free = { $size } to free

## Review actions and states
similar-keep = Keep
similar-discard = Discard
similar-undecided = Undecided
similar-accept-next = Accept and next
similar-not-similar = Not similar
similar-skip = Skip
similar-compare = Compare
similar-zoom = { PERCENT(100) }
similar-back = Back
similar-done = Done
similar-suggested-keep = Suggested: keep
similar-suggested-discard = Suggested: discard

## Toasts
similar-toast-accepted = { $count ->
    [one] Accepted { $count } group
   *[other] Accepted { $count } groups
}
similar-toast-rejected = { $count ->
    [one] { $count } file flagged as rejected
   *[other] { $count } files flagged as rejected
}
similar-toast-protected = Rated, flagged, labelled, tagged or edited photos are always kept.
# Why "100 %" (similar-zoom) is unavailable: the photo is smaller than its pane, so it is already shown at 100 % or enlarged.
similar-zoom-already = This photo already shows at { PERCENT(100) } or larger

## Accessibility
similar-a11y-member = { $name }: { $state }, { $reason }

## Key hints
similar-hint-list = ↑↓ choose · Enter review · A accept all · Tab switch · Esc close

## Why a photo is kept
similar-reason-rated = Rated
similar-reason-flagged = Flagged as a pick
similar-reason-labelled = Has a colour label
similar-reason-tagged = Has keywords
similar-reason-edited = Has edits
similar-reason-original = The original
similar-reason-raw-original = RAW original
similar-reason-highest-resolution = Highest resolution
similar-reason-deeper-colour = Deeper colour
similar-reason-least-compressed = Least compressed
similar-reason-has-metadata = Keeps its metadata
similar-reason-oldest = Oldest copy
similar-reason-sharpest = Sharpest
similar-reason-steadiest = Least motion blur
similar-reason-best-exposure = Best exposed
similar-reason-least-noise = Least noise
similar-reason-also-good = Also a good one

## Why a photo is suggested for discarding
similar-reason-same-file = Identical file
similar-reason-derivative = Derived copy
similar-reason-rendering-of-raw = Rendering of the RAW
similar-reason-smaller-copy = Smaller copy
similar-reason-shallower-colour = Less colour depth
similar-reason-more-compressed = More compressed
similar-reason-no-metadata = Metadata stripped
similar-reason-newer-copy = Newer copy
similar-reason-blurrier = Blurrier
similar-reason-motion-blur = Motion blur
similar-reason-overexposed = Overexposed
similar-reason-underexposed = Underexposed
similar-reason-noisier = Noisier
similar-reason-lower-resolution = Lower resolution
