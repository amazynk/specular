# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Limiet lokale kopieën
settings-library-local-copies-limit-help = Hoeveel ruimte lokale kopieën hoogstens mogen innemen in de gegevensmap van Specular.

local-keep-menu = Lokale kopie bewaren…
local-remove-menu = Lokale kopie verwijderen…
local-keep-location-menu = Lokale kopie van deze locatie bewaren…
local-remove-location-menu = Lokale kopie van deze locatie verwijderen…
local-keep-album-menu = Lokale kopie van dit album bewaren…
local-remove-album-menu = Lokale kopie van dit album verwijderen…
local-remove-held-elsewhere = Deze kopieën horen bij een bewaarde locatie of een bewaard album. Verwijder ze daar.

local-keep-ask = Een lokale kopie van { $item } bewaren?
local-keep-detail = { $count ->
    [one] 1 foto.
   *[other] { $count } foto’s.
}
    Originelen: de bestanden zoals ze zijn.
    Proxy’s: raws op 2.560 pixels, genoeg om te ontwikkelen. Andere bestanden worden helemaal gekopieerd.
    De kopieën gaan naar de gegevensmap van Specular. De originelen veranderen niet.
local-keep-originals = Originelen bewaren
local-keep-proxies = Proxy’s bewaren
local-keeping = { $item } kopiëren…
local-kept = { $item }: { $count ->
    [one] 1 lokale kopie
   *[other] { $count } lokale kopieën
}, { $size }.
local-kept-full = { $count ->
    [one] 1 foto paste niet binnen de limiet.
   *[other] { $count } foto’s pasten niet binnen de limiet.
}
local-kept-unreachable = { $count ->
    [one] 1 foto kon niet worden gelezen.
   *[other] { $count } foto’s konden niet worden gelezen.
}

local-remove-ask = De lokale kopie van { $item } verwijderen?
local-remove-detail = De kopieën in de gegevensmap van Specular worden gewist. De originelen worden niet aangeraakt. Wachtende wijzigingen blijven bewaard.
local-remove-ok = Lokale kopie verwijderen
local-removed = Lokale kopie van { $item } verwijderd, { $size } vrijgemaakt.

local-banner-offline = { $location } is offline. Je werkt met lokale kopieën.
local-banner-offline-many = { $count } locaties zijn offline. Je werkt met lokale kopieën.
local-banner-waiting = { $count ->
    [one] 1 wijziging wacht op het origineel.
   *[other] { $count } wijzigingen wachten op de originelen.
}
local-banner-conflicts = { $count ->
    [one] 1 bestand is gewijzigd terwijl je offline was.
   *[other] { $count } bestanden zijn gewijzigd terwijl je offline was.
}
local-review = Bekijken…
local-sidebar-originals = lokale kopie
local-sidebar-proxies = lokale proxy’s

local-synced = { $count ->
    [one] 1 wijziging naar het origineel geschreven.
   *[other] { $count } wijzigingen naar de originelen geschreven.
}
local-sync-failed = { $count ->
    [one] 1 wijziging kon niet worden geschreven. Ze blijft wachten.
   *[other] { $count } wijzigingen konden niet worden geschreven. Ze blijven wachten.
}

local-conflict-title = { $file } is gewijzigd terwijl je offline was. Welke versie bewaar je: { $what }?
local-conflict-yours = Die van jou: { $text }
local-conflict-theirs = Op schijf: { $text }
local-conflict-later = Annuleren beslist later. Er wordt niets overschreven.
local-keep-mine = Die van jou bewaren
local-keep-theirs = Die van het bestand bewaren
local-what-marks = beoordeling
local-what-keywords = trefwoorden
local-what-fields = IPTC-velden
local-what-edit = bewerking
local-stars = { $count ->
    [0] Geen sterren
    [one] 1 ster
   *[other] { $count } sterren
}
local-label = { $label ->
    [1] Rood
    [2] Geel
    [3] Groen
   *[4] Blauw
}
local-pick = Gekozen
local-reject = Afgewezen
local-no-keywords = geen trefwoorden
local-changed-elsewhere = elders gewijzigd
local-photo-changed = de foto zelf is gewijzigd
local-edit-here = hier bewerkt, { $when }
local-edit-elsewhere = elders bewerkt, { $when }
local-edit-none = geen bewerking

local-export-ask = { $count ->
    [one] 1 foto is offline. Exporteren vanuit de proxy?
   *[other] { $count } foto’s zijn offline. Exporteren vanuit de proxy’s?
}
local-export-detail = Proxy’s zijn 2.560 pixels aan de lange zijde. De originelen geven het volle formaat.
local-export-proxy = Exporteren vanuit proxy’s
local-export-wait = Wachten op de originelen
local-export-waiting = De export begint zodra de originelen terug zijn.

local-panel-title = Lokale kopieën
local-panel-used = { $used } van { $limit } in gebruik.
local-panel-originals = { $count ->
    [one] 1 origineel
   *[other] { $count } originelen
}
local-panel-proxies = { $count ->
    [one] 1 foto, raws als proxy
   *[other] { $count } foto’s, raws als proxy
}
local-panel-remove = Verwijderen…
local-panel-footer = Opgeslagen in { $folder }. De originelen veranderen nooit.
local-panel-keywords = offline lokale kopieën proxy’s ruimte
