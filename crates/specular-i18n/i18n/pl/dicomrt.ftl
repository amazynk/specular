# Raporty strukturalne DICOM, przebiegi, dawka w radioterapii i tomografia okulistyczna. machine-drafted, needs native review

## Drzewo zawartości raportu strukturalnego (przeglądarka tagów w trybie raportu)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Element
sr-col-concept = Pojęcie
sr-col-type = Typ
sr-col-relationship = Relacja
sr-col-value = Wartość
sr-show-image = Pokaż obraz
sr-jump-missing = Obrazu, do którego odwołuje się raport, nie ma w tym folderze
sr-jump-frame = Żaden obraz tego układu odniesienia nie zawiera tego punktu

## Wykresy (histogramy dawka-objętość, zapisy EKG)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Przeciągnij po wykresie, aby zmierzyć
plot-adjust-hint = + i − zmieniają wzmocnienie, [ i ] prędkość

## Zapis EKG
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Jeszcze jedna grupa kanałów nie jest pokazana
    [few] Jeszcze { $count } grupy kanałów nie są pokazane
    [many] Jeszcze { $count } grup kanałów nie jest pokazanych
   *[other] Jeszcze { $count } grupy kanałów nie są pokazane
}

## Dawka w radioterapii na MPR (izodozy, mapa kolorów, legenda, odczyt)
dose-value = Dawka { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dawka ukryta (Shift+U ją pokazuje)
dose-on-rx = Dawka: { $dose } { $unit } zaplanowana · maks. { $max } { $unit }
dose-on-max = Dawka: procent maksimum, { $max } { $unit }
dose-colour = Dawka w { $colormap } przy { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: histogram dawka-objętość jednej struktury
    [few] Shift+K: histogramy dawka-objętość { $count } struktur
    [many] Shift+K: histogramy dawka-objętość { $count } struktur
   *[other] Shift+K: histogramy dawka-objętość { $count } struktury
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dawka jako izodozy
dose-mode-colour = Dawka jako mapa kolorów

## Histogramy dawka-objętość (arkusz wykresów)
dvh-title = Histogramy dawka-objętość
dvh-col-roi = Struktura
dvh-col-volume = Objętość
dvh-axis-dose = Dawka ({ $unit })
dvh-axis-volume = Objętość (%)
dvh-prescription = Dawka przepisana { $dose } Gy · skumulowany
dvh-max = Dawka maksymalna { $dose } { $unit } · skumulowany
dvh-none = Brak zestawu struktur tego układu odniesienia, dla którego można wyznaczyć histogram dawki

## Tomografia okulistyczna (suwak B-skanów; obraz poglądowy w rogu)
oct-bscan = B-skan
oct-depth = głębokość { $depth } µm
