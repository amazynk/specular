# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Limite delle copie locali
settings-library-local-copies-limit-help = Lo spazio massimo che le copie locali possono occupare nella cartella dei dati di Specular.

local-keep-menu = Tieni una copia locale…
local-remove-menu = Rimuovi la copia locale…
local-keep-location-menu = Tieni una copia locale di questa posizione…
local-remove-location-menu = Rimuovi la copia locale di questa posizione…
local-keep-album-menu = Tieni una copia locale di questo album…
local-remove-album-menu = Rimuovi la copia locale di questo album…
local-remove-held-elsewhere = Queste copie appartengono a una posizione o a un album tenuto. Rimuovile da lì.

local-keep-ask = Tenere una copia locale di { $item }?
local-keep-detail = { $count ->
    [one] 1 foto.
    [many] { $count } foto.
   *[other] { $count } foto.
}
    Originali: i file così come sono.
    Proxy: i raw a 2.560 pixel, abbastanza per lo sviluppo. Gli altri file sono copiati interi.
    Le copie vanno nella cartella dei dati di Specular. Gli originali non cambiano.
local-keep-originals = Tieni gli originali
local-keep-proxies = Tieni i proxy
local-keeping = Copia di { $item }…
local-kept = { $item }: { $count ->
    [one] 1 copia locale
    [many] { $count } copie locali
   *[other] { $count } copie locali
}, { $size }.
local-kept-full = { $count ->
    [one] 1 foto non è entrata nel limite di dimensione.
    [many] { $count } foto non sono entrate nel limite di dimensione.
   *[other] { $count } foto non sono entrate nel limite di dimensione.
}
local-kept-unreachable = { $count ->
    [one] Non è stato possibile leggere 1 foto.
    [many] Non è stato possibile leggere { $count } foto.
   *[other] Non è stato possibile leggere { $count } foto.
}

local-remove-ask = Rimuovere la copia locale di { $item }?
local-remove-detail = Le copie nella cartella dei dati di Specular vengono eliminate. Gli originali non vengono toccati. Le modifiche in attesa restano.
local-remove-ok = Rimuovi copia locale
local-removed = Copia locale di { $item } rimossa, { $size } liberati.

local-banner-offline = { $location } non è in linea. Stai lavorando sulle copie locali.
local-banner-offline-many = { $count } posizioni non sono in linea. Stai lavorando sulle copie locali.
local-banner-waiting = { $count ->
    [one] 1 modifica attende l’originale.
    [many] { $count } modifiche attendono gli originali.
   *[other] { $count } modifiche attendono gli originali.
}
local-banner-conflicts = { $count ->
    [one] 1 file è cambiato mentre eri non in linea.
    [many] { $count } file sono cambiati mentre eri non in linea.
   *[other] { $count } file sono cambiati mentre eri non in linea.
}
local-review = Rivedi…
local-sidebar-originals = copia locale
local-sidebar-proxies = proxy locali

local-synced = { $count ->
    [one] 1 modifica scritta nel suo originale.
    [many] { $count } modifiche scritte nei loro originali.
   *[other] { $count } modifiche scritte nei loro originali.
}
local-sync-failed = { $count ->
    [one] Non è stato possibile scrivere 1 modifica. Resta in attesa.
    [many] Non è stato possibile scrivere { $count } modifiche. Restano in attesa.
   *[other] Non è stato possibile scrivere { $count } modifiche. Restano in attesa.
}

local-conflict-title = { $file } è cambiato mentre eri non in linea. Quale versione tieni: { $what }?
local-conflict-yours = La tua: { $text }
local-conflict-theirs = Sul disco: { $text }
local-conflict-later = Annulla decide più tardi. Nulla viene sovrascritto.
local-keep-mine = Tieni la tua
local-keep-theirs = Tieni quella del file
local-what-marks = valutazione
local-what-keywords = parole chiave
local-what-fields = campi IPTC
local-what-edit = modifica
local-stars = { $count ->
    [0] Nessuna stella
    [one] 1 stella
    [many] { $count } stelle
   *[other] { $count } stelle
}
local-label = { $label ->
    [1] Rosso
    [2] Giallo
    [3] Verde
   *[4] Blu
}
local-pick = Scelta
local-reject = Scartata
local-no-keywords = nessuna parola chiave
local-changed-elsewhere = cambiato altrove
local-photo-changed = è cambiata la foto stessa
local-edit-here = modificata qui, { $when }
local-edit-elsewhere = modificata altrove, { $when }
local-edit-none = nessuna modifica

local-export-ask = { $count ->
    [one] 1 foto non è in linea. Esportarla dal suo proxy?
    [many] { $count } foto non sono in linea. Esportarle dai loro proxy?
   *[other] { $count } foto non sono in linea. Esportarle dai loro proxy?
}
local-export-detail = I proxy hanno 2.560 pixel sul lato lungo. Gli originali danno la dimensione piena.
local-export-proxy = Esporta dai proxy
local-export-wait = Aspetta gli originali
local-export-waiting = L’esportazione parte quando gli originali tornano.

local-panel-title = Copie locali
local-panel-used = In uso { $used } di { $limit }.
local-panel-originals = { $count ->
    [one] 1 originale
    [many] { $count } originali
   *[other] { $count } originali
}
local-panel-proxies = { $count ->
    [one] 1 foto, raw come proxy
    [many] { $count } foto, raw come proxy
   *[other] { $count } foto, raw come proxy
}
local-panel-remove = Rimuovi…
local-panel-footer = Salvate in { $folder }. Gli originali non vengono mai cambiati.
local-panel-keywords = non in linea offline copie locali proxy spazio
