# Strukturované nálezy DICOM, křivky, dávka z radioterapie a oftalmologická tomografie. machine-drafted, needs native review

## Strom obsahu strukturovaného nálezu (prohlížeč tagů v režimu nálezu)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Položka
sr-col-concept = Pojem
sr-col-type = Typ
sr-col-relationship = Vztah
sr-col-value = Hodnota
sr-show-image = Zobrazit snímek
sr-jump-missing = Odkazovaný snímek není v této složce
sr-jump-frame = Žádný snímek tohoto referenčního rámce tento bod neobsahuje

## Grafy (histogramy dávka–objem, záznamy EKG)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Táhnutím přes graf změříte
plot-adjust-hint = + a − mění zesílení, [ a ] rychlost

## Záznam EKG
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Jedna další skupina kanálů není zobrazena
    [few] Další { $count } skupiny kanálů nejsou zobrazeny
    [many] Dalších { $count } skupiny kanálů není zobrazeno
   *[other] Dalších { $count } skupin kanálů není zobrazeno
}

## Dávka z radioterapie nad MPR (izodózy, barevná mapa, legenda, odečet)
dose-value = Dávka { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dávka skryta (Shift+U ji zobrazí)
dose-on-rx = Dávka: předepsáno { $dose } { $unit } · max. { $max } { $unit }
dose-on-max = Dávka: procenta maxima, { $max } { $unit }
dose-colour = Dávka v { $colormap } na { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: histogram dávka–objem jedné struktury
    [few] Shift+K: histogramy dávka–objem { $count } struktur
    [many] Shift+K: histogramy dávka–objem { $count } struktury
   *[other] Shift+K: histogramy dávka–objem { $count } struktur
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dávka jako izodózy
dose-mode-colour = Dávka jako barevná mapa

## Histogramy dávka–objem (list s grafy)
dvh-title = Histogramy dávka–objem
dvh-col-roi = Struktura
dvh-col-volume = Objem
dvh-axis-dose = Dávka ({ $unit })
dvh-axis-volume = Objem (%)
dvh-prescription = Předepsaná dávka { $dose } Gy · kumulativní
dvh-max = Maximální dávka { $dose } { $unit } · kumulativní
dvh-none = Žádná sada struktur tohoto referenčního rámce, nad kterou by šlo spočítat histogram dávky

## Oftalmologická tomografie (posuvník B-skenů; přehledový snímek v rohu)
oct-bscan = B-sken
oct-depth = hloubka { $depth } µm
