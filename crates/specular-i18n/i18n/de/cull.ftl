# machine-drafted, needs native review
# Specular — Cull-Modus (DESIGN.md §6.3).

palette-cull = Cull-Modus öffnen oder verlassen
cheatsheet-cull = Cull-Modus
settings-section-cull = Cull-Modus
settings-cull-auto-enter = RAW-Ordner sichten
settings-cull-auto-enter-help = Einen Ordner, der überwiegend RAW-Dateien enthält, direkt im Cull-Modus öffnen.
settings-cull-auto-advance = Nach dem Markieren zum nächsten Bild
settings-cull-auto-advance-help = Nach Auswahl, Ablehnung oder Sternebewertung weitergehen. Farbmarkierungen und Aufheben bleiben beim Bild.
settings-cull-show = Anzeige bei RAW+JPEG-Paaren
settings-cull-show-help = Eine RAW- und eine JPEG-Datei mit gleichem Namen werden als ein Bild gesichtet, Markierungen gelten für beide. Das JPEG öffnet schneller.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Serien stapeln
settings-cull-bursts-help = Kurz nacheinander von einer Kamera aufgenommene Bilder erscheinen als ein Stapel. S öffnet oder schließt einen Stapel, B behält das angezeigte Bild und lehnt den Rest des Stapels ab.
settings-cull-burst-gap = Serienpause
settings-cull-burst-gap-help = Die längste Pause zwischen zwei Aufnahmen einer Serie.
settings-cull-watch = Neue Dateien zeigen
settings-cull-watch-help = Dateien, die während des Sichtens im Ordner erscheinen (eine angeschlossene Kamera, ein Import), werden am Ende hinzugefügt, sobald sie vollständig geschrieben sind.
settings-cull-follow-newest = Der neuesten Datei folgen
settings-cull-follow-newest-help = Zu jeder neuen Datei springen, sobald sie ankommt. T schaltet dies beim Sichten um.
settings-cull-slot-1 = Ordner 1
settings-cull-slot-1-help = Alt+1 kopiert das Bild (mit seiner RAW- oder JPEG-Datei und seinen Begleitdateien) in diesen Ordner, Alt+Umschalt+1 verschiebt es dorthin.
settings-cull-slot-1-unset = Nicht festgelegt
settings-cull-slot-2 = Ordner 2
settings-cull-slot-2-help = Alt+2 kopiert das Bild (mit seiner RAW- oder JPEG-Datei und seinen Begleitdateien) in diesen Ordner, Alt+Umschalt+2 verschiebt es dorthin.
settings-cull-slot-2-unset = Nicht festgelegt
settings-cull-slot-3 = Ordner 3
settings-cull-slot-3-help = Alt+3 kopiert das Bild (mit seiner RAW- oder JPEG-Datei und seinen Begleitdateien) in diesen Ordner, Alt+Umschalt+3 verschiebt es dorthin.
settings-cull-slot-3-unset = Nicht festgelegt
settings-cull-collisions = Wenn ein Name vergeben ist
settings-cull-collisions-help = Dateien in den Ordnern werden nie ersetzt.
settings-cull-collisions-suffix = Eine Nummer anhängen
settings-cull-collisions-skip = Das Bild überspringen
# machine-drafted, needs native review
# Cull-Modus gewählt, ohne dass ein Ordner oder Galeriebilder geöffnet sind.
cull-nothing = Nichts zu sichten – öffnen Sie zuerst einen Ordner mit Fotos
cull-slot-unset = Kein Ordner { $slot } festgelegt – wählen Sie einen unter Einstellungen → Cull-Modus
cull-slot-missing = Ordner { $slot } ist nicht verfügbar: { $reason }
cull-copied = Nach { $folder } kopiert
cull-moved = Nach { $folder } verschoben
cull-slot-renamed = Als { $name } in { $folder } gespeichert
cull-slot-skipped = { $folder } enthält bereits { $name } – übersprungen
cull-slot-same = Das Bild liegt bereits in { $folder }
cull-slot-failed = { $op ->
    [move] { $file } konnte nicht verschoben werden
   *[copy] { $file } konnte nicht kopiert werden
}: { $reason }
cull-copy-kept = { $count ->
    [one] Eine inzwischen geänderte Kopie behalten
   *[other] { $count } inzwischen geänderte Kopien behalten
}
cull-undo-failed = Rückgängig für { $file } nicht möglich: { $reason }
cull-burst-open = Serie { $index } von { $count }
cull-burst-closed = Serie mit { $count }
cull-burst-toggle = Serie öffnen oder schließen
cull-burst-best = 1 behalten, { $count } abgelehnt
cull-burst-none = Nicht Teil einer Serie
cull-follow-on = Neuen Dateien wird gefolgt
cull-follow-off = Neuen Dateien wird nicht gefolgt
cull-new-files = { $count ->
    [one] 1 neue Datei
   *[other] { $count } neue Dateien
}
cull-key-copy = In Ordner 1–3 kopieren
cull-key-move = In Ordner 1–3 verschieben
cull-key-burst = Serie öffnen oder schließen
cull-key-best = Dieses behalten, den Rest der Serie ablehnen
cull-key-follow = Neuen Dateien folgen
cull-key-subframes = Subframe-Tabelle zeigen oder ausblenden
cull-key-sort = Nach der nächsten Spalte sortieren
cull-key-sort-reverse = Reihenfolge umkehren
cull-key-metric = Nächste Messgröße im Diagramm
cull-key-sigma-down = Auto-Ablehnung: k verringern
cull-key-sigma-up = Auto-Ablehnung: k erhöhen
cull-key-auto-reject = Vorgeschlagene Frames ablehnen
cull-key-move-rejects = Abgelehnte Frames in einen Unterordner verschieben
cull-key-same-stretch = Gleiche Streckung für alle Frames

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Serie öffnen
cull-tip-burst-close = Serie schließen
cull-tip-exit = Cull-Modus verlassen
cull-tip-label = Farbmarkierung: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Einzelbilder
cull-astro-title = { $count ->
    [one] 1 Einzelbild
   *[other] { $count } Einzelbilder
}
cull-astro-pending = { $count } werden gemessen
cull-astro-col-name = Einzelbild
cull-astro-col-filter = Filter
cull-astro-col-exposure = Bel.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Exz.
cull-astro-col-stars = Sterne
cull-astro-col-bg = Hintergrund
cull-astro-col-snr = SNR
cull-astro-col-noise = Rauschen
cull-astro-same-stretch = Gleiche Streckung
cull-astro-own-stretch = Eigene Streckung
cull-astro-suggest-high = { $metric } über { $value } (Median + { $k }σ): { $count ->
    [one] 1 Frame
   *[other] { $count } Frames
}
cull-astro-suggest-low = { $metric } unter { $value } (Median − { $k }σ): { $count ->
    [one] 1 Frame
   *[other] { $count } Frames
}
cull-astro-suggest-none = Kein Frame jenseits von { $k }σ
cull-astro-suggest-few = Auto-Ablehnung braucht 3 gemessene Frames
cull-astro-suggested = zur Ablehnung vorgeschlagen
cull-astro-k = k { $k }
cull-astro-reject = { $count } ablehnen
cull-astro-move = Abgelehnte verschieben…
cull-astro-chart = { $metric } über { $count ->
        [one] { $count } Frame
       *[other] { $count } Frames
    }, Median { $median }.
cull-astro-sort-by = Nach { $column } sortieren
cull-astro-ascending = aufsteigend
cull-astro-descending = absteigend
cull-astro-sorted = Nach { $column } sortiert
cull-astro-reversed = Reihenfolge umgekehrt
cull-astro-metric = Diagramm: { $metric }
cull-astro-k-set = Auto-Ablehnung bei { $k }σ
cull-astro-none = Keine Subframes hier
cull-astro-same-on = Gleiche Streckung für alle Frames
cull-astro-same-off = Jedes Frame einzeln gestreckt
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = Exzentrizität { $value }
cull-astro-line-stars = { $count ->
    [one] 1 Stern
   *[other] { $count } Sterne
}
cull-astro-measuring = Sterne werden gemessen…
cull-astro-reject-confirm = { $count ->
    [one] Das vorgeschlagene Frame ablehnen?
   *[other] Die { $count } vorgeschlagenen Frames ablehnen?
}
cull-astro-reject-detail = { $rule }. Rückgängig stellt die Markierungen wieder her.
cull-astro-reject-ok = Ablehnen
cull-astro-nothing-to-reject = Kein Frame abzulehnen
cull-astro-rejected = { $count ->
    [one] 1 Frame abgelehnt
   *[other] { $count } Frames abgelehnt
}
cull-move-rejects-confirm = { $count ->
    [one] Das abgelehnte Frame in den Ordner „{ $folder }“ verschieben?
   *[other] Die { $count } abgelehnten Frames in den Ordner „{ $folder }“ verschieben?
}
cull-move-rejects-detail = Die Dateien werden verschoben, nicht gelöscht. Rückgängig holt sie zurück.
cull-move-rejects-ok = Verschieben
cull-move-rejects-none = Keine abgelehnten Frames zum Verschieben
cull-moved-rejects = { $count ->
    [one] 1 Frame nach „{ $folder }“ verschoben
   *[other] { $count } Frames nach „{ $folder }“ verschoben
}
