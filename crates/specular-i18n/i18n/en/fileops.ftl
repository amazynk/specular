# File operations: rename, batch rename, copy and move to a folder, rotation saved to the
# file, undo and redo, set as wallpaper, share, send by email, settings export and import.

## Palette, cheat sheet and menus

palette-undo = Undo
palette-undo-help = Take back the last change to files: a move to the trash, a rename, a copy or move, a rotation.
palette-redo = Redo
palette-redo-help = Do again what the last undo took back.
palette-rename = Rename…
palette-rename-help = Give the file a new name; its sidecars and RAW+JPEG partner follow.
palette-batch-rename = Rename files…
palette-batch-rename-help = Rename the selected files, or the whole folder, from a pattern, with a preview.
palette-copy-to-folder = Copy to folder…
palette-copy-to-folder-help = Copy the file or the selection to another folder.
palette-move-to-folder = Move to folder…
palette-move-to-folder-help = Move the file or the selection to another folder.
palette-rotate-file-left = Rotate file left
palette-rotate-file-left-help = Turn the file itself a quarter to the left, losslessly where the format allows.
palette-rotate-file-right = Rotate file right
palette-rotate-file-right-help = Turn the file itself a quarter to the right, losslessly where the format allows.
palette-set-wallpaper-help = Make the image your desktop background.
palette-share = Share…
palette-share-help = Send the file to another app with your system’s share menu.
palette-send-by-email = Send by email…
palette-send-by-email-help = Start a new email in your mail app with the file attached.
palette-share-with-zynk = Share with Zynk…
palette-share-with-zynk-help = Make a web link to the file in the Zynk app.
palette-send-with-zynk = Send with Zynk…
palette-send-with-zynk-help = Send the file to a contact or one of your devices in the Zynk app.
palette-export-settings = Export settings…
palette-export-settings-help = Save all settings and your key bindings to a file.
palette-import-settings = Import settings…
palette-import-settings-help = Read settings from a file, after showing what would change.

disabled-in-archive-file = This page is inside an archive and has no file of its own
disabled-not-rotatable = This kind of file can’t record a rotation
disabled-not-on-this-desktop = Your desktop doesn’t offer this

## Settings

settings-browsing-rotate-trim-edges = Trim JPEG edges to rotate losslessly
settings-browsing-rotate-trim-edges-help = A JPEG whose size isn’t a multiple of 8 or 16 pixels can only be turned without loss by cutting off its last few pixels. Without this, its orientation tag is changed instead, and you are offered the trim each time.
settings-browsing-rotate-keep-modified = Keep the modification date when rotating
settings-browsing-rotate-keep-modified-help = A rotated file keeps the date it was last changed, so it stays where it was when sorted by date.
settings-browsing-recent-folders = Recent folders
settings-browsing-recent-folders-help = Folders used last for copy and move.

## Sheets

fileop-cancel = Cancel
rename-title = Rename
rename-field-label = Name
rename-button = Rename
rename-sidecars-follow = Also renamed: { $files }
rename-extension-changed = Changing the extension doesn’t convert the file; other apps may not open it.
rename-problem-empty = Type a name.
rename-problem-invalid-char = Names can’t contain “{ $char }”.
rename-problem-reserved = “{ $name }” is a name Windows keeps for devices.
rename-problem-too-long = That name is too long; names can have up to 255 bytes.
rename-problem-dot = “.” and “..” can’t be used as names.
rename-problem-trailing = Names can’t end with a space or a dot.
rename-problem-exists = Another file already has this name.
rename-problem-read-only = This folder can’t be changed.
rename-problem-duplicate = Two files would get this name.

batch-rename-title =
    { $count ->
        [one] Rename { $count } file
       *[other] Rename { $count } files
    }
batch-rename-field-label = Pattern
batch-rename-tokens = Use { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } and { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } file will be renamed
       *[other] { $count } files will be renamed
    }{ $problems ->
        [0] .
        [one] ; { $problems } can’t be and stays as it is.
       *[other] ; { $problems } can’t be and stay as they are.
    }
batch-rename-reading = Reading dates and cameras…
batch-rename-preview = Preview
batch-rename-unchanged = keeps its name
batch-rename-more =
    { $count ->
        [one] { $count } more below
       *[other] { $count } more below
    }

transfer-copy-title-one = Copy “{ $file }” to a folder
transfer-copy-title =
    { $count ->
        [one] Copy { $count } file to a folder
       *[other] Copy { $count } files to a folder
    }
transfer-move-title-one = Move “{ $file }” to a folder
transfer-move-title =
    { $count ->
        [one] Move { $count } file to a folder
       *[other] Move { $count } files to a folder
    }
transfer-recent = Recent folders
transfer-folder = Folder
transfer-choose-folder = Choose a folder…
transfer-copy-dialog = Copy to folder
transfer-move-dialog = Move to folder
transfer-sidecars =
    { $count ->
        [one] { $count } sidecar file goes along.
       *[other] { $count } sidecar files go along.
    }
transfer-taken =
    { $count ->
        [one] { $count } name is already used in { $folder }.
       *[other] { $count } names are already used in { $folder }.
    }
transfer-when-taken = When a name is already used
transfer-skip = Skip
transfer-keep-both = Keep both
transfer-replace-option = Replace
transfer-copy = Copy
transfer-move = Move
transfer-replace-title =
    { $count ->
        [one] Replace { $count } file in { $folder }?
       *[other] Replace { $count } files in { $folder }?
    }
transfer-replace-note = The files there now go to the trash. Undo brings them back.
transfer-replace = Replace
transfer-back = Back
transfer-stop = Stop

settings-file-import-title = Import settings
settings-file-import-dialog = Import settings
settings-file-export-dialog = Export settings
settings-file-import-changes = Changes
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = no key
settings-file-import-unknown = Not a setting of this version; left out.
settings-file-import-keymap = Key bindings
settings-file-import-nothing = Nothing would change: these settings are already in effect.
settings-file-import-summary =
    { $count ->
        [one] { $count } change. Import can be undone in Settings.
       *[other] { $count } changes. Import can be undone in Settings.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } entry looks like a password or key and is never imported.
       *[other] { $count } entries look like passwords or keys and are never imported.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } user command is not imported: commands start programs. Copy it into your settings file yourself if you trust it.
       *[other] { $count } user commands are not imported: commands start programs. Copy them into your settings file yourself if you trust them.
    }
settings-file-import-button = Import

## Results

fileop-renamed = Renamed to { $file }
fileop-renamed-count =
    { $count ->
        [one] Renamed { $count } file
       *[other] Renamed { $count } files
    }
fileop-copied =
    { $count ->
        [one] Copied { $count } file to { $folder }
       *[other] Copied { $count } files to { $folder }
    }
fileop-moved =
    { $count ->
        [one] Moved { $count } file to { $folder }
       *[other] Moved { $count } files to { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } was skipped: its name is already used there.
       *[other] { $count } were skipped: their names are already used there.
    }
fileop-stopped = Stopped; what was done stays done.
fileop-failed-file =
    { $count ->
        [one] Couldn’t copy or move { $file }: { $reason }
       *[other] Couldn’t copy or move { $count } files; the first, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Copying { $count } file… { PERCENT($percent) }
       *[other] Copying { $count } files… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Moving { $count } file… { PERCENT($percent) }
       *[other] Moving { $count } files… { PERCENT($percent) }
    }
fileop-done-status = Done
fileop-busy = Still working on the last file change; try again in a moment.
fileop-nothing-to-redo = Nothing to redo
fileop-undone = Undone: { $what }
fileop-redone = Done again: { $what }
fileop-kind-rename = rename
fileop-kind-batch-rename = rename of several files
fileop-kind-move = move to folder
fileop-kind-copy = copy to folder
fileop-kind-rotate = rotation
rename-failed = Couldn’t rename: { $reason }

undo-problem-changed = Can’t undo: { $file } was changed since, and undoing would lose that change.
undo-problem-missing = Can’t undo: { $file } isn’t there any more.
undo-problem-taken = Can’t undo: another file now has the name { $file }.
undo-problem-io = Can’t undo the change to { $file }: { $reason }

rotate-done-lossless = Rotated { $file } without any loss.
rotate-done-lossless-trimmed = Rotated { $file } without any loss; its edge was trimmed to { $width } × { $height }.
rotate-done-tag = Rotated { $file } by changing its orientation tag.
rotate-done-sidecar = Saved the rotation of { $file } in { $sidecar }; the RAW file itself is never changed.
rotate-tag-unreadable = Its picture data couldn’t be read far enough to turn it.
rotate-tag-multi-picture = It holds more than one picture, and turning the first would break the others.
rotate-tag-trailer = Data follows the picture, which turning would move.
rotate-tag-not-aligned = Turning its pixels without loss would trim it to { $width } × { $height }.
rotate-trim-offer = Trim and turn
rotate-problem-unsupported = { $file } is a kind of file that can’t record a rotation.
rotate-problem-too-large = { $file } is too large to rotate.
rotate-problem-damaged = { $file } is damaged where its orientation is kept: { $reason }
rotate-problem-io = Couldn’t rotate { $file }: { $reason }

share-failed = Couldn’t share: { $reason }
email-failed = Couldn’t start your mail app: { $reason }
zynk-opening = Opening Zynk…
zynk-failed = Couldn’t reach Zynk: { $reason }
zynk-get-title-send = Send with Zynk needs the Zynk app
zynk-get-title-share = Share with Zynk needs the Zynk app
zynk-get-about = Zynk is a free app for Mac, Windows, Linux, iPhone and Android. It sends photos, videos and whole folders straight to your other devices and to other people, at any size. Transfers are end-to-end encrypted and pick up where they stopped if the connection drops. For a recipient without Zynk, it makes a web link they can download from in their browser.
zynk-get-macos = Get it from the Mac App Store. Once it’s installed, { -app-name } hands your files straight to it.
zynk-get-windows = Get it from the Microsoft Store. Once it’s installed, { -app-name } hands your files straight to it.
zynk-get-linux = Zynk for Linux is an AppImage, with nothing to install: download it, put it in ~/Applications and make it executable (in its Properties, or with chmod +x). { -app-name } finds it there, and anywhere else while Zynk is running.
zynk-get = Get Zynk
zynk-send-old-windows = Send with Zynk needs Windows 10 version 2004 or later. Share with Zynk works on this Windows.
settings-file-export-done = Settings saved to { $file }
settings-file-export-failed = Couldn’t save the settings: { $reason }
settings-file-import-unreadable = Couldn’t read { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] Imported { $count } change
       *[other] Imported { $count } changes
    }
