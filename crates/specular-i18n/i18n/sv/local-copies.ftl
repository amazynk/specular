# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Gräns för lokala kopior
settings-library-local-copies-limit-help = Hur mycket plats lokala kopior högst får ta i Speculars datamapp.

local-keep-menu = Behåll en lokal kopia…
local-remove-menu = Ta bort lokal kopia…
local-keep-location-menu = Behåll en lokal kopia av den här platsen…
local-remove-location-menu = Ta bort lokal kopia av den här platsen…
local-keep-album-menu = Behåll en lokal kopia av det här albumet…
local-remove-album-menu = Ta bort lokal kopia av det här albumet…
local-remove-held-elsewhere = De här kopiorna hör till en behållen plats eller ett behållet album. Ta bort dem där.

local-keep-ask = Behålla en lokal kopia av { $item }?
local-keep-detail = { $count ->
    [one] 1 foto.
   *[other] { $count } foton.
}
    Original: filerna som de är.
    Proxyfiler: raw-filer i 2 560 pixlar, nog för att framkalla. Andra filer kopieras hela.
    Kopiorna hamnar i Speculars datamapp. Originalen ändras inte.
local-keep-originals = Behåll original
local-keep-proxies = Behåll proxyfiler
local-keeping = Kopierar { $item }…
local-kept = { $item }: { $count ->
    [one] 1 lokal kopia
   *[other] { $count } lokala kopior
}, { $size }.
local-kept-full = { $count ->
    [one] 1 foto fick inte plats inom storleksgränsen.
   *[other] { $count } foton fick inte plats inom storleksgränsen.
}
local-kept-unreachable = { $count ->
    [one] 1 foto kunde inte läsas.
   *[other] { $count } foton kunde inte läsas.
}

local-remove-ask = Ta bort den lokala kopian av { $item }?
local-remove-detail = Kopiorna i Speculars datamapp raderas. Originalen rörs inte. Väntande ändringar behålls.
local-remove-ok = Ta bort lokal kopia
local-removed = Lokal kopia av { $item } borttagen, { $size } frigjort.

local-banner-offline = { $location } är frånkopplad. Du arbetar med lokala kopior.
local-banner-offline-many = { $count } platser är frånkopplade. Du arbetar med lokala kopior.
local-banner-waiting = { $count ->
    [one] 1 ändring väntar på originalet.
   *[other] { $count } ändringar väntar på originalen.
}
local-banner-conflicts = { $count ->
    [one] 1 fil ändrades medan du var frånkopplad.
   *[other] { $count } filer ändrades medan du var frånkopplad.
}
local-review = Granska…
local-sidebar-originals = lokal kopia
local-sidebar-proxies = lokala proxyfiler

local-synced = { $count ->
    [one] 1 ändring skrevs till sitt original.
   *[other] { $count } ändringar skrevs till sina original.
}
local-sync-failed = { $count ->
    [one] 1 ändring kunde inte skrivas. Den fortsätter vänta.
   *[other] { $count } ändringar kunde inte skrivas. De fortsätter vänta.
}

local-conflict-title = { $file } ändrades medan du var frånkopplad. Vilken version behåller du: { $what }?
local-conflict-yours = Din: { $text }
local-conflict-theirs = På disken: { $text }
local-conflict-later = Avbryt bestämmer senare. Inget skrivs över.
local-keep-mine = Behåll din
local-keep-theirs = Behåll filens
local-what-marks = betyg
local-what-keywords = nyckelord
local-what-fields = IPTC-fält
local-what-edit = redigering
local-stars = { $count ->
    [0] Inga stjärnor
    [one] 1 stjärna
   *[other] { $count } stjärnor
}
local-label = { $label ->
    [1] Röd
    [2] Gul
    [3] Grön
   *[4] Blå
}
local-pick = Vald
local-reject = Förkastad
local-no-keywords = inga nyckelord
local-changed-elsewhere = ändrad på annat håll
local-photo-changed = själva fotot ändrades
local-edit-here = redigerad här, { $when }
local-edit-elsewhere = redigerad på annat håll, { $when }
local-edit-none = ingen redigering

local-export-ask = { $count ->
    [one] 1 foto är frånkopplat. Exportera det från dess proxyfil?
   *[other] { $count } foton är frånkopplade. Exportera dem från deras proxyfiler?
}
local-export-detail = Proxyfiler är 2 560 pixlar på långsidan. Originalen ger full storlek.
local-export-proxy = Exportera från proxyfiler
local-export-wait = Vänta på originalen
local-export-waiting = Exporten börjar när originalen är tillbaka.

local-panel-title = Lokala kopior
local-panel-used = Använder { $used } av { $limit }.
local-panel-originals = { $count ->
    [one] 1 original
   *[other] { $count } original
}
local-panel-proxies = { $count ->
    [one] 1 foto, raw-filer som proxyfiler
   *[other] { $count } foton, raw-filer som proxyfiler
}
local-panel-remove = Ta bort…
local-panel-footer = Sparas i { $folder }. Originalen ändras aldrig.
local-panel-keywords = offline frånkopplad lokala kopior proxyfiler utrymme
