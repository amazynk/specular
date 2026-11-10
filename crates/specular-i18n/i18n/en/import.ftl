# Specular — Import from cameras, phones and memory cards. English.
# The card is only read; files are deleted from it only when asked, after a verified copy.

palette-import = Import photos…
disabled-no-import-source = No memory card, camera or phone is connected

## Settings → Import
settings-section-import = Import
settings-import-on-card = When a card is inserted
settings-import-on-card-help = What happens when a memory card, camera or phone with photos is connected.
settings-import-on-card-ask = Ask
settings-import-on-card-open = Open Import
settings-import-on-card-nothing = Do nothing
settings-import-destination = Import to
settings-import-destination-help = The folder imported photos and videos go in. Not set, they go in your Pictures folder.
settings-import-destination-unset = Pictures folder
settings-import-folder-pattern = Folders
settings-import-folder-pattern-help = The folders made inside it: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} and {"{"}event{"}"}, separated by /.
settings-import-rename-pattern = Rename
settings-import-rename-pattern-help = New file names, as in Convert: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Empty keeps the camera's names.
settings-import-keep-pairs = Keep RAW and JPEG together
settings-import-keep-pairs-help = A RAW file and the JPEG taken with it go in one folder with one name, even when renamed.
settings-import-low-res-videos = Import low-resolution video copies
settings-import-low-res-videos-help = Some cameras record a small preview copy of each video (.LRV). Off, they stay on the card.
settings-import-eject = Eject when done
settings-import-eject-help = Ejects the card once everything is copied, so it can be taken out.

## The offer when a card is inserted
import-offer = Import from { $name }?
import-offer-button = Import

## The import view
import-title = Import
import-from = from { $name }
import-source-next = Next source
import-close = Close
import-scanning = Looking for new photos… { $done } of { $total }
import-scanning-start = Looking for new photos…
import-none-new = Everything on { $name } has been imported before.
import-empty = There are no photos or videos on { $name }.
import-no-source = No memory card, camera or phone is connected.
import-summary = { $count ->
    [one] 1 new item
   *[other] { $count } new items
} · { $size }
import-already = { $count } imported before
import-unreadable = { $count ->
    [one] 1 file could not be read
   *[other] { $count } files could not be read
}
import-selected = { $count } of { $total } selected
import-select-all = Select all
import-select-none = Select none
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Import to
import-choose = Choose…
import-folder-pattern = Folders
import-event = Event
import-event-hint = Optional, for {"{"}event{"}"}
import-rename = Rename
import-rename-hint = Empty keeps the camera's names
import-preview = First file: { $path }
import-pattern-error = { $error }
import-keep-pairs = Keep RAW and JPEG together
import-eject = Eject when done
import-delete = Delete from the card after import
import-free = { $size } to copy · { $free } free space
import-free-unknown = { $size } to copy
import-no-space = Not enough free space in { $folder }: this import needs { $need } and only { $free } is free. Free some space or choose another folder.
import-start = { $count ->
    [one] Import 1 item
   *[other] Import { $count } items
}
import-nothing-selected = Select the photos to import

import-confirm-delete = { $count ->
    [one] After it is copied and checked, 1 file will be deleted from { $name }. This cannot be undone.
   *[other] After they are copied and checked, { $count } files will be deleted from { $name }. This cannot be undone.
}
import-confirm-yes = Import and delete
import-confirm-no = Go back

import-progress = Importing { $done } of { $total } · { $size }
import-progress-eta = Importing { $done } of { $total } · { $size } · { $time }
import-paused = Paused · { $done } of { $total }
import-pause = Pause
import-resume = Resume
import-cancel = Cancel
import-hide = Hide
import-stopping = Stopping…
import-running = An import is already running.

import-done = { $count ->
    [one] Imported 1 file
   *[other] Imported { $count } files
} to { $folder }
import-done-existed = { $count ->
        [one] { $count } was already there
       *[other] { $count } were already there
    }
import-done-deleted = { $count } deleted from the card
import-done-failed = { $count ->
    [one] 1 file could not be imported
   *[other] { $count } files could not be imported
}
import-done-cancelled = Import cancelled after { $count ->
        [one] { $count } file
       *[other] { $count } files
    }. Importing again picks up where it stopped.
import-done-nothing = Nothing new was imported.
import-ejected = { $name } can be taken out.
import-eject-failed = { $name } could not be ejected: { $error }
import-failed-destination = The folder { $folder } cannot be used: { $error }
import-ledger-failed = The list of imported files could not be read ({ $error }); files imported before may be offered again.

## Screen reader
import-a11y-group = Import
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, selected
