# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Bibliothek

## Top bar
library-title = Bibliothek
library-locations-button = Orte

## Locations sidebar
library-locations = Orte
library-all = Alle Orte
library-add = Ort hinzufügen…
library-remove = { $location } aus der Bibliothek entfernen
library-offline = Offline
library-location-pending = { $count } zu indizieren
library-a11y-location = { $location }, { $count ->
    [one] { $count } Bild
   *[other] { $count } Bilder
}
library-a11y-location-offline = { $location }, Offline, { $count ->
    [one] { $count } Bild
   *[other] { $count } Bilder
}

## Indexing progress
library-walking = Gefundene Bilder: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Gefundene Bilder: { $count }
library-reading = Indizierung… { $done } von { $total }
library-held = Die Indizierung pausiert, während Sie blättern
library-up-to-date = Aktuell
# machine-drafted, needs native review
library-stopped = Indizierung angehalten
library-stopped-pending = Indizierung angehalten · { $count } zu indizieren
library-resume = Fortsetzen
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indizierung angehalten: { $reason }
library-reason-stalled = seit { $minutes ->
    [one] einer Minute
   *[other] { $minutes } Minuten
} hat sich nichts getan. Eine Datei oder ein Laufwerk antwortet möglicherweise nicht.
library-retry = Erneut versuchen
library-retry-description = Startet die Indizierung neu. Was bisher indiziert wurde, bleibt durchsuchbar.
library-save-diagnostics = Diagnosepaket speichern …
library-stop-description = Beendet die Indizierung. Was bisher indiziert wurde, bleibt durchsuchbar; Fortsetzen oder Neu einlesen (F5) macht weiter.
library-resume-description = Durchsucht alle Orte erneut und indiziert, was noch fehlt.
library-opening = Bibliothek wird geöffnet…
library-searching = Suche…

## Empty and error states
library-empty = Noch keine Orte. Fügen Sie einen Ordner hinzu: Seine Bilder und die seiner Unterordner erscheinen hier in einem Raster.
library-empty-offline = Diese Orte sind offline. Ihre Bilder erscheinen wieder, sobald sie zurück sind.
library-indexing = Suche nach Bildern…
# machine-drafted, needs native review
library-remove-confirm = „{ $location }“ aus der Bibliothek entfernen?
library-remove-confirm-detail = Ihre Fotos bleiben auf dem Datenträger, mit Bewertungen, Farbmarkierungen und Stichwörtern. Die Bibliothek vergisst, was sie dort indiziert hat; fügen Sie den Ordner wieder hinzu, wird er neu indiziert.
library-remove-confirm-ok = Entfernen
library-indexing-in = Suche nach Bildern in { $location }… bisher { $count } gefunden
# machine-drafted, needs native review
library-empty-stopped = Die Indizierung wurde angehalten, bevor diese Bilder gelesen wurden. Fortsetzen macht dort weiter.
library-unavailable = Die Bibliothek konnte nicht geöffnet werden: { $reason }
library-read-only = Diese Bibliothek wurde von einem neueren Specular geschrieben: Sie kann durchsucht, aber nicht aktualisiert werden.

## Screen readers
library-a11y-list = Bibliothek, { $count ->
    [one] { $count } Bild
   *[other] { $count } Bilder
}
library-a11y-added = { $location } zur Bibliothek hinzugefügt
library-a11y-removed = { $location } aus der Bibliothek entfernt

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Alles in „{ $folder }“ hinzufügen?
library-home-confirm-detail = Der Ordner enthält Programmdateien, Downloads und Caches, nicht nur Fotos. Die Indizierung dauert länger, und die Bibliothek füllt sich mit Symbolen und anderen Bildern.
library-home-confirm-pictures = Ihr Ordner „Bilder“ enthält nur Ihre Bilder.
library-home-confirm-use-pictures = „Bilder“ hinzufügen
library-home-confirm-keep = Home-Ordner hinzufügen

## Folder watches
# machine-drafted, needs native review
library-unwatched = Einige Ordner werden nicht überwacht
library-unwatched-detail = { $minutes ->
    [one] Änderungen erscheinen innerhalb einer Minute
   *[other] Änderungen erscheinen innerhalb von { $minutes } min
}
library-unwatched-description = Specular hat keine Ordnerüberwachungen mehr frei. Öffnet die Hilfe dazu, wie sich das Limit erhöhen lässt.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Bibliothek
settings-library-skip-folders = Ordner mit diesen Namen überspringen
settings-library-skip-folders-help = Die Bibliothek durchsucht keine Ordner mit diesen Namen, durch Kommas getrennt. Versteckte Ordner werden immer übersprungen.
settings-library-skip-marked = Ordner überspringen, die enthalten
settings-library-skip-marked-help = Auch Ordner, die eines davon enthalten, werden übersprungen: .git für Code-Checkouts, pyvenv.cfg für Python-Umgebungen, CACHEDIR.TAG für Caches. Der Ordner eines Orts selbst wird nie übersprungen.
settings-library-show-timeline = Zeitleiste
settings-library-show-timeline-help = Zeigt über dem Raster der Bibliothek, wie viele Fotos jedes Jahr, jeder Monat oder Tag hat.
