# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Local copies limit
settings-library-local-copies-limit-help = The most space local copies may take in Specular’s data folder.

local-keep-menu = Keep a local copy…
local-remove-menu = Remove local copy…
local-keep-location-menu = Keep a local copy of this location…
local-remove-location-menu = Remove local copy of this location…
local-keep-album-menu = Keep a local copy of this album…
local-remove-album-menu = Remove local copy of this album…
local-remove-held-elsewhere = These copies belong to a kept location or album. Remove it there.

local-keep-ask = Keep a local copy of { $item }?
local-keep-detail = { $count ->
    [one] 1 photo.
   *[other] { $count } photos.
}
    Originals: the files as they are.
    Proxies: raws at 2,560 pixels, enough to develop. Other files are copied whole.
    Copies go to Specular’s data folder. The originals aren’t changed.
local-keep-originals = Keep originals
local-keep-proxies = Keep proxies
local-keeping = Copying { $item }…
local-kept = { $item }: { $count ->
    [one] 1 local copy
   *[other] { $count } local copies
}, { $size }.
local-kept-full = { $count ->
    [one] 1 photo didn’t fit in the size limit.
   *[other] { $count } photos didn’t fit in the size limit.
}
local-kept-unreachable = { $count ->
    [one] 1 photo couldn’t be read.
   *[other] { $count } photos couldn’t be read.
}

local-remove-ask = Remove the local copy of { $item }?
local-remove-detail = The copies in Specular’s data folder are deleted. The originals aren’t touched. Changes still waiting are kept.
local-remove-ok = Remove local copy
local-removed = Local copy of { $item } removed, { $size } freed.

local-banner-offline = { $location } is offline. You’re working on local copies.
local-banner-offline-many = { $count } locations are offline. You’re working on local copies.
local-banner-waiting = { $count ->
    [one] 1 change is waiting for the original.
   *[other] { $count } changes are waiting for the originals.
}
local-banner-conflicts = { $count ->
    [one] 1 file changed while you were offline.
   *[other] { $count } files changed while you were offline.
}
local-review = Review…
local-sidebar-originals = local copy
local-sidebar-proxies = local proxies

local-synced = { $count ->
    [one] 1 change written to its original.
   *[other] { $count } changes written to their originals.
}
local-sync-failed = { $count ->
    [one] 1 change couldn’t be written. It stays waiting.
   *[other] { $count } changes couldn’t be written. They stay waiting.
}

local-conflict-title = { $file } changed while you were offline. Which { $what } do you keep?
local-conflict-yours = Yours: { $text }
local-conflict-theirs = On disk: { $text }
local-conflict-later = Cancel decides later. Nothing is overwritten.
local-keep-mine = Keep yours
local-keep-theirs = Keep the file’s
local-what-marks = rating
local-what-keywords = keywords
local-what-fields = IPTC fields
local-what-edit = edit
local-stars = { $count ->
    [0] No stars
    [one] 1 star
   *[other] { $count } stars
}
local-label = { $label ->
    [1] Red
    [2] Yellow
    [3] Green
   *[4] Blue
}
local-pick = Pick
local-reject = Rejected
local-no-keywords = no keywords
local-changed-elsewhere = changed elsewhere
local-photo-changed = the photo itself changed
local-edit-here = edited here, { $when }
local-edit-elsewhere = edited elsewhere, { $when }
local-edit-none = no edit

local-export-ask = { $count ->
    [one] 1 photo is offline. Export it from its proxy?
   *[other] { $count } photos are offline. Export them from their proxies?
}
local-export-detail = Proxies are 2,560 pixels on the long edge. The originals give the full size.
local-export-proxy = Export from proxies
local-export-wait = Wait for the originals
local-export-waiting = The export starts when the originals are back.

local-panel-title = Local copies
local-panel-used = Using { $used } of { $limit }.
local-panel-originals = { $count ->
    [one] 1 original
   *[other] { $count } originals
}
local-panel-proxies = { $count ->
    [one] 1 photo, raws as proxies
   *[other] { $count } photos, raws as proxies
}
local-panel-remove = Remove…
local-panel-footer = Stored in { $folder }. The originals are never changed.
local-panel-keywords = offline local copies proxies space
