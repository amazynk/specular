# machine-drafted, needs native review
# Specular — kaksoiskappaleet ja samankaltaiset kuvat (galleriaryhmät). Suomi.

similar-duplicates = Kaksoiskappaleet
similar-similar = Samankaltaiset kuvat

## Näkymä-valikon ja paletin kohteet
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } ryhmä
   *[other] { $groups } ryhmää
}, vapautettavissa { $size }
similar-menu-scanning = { $name } — etsitään ({ PERCENT($percent) })
similar-menu-none = { $name } — ei löytynyt

## Otsake
similar-summary = { $groups ->
    [one] { $groups } ryhmä tarkistettavana
   *[other] { $groups } ryhmää tarkistettavana
}, { $files ->
    [one] { $files } tiedosto
   *[other] { $files } tiedostoa
} hylättävänä, vapautettavissa { $size }
similar-progress = { $resolved }/{ $total } ratkaistu
similar-skipped = { $count } ohitettu
similar-scanning = Etsitään kaksoiskappaleita ja samankaltaisia kuvia… { $done }/{ $total }
# machine-drafted, needs native review
similar-checking-sharpness = Tarkistetaan terävyyttä… { $done }/{ $total }
# machine-drafted, needs native review
# Luettelo ensimmäisen haun aikana (sen edistyminen näkyy yläpuolella).
similar-waiting = Ryhmät näkyvät tässä, kun haku on valmis.
# machine-drafted, needs native review
similar-stopped = Haku pysäytetty: mitään ei ryhmitelty
similar-stopped-sharpness = Terävyyden tarkistus pysäytetty: parhaat valinnat pikkukuvien perusteella
similar-stopped-empty = Haku pysäytettiin. Sen mittaama säilyy: Hae uudelleen jatkaa siitä.
similar-scan-again = Hae uudelleen
similar-empty-duplicates = Kansiossa ei ole kaksoiskappaleita.
similar-empty-similar = Kansiossa ei ole samankaltaisia kuvia.
similar-all-done = Kaikki ryhmät tarkistettu.

## Samankaltaiset kuvat: kuinka samanlaisia kuvien on oltava ryhmittelyyn
similar-sensitivity = Herkkyys
similar-sensitivity-strict = Tiukka
similar-sensitivity-loose = Väljä

## Ryhmäluettelon alatunniste
similar-accept-all = Hyväksy kaikki ehdotukset
similar-show-skipped = Näytä ohitetut ryhmät
similar-discard-with = Hylkää: { $action }
similar-action-trash = Siirrä roskakoriin
similar-action-move = Siirrä kansioon…
similar-action-move-to = Siirrä kansioon ”{ $folder }”
similar-action-reject = Merkitse vain hylätyksi
similar-choose-folder = Valitse kansio hylätyille tiedostoille…

## Joukkovahvistus
similar-confirm = Hyväksytäänkö { $groups ->
    [one] { $groups } ryhmän
   *[other] { $groups } ryhmän
} ehdotukset? { $files ->
    [one] { $files } tiedosto
   *[other] { $files } tiedostoa
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Hyväksy ja hylkää { $files } tiedosto
   *[other] Hyväksy ja hylkää { $files } tiedostoa
}
similar-cancel = Peruuta

## Ryhmä
similar-group-title = Ryhmä { $index }/{ $total }
similar-group-photos = { $count ->
    [one] { $count } kuva
   *[other] { $count } kuvaa
}
similar-to-free = Vapautettavissa { $size }

## Tarkistustoiminnot ja tilat
similar-keep = Säilytä
similar-discard = Hylkää
similar-undecided = Päättämättä
similar-accept-next = Hyväksy ja seuraava
similar-not-similar = Ei samankaltainen
similar-skip = Ohita
similar-compare = Vertaa
similar-zoom = { PERCENT(100) }
similar-back = Takaisin
similar-done = Valmis
similar-suggested-keep = Ehdotus: säilytä
similar-suggested-discard = Ehdotus: hylkää

## Ilmoitukset
similar-toast-accepted = { $count ->
    [one] { $count } ryhmä hyväksytty
   *[other] { $count } ryhmää hyväksytty
}
similar-toast-rejected = { $count ->
    [one] { $count } tiedosto merkitty hylätyksi
   *[other] { $count } tiedostoa merkitty hylätyksi
}
similar-toast-protected = Arvioidut, merkityt, värimerkityt, avainsanoitetut tai muokatut kuvat säilytetään aina.
# machine-drafted, needs native review
similar-zoom-already = Tämä kuva näkyy jo { PERCENT(100) }:n kokoisena tai suurempana

## Saavutettavuus
similar-a11y-member = { $name }: { $state }, { $reason }

## Näppäinvihjeet
similar-hint-list = ↑↓ valitse · Enter tarkista · A hyväksy kaikki · Tab vaihda · Esc sulje

## Miksi kuva säilytetään
similar-reason-rated = Arvioitu
similar-reason-flagged = Merkitty valituksi
similar-reason-labelled = Värimerkitty
similar-reason-tagged = Avainsanoja
similar-reason-edited = Muokattu
similar-reason-original = Alkuperäinen
similar-reason-raw-original = RAW-alkuperäinen
similar-reason-highest-resolution = Suurin tarkkuus
similar-reason-deeper-colour = Suurempi värisyvyys
similar-reason-least-compressed = Vähiten pakattu
similar-reason-has-metadata = Metatiedot tallella
similar-reason-oldest = Vanhin kopio
similar-reason-sharpest = Terävin
similar-reason-steadiest = Vähiten liike-epäterävyyttä
similar-reason-best-exposure = Parhaiten valotettu
similar-reason-least-noise = Vähiten kohinaa
similar-reason-also-good = Myös hyvä

## Miksi kuvaa ehdotetaan hylättäväksi
similar-reason-same-file = Identtinen tiedosto
similar-reason-derivative = Johdettu kopio
similar-reason-rendering-of-raw = RAW-tiedostosta tuotettu
similar-reason-smaller-copy = Pienempi kopio
similar-reason-shallower-colour = Pienempi värisyvyys
similar-reason-more-compressed = Enemmän pakattu
similar-reason-no-metadata = Metatiedot poistettu
similar-reason-newer-copy = Uudempi kopio
similar-reason-blurrier = Epäterävämpi
similar-reason-motion-blur = Liike-epäterävyys
similar-reason-overexposed = Ylivalottunut
similar-reason-underexposed = Alivalottunut
similar-reason-noisier = Kohinaisempi
similar-reason-lower-resolution = Pienempi tarkkuus
