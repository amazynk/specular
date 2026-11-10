# DICOM gestructureerde verslagen, golfvormen, RT-dosis en oftalmologische tomografie. machine-drafted, needs native review

## Inhoudsboom van het gestructureerde verslag (de tagbrowser in verslagmodus)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Item
sr-col-concept = Concept
sr-col-type = Type
sr-col-relationship = Relatie
sr-col-value = Waarde
sr-show-image = Afbeelding tonen
sr-jump-missing = De verwezen afbeelding staat niet in deze map
sr-jump-frame = Geen afbeelding van dit referentiekader bevat dat punt

## Grafieken (dosis-volumehistogrammen, ECG-strips)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Sleep over een grafiek om te meten
plot-adjust-hint = + en − wijzigen de versterking, [ en ] de snelheid

## ECG-strip
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Nog één groep kanalen wordt niet getoond
   *[other] Nog { $count } groepen kanalen worden niet getoond
}

## RT-dosis over de MPR (isodoselijnen, kleurenkaart, legenda, uitlezing)
dose-value = Dosis { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dosis verborgen (Shift+U toont hem)
dose-on-rx = Dosis: { $dose } { $unit } voorgeschreven · max. { $max } { $unit }
dose-on-max = Dosis: percentage van het maximum, { $max } { $unit }
dose-colour = Dosis in { $colormap } op { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: het dosis-volumehistogram van één structuur
   *[other] Shift+K: de dosis-volumehistogrammen van { $count } structuren
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dosis als isodoselijnen
dose-mode-colour = Dosis als kleurenkaart

## Dosis-volumehistogrammen (het grafiekblad)
dvh-title = Dosis-volumehistogrammen
dvh-col-roi = Structuur
dvh-col-volume = Volume
dvh-axis-dose = Dosis ({ $unit })
dvh-axis-volume = Volume (%)
dvh-prescription = Voorschrift { $dose } Gy · cumulatief
dvh-max = Maximale dosis { $dose } { $unit } · cumulatief
dvh-none = Geen structuurset van dit referentiekader om het dosishistogram over te berekenen

## Oftalmologische tomografie (de B-scanschuifregelaar; het overzichtsbeeld in een hoek)
oct-bscan = B-scan
oct-depth = { $depth } µm diep
