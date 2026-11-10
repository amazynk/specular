# Specular — Export slideshow as video (DESIGN.md §6.5). English.

palette-export-slideshow = Export slideshow as video…
settings-viewing-toolbar-items-export-slideshow = Export slideshow as video

video-export-title = Export slideshow as video
video-export-resolution = Resolution
# $width and $height in pixels.
video-export-resolution-window = Window ({ $width } × { $height })
video-export-fps = Frame rate
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Quality
video-export-quality-high = High
video-export-quality-standard = Standard
video-export-quality-compact = Compact
# Which players can play the video: smaller files for current players (High profile),
# or the largest set of players including old ones (Baseline profile).
video-export-compatibility = Compatibility
video-export-compatibility-current = Current players
video-export-compatibility-all = All players
video-export-images = { $count ->
    [one] One image, in the slideshow's order, transition and captions
   *[other] { $count } images, in the slideshow's order, transition and captions
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = About { $length } long, about { $size }
video-export-export = Export…
video-export-replace = Replace
# $file a file name.
video-export-replace-question = { $file } already exists. Replace it?
# $percent a formatted percentage ("42%").
video-export-progress = Exporting… { $percent }
video-export-cancelling = Cancelling…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Saved { $file } ({ $size })
video-export-skipped = { $count ->
    [one] One image could not be read and was left out.
   *[other] { $count } images could not be read and were left out.
}
video-export-failed = Could not export the video: { $reason }
video-export-cancelled = Export cancelled
video-export-no-images = None of the images could be read.
video-export-show-in-folder = Show in folder
