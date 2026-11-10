# Develop: history, snapshots, styles, presets, versions, copy and paste, match.

## Actions
palette-develop-copy = Copy edit
palette-develop-copy-help = Copy the photo's edit, all but crop and lens, to paste on other photos.
palette-develop-copy-some = Copy some modules…
palette-develop-copy-some-help = Choose which modules of the photo's edit to copy.
palette-develop-paste = Paste edit
palette-develop-paste-help = Paste the copied edit on the photo, or on the photos selected in the gallery.
palette-develop-paste-some = Paste some modules…
palette-develop-paste-some-help = Choose which copied modules to paste, adding to the edit or replacing.
palette-develop-paste-picks = Paste edit on picks
palette-develop-paste-picks-help = Paste the copied edit on every picked photo.
palette-develop-reference = Use as match reference
palette-develop-reference-help = Other photos can then match this photo's exposure and white balance.
palette-develop-match = Match to reference
palette-develop-match-help = Set the exposure and white balance of the photo, the selection or the picks to match the reference.
palette-develop-sync = Sync edit to picks
palette-develop-sync-help = Put this photo's edit, all but crop and lens, on the picked photos.
palette-develop-sync-some = Sync some modules to picks…
palette-develop-sync-some-help = Choose which modules of this photo's edit go on the picked photos.
palette-develop-next-capture = Next capture style…
palette-develop-next-capture-help = The style each new file arriving in the folder gets.
palette-develop-auto-rules = Auto-apply rules…
palette-develop-auto-rules-help = The rules that apply a style when a photo is first opened in Develop.
palette-develop-import-style = Import style…
palette-develop-import-style-help = Add a style from a file.
palette-develop-snapshot = Take snapshot
palette-develop-snapshot-help = Name the edit as it is now, to compare with later or go back to.
palette-develop-compress = Compress history
palette-develop-compress-help = Drop history steps that later ones replaced. The edit stays the same.
palette-develop-new-version = New version
palette-develop-new-version-help = A new edit of the same photo, with its own history. The file is not copied.
palette-develop-new-style = New style…
palette-develop-new-style-help = Save chosen modules of the edit as a style.

## Panel
develop-styles = Styles
develop-snapshots = History and snapshots
develop-versions = Versions
develop-style-new = New style from this edit…
develop-style-import = Import style…
develop-rules = Auto-apply rules…
develop-snapshot-take = Take snapshot
develop-history-compress = Compress history
develop-snapshot-comparing = Comparing: { $name }
develop-snapshot-compare-off = Comparison off
develop-version-number = Version { $number }
develop-version-open = { $name } (open)
develop-version-new = New version
develop-version-duplicate = Duplicate this version

## History rows
develop-history-paste = Paste
develop-history-sync = Sync
develop-history-match = Match to reference
develop-history-snapshot = Snapshot: { $name }
develop-history-previous = Same as previous

## Menus
develop-rename = Rename…
develop-delete = Delete
develop-save = Save
develop-name = Name
develop-style-apply = Apply
develop-style-apply-replace = Apply, replacing these modules
develop-style-auto = Apply to new photos from this camera
develop-style-export = Export…
develop-snapshot-restore = Go back to this snapshot
develop-version-open-it = Open
develop-preset-save = Save as preset…
develop-preset-save-module = Save { $module } as preset…
develop-preset-delete = Delete “{ $name }”…

## Sheets
develop-sheet-modules = Modules
develop-sheet-included = Included
develop-sheet-left-out = Left out
develop-sheet-more = { $count } more
develop-copy-title = Copy modules
develop-copy-ok = Copy
develop-paste-title = Paste modules
develop-paste-ok = Paste
develop-paste-mode = Modules already in the edit
develop-paste-append = Add
develop-paste-replace = Replace
develop-style-new-title = New style
develop-style-save = Save style
develop-style-default-name = My style { $number }
develop-style-rename-title = Rename style
develop-preset-save-title = { $module } preset
develop-preset-default-name = My preset
develop-snapshot-take-title = Take snapshot
develop-snapshot-default-name = Snapshot { $number }
develop-snapshot-rename-title = Rename snapshot
develop-version-new-title = New version
develop-version-duplicate-title = Duplicate version
develop-version-create = Create
develop-version-rename-title = Rename version

## Questions
develop-style-delete-ask = Delete the style “{ $name }”?
develop-style-delete-detail = Photos it was applied to keep their edits. The style file goes.
develop-preset-delete-ask = Delete the preset “{ $name }”?
develop-preset-delete-detail = Edits it was applied to stay as they are.
develop-snapshot-delete-ask = Delete the snapshot “{ $name }”?
develop-snapshot-delete-detail = The edit and its history stay as they are.
develop-version-delete-ask = Delete “{ $name }”?
develop-version-delete-detail = This version's edit and history go. The photo and its other versions stay on disk.
develop-rule-delete-ask = Delete the rule “{ $rule }”?
develop-rule-delete-detail = The style stays, and photos already opened keep their edits.

## Rules
develop-rule-text = { $style }: { $when }
develop-rule-every-photo = every photo
develop-rule-iso-min = ISO { $iso } and up
develop-rule-iso-max = ISO up to { $iso }
develop-rule-iso-range = ISO { $low }–{ $high }
develop-rule-raw = raw files
develop-rule-other = not raw
develop-rule-any-camera = any camera
develop-next-placeholder = Style for each new file
develop-next-off-row = None
develop-next-previous-row = Same as the previous photo

## Messages
develop-copied = { $count ->
    [one] Copied 1 module from { $name }
   *[other] Copied { $count } modules from { $name }
}
develop-pasted = { $count ->
    [one] Pasted on 1 photo
   *[other] Pasted on { $count } photos
}
# The downloads indicator's row while edits are saved to photos that are not open.
develop-batch-row = Saving edits
develop-batch-row-detail = { $done } of { $total } photos
develop-batch-saving = Saving
develop-batch-stopping = Stopping
develop-batch-stopped = Stopped. Saved { $done } of { $total } photos; the rest are unchanged.
develop-nothing-to-copy = No photo to copy from.
develop-nothing-copied = Nothing copied yet. Copy an edit first (Ctrl+Alt+C).
develop-no-photos = No photos to change.
develop-no-picks = No picked photos.
develop-reference-set = Reference: { $name }
develop-no-reference = No reference yet. Open the reference photo and choose Use as match reference (Ctrl+Alt+R).
develop-reference-unreadable = The reference photo could not be read.
develop-matching = { $count ->
    [one] Matching 1 photo…
   *[other] Matching { $count } photos…
}
develop-matched = { $count ->
    [one] Matched 1 photo
   *[other] Matched { $count } photos
}
develop-matched-some = Matched { $count }. { $failed } could not be read or measured.
develop-style-saved = Style saved: { $name }
develop-style-applied = Style applied: { $name }
develop-style-imported = Style imported: { $name }
develop-style-not-a-style = This file is not a style.
develop-style-exported = Style exported: { $name }
develop-style-export-failed = The style could not be written there.
develop-style-import-dialog = Import a style
develop-style-export-dialog = Export the style
develop-style-file-kind = Style
develop-preset-saved = Preset saved: { $name }
develop-preset-applied = Preset applied: { $name }
develop-snapshot-taken = Snapshot taken: { $name }
develop-history-compressed = { $count ->
    [0] Nothing to compress
    [one] 1 step dropped
   *[other] { $count } steps dropped
}
develop-version-created = Version created: { $name }
develop-version-opened = Version open: { $name }
develop-version-deleted = Version deleted: { $name }
develop-rule-added = { $name } applies to new photos from { $camera }.
develop-rule-applied = Auto-apply rule: { $name }
develop-rules-none = No auto-apply rules. Add one from a style's menu.
develop-next-off = New files get no style.
develop-next-previous = New files get the previous photo's edit.
develop-next-style = New files get the style { $name }.
develop-next-applied = { $count ->
    [one] Next capture style on 1 new file
   *[other] Next capture style on { $count } new files
}

## Shipped styles and presets
develop-style-specular = Specular punchy
develop-style-soft = Soft
develop-style-vivid = Vivid
develop-style-mono = Black and white
develop-preset-plus-one = Brighter (+1 EV)
develop-preset-minus-one = Darker (−1 EV)
develop-preset-low-contrast = Low contrast
develop-preset-high-contrast = High contrast
develop-preset-open-shadows = Open shadows
develop-preset-crisp = Crisp
develop-preset-muted = Muted
develop-preset-high-iso = High ISO
develop-preset-screen = For screens

## Gallery
gallery-badge-versions = { $count } versions
