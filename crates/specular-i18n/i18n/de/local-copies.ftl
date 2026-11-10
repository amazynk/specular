# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Grenze für lokale Kopien
settings-library-local-copies-limit-help = So viel Platz dürfen lokale Kopien im Datenordner von Specular höchstens belegen.

local-keep-menu = Lokale Kopie behalten…
local-remove-menu = Lokale Kopie entfernen…
local-keep-location-menu = Lokale Kopie dieses Orts behalten…
local-remove-location-menu = Lokale Kopie dieses Orts entfernen…
local-keep-album-menu = Lokale Kopie dieses Albums behalten…
local-remove-album-menu = Lokale Kopie dieses Albums entfernen…
local-remove-held-elsewhere = Diese Kopien gehören zu einem behaltenen Ort oder Album. Entfernen Sie sie dort.

local-keep-ask = Eine lokale Kopie von { $item } behalten?
local-keep-detail = { $count ->
    [one] 1 Foto.
   *[other] { $count } Fotos.
}
    Originale: die Dateien, wie sie sind.
    Proxys: Raws mit 2.560 Pixeln, genug zum Entwickeln. Andere Dateien werden ganz kopiert.
    Die Kopien kommen in den Datenordner von Specular. Die Originale bleiben unverändert.
local-keep-originals = Originale behalten
local-keep-proxies = Proxys behalten
local-keeping = { $item } wird kopiert…
local-kept = { $item }: { $count ->
    [one] 1 lokale Kopie
   *[other] { $count } lokale Kopien
}, { $size }.
local-kept-full = { $count ->
    [one] 1 Foto passte nicht mehr in die Größengrenze.
   *[other] { $count } Fotos passten nicht mehr in die Größengrenze.
}
local-kept-unreachable = { $count ->
    [one] 1 Foto konnte nicht gelesen werden.
   *[other] { $count } Fotos konnten nicht gelesen werden.
}

local-remove-ask = Die lokale Kopie von { $item } entfernen?
local-remove-detail = Die Kopien im Datenordner von Specular werden gelöscht. Die Originale bleiben unberührt. Wartende Änderungen bleiben erhalten.
local-remove-ok = Lokale Kopie entfernen
local-removed = Lokale Kopie von { $item } entfernt, { $size } frei.

local-banner-offline = { $location } ist offline. Sie arbeiten mit lokalen Kopien.
local-banner-offline-many = { $count } Orte sind offline. Sie arbeiten mit lokalen Kopien.
local-banner-waiting = { $count ->
    [one] 1 Änderung wartet auf das Original.
   *[other] { $count } Änderungen warten auf die Originale.
}
local-banner-conflicts = { $count ->
    [one] 1 Datei wurde geändert, während Sie offline waren.
   *[other] { $count } Dateien wurden geändert, während Sie offline waren.
}
local-review = Prüfen…
local-sidebar-originals = lokale Kopie
local-sidebar-proxies = lokale Proxys

local-synced = { $count ->
    [one] 1 Änderung ins Original geschrieben.
   *[other] { $count } Änderungen in die Originale geschrieben.
}
local-sync-failed = { $count ->
    [one] 1 Änderung konnte nicht geschrieben werden. Sie wartet weiter.
   *[other] { $count } Änderungen konnten nicht geschrieben werden. Sie warten weiter.
}

local-conflict-title = { $file } wurde geändert, während Sie offline waren. Welche Fassung behalten Sie: { $what }?
local-conflict-yours = Ihre: { $text }
local-conflict-theirs = Auf dem Datenträger: { $text }
local-conflict-later = Abbrechen entscheidet später. Nichts wird überschrieben.
local-keep-mine = Ihre behalten
local-keep-theirs = Die der Datei behalten
local-what-marks = Bewertung
local-what-keywords = Stichwörter
local-what-fields = IPTC-Felder
local-what-edit = Bearbeitung
local-stars = { $count ->
    [0] Keine Sterne
    [one] 1 Stern
   *[other] { $count } Sterne
}
local-label = { $label ->
    [1] Rot
    [2] Gelb
    [3] Grün
   *[4] Blau
}
local-pick = Ausgewählt
local-reject = Abgelehnt
local-no-keywords = keine Stichwörter
local-changed-elsewhere = anderswo geändert
local-photo-changed = das Foto selbst wurde geändert
local-edit-here = hier bearbeitet, { $when }
local-edit-elsewhere = anderswo bearbeitet, { $when }
local-edit-none = keine Bearbeitung

local-export-ask = { $count ->
    [one] 1 Foto ist offline. Aus seinem Proxy exportieren?
   *[other] { $count } Fotos sind offline. Aus ihren Proxys exportieren?
}
local-export-detail = Proxys haben 2.560 Pixel an der langen Kante. Die Originale liefern die volle Größe.
local-export-proxy = Aus Proxys exportieren
local-export-wait = Auf die Originale warten
local-export-waiting = Der Export beginnt, wenn die Originale wieder da sind.

local-panel-title = Lokale Kopien
local-panel-used = { $used } von { $limit } belegt.
local-panel-originals = { $count ->
    [one] 1 Original
   *[other] { $count } Originale
}
local-panel-proxies = { $count ->
    [one] 1 Foto, Raws als Proxys
   *[other] { $count } Fotos, Raws als Proxys
}
local-panel-remove = Entfernen…
local-panel-footer = Gespeichert in { $folder }. Die Originale werden nie geändert.
local-panel-keywords = offline lokale Kopien Proxys Speicherplatz
