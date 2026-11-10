# Specular — Batch convert (DESIGN.md §6.5). English.
# Every output is a new file; the sources are never changed.

palette-convert = Convert…

## Settings → Convert
settings-section-convert = Convert
settings-convert-format = Format
settings-convert-format-help = The format Convert… writes. PNG and TIFF are lossless; WebP and JPEG XL can be either.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-avif = AVIF
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = JPEG quality
settings-convert-jpeg-quality-help = Higher keeps more detail in larger files. 90 is hard to tell from the original.
settings-convert-jpeg-full-chroma = Full-resolution colour in JPEG
settings-convert-jpeg-full-chroma-help = Keeps colour at full resolution (4:4:4) instead of half (4:2:0): sharper colour edges, larger files.
settings-convert-png-effort = PNG compression
settings-convert-png-effort-help = Higher makes smaller files more slowly. 1 is the fastest.
settings-convert-webp-lossless = Lossless WebP
settings-convert-webp-lossless-help = Keeps every pixel exactly. Off, WebP files are much smaller at the WebP quality.
settings-convert-webp-quality = WebP quality
settings-convert-webp-quality-help = Higher keeps more detail in larger files. Used when WebP is not lossless.
settings-convert-webp-effort = WebP effort
settings-convert-webp-effort-help = Higher makes smaller files more slowly. Used when WebP is not lossless.
settings-convert-jxl-lossless = Lossless JPEG XL
settings-convert-jxl-lossless-help = Keeps every pixel exactly. Off, JPEG XL files are much smaller at the JPEG XL quality.
settings-convert-jxl-quality = JPEG XL quality
settings-convert-jxl-quality-help = Higher keeps more detail in larger files. 90 is hard to tell from the original. Used when JPEG XL is not lossless.
settings-convert-jxl-effort = JPEG XL effort
settings-convert-jxl-effort-help = Higher makes smaller files more slowly.
settings-convert-avif-quality = AVIF quality
settings-convert-avif-quality-help = Higher keeps more detail in larger files.
settings-convert-avif-effort = AVIF effort
settings-convert-avif-effort-help = Higher makes smaller files more slowly. 1 is the fastest.
settings-convert-tiff-compression = TIFF compression
settings-convert-tiff-compression-help = Deflate is lossless and read by every TIFF reader.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = None
settings-convert-depth = Bit depth
settings-convert-depth-help = Automatic writes 16 bits per sample when the image has more than 8 and the format holds them (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automatic
settings-convert-depth-8 = 8 bits
settings-convert-depth-16 = 16 bits
settings-convert-resize = Resize
settings-convert-resize-help = Images are scaled down in linear light with a high-quality filter.
settings-convert-resize-none = Original size
settings-convert-resize-fit = Fit within
settings-convert-resize-long-edge = Long edge
settings-convert-resize-percent = Percentage
settings-convert-width = Width
settings-convert-width-help = The widest an image may become with Fit within.
settings-convert-height = Height
settings-convert-height-help = The tallest an image may become with Fit within.
settings-convert-long-edge = Long edge
settings-convert-long-edge-help = The length of an image's longer side with Long edge.
settings-convert-percent = Percentage
settings-convert-percent-help = The size relative to the original with Percentage.
settings-convert-upscale = Enlarge small images
settings-convert-upscale-help = Off, images already smaller than the size asked for keep their size.
settings-convert-colour = Colour
settings-convert-colour-help = Keep stores the image's colour profile with it. Convert to sRGB suits the web and devices without colour management.
settings-convert-colour-keep = Keep the profile
settings-convert-colour-srgb = Convert to sRGB
settings-convert-hdr = HDR images
settings-convert-hdr-help = PNG and JPEG XL keep HDR as such; other formats are always tone-mapped to SDR.
settings-convert-hdr-keep = Keep where possible
settings-convert-hdr-tone-map = Tone-map to SDR
settings-convert-metadata = Metadata
settings-convert-metadata-help = Exif and XMP, such as the camera, the capture date and the location. The colour profile is always kept.
settings-convert-metadata-all = Keep all
settings-convert-metadata-no-location = Keep all but location
settings-convert-metadata-strip = Remove
settings-convert-folder = Output folder
settings-convert-folder-help = Where converted files go. Not set, they go to a subfolder next to each image.
settings-convert-folder-unset = Subfolder next to the images
settings-convert-subfolder = Subfolder name
settings-convert-subfolder-help = The subfolder used when no output folder is set. Empty: the images' own folder.
settings-convert-pattern = File names
settings-convert-pattern-help = {"{"}name{"}"} is the original name, {"{"}n:03{"}"} the number in the batch, {"{"}date:%Y-%m-%d{"}"} the capture date, {"{"}w{"}"} and {"{"}h{"}"} the new size, {"{"}ext{"}"} the format's extension.
settings-convert-conflicts = When a name is taken
settings-convert-conflicts-help = Existing files are never replaced.
settings-convert-conflicts-suffix = Add a number
settings-convert-conflicts-skip = Skip the image

## The dialog
convert-title = Convert
convert-images = Images
convert-images-current = This image
convert-images-selection = Selection ({ $count })
convert-images-folder = Folder ({ $count })
convert-images-picked = Picks ({ $count })
convert-quality = Quality
convert-lossless = Lossless
convert-chroma = Colour detail
convert-chroma-half = 4:2:0 (smaller)
convert-chroma-full = 4:4:4 (sharper)
convert-effort = Effort
convert-on = On
convert-off = Off
convert-pixels = { $value } px
convert-effort-fastest = { $value } (fastest)
convert-folder-subfolder = Subfolder “{ $name }”
convert-folder-same = Same folder as the image
convert-choose-folder = Choose folder…
convert-name = Name
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = A {"{"} is not closed.
convert-name-error-stray = A {"}"} has no {"{"}. Write {"}}"} for a brace.
convert-name-error-unknown = { $token } is not a name part. Use name, n, date, w, h or ext.
convert-name-error-format = { $token } has a bad format.
convert-name-error-empty = Enter a name pattern.
convert-start = Convert
convert-stop = Stop
convert-hide = Hide
convert-close = Close
convert-progress = Converting { $done } of { $total }
convert-progress-eta = Converting { $done } of { $total } · about { $time } left
convert-eta-seconds = { $count ->
    [one] { $count } second
   *[other] { $count } seconds
}
convert-eta-minutes = { $count ->
    [one] { $count } minute
   *[other] { $count } minutes
}
convert-done = { $count ->
    [one] Converted { $count } image
   *[other] Converted { $count } images
}
convert-done-skipped = { $count } skipped: the name was taken
convert-done-failed = { $count } failed
convert-stopped = Stopped after { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = cannot be opened ({ $error })
convert-failure-decode = cannot be read ({ $error })
convert-failure-encode = cannot be written in this format ({ $error })
convert-failure-write = cannot be saved ({ $error })
convert-failure-same = the new file would replace the original
convert-report-more = and { $count } more
convert-running = A conversion is already running.
