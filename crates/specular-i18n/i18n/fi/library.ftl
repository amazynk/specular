# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Kirjasto

## Top bar
library-title = Kirjasto
library-locations-button = Sijainnit

## Locations sidebar
library-locations = Sijainnit
library-all = Kaikki sijainnit
library-add = Lisää sijainti…
library-remove = Poista { $location } kirjastosta
library-offline = Ei yhteyttä
library-location-pending = { $count } indeksoimatta
library-a11y-location = { $location }, { $count ->
    [one] { $count } kuva
   *[other] { $count } kuvaa
}
library-a11y-location-offline = { $location }, Ei yhteyttä, { $count ->
    [one] { $count } kuva
   *[other] { $count } kuvaa
}

## Indexing progress
library-walking = Kuvia löydetty: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Kuvia löydetty: { $count }
library-reading = Indeksoidaan… { $done }/{ $total }
library-held = Indeksointi pysähtyy selaamisen ajaksi
library-up-to-date = Ajan tasalla
# machine-drafted, needs native review
library-stopped = Indeksointi pysäytetty
library-stopped-pending = Indeksointi pysäytetty · { $count } indeksoimatta
library-resume = Jatka
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indeksointi pysähtyi: { $reason }
library-reason-stalled = mikään ei ole edennyt { $minutes ->
    [one] minuuttiin
   *[other] { $minutes } minuuttiin
}. Tiedosto tai asema ei ehkä vastaa.
library-retry = Yritä uudelleen
library-retry-description = Aloittaa indeksoinnin uudelleen. Jo indeksoitu pysyy haettavana.
library-save-diagnostics = Tallenna diagnostiikkapaketti…
library-stop-description = Lopettaa indeksoinnin. Tähän mennessä indeksoitu pysyy haettavana; Jatka tai Etsi uudelleen (F5) jatkaa.
library-resume-description = Käy kaikki sijainnit uudelleen läpi ja indeksoi jäljellä olevat.
library-opening = Avataan kirjastoa…
library-searching = Haetaan…

## Empty and error states
library-empty = Ei vielä sijainteja. Lisää kansio: sen ja sen alikansioiden kuvat näkyvät tässä yhdessä ruudukossa.
library-empty-offline = Nämä sijainnit eivät ole käytettävissä. Niiden kuvat näkyvät taas, kun ne palaavat.
library-indexing = Etsitään kuvia…
# machine-drafted, needs native review
library-remove-confirm = Poistetaanko ”{ $location }” kirjastosta?
library-remove-confirm-detail = Kuvasi pysyvät levyllä arvioineen, värimerkkeineen ja avainsanoineen. Kirjasto unohtaa, mitä se indeksoi sieltä; jos lisäät kansion uudelleen, se indeksoidaan uudelleen.
library-remove-confirm-ok = Poista
library-indexing-in = Etsitään kuvia sijainnista { $location }… { $count } löydetty tähän mennessä
# machine-drafted, needs native review
library-empty-stopped = Indeksointi pysäytettiin ennen kuin näitä kuvia luettiin. Jatka jatkaa siitä.
library-unavailable = Kirjastoa ei voitu avata: { $reason }
library-read-only = Tämän kirjaston kirjoitti uudempi Specular: siitä voi hakea, mutta sitä ei voi päivittää.

## Screen readers
library-a11y-list = Kirjasto, { $count ->
    [one] { $count } kuva
   *[other] { $count } kuvaa
}
library-a11y-added = { $location } lisätty kirjastoon
library-a11y-removed = { $location } poistettu kirjastosta

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Lisätäänkö kaikki kansiosta ”{ $folder }”?
library-home-confirm-detail = Siinä on ohjelmatiedostoja, latauksia ja välimuisteja, ei vain valokuvia. Indeksointi kestää kauemmin, ja kirjasto täyttyy kuvakkeista ja muista kuvista.
library-home-confirm-pictures = Kuvat-kansiossasi on vain kuvasi.
library-home-confirm-use-pictures = Lisää Kuvat
library-home-confirm-keep = Lisää kotikansio

## Folder watches
# machine-drafted, needs native review
library-unwatched = Joitakin kansioita ei seurata
library-unwatched-detail = { $minutes ->
    [one] Muutokset näkyvät minuutin kuluessa
   *[other] Muutokset näkyvät { $minutes } min kuluessa
}
library-unwatched-description = Specularin kansioseurannat ovat loppuneet. Avaa ohjeen rajan nostamisesta.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Kirjasto
settings-library-skip-folders = Ohita kansiot nimeltä
settings-library-skip-folders-help = Kirjasto ei tutki kansioita, joilla on nämä nimet, pilkuilla erotettuina. Piilotetut kansiot ohitetaan aina.
settings-library-skip-marked = Ohita kansiot, joissa on
settings-library-skip-marked-help = Ei myöskään kansioita, joissa on jokin näistä: .git koodin työkopioille, pyvenv.cfg Python-ympäristöille, CACHEDIR.TAG välimuisteille. Sijainnin omaa kansiota ei koskaan ohiteta.
settings-library-show-timeline = Aikajana
settings-library-show-timeline-help = Näyttää kirjaston ruudukon yllä, montako kuvaa kullakin vuodella, kuukaudella tai päivällä on.
