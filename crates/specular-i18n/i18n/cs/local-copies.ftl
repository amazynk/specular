# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Limit místních kopií
settings-library-local-copies-limit-help = Kolik místa smějí místní kopie nejvýše zabrat ve složce dat aplikace Specular.

local-keep-menu = Ponechat místní kopii…
local-remove-menu = Odebrat místní kopii…
local-keep-location-menu = Ponechat místní kopii tohoto umístění…
local-remove-location-menu = Odebrat místní kopii tohoto umístění…
local-keep-album-menu = Ponechat místní kopii tohoto alba…
local-remove-album-menu = Odebrat místní kopii tohoto alba…
local-remove-held-elsewhere = Tyto kopie patří k ponechanému umístění nebo albu. Odeberte je tam.

local-keep-ask = Ponechat místní kopii { $item }?
local-keep-detail = { $count ->
    [one] { $count } fotka.
    [few] { $count } fotky.
    [many] { $count } fotky.
   *[other] { $count } fotek.
}
    Originály: soubory tak, jak jsou.
    Proxy: snímky raw ve 2 560 pixelech, dost pro vyvolání. Ostatní soubory se zkopírují celé.
    Kopie se uloží do složky dat aplikace Specular. Originály se nemění.
local-keep-originals = Ponechat originály
local-keep-proxies = Ponechat proxy
local-keeping = Kopírování { $item }…
local-kept = { $item }: { $count ->
    [one] { $count } místní kopie
    [few] { $count } místní kopie
    [many] { $count } místní kopie
   *[other] { $count } místních kopií
}, { $size }.
local-kept-full = { $count ->
    [one] { $count } fotka se nevešla do limitu velikosti.
    [few] { $count } fotky se nevešly do limitu velikosti.
    [many] { $count } fotky se nevešlo do limitu velikosti.
   *[other] { $count } fotek se nevešlo do limitu velikosti.
}
local-kept-unreachable = { $count ->
    [one] { $count } fotku nešlo přečíst.
    [few] { $count } fotky nešlo přečíst.
    [many] { $count } fotky nešlo přečíst.
   *[other] { $count } fotek nešlo přečíst.
}

local-remove-ask = Odebrat místní kopii { $item }?
local-remove-detail = Kopie ve složce dat aplikace Specular se smažou. Originálů se to nedotkne. Čekající změny zůstanou.
local-remove-ok = Odebrat místní kopii
local-removed = Místní kopie { $item } odebrána, uvolněno { $size }.

local-banner-offline = { $location } je offline. Pracujete s místními kopiemi.
local-banner-offline-many = Offline umístění: { $count }. Pracujete s místními kopiemi.
local-banner-waiting = { $count ->
    [one] { $count } změna čeká na originál.
    [few] { $count } změny čekají na originály.
    [many] { $count } změny čeká na originály.
   *[other] { $count } změn čeká na originály.
}
local-banner-conflicts = { $count ->
    [one] { $count } soubor se změnil, zatímco jste byli offline.
    [few] { $count } soubory se změnily, zatímco jste byli offline.
    [many] { $count } souboru se změnilo, zatímco jste byli offline.
   *[other] { $count } souborů se změnilo, zatímco jste byli offline.
}
local-review = Zkontrolovat…
local-sidebar-originals = místní kopie
local-sidebar-proxies = místní proxy

local-synced = { $count ->
    [one] { $count } změna zapsána do originálu.
    [few] { $count } změny zapsány do originálů.
    [many] { $count } změny zapsáno do originálů.
   *[other] { $count } změn zapsáno do originálů.
}
local-sync-failed = { $count ->
    [one] { $count } změnu nešlo zapsat. Dál čeká.
    [few] { $count } změny nešlo zapsat. Dál čekají.
    [many] { $count } změny nešlo zapsat. Dál čekají.
   *[other] { $count } změn nešlo zapsat. Dál čekají.
}

local-conflict-title = { $file } se změnil, zatímco jste byli offline. Kterou verzi ponecháte: { $what }?
local-conflict-yours = Vaše: { $text }
local-conflict-theirs = Na disku: { $text }
local-conflict-later = Zrušit rozhodne později. Nic se nepřepíše.
local-keep-mine = Ponechat vaši
local-keep-theirs = Ponechat verzi souboru
local-what-marks = hodnocení
local-what-keywords = klíčová slova
local-what-fields = pole IPTC
local-what-edit = úprava
local-stars = { $count ->
    [0] Bez hvězdiček
    [one] { $count } hvězdička
    [few] { $count } hvězdičky
    [many] { $count } hvězdičky
   *[other] { $count } hvězdiček
}
local-label = { $label ->
    [1] Červená
    [2] Žlutá
    [3] Zelená
   *[4] Modrá
}
local-pick = Vybraná
local-reject = Zamítnutá
local-no-keywords = žádná klíčová slova
local-changed-elsewhere = změněno jinde
local-photo-changed = změnila se samotná fotka
local-edit-here = upraveno zde, { $when }
local-edit-elsewhere = upraveno jinde, { $when }
local-edit-none = bez úprav

local-export-ask = { $count ->
    [one] { $count } fotka je offline. Exportovat ji z proxy?
    [few] { $count } fotky jsou offline. Exportovat je z proxy?
    [many] { $count } fotky je offline. Exportovat je z proxy?
   *[other] { $count } fotek je offline. Exportovat je z proxy?
}
local-export-detail = Proxy mají na delší straně 2 560 pixelů. Originály dají plnou velikost.
local-export-proxy = Exportovat z proxy
local-export-wait = Počkat na originály
local-export-waiting = Export začne, až budou originály zpět.

local-panel-title = Místní kopie
local-panel-used = Využito { $used } z { $limit }.
local-panel-originals = { $count ->
    [one] { $count } originál
    [few] { $count } originály
    [many] { $count } originálu
   *[other] { $count } originálů
}
local-panel-proxies = { $count ->
    [one] { $count } fotka, raw jako proxy
    [few] { $count } fotky, raw jako proxy
    [many] { $count } fotky, raw jako proxy
   *[other] { $count } fotek, raw jako proxy
}
local-panel-remove = Odebrat…
local-panel-footer = Uloženo v { $folder }. Originály se nikdy nemění.
local-panel-keywords = offline místní kopie proxy místo
