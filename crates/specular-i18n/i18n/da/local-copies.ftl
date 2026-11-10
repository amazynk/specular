# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Grænse for lokale kopier
settings-library-local-copies-limit-help = Hvor meget plads lokale kopier højst må fylde i Speculars datamappe.

local-keep-menu = Behold en lokal kopi…
local-remove-menu = Fjern lokal kopi…
local-keep-location-menu = Behold en lokal kopi af denne placering…
local-remove-location-menu = Fjern lokal kopi af denne placering…
local-keep-album-menu = Behold en lokal kopi af dette album…
local-remove-album-menu = Fjern lokal kopi af dette album…
local-remove-held-elsewhere = Disse kopier hører til en beholdt placering eller et beholdt album. Fjern dem der.

local-keep-ask = Behold en lokal kopi af { $item }?
local-keep-detail = { $count ->
    [one] 1 foto.
   *[other] { $count } fotos.
}
    Originaler: filerne, som de er.
    Proxyfiler: raws i 2.560 pixel, nok til at fremkalde. Andre filer kopieres hele.
    Kopierne lægges i Speculars datamappe. Originalerne ændres ikke.
local-keep-originals = Behold originaler
local-keep-proxies = Behold proxyfiler
local-keeping = Kopierer { $item }…
local-kept = { $item }: { $count ->
    [one] 1 lokal kopi
   *[other] { $count } lokale kopier
}, { $size }.
local-kept-full = { $count ->
    [one] 1 foto kunne ikke være inden for størrelsesgrænsen.
   *[other] { $count } fotos kunne ikke være inden for størrelsesgrænsen.
}
local-kept-unreachable = { $count ->
    [one] 1 foto kunne ikke læses.
   *[other] { $count } fotos kunne ikke læses.
}

local-remove-ask = Fjern den lokale kopi af { $item }?
local-remove-detail = Kopierne i Speculars datamappe slettes. Originalerne røres ikke. Ventende ændringer beholdes.
local-remove-ok = Fjern lokal kopi
local-removed = Lokal kopi af { $item } fjernet, { $size } frigjort.

local-banner-offline = { $location } er offline. Du arbejder på lokale kopier.
local-banner-offline-many = { $count } placeringer er offline. Du arbejder på lokale kopier.
local-banner-waiting = { $count ->
    [one] 1 ændring venter på originalen.
   *[other] { $count } ændringer venter på originalerne.
}
local-banner-conflicts = { $count ->
    [one] 1 fil blev ændret, mens du var offline.
   *[other] { $count } filer blev ændret, mens du var offline.
}
local-review = Gennemse…
local-sidebar-originals = lokal kopi
local-sidebar-proxies = lokale proxyfiler

local-synced = { $count ->
    [one] 1 ændring skrevet til sin original.
   *[other] { $count } ændringer skrevet til deres originaler.
}
local-sync-failed = { $count ->
    [one] 1 ændring kunne ikke skrives. Den venter stadig.
   *[other] { $count } ændringer kunne ikke skrives. De venter stadig.
}

local-conflict-title = { $file } blev ændret, mens du var offline. Hvilken version beholder du: { $what }?
local-conflict-yours = Din: { $text }
local-conflict-theirs = På disken: { $text }
local-conflict-later = Annuller beslutter senere. Intet overskrives.
local-keep-mine = Behold din
local-keep-theirs = Behold filens
local-what-marks = bedømmelse
local-what-keywords = nøgleord
local-what-fields = IPTC-felter
local-what-edit = redigering
local-stars = { $count ->
    [0] Ingen stjerner
    [one] 1 stjerne
   *[other] { $count } stjerner
}
local-label = { $label ->
    [1] Rød
    [2] Gul
    [3] Grøn
   *[4] Blå
}
local-pick = Udvalgt
local-reject = Afvist
local-no-keywords = ingen nøgleord
local-changed-elsewhere = ændret et andet sted
local-photo-changed = selve fotoet blev ændret
local-edit-here = redigeret her, { $when }
local-edit-elsewhere = redigeret et andet sted, { $when }
local-edit-none = ingen redigering

local-export-ask = { $count ->
    [one] 1 foto er offline. Eksportér det fra dets proxyfil?
   *[other] { $count } fotos er offline. Eksportér dem fra deres proxyfiler?
}
local-export-detail = Proxyfiler er 2.560 pixel på den lange side. Originalerne giver den fulde størrelse.
local-export-proxy = Eksportér fra proxyfiler
local-export-wait = Vent på originalerne
local-export-waiting = Eksporten starter, når originalerne er tilbage.

local-panel-title = Lokale kopier
local-panel-used = Bruger { $used } af { $limit }.
local-panel-originals = { $count ->
    [one] 1 original
   *[other] { $count } originaler
}
local-panel-proxies = { $count ->
    [one] 1 foto, raws som proxyfiler
   *[other] { $count } fotos, raws som proxyfiler
}
local-panel-remove = Fjern…
local-panel-footer = Gemt i { $folder }. Originalerne ændres aldrig.
local-panel-keywords = offline lokale kopier proxyfiler plads
