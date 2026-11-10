# Specular — The start screen (shown when Specular opens without a file, and when the last
# image is closed), its "Things to try" cards, and the one-time hints. English.

## Start screen

# Heading of the start screen: the application's name.
welcome-title = { -app-name }
# The one line under the heading that says what Specular is.
welcome-tagline = A fast, faithful viewer for photos and for technical, scientific and medical images.
# A large entry point: opens the file chooser.
welcome-open = Open a file…
welcome-open-about = An image, a document or a data file.
# A large entry point: opens the folder chooser, then shows the folder as a gallery.
welcome-browse = Browse a folder…
welcome-browse-about = Choose a folder and see it as a gallery.
# A large entry point: the library (every photo in the folders added to it).
welcome-pictures = Your pictures
welcome-pictures-about = Every photo in your library, by date and place.
# That entry point while the library has no folders yet: its label (choosing it asks to add
# the folder) and the line under it. $folder is the name of the operating system's Pictures
# folder.
welcome-pictures-setup-title = Set up your library…
welcome-pictures-setup = Starting with your { $folder } folder.
# Asked (never done silently) when "Your pictures" is chosen and the library has no folders.
welcome-pictures-ask = Add your { $folder } folder to the library?
# The accept button of that question.
welcome-pictures-add = Add folder
# Under the large entry points.
welcome-drop = You can also drop files or folders anywhere in this window.
# Heading over the recent folders' thumbnail cards.
welcome-recent = Recent folders
# Accessible name of a recent folder card: the folder's name and the folder it is in.
welcome-recent-name = { $name }, in { $parent }
# Heading over the small feature cards.
welcome-tips = Things to try
# A link at the end of the "Things to try" heading; hides the cards (a setting).
welcome-tips-hide = Hide these tips
# Announced and shown once the cards are hidden.
welcome-tips-hidden = Tips hidden. “Show welcome” in the command palette brings them back.

## Things to try: a name and one line each. The key shown beside the name is the user's own
## binding, read from the keymap.

welcome-tip-gallery = Gallery
welcome-tip-gallery-about = See a whole folder at once.
welcome-tip-compare = Compare
welcome-tip-compare-about = Select two to four images in the gallery to see them side by side.
welcome-tip-cull = Cull
welcome-tip-cull-about = Pick, reject and rate a shoot with one key each.
welcome-tip-slideshow = Slideshow
welcome-tip-slideshow-about = Show a folder full screen, one image after another.
welcome-tip-palette = Command palette
welcome-tip-palette-about = Find any command or setting by name.
welcome-tip-studies = Studies
welcome-tip-studies-about = The patients, studies and series of a DICOM disc or folder.
welcome-tip-dicom-servers = DICOMweb servers
welcome-tip-dicom-servers-about = Browse and open studies from a PACS.
welcome-tip-dicom-tags = DICOM tags
welcome-tip-dicom-tags-about = Every tag of an image, searchable.
welcome-tip-window-level = Window and level
welcome-tip-window-level-about = Drag to set the contrast of a DICOM image.
welcome-tip-omero-servers = OMERO servers
welcome-tip-omero-servers-about = Browse microscopy projects on OMERO and IDR.
welcome-tip-stretch = Stretch
welcome-tip-stretch-about = Bring out faint detail in astronomical and scientific data.
welcome-tip-drawing-layers = Drawing layers
welcome-tip-drawing-layers-about = Show and hide the layers of a technical drawing.
# Shown (one line) when a card's feature needs an open image. $key is its key.
welcome-tip-needs-image = Open an image first, then press { $key }.
# Shown (one line) when the Compare card is chosen. $key is compare's key in the gallery.
welcome-tip-needs-gallery = In the gallery, select two to four images, then press { $key }.
# Accessible description of a card: its key.
welcome-tip-key = Shortcut: { $key }

## One-time hints: shown once each, quietly, at most one per start. Settings › General › Show
## tips turns them all off.

# The first image opened from a folder with other images. $key: the gallery's key.
hint-gallery = Press { $key } to see this folder as a gallery.
# The first time in the gallery. $compare and $cull are keys.
hint-gallery-first = Select two to four images and press { $compare } to compare them, or { $cull } to cull the folder.
# The first folder of raw photos. $key: cull mode's key.
hint-raw = A folder of raw photos: press { $key } to cull it (pick, reject and rate).
# The first file with several pages. $next and $prev are the page keys.
hint-pages = This file has { $count } pages. { $next } and { $prev } turn them.
# The button of a hint: turns all hints off (Settings › General › Show tips).
hint-turn-off = Turn off tips
# Announced after "Turn off tips".
hint-turned-off = Tips turned off. Settings › General › Show tips turns them back on.

## Command palette

palette-show-welcome = Show welcome
palette-show-welcome-help = The start screen, with Open, Browse, your pictures and things to try.
palette-close = Close
palette-close-help = Close the image or folder and go back to the start screen. On the start screen, quit.

## First-run sheet (beneath the profile question)

# Heading of the start choice on the first-run sheet; the choices are the
# settings-general-on-start-* labels.
profile-on-start = When { -app-name } opens without a file:
# The first-run sheet's button that keeps every default.
profile-skip = Skip
