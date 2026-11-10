# Specular — the gallery (thumbnail mode, DESIGN §10) and Settings → Gallery.

## Palette and menus

palette-gallery = Gallery
gallery-palette-view = Gallery view…
gallery-palette-layout = Layout
gallery-palette-sort = Sort by
gallery-palette-group = Group by
gallery-palette-filter = Filter…
gallery-palette-quick-filters = Quick filters
gallery-palette-details = Details view
gallery-palette-info-overlay = Info on thumbnails
gallery-palette-stacks = Stack RAW+JPEG and bursts
gallery-palette-subfolders = Include subfolders
gallery-palette-compare = Compare selected
gallery-palette-rate = Rate…
gallery-palette-settings = Gallery settings…
gallery-view-menu = View
gallery-more-menu = More
gallery-open = Open
gallery-new-window = Open in new window
gallery-reveal = Show in folder
gallery-properties = Properties
gallery-copy-paths = Copy paths
gallery-select-all = Select all
gallery-select-none = Select none
gallery-select-invert = Invert selection
gallery-next-unrated = Next unrated
gallery-stack-toggle = Expand or collapse stack
gallery-run-on = { $name } ({ $count } files)
gallery-depth-off = This folder only
gallery-depth-all = All subfolders
gallery-depth-levels = { $depth ->
    [one] One level of subfolders
   *[other] { $depth } levels of subfolders
}

## Status and empty states

gallery-items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
gallery-selected = { $count } of { $total } selected
gallery-shown-of = { $count } of { $total } shown
gallery-listing = Reading folder… { $count }
gallery-empty = This folder has no images.
gallery-empty-filter = No images match the filter.
gallery-list-error = Can’t read this folder: { $reason }
gallery-no-folder = No folder open
gallery-kind-folder = Folder
gallery-drag-count = { $count ->
    [one] { $count } file
   *[other] { $count } files
}
gallery-badge-pages = { $count } pages

## Filter bar

# The filter field matches file and folder names (the chips filter by rating, format, …).
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Name, or rating>=3 camera:X-T5 …
gallery-filter-clear = Clear filter
gallery-filter-rating = { $stars ->
    [one] ★ 1 or more
   *[other] ★ { $stars } or more
}
gallery-filter-label = Label: { $label }
gallery-filter-picks = Picks
gallery-filter-hide-rejects = Hide rejects
gallery-filter-unrated = Unrated
gallery-filter-raw = RAW
gallery-filter-animated = Animated
gallery-filter-hdr = HDR
gallery-filter-landscape = Landscape
gallery-filter-portrait = Portrait

## Query errors and facets

gallery-query-unknown-field = Unknown field “{ $field }”
gallery-query-unknown-field-suggest = Unknown field “{ $field }”: did you mean “{ $suggestion }”?
gallery-query-missing-value = “{ $field }” needs a value
gallery-query-bad-number = “{ $value }” is not a number ({ $field })
gallery-query-bad-date = “{ $value }” is not a date ({ $field }): use 2024, 2024-05 or 2024-05-12
gallery-query-bad-word = “{ $value }” does not fit { $field }: use { $words }
gallery-query-bad-exif-tag = “{ $value }” is not an Exif tag name
gallery-query-no-compare = { $field } cannot be compared with < or > here
gallery-query-unterminated = A closing quote is missing
gallery-query-unavailable = “{ $field }” needs the Library (not available here)
gallery-facets = Facets
gallery-facets-counting = Counting…
gallery-facets-empty = Nothing to count
gallery-facets-search = Search facets
gallery-facets-no-match = No values match
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Start screen
gallery-facet-format = Format
gallery-facet-camera = Camera
gallery-facet-lens = Lens
gallery-facet-rating = Rating
gallery-facet-year = Year
gallery-facet-orientation = Orientation
gallery-facet-label = Label
gallery-facet-properties = Properties
gallery-facet-square = Square
gallery-facet-alpha = Transparency
gallery-facet-gps = Location
gallery-facet-multipage = Multiple pages
# The search field's list of is:/has: words (gallery-filter-* and gallery-facet-* name the rest).
gallery-facet-profile = Colour profile
gallery-facet-date = Capture date
gallery-facet-picked = Picked
gallery-facet-rejected = Rejected
gallery-facet-flagged = Picked or rejected
gallery-facet-rated = Rated
gallery-facet-labelled = Labelled
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } file
   *[other] { $count } files
}
# The search field's value list (under “camera:” and the like): how to choose several.
gallery-picker-hint = Space or Ctrl+Enter selects several
# The value list for place: (where photos were taken, from their GPS positions).
gallery-facet-place = Place
# The value list's names of the number fields (“bits:”, “pages:”, “width:”, …).
gallery-facet-bits = Bit depth
gallery-facet-pages = Pages
gallery-facet-width = Width
gallery-facet-height = Height
gallery-facet-megapixels = Megapixels
# Their values, e.g. “16-bit”, “3 pages”, “1920 px”, “24 MP”.
gallery-picker-bits = { $bits }-bit
gallery-picker-pages = { $pages ->
    [one] { $pages } page
   *[other] { $pages } pages
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
# “dupes:” and “similar:”: any file in a group, the suggested keepers, the rest.
gallery-picker-group-any = Any in a group
gallery-picker-group-keep = Suggested to keep
gallery-picker-group-discard = Suggested to discard
# The value list of a number field compares: the operator buttons (=, ≥, ≤) and, for
# screen readers, what a value row selects (e.g. “at least 1920 px”).
gallery-picker-ops = Compare
gallery-picker-op-eq = Equal to
gallery-picker-op-ge = At least
gallery-picker-op-le = At most
gallery-picker-ge = at least { $value }
gallery-picker-gt = more than { $value }
gallery-picker-le = at most { $value }
gallery-picker-lt = less than { $value }

## Groups

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
# An event group's header with the place most of it was taken: the nearest town (offline
# lookup, "Lisbon, Portugal") and the day or days (a date, or gallery-group-event).
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = This folder
gallery-group-unknown = Unknown
gallery-group-unrated = Unrated

## Culling

gallery-stars = { $stars ->
    [one] 1 star
   *[other] { $stars } stars
}
gallery-rate-none = No rating
gallery-pick = Pick
gallery-reject = Reject
gallery-unflag = No flag
gallery-label-red = Red
gallery-label-yellow = Yellow
gallery-label-green = Green
gallery-label-blue = Blue
gallery-label-none = No label
gallery-compare-hint = Select 2 to 4 images to compare them.

## Files

gallery-trash-count = { $count ->
    [one] Move { $count } file to trash
   *[other] Move { $count } files to trash
}
gallery-moved-to = { $count ->
    [one] Moved { $count } file to { $folder }
   *[other] Moved { $count } files to { $folder }
}
gallery-copied-to = { $count ->
    [one] Copied { $count } file to { $folder }
   *[other] Copied { $count } files to { $folder }
}
gallery-move-failed = Couldn’t move { $file }: { $reason }
gallery-drop-move = Move to { $folder }
gallery-drop-copy = Copy to { $folder }
gallery-move-here = Move selected here
gallery-copy-here = Copy selected here
gallery-not-an-image = { $file } isn’t an image { -app-name } can open.
gallery-cache-cleared = Thumbnail cache cleared
# A tile's tooltip when its thumbnail could not be made; $reason is an error-reason-* text.
gallery-thumb-failed = No thumbnail: { $reason }
gallery-cache-clear-failed = Couldn’t clear the thumbnail cache: { $reason }

## Properties

gallery-prop-name = Name
gallery-prop-folder = Folder
gallery-prop-size = File size
gallery-prop-dimensions = Dimensions
gallery-prop-format = Format
gallery-prop-date = Date
gallery-prop-camera = Camera
gallery-prop-lens = Lens
gallery-prop-rating = Rating
gallery-prop-label = Label
gallery-prop-items = Items

## Screen readers

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } item
   *[other] { $count } items
}
gallery-a11y-dims = { $width } by { $height }
gallery-a11y-folder = folder
gallery-a11y-up = Up to { $folder }
gallery-a11y-archive = archive
gallery-a11y-stack = stack of { $count }
gallery-a11y-filter-bar = Filter
gallery-a11y-filter-field = Filter by name or field
gallery-a11y-chip = Filter term { $term }
gallery-a11y-chip-remove = Press to remove
gallery-a11y-results = { $count ->
    [0] No items match
    [one] { $count } item matches
   *[other] { $count } items match
}

## Settings → Gallery

settings-section-gallery = Gallery
settings-gallery-layout = Layout
settings-gallery-layout-help = Squares crop thumbnails to a grid; justified rows keep each image’s shape; uncropped fits whole images into equal cells.
settings-gallery-layout-squares = Squares
settings-gallery-layout-justified = Justified rows
settings-gallery-layout-uncropped = Uncropped
settings-gallery-layout-details = Details
settings-gallery-thumbnail-size = Thumbnail size
settings-gallery-thumbnail-size-help = Also Ctrl+mouse wheel, pinch, or Ctrl+plus and minus in the gallery.
settings-gallery-captions = Show file names
settings-gallery-captions-help = A caption under every thumbnail.
settings-gallery-badges = Show badges
settings-gallery-badges-help = Small marks for RAW, animated, HDR, multi-page, rating and label.
settings-gallery-sort-order = Sort by
settings-gallery-sort-order-help = The order of the gallery. Folders always come first.
settings-gallery-sort-order-name = Name
settings-gallery-sort-order-date-taken = Date taken
settings-gallery-sort-order-modified = Date modified
settings-gallery-sort-order-size = File size
settings-gallery-sort-order-type = File type
settings-gallery-sort-order-dimensions = Dimensions
settings-gallery-sort-descending = Reverse order
settings-gallery-sort-descending-help = Sort from last to first.
settings-gallery-group-by = Group by
settings-gallery-group-by-help = Section headers that stay at the top while you scroll.
settings-gallery-group-by-none = Nothing
settings-gallery-group-by-day = Day
settings-gallery-group-by-month = Month
settings-gallery-group-by-year = Year
settings-gallery-group-by-event = Event
settings-gallery-group-by-folder = Folder
settings-gallery-group-by-camera = Camera
settings-gallery-group-by-format = Format
settings-gallery-group-by-rating = Rating
settings-gallery-group-by-target = Target
settings-gallery-group-by-filter = Filter
settings-gallery-per-folder-view = Remember the view for each folder
settings-gallery-per-folder-view-help = Layout, size, sort and grouping are kept per folder; otherwise one view is used everywhere.
settings-gallery-open-folders-in-gallery = Open folders in the gallery
settings-gallery-open-folders-in-gallery-help = When you open a folder, start with its thumbnails instead of the first image.
settings-gallery-escape-opens-gallery = Escape opens the gallery
settings-gallery-escape-opens-gallery-help = When there is nothing else to close, Escape in the viewer goes to the gallery instead of doing nothing.
settings-gallery-wrap-around = Arrow keys wrap around
settings-gallery-wrap-around-help = At the end of the list the arrow keys go round to the start, and from the start to the end. Holding a key down still stops at the end.
settings-gallery-show-hidden = Show hidden files
settings-gallery-show-hidden-help = Include files and folders whose names start with a dot.
settings-gallery-show-other-files = Show other files
settings-gallery-show-other-files-help = Include files { -app-name } can’t open, shown with a generic icon.
settings-gallery-show-archives = Show archives
settings-gallery-show-archives-help = Include comic books and other archives (zip, cbz, rar, 7z…) as tiles. Opening an archive directly still works.
settings-gallery-subfolder-depth = Subfolder levels
settings-gallery-subfolder-depth-help = Show images from subfolders too, down to this many levels (0: this folder only).
settings-gallery-stacks = Stack related files
settings-gallery-stacks-help = Show RAW+JPEG pairs and bursts as one tile. Press S to expand a stack.
settings-gallery-info-overlay = Info on thumbnails
settings-gallery-info-overlay-help = Dimensions and exposure over each thumbnail.
settings-gallery-prefer-embedded = Prefer embedded previews
settings-gallery-prefer-embedded-help = Use the preview stored in the file when it is big enough. Much faster; a little less exact.
settings-gallery-sharper-raw-thumbnails = Sharper raw thumbnails
settings-gallery-sharper-raw-thumbnails-help = Edited raw photos are developed at 1024 px for their thumbnails. Sharper, a little slower. Off: 512 px.
settings-gallery-rating-sidecars = Write ratings to XMP sidecars
settings-gallery-rating-sidecars-help = Ratings, labels and flags also go into a .xmp file next to the image, which other photo applications read.
settings-gallery-cache-size = Thumbnail cache size
settings-gallery-cache-size-help = Oldest thumbnails are removed when the cache is full.
settings-gallery-cache-location = Thumbnail cache location
settings-gallery-cache-location-help = A folder on a fast drive works best.
settings-gallery-clear-cache = Clear thumbnail cache
settings-gallery-clear-cache-help = Deletes the saved thumbnails. They are made again as you browse.

## Compare

compare-side = Side by side
compare-overlay = Overlay
compare-difference = Difference
compare-flicker = Flicker
compare-blend = Blend
compare-kind-absolute = Absolute error
compare-kind-squared = Squared error
compare-kind-relative = Relative error
compare-scale-down = Less sensitive
compare-scale-up = More sensitive
# The error that shows as the brightest colour of the difference ramp.
compare-scale = Full scale { $value }
compare-space = Linear light, as displayed
compare-space-help = Errors and metrics are measured in linear light as the image is displayed, after tone mapping, per channel including alpha.
# MAE: mean absolute error, MSE: mean squared error, PSNR: peak signal-to-noise ratio.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = sampled at { $width } × { $height }
compare-aligned = Aligned top-left, { $width } × { $height } compared
compare-vs = { $b } against { $a }
compare-overlay-label = { $b } over { $a } at { PERCENT($percent) }
compare-pending = Measuring…
# The difference view could not be set up; $reason is error-reason-internal.
compare-diff-failed = Can’t show the difference: { $reason }
compare-no-overlap = Nothing to compare in view
compare-zoom-fit = Fit
compare-zoom = { PERCENT($percent) }
compare-showing = Showing { $name }
compare-a11y-group = Compare
compare-a11y-blend = Blend between the images
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Compare side by side
gallery-palette-compare-overlay = Compare as overlay
gallery-palette-compare-difference = Compare difference: { $kind }
gallery-palette-compare-flicker = Flicker between compared images
gallery-palette-compare-actual = Compare at { PERCENT(100) }

## Map view
gallery-palette-map = Map
map-title = Map
map-summary = { $count ->
    [one] { $count } photo with a location
   *[other] { $count } photos with a location
}
map-photos = { $count ->
    [one] { $count } photo
   *[other] { $count } photos
}
map-zoom-in = Zoom in
map-zoom-out = Zoom out
map-show-all = Show all
map-done = Done
map-credit = Map data: Natural Earth
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Map: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Online map tiles
map-basemap-consent-server = Map tiles come from { $host }.
map-basemap-consent-sees = That server sees your internet address and the areas you view.
map-basemap-consent-private = Your photos, their names and their locations are not sent.
map-basemap-consent-cache = Tiles are kept on this computer, up to { $size }.
map-basemap-allow = Use online map
map-basemap-decline = Keep offline map
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Base map not shown: { $reason }
map-basemap-bad-address = the tile server address needs {"{"}z{"}"}, {"{"}x{"}"} and {"{"}y{"}"}.
map-basemap-unknown-tiles = the map file's tiles are of an unknown kind.
map-loading = Reading photo locations…
map-empty = None of these photos has a location
map-hidden = Photo locations are hidden (Settings › Metadata and privacy)
map-a11y-group = Map of photo locations
map-a11y-status = { $count ->
    [one] { $count } place shown, zoom level { $zoom }
   *[other] { $count } places shown, zoom level { $zoom }
}
