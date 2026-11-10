# machine-drafted, needs native review
# Specular — Hintergrundaufgaben: die Anzeige, solange etwas im Hintergrund läuft, und ihr Bereich. Deutsch.

## Der Bereich

work-title = Hintergrundaufgaben
work-show = Hintergrundaufgaben anzeigen
work-empty = Im Hintergrund läuft nichts.

## Aufgaben

work-kind-library = Fotobibliothek
work-kind-ai-search = KI-Suche
work-kind-faces = Personen
work-kind-text = Text in Fotos
work-kind-culling = Bewertungen zum Sichten
work-kind-download = Download des KI-Modells
work-kind-similar = Ähnliche Aufnahmen

## Schritte

work-step-finding = Fotos werden gesucht
work-step-reading = Fotodetails werden gelesen
work-step-signatures = Fotos werden verglichen
work-step-grouping = Ähnliche Aufnahmen werden gruppiert
work-step-text-index = Suchindex wird erstellt
work-step-embedding = Fotos werden für die KI-Suche indiziert
work-step-switching = Neue Indizierung für die neue Modellgröße
work-step-faces = Gesichter werden gesucht
work-step-ocr = Text in Fotos wird gelesen
work-step-scoring = Schärfe und Belichtung werden bewertet
work-step-downloading = Wird heruntergeladen
work-step-focus = Schärfe wird geprüft
work-step-item = { $step } · { $item }

## Fortschritt

work-count-photos = { $done } von { $total } { $count ->
    [one] Foto
   *[other] Fotos
}
work-count = { $done } von { $total }
work-count-left = { $count } · noch { $left }
work-so-far-photos = Bisher { $done } { $count ->
    [one] Foto
   *[other] Fotos
}
work-so-far = Bisher { $done }

## Verbleibende Zeit

work-eta-estimating = Wird geschätzt …
work-eta-seconds = Noch etwa { $n ->
    [one] { $n } Sekunde
   *[other] { $n } Sekunden
}
work-eta-minutes = Noch etwa { $n ->
    [one] { $n } Minute
   *[other] { $n } Minuten
}
work-eta-hours = Noch etwa { $h ->
    [one] { $h } Stunde
   *[other] { $h } Stunden
}
work-eta-hours-minutes = Noch etwa { $h ->
    [one] { $h } Stunde
   *[other] { $h } Stunden
} { $m ->
    [one] { $m } Minute
   *[other] { $m } Minuten
}

## Warum eine Aufgabe wartet

work-paused-browsing = Angehalten, solange Sie Fotos durchsehen
work-paused-battery = Im Akkubetrieb angehalten
work-paused-low-power = Im Energiesparmodus angehalten
work-paused-heat = Angehalten, solange der Computer heiß ist
work-paused-memory = Angehalten, solange der Arbeitsspeicher knapp ist
work-paused-user = Angehalten

## Was folgt

work-then = Danach: { $steps }

## Schaltflächen

work-pause = Anhalten
work-resume = Fortsetzen
work-settings = Einstellungen …

## Die Anzeige

work-chip = { $job } · { $done } von { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Hintergrundaufgaben: { $status }

## Ansagen

work-started = { $job } im Hintergrund gestartet
work-finished = { $job } abgeschlossen
