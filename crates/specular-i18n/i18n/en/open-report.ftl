# Specular — Open report: why an image took as long as it did to open (information panel,
# section "Performance"; DESIGN.md §12.7). English.
# $percent is a formatted percentage ("70 %"), $ms, $cpu, $hw and $speed formatted numbers.
# $api is a hardware decoding interface ("VA-API"), $codec a codec name ("JPEG", "HEVC").
# $engine in open-report-decoder-checked is a paths-engine-* name (standalone, capitalised).

open-report-title = Performance

## Headlines
open-report-fast = Opened in { $ms } ms
open-report-display-cache = Shown from the display cache
open-report-slow-read = Slow storage: { $percent } of the time went to reading the file ({ $speed } MB/s)
# The file was too small for a meaningful speed: the time went to the storage answering.
open-report-slow-read-no-speed = Slow storage: { $percent } of the time went to reading the file (a small file: mostly waiting for the storage to respond)
open-report-archive-read = { $percent } of the time went to unpacking the archive
open-report-startup = { $percent } of the time went to starting { -app-name }
open-report-checked = Decoded twice: the hardware decoder’s first result was checked against the CPU’s ({ $percent } of the time)
open-report-hardware = Decoded by the hardware decoder, which took { $percent } of the time
open-report-decode = Large image: decoding { $mpx } megapixels took { $percent } of the time
open-report-upload = { $percent } of the time went to sending the image to the GPU
open-report-waiting = { $percent } of the time went to waiting for other work to finish
open-report-stage = { $stage } took { $percent } of the time
open-report-memory-short = Memory was short, so the system may have had to swap

## Why a decode ran on the CPU although a hardware decoder can do it
open-report-sw-not-probed = Decoded in software: the hardware decoder was still being detected
open-report-sw-no-device = Decoded in software: { $api } is not available on this computer
open-report-sw-no-codec = Decoded in software: { $api } on this GPU has no { $codec } decoder
open-report-sw-blocked-mismatch = Decoded in software: the hardware decoder was turned off after its result did not match the CPU’s
open-report-sw-blocked-error = Decoded in software: the hardware decoder was turned off after an error
open-report-sw-blocked-lost = Decoded in software: the hardware decoder was turned off after it stopped responding
open-report-sw-failed = Decoded in software: the hardware decoder failed on this image
open-report-sw-busy = Decoded in software: the hardware decoder was busy
open-report-sw-unsupported = Decoded in software: the hardware decoder cannot take this image
open-report-sw-unavailable = Decoded in software: hardware decoding is off until { -app-name } restarts
open-report-sw-cpu-only = Decoded in software: processing is set to “CPU only”
open-report-sw-setting-off = Decoded in software: hardware decoding is turned off in the settings
open-report-sw-unchecked = Decoded in software: the hardware decoder is not checked yet (it is tried first on an image prepared in the background)
open-report-sw-lower-power = Decoded in software: on battery, the CPU uses less power for this
open-report-sw-faster = Decoded in software: faster on this computer (about { $cpu } ms, against { $hw } ms on the hardware decoder)
open-report-sw-scaled = Decoded in software: the hardware decoder only decodes images at full size

## Hints
open-report-hint-copy-local = Copy the file to a local disk to open it faster.
open-report-hint-extract = Extract the archive to a folder to open its pages faster.
open-report-hint-try-again = To try the hardware decoder again: Settings → Performance → Try again.
open-report-hint-automatic = Set “Where images are processed” to Automatic in Settings → Performance.
open-report-hint-turn-on = Set “Hardware decoding” to Automatic in Settings → Performance.
open-report-hint-free-memory = Close other programs to free memory.
open-report-hint-once-only = Only the first image is checked like this.

## Rows
open-report-total = Until sharp
open-report-first-pixels = First pixels
open-report-whole = Whole image shown
open-report-stage-startup = Starting up
open-report-stage-read = Reading the file
open-report-stage-parse = Reading the header
open-report-stage-colour = Preparing colour management
open-report-stage-decode = Decoding
open-report-stage-upload = Sending to the GPU
open-report-stage-waiting = Waiting for other work
open-report-decoder = Decoded by
open-report-decoder-checked = { $engine }, checked against the CPU
open-report-decoder-cache = Display cache
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, at 1/{ $scale } size
open-report-decoder-scaled-full = { $engine }, at full size, then reduced to 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, at 1/{ $scale } size, without the quick-start index
open-report-colour-conversion = Colours converted by

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Recurring
open-report-advice-change = Would help
open-report-advice-gain = Estimated gain
open-report-advice-software = { $count ->
    [one] { $count } of the last { $total } slow { $format } images was decoded in software for this reason
   *[other] { $count } of the last { $total } slow { $format } images were decoded in software for this reason
}
open-report-advice-slow-storage = { $count ->
    [one] { $count } of the last { $total } slow images was held up reading the file (about { $speed } MB/s)
   *[other] { $count } of the last { $total } slow images were held up reading the file (about { $speed } MB/s)
}
open-report-advice-slow-storage-no-speed = { $count ->
    [one] { $count } of the last { $total } slow images was held up reading the file (small files: mostly waiting for the storage to respond)
   *[other] { $count } of the last { $total } slow images were held up reading the file (small files: mostly waiting for the storage to respond)
}
open-report-advice-archive = { $count ->
    [one] { $count } of the last { $total } slow images was held up unpacking the archive
   *[other] { $count } of the last { $total } slow images were held up unpacking the archive
}
open-report-advice-hardware-decoder = A GPU with a { $codec } hardware decoder
open-report-advice-gain-range = About { $low } to { $high } times faster
open-report-advice-gain-one = About { $low } times faster

## Performance insights: the indicator in the corner and the panel it opens
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Performance insights
open-report-insights-indicator = Why is this slow?
open-report-insights-announce = Performance insight: images keep opening slowly for the same reason. Open “Why is this slow?” to see what would help.
open-report-insights-hide = Hide
open-report-insights-last = Last slow image: { $total } until sharp
open-report-insights-now = What you can do now
open-report-insights-hardware = Hardware that would help
open-report-insights-none = No recurring slowdown
open-report-insights-none-detail = Insights appear when several images open slowly for the same reason. { -app-name } keeps a week of slow opens on this computer to find them; nothing is sent anywhere.
open-report-insights-software = { $format } images are decoded on the CPU
open-report-insights-storage = Reading the files holds images up
open-report-insights-archive = Unpacking archives holds images up
open-report-insights-nothing-now = Nothing in { -app-name }’s settings changes this.
open-report-insights-display-cache = Turn on the display cache: images you have opened before then open from a fast copy on this computer.
open-report-insights-prefetch = Keep more neighbouring images ready, so the next ones are prepared while you look.
open-report-insights-hw-ssd = A fast internal drive (an NVMe SSD) for these files
open-report-insights-action-turn-on = Turn on
open-report-insights-action-try-again = Try again
open-report-insights-action-automatic = Set to Automatic
open-report-insights-dismiss-storage = Don’t show again for storage
open-report-insights-dismiss-decoder = Don’t show again for decoding
open-report-insights-off = Turn insights off
open-report-insights-on = Turn insights on
open-report-insights-restore = Show hidden insights again
open-report-insights-power = This computer is saving power (on battery or in a power-saving mode), which slows decoding on the CPU. Plugged in, images may open faster.

## Performance insights in the gallery: the same, about making thumbnails
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Performance insight: thumbnails keep appearing slowly for the same reason. Open “Why is this slow?” to see what would help.
open-report-insights-last-thumbnail = Last slow thumbnail: { $total } to make
open-report-insights-thumbnails-software = Thumbnails of { $format } images are decoded on the CPU
open-report-insights-thumbnails-storage = Reading the files holds thumbnails up
open-report-insights-thumbnails-archive = Unpacking archives holds thumbnails up
open-report-insights-thumbnail-cache = Thumbnails cannot be kept on this computer (the thumbnail cache is unavailable), so they are made again each time the folder is shown.
open-report-advice-thumbnails-software = { $count ->
    [one] { $count } of the last { $total } slow { $format } thumbnails was decoded in software for this reason
   *[other] { $count } of the last { $total } slow { $format } thumbnails were decoded in software for this reason
}
open-report-advice-thumbnails-slow-storage = { $count ->
    [one] { $count } of the last { $total } slow thumbnails was held up reading the file (about { $speed } MB/s)
   *[other] { $count } of the last { $total } slow thumbnails were held up reading the file (about { $speed } MB/s)
}
open-report-advice-thumbnails-slow-storage-no-speed = { $count ->
    [one] { $count } of the last { $total } slow thumbnails was held up reading the file (small files: mostly waiting for the storage to respond)
   *[other] { $count } of the last { $total } slow thumbnails were held up reading the file (small files: mostly waiting for the storage to respond)
}
open-report-advice-thumbnails-archive = { $count ->
    [one] { $count } of the last { $total } slow thumbnails was held up unpacking the archive
   *[other] { $count } of the last { $total } slow thumbnails were held up unpacking the archive
}
