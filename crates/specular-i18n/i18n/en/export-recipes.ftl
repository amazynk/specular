# Specular — Export recipes: saved ways to export, several run at once. English.
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Export with recipes…
palette-export-recipes-help = Export the selection with one or more saved recipes.
palette-develop-export-recipes = Export with recipes…
palette-develop-export-recipes-help = Export the edited photo, or several, with saved recipes.

export-recipes-title = Export with recipes
export-recipes-new = New
export-recipes-edit = Edit…
export-recipes-duplicate = Duplicate
export-recipes-delete = Delete…
export-recipes-start = Export
export-recipes-save = Save
export-recipes-open-folder = Open folder
export-recipes-choose-image = Choose image…
export-recipes-no-images = No images to export.
export-recipes-none-checked = Tick at least one recipe.
export-recipes-checked = { $name }: on
export-recipes-unchecked = { $name }: off

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Print, full size
export-recipes-starter-email = Email, small
export-recipes-unnamed = Unnamed recipe
export-recipes-new-name = New recipe
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (copy)

## The list's one-line summary.
export-recipes-summary-mark = watermark
export-recipes-size-full = Full size
export-recipes-size-long = { $px } px long edge
export-recipes-size-short = { $px } px short edge
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Next to the original
export-recipes-folder-sub = Folder “{ $name }”
export-recipes-folder-ask = Ask each time

## The editor.
export-recipes-new-title = New recipe
export-recipes-edit-title = Edit recipe
export-recipes-name = Recipe name
export-recipes-text = Custom text
export-recipes-pattern = File name
export-recipes-mark-text-field = Watermark text
export-recipes-subfolder = Folder name
export-recipes-format = Format
export-recipes-quality = Quality
export-recipes-lossless = Lossless
export-recipes-bits = Bit depth
export-recipes-bits-8 = 8-bit
export-recipes-bits-16 = 16-bit
export-recipes-space = Colour space
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profile
export-recipes-profile-embed = Embed
export-recipes-profile-none = Leave out
export-recipes-folder = Folder
export-recipes-folder-subfolder = Subfolder
export-recipes-collision = Name taken
export-recipes-collision-unique = Add a number
export-recipes-collision-overwrite = Replace (asks)
export-recipes-collision-skip = Skip
export-recipes-after = When done
export-recipes-after-nothing = Nothing
export-recipes-after-open = Open the folder
export-recipes-resize = Size
export-recipes-resize-long = Long edge
export-recipes-resize-short = Short edge
export-recipes-resize-mp = Megapixels
export-recipes-resize-exact = Exact size
export-recipes-resize-percent = Percent
export-recipes-edge = Edge
export-recipes-width = Width
export-recipes-height = Height
export-recipes-megapixels = Megapixels
export-recipes-percent = Percent
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Enlarge
export-recipes-sharpen = Sharpen for
export-recipes-sharpen-off = Off
export-recipes-sharpen-screen = Screen
export-recipes-sharpen-matte = Matte paper
export-recipes-sharpen-glossy = Glossy paper
export-recipes-amount = Amount
export-recipes-amount-low = Low
export-recipes-amount-standard = Standard
export-recipes-amount-high = High
export-recipes-metadata = Metadata
export-recipes-meta-all = All
export-recipes-meta-copyright = Copyright only
export-recipes-meta-contact = Copyright and contact
export-recipes-meta-none = None
export-recipes-location = Location
export-recipes-people = People shown
export-recipes-keep = Keep
export-recipes-remove = Remove
export-recipes-mark = Watermark
export-recipes-mark-none = None
export-recipes-mark-text = Text
export-recipes-mark-image = Image (choose one)
export-recipes-position = Position
export-recipes-pos-top-left = Top left
export-recipes-pos-top = Top
export-recipes-pos-top-right = Top right
export-recipes-pos-left = Left
export-recipes-pos-centre = Centre
export-recipes-pos-right = Right
export-recipes-pos-bottom-left = Bottom left
export-recipes-pos-bottom = Bottom
export-recipes-pos-bottom-right = Bottom right
export-recipes-opacity = Opacity
export-recipes-scale = Width
export-recipes-yes = Yes
export-recipes-no = No
export-recipes-error-name = Give the recipe a name.
export-recipes-error-mark-text = Type the watermark text.
export-recipes-error-mark-image = Choose the watermark image.
export-recipes-error-subfolder = Type the folder name.
export-recipes-saved = Recipe “{ $name }” saved.

## Delete.
export-recipes-delete-title = Delete the recipe “{ $name }”?
export-recipes-delete-detail = Files it exported stay on disk.
export-recipes-delete-confirm = Delete
export-recipes-delete-keep = Keep
export-recipes-deleted = Recipe “{ $name }” deleted.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Folder for “{ $name }”
export-recipes-asking = Choose the folder for each recipe that asks.
export-recipes-no-folder = No folder chosen. Nothing was exported.
export-recipes-planning = Naming the files…
export-recipes-plan-pattern = Recipe “{ $name }”: { $reason }
export-recipes-replace-title = { $count ->
    [one] Replace 1 file?
   *[other] Replace { $count } files?
}
export-recipes-replace-detail = Originals are never replaced.
export-recipes-replace-confirm = Replace
export-recipes-replace-kept = Nothing was exported.
export-recipes-running = Exporting…
export-recipes-progress = { $done } of { $total } files
export-recipes-done = { $count ->
    [one] 1 file exported.
   *[other] { $count } files exported.
}
export-recipes-stopped = { $count ->
    [one] Stopped. 1 file exported.
   *[other] Stopped. { $count } files exported.
}
export-recipes-done-skipped = { $count ->
    [one] 1 skipped: the name was taken.
   *[other] { $count } skipped: the names were taken.
}
export-recipes-done-failed = { $count ->
    [one] 1 failed:
   *[other] { $count } failed:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
