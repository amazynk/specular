# DICOM series and the tag browser.

## Command palette
palette-dicom-tags = Show DICOM tags
palette-next-series = Next series
palette-prev-series = Previous series

## Why a command is unavailable
disabled-not-dicom = The image is not a DICOM file
disabled-no-other-series = There is no other series in this folder

## Tag browser
dicom-tags-title = DICOM tags
# $file is the file name.
dicom-tags-title-file = DICOM tags: { $file }
dicom-tags-filter = Filter tags
dicom-tags-show-patient = Show patient data
dicom-tags-col-tag = Tag
dicom-tags-col-keyword = Keyword
dicom-tags-col-vr = VR
dicom-tags-col-length = Length
dicom-tags-col-value = Value
# Shown in place of a patient-identifying value while patient data is hidden.
dicom-tags-hidden = Hidden
dicom-tags-hidden-count = { $count ->
    [one] { $count } patient value hidden
   *[other] { $count } patient values hidden
}
dicom-tags-no-match = No tags match
dicom-tags-reading = Reading tags…
# $reason says why the file could not be read.
dicom-tags-error = Could not read the tags: { $reason }
# A sequence's value: how many items it holds.
dicom-tags-items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
# Encapsulated pixel data: how many fragments it holds.
dicom-tags-fragments = { $count ->
    [one] { $count } fragment
   *[other] { $count } fragments
}
# One item of a sequence; $index counts from 1.
dicom-tags-item = Item { $index }
# A binary value, by its length.
dicom-tags-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } bytes
}
dicom-tags-empty = (empty)
dicom-tags-omitted = { $count ->
    [one] { $count } more element not listed
   *[other] { $count } more elements not listed
}

## Series (info panel group and messages)
dicom-series-group = Series
# $index and $count count the slices of the series.
dicom-series-slice = Slice
dicom-series-slice-value = { $index } of { $count }
# $frames: frames in this file (a multi-frame file is one item of its series).
dicom-series-frames = { $frames ->
        [one] { $frames } frame
       *[other] { $frames } frames
    }
dicom-series-location = Slice location
# $mm is a distance in millimetres.
dicom-series-mm = { $mm } mm
dicom-series-spacing = Slice spacing
dicom-series-which = Series
# $index and $count count the series found with this file.
dicom-series-which-value = { $index } of { $count }
dicom-series-order = Order
dicom-series-order-position = By position
dicom-series-order-instance = By instance number
dicom-series-order-time = By acquisition time
dicom-series-order-name = By file name
dicom-series-gaps = Missing slices
# $count slices missing, judged from the spacing.
dicom-series-gaps-value = { $count ->
    [one] { $count } slice missing
   *[other] { $count } slices missing
}
# $slices lists the slice numbers after which slices are missing ("7, 12").
dicom-series-gaps-where = After slice { $slices }
dicom-series-irregular = Spacing is irregular
dicom-series-tilt = Gantry tilt
# $degrees is the angle between the slice normal and the line of slice positions.
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Duplicates skipped
# Toast when a DICOM file opens with its series. $count: slices; $series: series found.
dicom-series-toast = { $series ->
    [one] { $count ->
        [one] Series of { $count } slice
       *[other] Series of { $count } slices
    }
   *[other] { $count ->
        [one] Series { $index } of { $series }: { $count } slice
       *[other] Series { $index } of { $series }: { $count } slices
    }
}
# Added when the folder held more files than are read for a series.
dicom-series-capped = Only the first { $count } files of the folder were read

## Studies browser (a patient's CD, DVD or USB stick)
studies-title = Studies
# $medium is the disc's or the folder's name.
studies-title-medium = Studies: { $medium }
studies-show-patient = Show patient data
# Shown in place of a patient's name or birth date while patient data is hidden.
studies-patient-hidden = Hidden
studies-patient-hidden-note = Patient names and birth dates are hidden
studies-none = No studies found
studies-reading = Reading the disc…
# $done and $total count the files read so far and the files to read.
studies-reading-count = Reading the disc: { $done } of { $total ->
        [one] { $total } file
       *[other] { $total } files
    }
studies-opening = Opening the series…
# $reason says why the disc could not be read.
studies-error = Could not read the disc: { $reason }
studies-missing = { $count ->
    [one] { $count } file listed on the disc is missing
   *[other] { $count } files listed on the disc are missing
}
studies-capped = Only the first { $count } files were read
# The reading was cancelled: what was read so far stays listed. $done and $total count
# files as in studies-reading-count.
studies-stopped = Reading stopped after { $done } of { $total ->
        [one] { $total } file
       *[other] { $total } files
    }: the list is incomplete
studies-stopped-early = Reading stopped: the list is incomplete
studies-cancel-description = Stops reading the disc. What was read so far stays listed.
# Stands in for the patient's name while patient data is hidden; $index counts from 1.
studies-patient-n = Patient { $index }
studies-patient-unknown = Unknown patient
# $date is the patient's birth date.
studies-born = Born { $date }
studies-no-description = No description
# A series' title: its number and its description.
studies-series = Series { $number }: { $description }
studies-series-number = Series { $number }
studies-images = { $count ->
    [one] { $count } image
   *[other] { $count } images
}
# Images of a series that the disc lists but does not hold.
studies-images-missing = { $count } missing
# Objects of a series that are not images (reports, presentation states).
studies-others = { $count ->
    [one] { $count } other object
   *[other] { $count } other objects
}
studies-no-images = This series has no images on the disc
# Toast when a disc or USB stick with a DICOMDIR is inserted.
studies-medium-inserted = A disc with medical studies was inserted
studies-medium-show = Show studies
palette-studies = Show studies on this disc
