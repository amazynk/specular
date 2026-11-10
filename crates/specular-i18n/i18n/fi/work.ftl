# machine-drafted, needs native review
# Specular — Taustatyöt: ilmaisin, joka näkyy, kun jokin on käynnissä taustalla, ja sen avaama paneeli. Suomi.

## Paneeli

work-title = Taustatyöt
work-show = Näytä taustatyöt
work-empty = Taustalla ei ole mitään käynnissä.

## Työt

work-kind-library = Kuvakirjasto
work-kind-ai-search = Tekoälyhaku
work-kind-faces = Henkilöt
work-kind-text = Teksti kuvissa
work-kind-culling = Karsintapisteet
work-kind-download = Tekoälymallin lataus
work-kind-similar = Samankaltaiset kuvat

## Vaiheet

work-step-finding = Etsitään kuvia
work-step-reading = Luetaan kuvien tietoja
work-step-signatures = Verrataan kuvia
work-step-grouping = Ryhmitellään samankaltaisia kuvia
work-step-text-index = Luodaan hakuhakemistoa
work-step-embedding = Indeksoidaan kuvia tekoälyhakua varten
work-step-switching = Indeksoidaan uudelleen uutta mallikokoa varten
work-step-faces = Etsitään kasvoja
work-step-ocr = Luetaan tekstiä kuvista
work-step-scoring = Arvioidaan terävyyttä ja valotusta
work-step-downloading = Ladataan
work-step-focus = Tarkistetaan terävyyttä
work-step-item = { $step } · { $item }

## Edistyminen

work-count-photos = { $done } / { $total } { $count ->
    [one] kuva
   *[other] kuvaa
}
work-count = { $done } / { $total }
work-count-left = { $count } · { $left } jäljellä
work-so-far-photos = { $done } { $count ->
    [one] kuva
   *[other] kuvaa
} tähän mennessä
work-so-far = { $done } tähän mennessä

## Jäljellä oleva aika

work-eta-estimating = Arvioidaan…
work-eta-seconds = Noin { $n ->
    [one] { $n } sekunti
   *[other] { $n } sekuntia
} jäljellä
work-eta-minutes = Noin { $n ->
    [one] { $n } minuutti
   *[other] { $n } minuuttia
} jäljellä
work-eta-hours = Noin { $h ->
    [one] { $h } tunti
   *[other] { $h } tuntia
} jäljellä
work-eta-hours-minutes = Noin { $h ->
    [one] { $h } tunti
   *[other] { $h } tuntia
} { $m ->
    [one] { $m } minuutti
   *[other] { $m } minuuttia
} jäljellä

## Miksi työ odottaa

work-paused-browsing = Keskeytetty, kun selaat kuvia
work-paused-battery = Keskeytetty akkuvirralla
work-paused-low-power = Keskeytetty virransäästötilassa
work-paused-heat = Keskeytetty, kunnes tietokone jäähtyy
work-paused-memory = Keskeytetty, kunnes muistia vapautuu
work-paused-user = Keskeytetty

## Mitä seuraa

work-then = Sitten: { $steps }

## Painikkeet

work-pause = Keskeytä
work-resume = Jatka
work-settings = Asetukset…

## Ilmaisin

work-chip = { $job } · { $done } / { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Taustatyöt: { $status }

## Ilmoitukset

work-started = { $job }: aloitettu taustalla
work-finished = { $job }: valmis
