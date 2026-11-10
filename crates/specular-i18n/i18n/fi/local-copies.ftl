# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Paikallisten kopioiden raja
settings-library-local-copies-limit-help = Kuinka paljon tilaa paikalliset kopiot saavat enintään viedä Specularin datakansiossa.

local-keep-menu = Pidä paikallinen kopio…
local-remove-menu = Poista paikallinen kopio…
local-keep-location-menu = Pidä tämän sijainnin paikallinen kopio…
local-remove-location-menu = Poista tämän sijainnin paikallinen kopio…
local-keep-album-menu = Pidä tämän albumin paikallinen kopio…
local-remove-album-menu = Poista tämän albumin paikallinen kopio…
local-remove-held-elsewhere = Nämä kopiot kuuluvat pidettyyn sijaintiin tai albumiin. Poista ne sieltä.

local-keep-ask = Pidetäänkö kohteesta { $item } paikallinen kopio?
local-keep-detail = { $count ->
    [one] 1 kuva.
   *[other] { $count } kuvaa.
}
    Alkuperäiset: tiedostot sellaisinaan.
    Proxy-kuvat: raw-kuvat 2 560 pikselin kokoisina, riittää kehittämiseen. Muut tiedostot kopioidaan kokonaan.
    Kopiot menevät Specularin datakansioon. Alkuperäisiä ei muuteta.
local-keep-originals = Pidä alkuperäiset
local-keep-proxies = Pidä proxy-kuvat
local-keeping = Kopioidaan { $item }…
local-kept = { $item }: { $count ->
    [one] 1 paikallinen kopio
   *[other] { $count } paikallista kopiota
}, { $size }.
local-kept-full = { $count ->
    [one] 1 kuva ei mahtunut kokorajaan.
   *[other] { $count } kuvaa ei mahtunut kokorajaan.
}
local-kept-unreachable = { $count ->
    [one] 1 kuvaa ei voitu lukea.
   *[other] { $count } kuvaa ei voitu lukea.
}

local-remove-ask = Poistetaanko kohteen { $item } paikallinen kopio?
local-remove-detail = Specularin datakansiossa olevat kopiot poistetaan. Alkuperäisiin ei kosketa. Odottavat muutokset säilyvät.
local-remove-ok = Poista paikallinen kopio
local-removed = Kohteen { $item } paikallinen kopio poistettu, { $size } vapautui.

local-banner-offline = { $location } ei ole yhteydessä. Työskentelet paikallisilla kopioilla.
local-banner-offline-many = { $count } sijaintia ei ole yhteydessä. Työskentelet paikallisilla kopioilla.
local-banner-waiting = { $count ->
    [one] 1 muutos odottaa alkuperäistä.
   *[other] { $count } muutosta odottaa alkuperäisiä.
}
local-banner-conflicts = { $count ->
    [one] 1 tiedosto muuttui, kun olit ilman yhteyttä.
   *[other] { $count } tiedostoa muuttui, kun olit ilman yhteyttä.
}
local-review = Tarkista…
local-sidebar-originals = paikallinen kopio
local-sidebar-proxies = paikalliset proxy-kuvat

local-synced = { $count ->
    [one] 1 muutos kirjoitettu alkuperäiseen.
   *[other] { $count } muutosta kirjoitettu alkuperäisiin.
}
local-sync-failed = { $count ->
    [one] 1 muutosta ei voitu kirjoittaa. Se jää odottamaan.
   *[other] { $count } muutosta ei voitu kirjoittaa. Ne jäävät odottamaan.
}

local-conflict-title = { $file } muuttui, kun olit ilman yhteyttä. Kumman version pidät: { $what }?
local-conflict-yours = Sinun: { $text }
local-conflict-theirs = Levyllä: { $text }
local-conflict-later = Peruuta päättää myöhemmin. Mitään ei korvata.
local-keep-mine = Pidä sinun
local-keep-theirs = Pidä tiedoston
local-what-marks = arvio
local-what-keywords = avainsanat
local-what-fields = IPTC-kentät
local-what-edit = muokkaus
local-stars = { $count ->
    [0] Ei tähtiä
    [one] 1 tähti
   *[other] { $count } tähteä
}
local-label = { $label ->
    [1] Punainen
    [2] Keltainen
    [3] Vihreä
   *[4] Sininen
}
local-pick = Valittu
local-reject = Hylätty
local-no-keywords = ei avainsanoja
local-changed-elsewhere = muutettu muualla
local-photo-changed = itse kuva muuttui
local-edit-here = muokattu täällä, { $when }
local-edit-elsewhere = muokattu muualla, { $when }
local-edit-none = ei muokkausta

local-export-ask = { $count ->
    [one] 1 kuva ei ole saatavilla. Viedäänkö se proxy-kuvasta?
   *[other] { $count } kuvaa ei ole saatavilla. Viedäänkö ne proxy-kuvista?
}
local-export-detail = Proxy-kuvat ovat 2 560 pikseliä pitkältä sivulta. Alkuperäisistä saa täyden koon.
local-export-proxy = Vie proxy-kuvista
local-export-wait = Odota alkuperäisiä
local-export-waiting = Vienti alkaa, kun alkuperäiset ovat taas saatavilla.

local-panel-title = Paikalliset kopiot
local-panel-used = Käytössä { $used } / { $limit }.
local-panel-originals = { $count ->
    [one] 1 alkuperäinen
   *[other] { $count } alkuperäistä
}
local-panel-proxies = { $count ->
    [one] 1 kuva, raw-kuvat proxyina
   *[other] { $count } kuvaa, raw-kuvat proxyina
}
local-panel-remove = Poista…
local-panel-footer = Tallennettu kansioon { $folder }. Alkuperäisiä ei koskaan muuteta.
local-panel-keywords = offline ei yhteyttä paikalliset kopiot proxy tila
