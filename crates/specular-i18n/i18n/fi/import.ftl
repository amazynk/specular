# machine-drafted, needs native review
# Specular — Tuonti kameroista, puhelimista ja muistikorteilta. Suomi.
# Korttia vain luetaan; tiedostoja poistetaan siltä vain pyydettäessä, tarkistetun kopion jälkeen.

palette-import = Tuo kuvia…
disabled-no-import-source = Muistikorttia, kameraa tai puhelinta ei ole liitetty

## Asetukset → Tuonti
settings-section-import = Tuonti
settings-import-on-card = Kun kortti asetetaan
settings-import-on-card-help = Mitä tapahtuu, kun liität muistikortin, kameran tai puhelimen, jossa on kuvia.
settings-import-on-card-ask = Kysy
settings-import-on-card-open = Avaa tuonti
settings-import-on-card-nothing = Älä tee mitään
settings-import-destination = Tuo kansioon
settings-import-destination-help = Kansio, johon tuodut kuvat ja videot menevät. Jos sitä ei ole asetettu, ne menevät Kuvat-kansioon.
settings-import-destination-unset = Kuvat-kansio
settings-import-folder-pattern = Kansiot
settings-import-folder-pattern-help = Sen sisään luotavat kansiot: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} ja {"{"}event{"}"}, eroteltuina merkillä /.
settings-import-rename-pattern = Nimeä uudelleen
settings-import-rename-pattern-help = Uudet tiedostonimet, kuten Muunna-toiminnossa: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Tyhjä säilyttää kameran nimet.
settings-import-keep-pairs = Pidä RAW ja JPEG yhdessä
settings-import-keep-pairs-help = RAW-tiedosto ja sen kanssa otettu JPEG menevät samaan kansioon samalla nimellä, myös uudelleennimettyinä.
settings-import-low-res-videos = Tuo pienitarkkuuksiset videokopiot
settings-import-low-res-videos-help = Jotkin kamerat tallentavat jokaisesta videosta pienen esikatselukopion (.LRV). Pois päältä: ne jäävät kortille.
settings-import-eject = Irrota lopuksi
settings-import-eject-help = Irrottaa kortin, kun kaikki on kopioitu, jotta sen voi poistaa.

## Tarjous, kun kortti asetetaan
import-offer = Tuodaanko kohteesta { $name }?
import-offer-button = Tuo

## Tuontinäkymä
import-title = Tuonti
import-from = kohteesta { $name }
import-source-next = Seuraava lähde
import-close = Sulje
import-scanning = Etsitään uusia kuvia… { $done }/{ $total }
import-scanning-start = Etsitään uusia kuvia…
import-none-new = Kaikki kohteen { $name } sisältö on tuotu aiemmin.
import-empty = Kohteessa { $name } ei ole kuvia eikä videoita.
import-no-source = Muistikorttia, kameraa tai puhelinta ei ole liitetty.
import-summary = { $count ->
    [one] { $count } uusi kohde
   *[other] { $count } uutta kohdetta
} · { $size }
import-already = { $count } tuotu aiemmin
import-unreadable = { $count ->
    [one] { $count } tiedostoa ei voitu lukea
   *[other] { $count } tiedostoa ei voitu lukea
}
import-selected = { $count }/{ $total } valittu
import-select-all = Valitse kaikki
import-select-none = Poista valinnat
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Tuo kansioon
import-choose = Valitse…
import-folder-pattern = Kansiot
import-event = Tapahtuma
import-event-hint = Valinnainen, kohteelle {"{"}event{"}"}
import-rename = Nimeä uudelleen
import-rename-hint = Tyhjä säilyttää kameran nimet
import-preview = Ensimmäinen tiedosto: { $path }
import-pattern-error = { $error }
import-keep-pairs = Pidä RAW ja JPEG yhdessä
import-eject = Irrota lopuksi
import-delete = Poista kortilta tuonnin jälkeen
import-free = { $size } kopioitavana · { $free } vapaata tilaa
import-free-unknown = { $size } kopioitavana
import-no-space = Kansiossa { $folder } ei ole tarpeeksi vapaata tilaa: tuonti tarvitsee { $need } ja vapaana on vain { $free }. Vapauta tilaa tai valitse toinen kansio.
import-start = { $count ->
    [one] Tuo { $count } kohde
   *[other] Tuo { $count } kohdetta
}
import-nothing-selected = Valitse tuotavat kuvat

import-confirm-delete = { $count ->
    [one] Kun tiedosto on kopioitu ja tarkistettu, { $count } tiedosto poistetaan kohteesta { $name }. Tätä ei voi kumota.
   *[other] Kun tiedostot on kopioitu ja tarkistettu, { $count } tiedostoa poistetaan kohteesta { $name }. Tätä ei voi kumota.
}
import-confirm-yes = Tuo ja poista
import-confirm-no = Takaisin

import-progress = Tuodaan { $done }/{ $total } · { $size }
import-progress-eta = Tuodaan { $done }/{ $total } · { $size } · { $time }
import-paused = Keskeytetty · { $done }/{ $total }
import-pause = Keskeytä
import-resume = Jatka
import-cancel = Peruuta
import-hide = Piilota
import-stopping = Pysäytetään…
import-running = Tuonti on jo käynnissä.

import-done = { $count ->
    [one] { $count } tiedosto tuotu
   *[other] { $count } tiedostoa tuotu
} kansioon { $folder }
import-done-existed = { $count } oli jo olemassa
import-done-deleted = { $count } poistettu kortilta
import-done-failed = { $count ->
    [one] { $count } tiedostoa ei voitu tuoda
   *[other] { $count } tiedostoa ei voitu tuoda
}
import-done-cancelled = Tuonti peruttiin { $count ->
        [one] { $count } tiedoston
       *[other] { $count } tiedoston
    } jälkeen. Uusi tuonti jatkaa siitä, mihin se jäi.
import-done-nothing = Mitään uutta ei tuotu.
import-ejected = { $name } voidaan poistaa.
import-eject-failed = Kohdetta { $name } ei voitu irrottaa: { $error }
import-failed-destination = Kansiota { $folder } ei voi käyttää: { $error }
import-ledger-failed = Tuotujen tiedostojen luetteloa ei voitu lukea ({ $error }); aiemmin tuotuja tiedostoja voidaan tarjota uudelleen.

## Näytönlukija
import-a11y-group = Tuonti
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, valittu
