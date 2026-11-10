# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

## Command palette
palette-export-anonymised = Export anonymised copy…
palette-export-patient-disc = Export patient disc folder…
palette-mask-burned-in-text = Mask burned-in text

## The export dialog
anon-title = Export anonymised copy
anon-disc-title = Export patient disc folder
anon-images = Images
anon-images-current = This image
# $count is the number of files in the series.
anon-images-series = { $count ->
    [one] This series ({ $count } file)
   *[other] This series ({ $count } files)
}
# $count is the number of files in the study.
anon-images-study = { $count ->
    [one] This study ({ $count } file)
   *[other] This study ({ $count } files)
}
anon-images-reading = Reading the folder…
anon-anonymise = Anonymise
anon-dates = Dates and times
anon-dates-remove = Remove
anon-dates-keep = Keep
anon-dates-shift = Shift
anon-patient = Patient characteristics
anon-device = Device identity
anon-institution = Institution identity
anon-descriptions = Descriptions
anon-descriptions-remove = Remove
anon-descriptions-clean = Clean
anon-structured = Report content
anon-uids = UIDs
anon-uids-replace = Replace
anon-uids-keep = Keep
anon-overlays = Overlays and curves
anon-keep = Keep
anon-remove = Remove
anon-on = On
anon-off = Off
anon-burned-in = Burned-in text
# $count is the number of mask rectangles.
anon-masks = { $count ->
    [one] { $count } area masked
   *[other] { $count } areas masked
}
anon-masks-none = No masks
anon-folder = Folder
anon-folder-choose = Choose…
anon-export = Export
# Under the rows: why the images may need masks.
anon-risk-present = These images say they show patient text: mask it before export.
anon-risk-possible = Images of this kind often show patient text: check before export.
# $done and $total count files.
anon-progress = Exporting { $done } of { $total }…
anon-stopping = Stopping…

## The mask tool
anon-mask-title = Mask burned-in text
anon-mask-hint = Drag over text to mask it. Masked areas become black in every frame.
anon-mask-suggest = Suggest
anon-mask-remove = Remove
anon-mask-clear = Clear
anon-mask-done = Done
# $n is the area's number, $width and $height its size in pixels.
anon-mask-area = Area { $n }, { $width } × { $height } pixels
# $count is the number of areas the detector found.
anon-mask-suggested = { $count ->
    [0] No text found near the edges
    [one] { $count } text area suggested
   *[other] { $count } text areas suggested
}

## Results
# $count is the number of files written, $folder the folder's name.
anon-done = { $count ->
    [one] { $count } anonymised file written to { $folder }
   *[other] { $count } anonymised files written to { $folder }
}
anon-disc-done = { $count ->
    [one] Patient disc folder with { $count } image written to { $folder }
   *[other] Patient disc folder with { $count } images written to { $folder }
}
# Added to the result when files were left out.
anon-skipped-burned-in = { $count ->
    [one] { $count } file with burned-in text and no masks was skipped
   *[other] { $count } files with burned-in text and no masks were skipped
}
anon-skipped-failed = { $count ->
    [one] { $count } file could not be exported
   *[other] { $count } files could not be exported
}
anon-stopped = Export stopped
# $reason says what went wrong.
anon-failed = Could not export: { $reason }
anon-not-empty = The folder already holds a patient disc
