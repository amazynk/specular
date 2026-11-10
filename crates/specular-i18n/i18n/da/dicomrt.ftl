# DICOM-strukturerede rapporter, kurveformer, RT-dosis og oftalmologisk tomografi. machine-drafted, needs native review

## Indholdstræ for den strukturerede rapport (tagbrowseren i rapporttilstand)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Element
sr-col-concept = Begreb
sr-col-type = Type
sr-col-relationship = Relation
sr-col-value = Værdi
sr-show-image = Vis billedet
sr-jump-missing = Det refererede billede er ikke i denne mappe
sr-jump-frame = Intet billede med denne referenceramme indeholder det punkt

## Kurver (dosis-volumen-histogrammer, EKG-ark)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Træk hen over en kurve for at måle
plot-adjust-hint = + og − ændrer forstærkningen, [ og ] hastigheden

## EKG-ark
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Én gruppe kanaler mere vises ikke
   *[other] { $count } grupper kanaler mere vises ikke
}

## RT-dosis over MPR (isodosekurver, farvekort, forklaring, aflæsning)
dose-value = Dosis { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dosis skjult (Shift+U viser den)
dose-on-rx = Dosis: { $dose } { $unit } ordineret · maks. { $max } { $unit }
dose-on-max = Dosis: procent af maksimum, { $max } { $unit }
dose-colour = Dosis i { $colormap } ved { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: dosis-volumen-histogrammet for én struktur
   *[other] Shift+K: dosis-volumen-histogrammerne for { $count } strukturer
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dosis som isodosekurver
dose-mode-colour = Dosis som farvekort

## Dosis-volumen-histogrammer (kurvearket)
dvh-title = Dosis-volumen-histogrammer
dvh-col-roi = Struktur
dvh-col-volume = Volumen
dvh-axis-dose = Dosis ({ $unit })
dvh-axis-volume = Volumen (%)
dvh-prescription = Ordination { $dose } Gy · kumulativ
dvh-max = Maksimal dosis { $dose } { $unit } · kumulativ
dvh-none = Intet struktursæt med denne referenceramme at lave dosishistogrammet over

## Oftalmologisk tomografi (B-scan-skyderen; oversigtsbilledet i et hjørne)
oct-bscan = B-scan
oct-depth = { $depth } µm dybt
