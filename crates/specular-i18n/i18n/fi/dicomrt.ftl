# DICOM-rakenteiset raportit, aaltomuodot, sädehoitoannos ja silmän tomografia. machine-drafted, needs native review

## Rakenteisen raportin sisältöpuu (tagiselain raporttitilassa)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Kohde
sr-col-concept = Käsite
sr-col-type = Tyyppi
sr-col-relationship = Suhde
sr-col-value = Arvo
sr-show-image = Näytä kuva
sr-jump-missing = Viitattu kuva ei ole tässä kansiossa
sr-jump-frame = Mikään tämän viitekehyksen kuva ei sisällä tätä pistettä

## Kuvaajat (annos-tilavuushistogrammit, EKG-arkit)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Mittaa vetämällä kuvaajan yli
plot-adjust-hint = + ja − muuttavat vahvistusta, [ ja ] nopeutta

## EKG-arkki
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Yhtä muuta kanavaryhmää ei näytetä
   *[other] { $count } muuta kanavaryhmää ei näytetä
}

## Sädehoitoannos MPR:n päällä (isoannoskäyrät, värikartta, selite, lukema)
dose-value = Annos { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Annos piilotettu (Shift+U näyttää sen)
dose-on-rx = Annos: { $dose } { $unit } määrätty · maks. { $max } { $unit }
dose-on-max = Annos: prosenttia maksimista, { $max } { $unit }
dose-colour = Annos värikartalla { $colormap }, { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: yhden rakenteen annos-tilavuushistogrammi
   *[other] Shift+K: { $count } rakenteen annos-tilavuushistogrammit
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Annos isoannoskäyrinä
dose-mode-colour = Annos värikarttana

## Annos-tilavuushistogrammit (kuvaaja-arkki)
dvh-title = Annos-tilavuushistogrammit
dvh-col-roi = Rakenne
dvh-col-volume = Tilavuus
dvh-axis-dose = Annos ({ $unit })
dvh-axis-volume = Tilavuus (%)
dvh-prescription = Määrätty annos { $dose } Gy · kumulatiivinen
dvh-max = Maksimiannos { $dose } { $unit } · kumulatiivinen
dvh-none = Tällä viitekehyksellä ei ole rakennejoukkoa, jonka yli annoshistogrammi laskettaisiin

## Silmän tomografia (B-skannauksen liukusäädin; yleiskuva nurkassa)
oct-bscan = B-skannaus
oct-depth = { $depth } µm:n syvyydessä
