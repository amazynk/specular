# Develop: tone zones, batch conflicts, RAW+JPEG pairs, CMYK sources.

develop-tone-zones = Tone zones
develop-tone-zone = { $ev } EV
develop-tone-zones-adjust = Adjust on the picture
develop-tone-zones-done = Done adjusting
develop-tone-zones-adjusting = Drag up or down on the picture to raise or lower that tone. Esc ends.
develop-tone-zone-picked = Zone { $zone }

develop-batch-conflict-title = { $count ->
    [one] One photo changed outside Specular
   *[other] { $count } photos changed outside Specular
}
develop-batch-conflict-detail = Their sidecar files are newer than the library's copies.
develop-batch-conflict-keep = Keep: the paste goes on top of the file's changes.
develop-batch-conflict-replace = Replace: the paste goes on the library's copy. The file's changes are lost.
develop-batch-conflict-skip-detail = Skip: those photos stay as they are.
develop-batch-conflict-file = Keep the file's changes
develop-batch-conflict-library = Replace them
develop-batch-conflict-skip = Skip those photos

develop-pair-raw = Developing { $file }, the RAW of this pair
develop-source-cmyk = This CMYK photo is edited in RGB. It was converted with its profile.
