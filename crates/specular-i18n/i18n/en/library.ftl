# Specular — Library (every watched location in one grid, DESIGN §10). English.

palette-library = Library

## Top bar
library-title = Library
library-locations-button = Locations

## Locations sidebar
library-locations = Locations
library-all = All locations
library-add = Add location…
# The remove button of a location row (screen readers and its tooltip).
library-remove = Remove { $location } from the Library
library-offline = Offline
library-location-pending = { $count } to index
# Screen readers: one location row.
library-a11y-location = { $location }, { $count ->
    [one] { $count } item
   *[other] { $count } items
}
library-a11y-location-offline = { $location }, offline, { $count ->
    [one] { $count } item
   *[other] { $count } items
} kept

## Indexing progress (sidebar and screen readers)
# The count comes first: a narrow sidebar cuts the end of the line, never the count.
library-walking = Images found: { $count } · { $location }
library-walking-count = Images found: { $count }
library-reading = Indexing… { $done } of { $total }
library-held = Indexing pauses while you browse
library-up-to-date = Up to date
# The user stopped the indexing (the sidebar's Cancel): what was indexed stays searchable.
library-stopped = Indexing stopped
library-stopped-pending = Indexing stopped · { $count } to index
# Takes a stopped indexing up again.
library-resume = Resume
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indexing stopped: { $reason }
library-reason-stalled = nothing moved for { $minutes ->
    [one] a minute
   *[other] { $minutes } minutes
}. A file or drive may not be answering.
library-retry = Retry
library-retry-description = Starts indexing again. What was indexed stays searchable.
library-save-diagnostics = Save diagnostics…
library-stop-description = Stops indexing. What was indexed so far stays searchable; Resume or Rescan (F5) goes on.
library-resume-description = Looks through every location again and indexes what is left.
# The catalogue is being opened (in the background; a big one may take a few seconds).
library-opening = Opening the library…
library-searching = Searching…

## Empty and error states
library-empty = No locations yet. Add a folder: its images, and those in its subfolders, show here in one grid.
library-empty-offline = These locations are offline. Their images show again when they are back.
library-indexing = Looking for images…
# Asked before a location is removed (its X, Delete, or an assistive technology).
library-remove-confirm = Remove “{ $location }” from the Library?
library-remove-confirm-detail = Your photos stay on disk, with their ratings, labels and keywords. The Library forgets what it indexed there; adding the folder again indexes it again.
library-remove-confirm-ok = Remove
library-indexing-in = Looking for images in { $location }… { $count } found so far
library-empty-stopped = Indexing was stopped before these images were read. Resume goes on from there.
library-unavailable = The Library could not be opened: { $reason }
library-read-only = This Library was written by a newer Specular: it can be searched but not updated.

## Screen readers
library-a11y-list = Library, { $count ->
    [one] { $count } item
   *[other] { $count } items
}
library-a11y-added = { $location } added to the Library
library-a11y-removed = { $location } removed from the Library

## Adding the home folder
library-home-confirm = Add all of “{ $folder }”?
library-home-confirm-detail = It holds program files, downloads and caches, not just photos. Indexing takes longer, and the Library fills with icons and other images.
library-home-confirm-pictures = Your Pictures folder holds just your pictures.
library-home-confirm-use-pictures = Add Pictures
library-home-confirm-keep = Add home folder

## Folder watches
library-unwatched = Some folders aren't watched
library-unwatched-detail = { $minutes ->
    [one] Changes show up within a minute
   *[other] Changes show up within { $minutes } min
}
library-unwatched-description = Specular ran out of folder watches. Opens the help on how to raise the limit.

## Settings → Library
settings-section-library = Library
settings-library-skip-folders = Skip folders named
settings-library-skip-folders-help = The Library doesn't look in folders with these names, separated by commas. Hidden folders are always skipped.
settings-library-skip-marked = Skip folders holding
settings-library-skip-marked-help = Nor in folders holding one of these: .git for code checkouts, pyvenv.cfg for Python environments, CACHEDIR.TAG for caches. A location's own folder is never skipped.
settings-library-show-timeline = Date timeline
settings-library-show-timeline-help = Shows how many photos each year, month or day has, above the Library’s grid.
