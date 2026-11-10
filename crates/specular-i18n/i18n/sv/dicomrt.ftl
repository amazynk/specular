# DICOM strukturerade rapporter, vågformer, RT-dos och oftalmologisk tomografi. machine-drafted, needs native review

## Innehållsträd för den strukturerade rapporten (taggläsaren i rapportläge)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Post
sr-col-concept = Begrepp
sr-col-type = Typ
sr-col-relationship = Relation
sr-col-value = Värde
sr-show-image = Visa bilden
sr-jump-missing = Den refererade bilden finns inte i den här mappen
sr-jump-frame = Ingen bild med den här referensramen innehåller den punkten

## Diagram (dos-volymhistogram, EKG-ark)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Dra över ett diagram för att mäta
plot-adjust-hint = + och − ändrar förstärkningen, [ och ] hastigheten

## EKG-ark
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Ytterligare en kanalgrupp visas inte
   *[other] Ytterligare { $count } kanalgrupper visas inte
}

## RT-dos över MPR (isodoslinjer, färgkarta, teckenförklaring, avläsning)
dose-value = Dos { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dos dold (Shift+U visar den)
dose-on-rx = Dos: { $dose } { $unit } ordinerad · max { $max } { $unit }
dose-on-max = Dos: procent av max, { $max } { $unit }
dose-colour = Dos i { $colormap } vid { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: dos-volymhistogrammet för en struktur
   *[other] Shift+K: dos-volymhistogrammen för { $count } strukturer
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dos som isodoslinjer
dose-mode-colour = Dos som färgkarta

## Dos-volymhistogram (diagrambladet)
dvh-title = Dos-volymhistogram
dvh-col-roi = Struktur
dvh-col-volume = Volym
dvh-axis-dose = Dos ({ $unit })
dvh-axis-volume = Volym (%)
dvh-prescription = Ordination { $dose } Gy · kumulativ
dvh-max = Maxdos { $dose } { $unit } · kumulativ
dvh-none = Inget strukturset med den här referensramen att beräkna doshistogrammet över

## Oftalmologisk tomografi (B-scan-reglaget; översiktsbilden i ett hörn)
oct-bscan = B-scan
oct-depth = { $depth } µm djupt
