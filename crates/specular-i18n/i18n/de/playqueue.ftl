# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Warteschlange, Wiederholen und Zufall
# machine-drafted, needs native review
palette-next-track = Nächster Titel
# machine-drafted, needs native review
palette-next-track-help = Den nächsten Titel der Warteschlange abspielen, sonst die nächste Datei derselben Art im Ordner.
# machine-drafted, needs native review
palette-prev-track = Vorheriger Titel
# machine-drafted, needs native review
palette-prev-track-help = Den vorherigen Titel abspielen. Nach 3 Sekunden diesen von vorn.
# machine-drafted, needs native review
palette-cycle-repeat = Wiederholen
# machine-drafted, needs native review
palette-cycle-repeat-help = Wiederholen aus, alle oder einen.
# machine-drafted, needs native review
palette-shuffle = Zufällige Wiedergabe
# machine-drafted, needs native review
palette-shuffle-help = In zufälliger Reihenfolge abspielen, jeden Titel einmal pro Runde.
# machine-drafted, needs native review
palette-play-queue = Warteschlange
# machine-drafted, needs native review
palette-play-queue-help = Zeigen, was als Nächstes läuft: umordnen, entfernen, speichern.
# machine-drafted, needs native review
palette-queue-play-next = Als Nächstes abspielen
# machine-drafted, needs native review
palette-queue-play-next-help = Die Datei (in der Galerie die Auswahl) nach dem laufenden Titel abspielen.
# machine-drafted, needs native review
palette-queue-add = Zur Warteschlange hinzufügen
# machine-drafted, needs native review
palette-queue-add-help = Die Datei (in der Galerie die Auswahl) ans Ende der Warteschlange stellen.
# machine-drafted, needs native review
palette-save-queue = Warteschlange als Playlist speichern …
# machine-drafted, needs native review
palette-save-queue-help = Die Warteschlange als M3U8- oder XSPF-Playlist speichern.
# machine-drafted, needs native review
transport-prev-track = Vorheriger Titel
# machine-drafted, needs native review
transport-next-track = Nächster Titel
# machine-drafted, needs native review
queue-title = Warteschlange
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 Titel
   *[other] { $count } Titel
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 Titel
   *[other] { $count } Titel
} · { $time }
# machine-drafted, needs native review
queue-empty = Die Warteschlange ist leer.
    Fügen Sie Tondateien und Videos mit „Als Nächstes abspielen“ oder „Zur Warteschlange hinzufügen“ hinzu, oder öffnen Sie eine Playlist.
# machine-drafted, needs native review
queue-empty-short = Die Warteschlange ist leer.
# machine-drafted, needs native review
queue-playing = Läuft
# machine-drafted, needs native review
queue-repeat = Wiederholen
# machine-drafted, needs native review
queue-repeat-off = Wiederholen: aus
# machine-drafted, needs native review
queue-repeat-all = Wiederholen: alle
# machine-drafted, needs native review
queue-repeat-one = Wiederholen: einen
# machine-drafted, needs native review
queue-shuffle = Zufall
# machine-drafted, needs native review
queue-shuffle-on = Zufall: ein
# machine-drafted, needs native review
queue-shuffle-off = Zufall: aus
# machine-drafted, needs native review
queue-save = Als Playlist speichern …
# machine-drafted, needs native review
queue-save-tip = Die Warteschlange als M3U8- oder XSPF-Playlist speichern
# machine-drafted, needs native review
queue-clear = Leeren …
# machine-drafted, needs native review
queue-clear-tip = Die Warteschlange leeren. Die Dateien bleiben auf dem Datenträger.
# machine-drafted, needs native review
queue-remove = Aus der Warteschlange entfernen (Datei bleibt erhalten)
# machine-drafted, needs native review
queue-network-item = Netzwerkeintrag, wird nicht abgespielt: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Kein nächster Titel.
# machine-drafted, needs native review
queue-at-start = Kein vorheriger Titel.
# machine-drafted, needs native review
queue-missing = { $name } ist nicht mehr da. Übersprungen.
# machine-drafted, needs native review
queue-network-not-played = Netzwerkeinträge werden noch nicht abgespielt.
# machine-drafted, needs native review
queue-nothing-to-add = Nichts hinzuzufügen. Nur Tondateien, Videos und Playlists kommen in die Warteschlange.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 Titel zur Warteschlange hinzugefügt.
   *[other] { $count } Titel zur Warteschlange hinzugefügt.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 Titel läuft als Nächstes.
   *[other] { $count } Titel laufen als Nächstes.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 Titel in der Warteschlange.
   *[other] { $count } Titel in der Warteschlange.
}
# machine-drafted, needs native review
queue-playlist-nothing = Nichts in { $name } lässt sich hier abspielen. Die Warteschlange zeigt, was sie enthält.
# machine-drafted, needs native review
queue-playlist-failed = { $name } lässt sich nicht öffnen: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = Die Datei lässt sich nicht lesen.
# machine-drafted, needs native review
queue-playlist-damaged = Es ist keine Playlist, die Specular lesen kann.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Warteschlange leeren (1 Titel)?
   *[other] Warteschlange leeren ({ $count } Titel)?
}
# machine-drafted, needs native review
queue-clear-detail = Nur die Liste wird geleert. Die Dateien bleiben auf dem Datenträger.
# machine-drafted, needs native review
queue-clear-ok = Warteschlange leeren
# machine-drafted, needs native review
queue-cleared = Warteschlange geleert.
# machine-drafted, needs native review
queue-save-title = Warteschlange als Playlist speichern
# machine-drafted, needs native review
queue-default-name = Warteschlange
# machine-drafted, needs native review
queue-format-m3u8 = M3U8-Playlist
# machine-drafted, needs native review
queue-format-xspf = XSPF-Playlist
# machine-drafted, needs native review
queue-saved = { $name } gespeichert.
# machine-drafted, needs native review
queue-save-failed = { $name } lässt sich nicht speichern: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Nächste Tondatei abspielen
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Wenn eine Tondatei endet, wird die nächste Tondatei im Ordner abgespielt.
# machine-drafted, needs native review
settings-viewing-play-repeat = Wiederholen
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Für Tondateien, Videos und die Warteschlange.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Aus
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Alle
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Einen
# machine-drafted, needs native review
settings-viewing-play-shuffle = Zufällige Wiedergabe
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = In zufälliger Reihenfolge abspielen, jeden Titel einmal pro Runde.
