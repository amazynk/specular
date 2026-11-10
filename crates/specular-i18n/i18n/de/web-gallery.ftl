# Specular — Kundengalerien: ein statischer HTML-Ordner mit Fotos, und die Auswahl des Kunden zurücklesen.

# machine-drafted, needs native review

palette-client-gallery = Kundengalerie…
palette-client-gallery-help = Die Fotos als Webseiten-Ordner für einen Kunden schreiben, mit einer Auswahl, die er zurückschicken kann.
palette-import-client-picks = Kundenauswahl importieren…
palette-import-client-picks-help = Die Fotos markieren, die ein Kunde in seiner Galerie ausgewählt hat.

webgallery-heading = Kundengalerie
webgallery-subtitle = { $count ->
    [one] Ein Foto
   *[other] { $count } Fotos
}
webgallery-title-label = Titel
webgallery-title-default = Probeabzüge
webgallery-watermark-label = Wasserzeichentext (leer: keiner)
webgallery-source = Fotos
webgallery-source-current = Dieses Foto
webgallery-source-selection = Auswahl
webgallery-source-album = Album „{ $name }“
webgallery-source-search = Suchergebnisse
webgallery-source-folder = Dieser Ordner
webgallery-source-picks = Favoriten
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Design
webgallery-theme-light = Hell
webgallery-theme-dark = Dunkel
webgallery-theme-minimal = Schlicht
webgallery-size = Lange Kante
webgallery-size-value = { $px } px
webgallery-format = Format
webgallery-captions = Unter jedem Foto
webgallery-captions-off = Nichts
webgallery-captions-name = Dateiname
webgallery-captions-title = Titel
webgallery-captions-all = Dateiname, Titel und Bildunterschrift
webgallery-exif = Kamera und Belichtung zeigen
webgallery-location = Standort (GPS) in den Fotos behalten
webgallery-note-private = Eine statische Galerie lässt sich nicht mit einem Passwort schützen.
webgallery-note-share = Wer den Ordner oder seine Adresse hat, sieht jedes Foto. Teilen Sie den Link nur mit Ihrem Kunden, oder nutzen Sie die Zugriffskontrolle Ihres Webhosts.
webgallery-note-picks = Kunden wählen mit einem Herz aus. Die Auswahl bleibt in ihrem Browser, bis sie Ihnen die Liste schicken.
webgallery-write = Galerie schreiben…
webgallery-busy = Schreibe { $done } von { $total }…
webgallery-stop = Anhalten
webgallery-done = { $count ->
    [one] Galerie geschrieben: ein Foto in { $folder }
   *[other] Galerie geschrieben: { $count } Fotos in { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Ein Foto konnte nicht geschrieben werden.
   *[other] { $failed } Fotos konnten nicht geschrieben werden.
}
webgallery-stopped = Galerie angehalten. Nichts wurde behalten.
webgallery-failed = Galerie nicht geschrieben: { $reason }
webgallery-nothing = Kein Foto konnte geschrieben werden.

webgallery-page-count = { $count ->
    [one] Ein Foto
   *[other] { $count } Fotos
}
webgallery-page-hint = Tippen Sie auf das Herz bei den Fotos, die Sie möchten. Ihre Auswahl bleibt in diesem Browser, bis Sie sie schicken.
webgallery-page-picked = Ausgewählt: {"{"}n{"}"}
webgallery-page-pick = Auswählen
webgallery-page-copy = Meine Auswahl kopieren
webgallery-page-copied = Kopiert. Fügen Sie die Liste in eine Nachricht ein.
webgallery-page-copy-by-hand = Kopieren Sie die markierte Liste und fügen Sie sie in eine Nachricht ein.
webgallery-page-download-txt = picks.txt herunterladen
webgallery-page-download-json = picks.json herunterladen
webgallery-page-clear = Auswahl löschen
webgallery-page-clear-ask = Ihre gesamte Auswahl in diesem Browser löschen?
webgallery-page-no-storage = Dieser Browser behält Ihre Auswahl nicht. Schicken Sie sie, bevor Sie die Seite schließen.
webgallery-page-noscript = Zum Auswählen wird JavaScript benötigt. Die Fotos öffnen sich trotzdem.
webgallery-page-previous = Zurück
webgallery-page-next = Weiter
webgallery-page-close = Schließen
webgallery-page-position = {"{"}i{"}"} von {"{"}n{"}"}
webgallery-page-viewer = Fotobetrachter
webgallery-page-picks = Ihre Auswahl

clientpicks-heading = Kundenauswahl importieren
clientpicks-subtitle = { $count ->
    [one] Abgeglichen mit einem angezeigten Foto
   *[other] Abgeglichen mit den { $count } angezeigten Fotos
}
clientpicks-paste-label = Namen (Liste des Kunden einfügen)
clientpicks-choose = Auswahldatei wählen…
clientpicks-from-file = Aus { $name }
clientpicks-apply-as = Markieren
clientpicks-as-pick = Als Favoriten
clientpicks-as-stars = { $stars ->
    [one] Ein Stern
   *[other] { $stars } Sterne
}
clientpicks-as-label = Farbmarkierung { $label }
clientpicks-empty = Fügen Sie die Liste des Kunden ein, oder wählen Sie seine picks.txt oder picks.json.
clientpicks-matched = { $matched } von { $total } Namen passen zu Fotos hier.
clientpicks-missing = Nicht gefunden: { $names }
clientpicks-ambiguous = In mehr als einem Ordner, unverändert: { $names }
clientpicks-more = { $names } und { $more } weitere
clientpicks-apply = Anwenden
clientpicks-applied = { $count ->
    [one] Ein Foto markiert.
   *[other] { $count } Fotos markiert.
}
clientpicks-unchanged = Diese Fotos waren schon markiert.
clientpicks-read-failed = { $name } kann nicht gelesen werden: { $reason }
