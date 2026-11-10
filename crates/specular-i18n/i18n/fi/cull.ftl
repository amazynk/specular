# machine-drafted, needs native review
# Specular — Karsintatila (DESIGN.md §6.3).

palette-cull = Siirry karsintatilaan tai poistu siitä
cheatsheet-cull = Karsintatila
settings-section-cull = Karsintatila
settings-cull-auto-enter = Karsi RAW-kansiot
settings-cull-auto-enter-help = Avaa enimmäkseen RAW-tiedostoja sisältävä kansio suoraan karsintatilaan.
settings-cull-auto-advance = Seuraava kuva merkinnän jälkeen
settings-cull-auto-advance-help = Siirry eteenpäin valinnan, hylkäyksen tai tähtiarvion jälkeen. Värimerkit ja merkinnän poisto pysyvät kuvassa.
settings-cull-show = Näytä RAW+JPEG-pareista
settings-cull-show-help = Samanniminen RAW- ja JPEG-tiedosto karsitaan yhtenä kuvana, ja merkinnät koskevat molempia. JPEG avautuu nopeammin.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Pinoa sarjat
settings-cull-bursts-help = Yhdellä kameralla nopeasti peräkkäin otetut kuvat näytetään yhtenä pinona. S avaa tai sulkee pinon, B säilyttää näytetyn kuvan ja hylkää pinon muut.
settings-cull-burst-gap = Sarjan tauko
settings-cull-burst-gap-help = Pisin tauko kahden saman sarjan kuvan välillä.
settings-cull-watch = Näytä uudet tiedostot
settings-cull-watch-help = Karsinnan aikana kansioon ilmestyvät tiedostot (liitetty kamera, tuonti) lisätään loppuun, kun ne on kirjoitettu kokonaan.
settings-cull-follow-newest = Seuraa uusinta tiedostoa
settings-cull-follow-newest-help = Siirry jokaiseen uuteen tiedostoon sen saapuessa. T vaihtaa tätä karsinnan aikana.
settings-cull-slot-1 = Kansio 1
settings-cull-slot-1-help = Alt+1 kopioi kuvan (RAW- tai JPEG-pareineen ja sivutiedostoineen) tähän kansioon, Alt+Vaihto+1 siirtää sen sinne.
settings-cull-slot-1-unset = Ei asetettu
settings-cull-slot-2 = Kansio 2
settings-cull-slot-2-help = Alt+2 kopioi kuvan (RAW- tai JPEG-pareineen ja sivutiedostoineen) tähän kansioon, Alt+Vaihto+2 siirtää sen sinne.
settings-cull-slot-2-unset = Ei asetettu
settings-cull-slot-3 = Kansio 3
settings-cull-slot-3-help = Alt+3 kopioi kuvan (RAW- tai JPEG-pareineen ja sivutiedostoineen) tähän kansioon, Alt+Vaihto+3 siirtää sen sinne.
settings-cull-slot-3-unset = Ei asetettu
settings-cull-collisions = Kun nimi on varattu
settings-cull-collisions-help = Kansioiden tiedostoja ei koskaan korvata.
settings-cull-collisions-suffix = Lisää numero
settings-cull-collisions-skip = Ohita kuva
# machine-drafted, needs native review
# Karsintatila valittu, kun yhtään kansiota tai gallerian kuvia ei ole avattu.
cull-nothing = Ei karsittavaa – avaa ensin kuvakansio
cull-slot-unset = Kansiota { $slot } ei ole asetettu – valitse se kohdassa Asetukset → Karsintatila
cull-slot-missing = Kansio { $slot } ei ole käytettävissä: { $reason }
cull-copied = Kopioitu kohteeseen { $folder }
cull-moved = Siirretty kohteeseen { $folder }
cull-slot-renamed = Tallennettu nimellä { $name } kohteeseen { $folder }
cull-slot-skipped = Kohteessa { $folder } on jo { $name } – ohitettu
cull-slot-same = Kuva on jo kohteessa { $folder }
cull-slot-failed = { $op ->
    [move] Tiedostoa { $file } ei voitu siirtää
   *[copy] Tiedostoa { $file } ei voitu kopioida
}: { $reason }
cull-copy-kept = { $count ->
    [one] Säilytettiin kopio, jota on sittemmin muutettu
   *[other] Säilytettiin { $count } kopiota, joita on sittemmin muutettu
}
cull-undo-failed = Kumoaminen ei onnistunut tiedostolle { $file }: { $reason }
cull-burst-open = Sarja { $index }/{ $count }
cull-burst-closed = Sarja, { $count } kuvaa
cull-burst-toggle = Avaa tai sulje sarja
cull-burst-best = 1 säilytetty, { $count } hylätty
cull-burst-none = Ei osa sarjaa
cull-follow-on = Uusia tiedostoja seurataan
cull-follow-off = Uusia tiedostoja ei seurata
cull-new-files = { $count ->
    [one] 1 uusi tiedosto
   *[other] { $count } uutta tiedostoa
}
cull-key-copy = Kopioi kansioon 1–3
cull-key-move = Siirrä kansioon 1–3
cull-key-burst = Avaa tai sulje sarja
cull-key-best = Säilytä tämä, hylkää sarjan muut
cull-key-follow = Seuraa uusia tiedostoja
cull-key-subframes = Näytä tai piilota osakuvataulukko
cull-key-sort = Lajittele seuraavan sarakkeen mukaan
cull-key-sort-reverse = Käännä järjestys
cull-key-metric = Kaavioon seuraava mittari
cull-key-sigma-down = Automaattinen hylkäys: pienennä k
cull-key-sigma-up = Automaattinen hylkäys: suurenna k
cull-key-auto-reject = Hylkää ehdotetut kuvat
cull-key-move-rejects = Siirrä hylätyt kuvat alikansioon
cull-key-same-stretch = Sama venytys kaikille kuville

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Avaa sarja
cull-tip-burst-close = Sulje sarja
cull-tip-exit = Poistu karsintatilasta
cull-tip-label = Tunniste: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Osakuvat
cull-astro-title = { $count ->
    [one] 1 osakuva
   *[other] { $count } osakuvaa
}
cull-astro-pending = mitataan { $count }
cull-astro-col-name = Kuva
cull-astro-col-filter = Suodin
cull-astro-col-exposure = Valot.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Eksk.
cull-astro-col-stars = Tähdet
cull-astro-col-bg = Tausta
cull-astro-col-snr = SNR
cull-astro-col-noise = Kohina
cull-astro-same-stretch = Sama venytys
cull-astro-own-stretch = Oma venytys
cull-astro-suggest-high = { $metric } yli { $value } (mediaani + { $k }σ): { $count ->
    [one] 1 kuva
   *[other] { $count } kuvaa
}
cull-astro-suggest-low = { $metric } alle { $value } (mediaani − { $k }σ): { $count ->
    [one] 1 kuva
   *[other] { $count } kuvaa
}
cull-astro-suggest-none = Ei kuvia { $k }σ:n ulkopuolella
cull-astro-suggest-few = Automaattinen hylkäys tarvitsee 3 mitattua kuvaa
cull-astro-suggested = ehdotettu hylättäväksi
cull-astro-k = k { $k }
cull-astro-reject = Hylkää { $count }
cull-astro-move = Siirrä hylätyt…
cull-astro-chart = { $metric }, { $count ->
        [one] { $count } kuva
       *[other] { $count } kuvaa
    }, mediaani { $median }.
cull-astro-sort-by = Lajittele: { $column }
cull-astro-ascending = nouseva
cull-astro-descending = laskeva
cull-astro-sorted = Lajiteltu: { $column }
cull-astro-reversed = Järjestys käännetty
cull-astro-metric = Kaavio: { $metric }
cull-astro-k-set = Automaattinen hylkäys: { $k }σ
cull-astro-none = Ei osakuvia täällä
cull-astro-same-on = Sama venytys kaikille kuville
cull-astro-same-off = Jokainen kuva venytetään erikseen
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = eksentrisyys { $value }
cull-astro-line-stars = { $count ->
    [one] 1 tähti
   *[other] { $count } tähteä
}
cull-astro-measuring = mitataan tähtiä…
cull-astro-reject-confirm = { $count ->
    [one] Hylätäänkö ehdotettu kuva?
   *[other] Hylätäänkö { $count } ehdotettua kuvaa?
}
cull-astro-reject-detail = { $rule }. Kumoa palauttaa merkinnät.
cull-astro-reject-ok = Hylkää
cull-astro-nothing-to-reject = Ei hylättäviä kuvia
cull-astro-rejected = { $count ->
    [one] Hylättiin 1 kuva
   *[other] Hylättiin { $count } kuvaa
}
cull-move-rejects-confirm = { $count ->
    [one] Siirretäänkö hylätty kuva kansioon ”{ $folder }”?
   *[other] Siirretäänkö { $count } hylättyä kuvaa kansioon ”{ $folder }”?
}
cull-move-rejects-detail = Tiedostot siirretään, niitä ei poisteta. Kumoa palauttaa ne.
cull-move-rejects-ok = Siirrä
cull-move-rejects-none = Ei siirrettäviä hylättyjä kuvia
cull-moved-rejects = { $count ->
    [one] Siirrettiin 1 kuva kansioon ”{ $folder }”
   *[other] Siirrettiin { $count } kuvaa kansioon ”{ $folder }”
}
