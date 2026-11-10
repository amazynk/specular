# DICOM structured reports, waveforms, RT dose and ophthalmic tomography.

## Structured report content tree (the tag browser in report mode)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Item
sr-col-concept = Concept
sr-col-type = Type
sr-col-relationship = Relationship
sr-col-value = Value
sr-show-image = Show the image
sr-jump-missing = The referenced image is not in this folder
sr-jump-frame = No image of this frame of reference holds that point

## Plots (dose volume histograms, ECG sheets)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Drag across a plot to measure
plot-adjust-hint = + and − change the gain, [ and ] the speed

## ECG sheet
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] One more group of channels is not shown
   *[other] { $count } more groups of channels are not shown
}

## RT dose over MPR (isodose lines, colour map, legend, readout)
dose-value = Dose { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dose hidden (Shift+U shows it)
dose-on-rx = Dose: { $dose } { $unit } prescribed · max { $max } { $unit }
dose-on-max = Dose: percent of the max, { $max } { $unit }
dose-colour = Dose in { $colormap } at { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: the dose volume histogram of one structure
   *[other] Shift+K: the dose volume histograms of { $count } structures
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dose as isodose lines
dose-mode-colour = Dose as a colour map

## Dose volume histograms (the plot sheet)
dvh-title = Dose volume histograms
dvh-col-roi = Structure
dvh-col-volume = Volume
dvh-axis-dose = Dose ({ $unit })
dvh-axis-volume = Volume (%)
dvh-prescription = Prescription { $dose } Gy · cumulative
dvh-max = Maximum dose { $dose } { $unit } · cumulative
dvh-none = No structure set of this frame of reference to histogram the dose over

## Ophthalmic tomography (the B-scan slider; the localiser in a corner)
oct-bscan = B-scan
oct-depth = { $depth } µm deep
