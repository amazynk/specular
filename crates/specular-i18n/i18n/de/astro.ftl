# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = DS9-Regionen überlagern…
palette-science-load-regions-help = Die Formen einer DS9-Regionsdatei (.reg) auf das Bild zeichnen.
palette-science-export-regions = Regionen als DS9 speichern…
palette-science-export-regions-help = Die Messungen und die überlagerten Regionen als DS9-Regionsdatei speichern.
palette-science-edit-regions = Regionen als Messungen bearbeiten
palette-science-edit-regions-help = Die überlagerten Regionen in Messungen umwandeln, die Sie verschieben und ändern können.
palette-science-clear-regions = Überlagerte Regionen entfernen
palette-science-clear-regions-help = Die DS9-Regionen vom Bild entfernen.
palette-science-plate-solve = Plate Solving
palette-science-plate-solve-help = Die Himmelskoordinaten des Bildes mit ASTAP oder astrometry.net bestimmen. Erneut ausführen zum Anhalten.
palette-science-save-plate-solution = Plate-Solving-Ergebnis speichern
palette-science-save-plate-solution-help = Die gefundenen Himmelskoordinaten neben dem Bild als .wcs-Datei speichern.
palette-science-sky-survey = Himmelsdurchmusterung als Hintergrund
palette-science-sky-survey-help = Die nächste Himmelsdurchmusterung rund um das Bild zeigen, oder keine.
palette-science-samp-send-image = Bild an SAMP-Anwendungen senden
palette-science-samp-send-image-help = Dieses Bild in DS9, Aladin und den anderen Anwendungen am SAMP-Hub öffnen.
palette-science-samp-send-position = Position an SAMP-Anwendungen senden
palette-science-samp-send-position-help = Die anderen Anwendungen auf die Himmelsposition unter dem Zeiger ausrichten.

disabled-samp-off = Verbinden Sie sich zuerst mit einem SAMP-Hub (Einstellungen › Integration).

settings-integration-samp = Mit einem SAMP-Hub verbinden
settings-integration-samp-help = Bilder, Tabellen und Himmelspositionen mit DS9, TOPCAT, Aladin und anderen Astronomie-Anwendungen austauschen. Nur ein Hub auf diesem Computer wird verwendet.
settings-integration-astap-path = ASTAP-Programm
settings-integration-astap-path-help = Der Plate Solver ASTAP, falls er nicht von selbst gefunden wird.
settings-integration-solve-field-path = astrometry.net-Programm
settings-integration-solve-field-path-help = Das Programm solve-field von astrometry.net, falls es nicht von selbst gefunden wird.

astro-regions-filter = DS9-Regionen
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } Region.
       *[other] { $count } Regionen.
    }
astro-regions-refused = { $count ->
        [one] { $count } Zeile nicht gelesen.
       *[other] { $count } Zeilen nicht gelesen.
    } Erste: Zeile { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } Panda als innerer und äußerer Rand gezeichnet.
       *[other] { $count } Pandas als innere und äußere Ränder gezeichnet.
    }
astro-regions-none-here = { $name }: Keine Region liegt auf diesem Bild.
astro-regions-failed = { $name } konnte nicht gelesen werden: { $reason }
astro-regions-cleared = Regionen entfernt.
astro-regions-nothing = Keine Regionen oder Messungen zum Speichern.
astro-regions-saved = { $name } gespeichert.
astro-regions-skipped = { $count ->
        [one] { $count } Form außerhalb der Himmelskoordinaten wurde ausgelassen.
       *[other] { $count } Formen außerhalb der Himmelskoordinaten wurden ausgelassen.
    }
astro-regions-save-failed = Die Regionen konnten nicht gespeichert werden: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } Region ist jetzt eine Messung.
       *[other] { $count } Regionen sind jetzt Messungen.
    }
astro-regions-no-wcs = Himmelsregionen brauchen ein Bild mit Himmelskoordinaten.

astro-solve-no-solver = Kein Plate Solver gefunden. Installieren Sie ASTAP oder astrometry.net, oder wählen Sie einen unter Einstellungen › Integration.
astro-solve-running = Lösen mit { $solver }… { $seconds } s
astro-solve-stop = Stopp
astro-solve-stopped = Plate Solving angehalten.
astro-solve-done = Gelöst: { $ra }, { $dec }, { $scale } pro Pixel.
astro-solve-failed = Plate Solving fehlgeschlagen: { $reason }
astro-solve-not-local = Nur Bilder in einem Ordner auf diesem Computer können gelöst werden.
astro-solve-none = Für dieses Bild gibt es noch kein Plate-Solving-Ergebnis.
astro-solve-saved = { $name } gespeichert. Das Bild selbst ist unverändert.
astro-solve-sidecar = Himmelskoordinaten aus { $name }.

astro-samp-connected = Mit dem SAMP-Hub verbunden.
astro-samp-no-hub = SAMP: { $reason }. Neuer Versuch, sobald ein Hub startet.
astro-samp-lost = Der SAMP-Hub wurde beendet. Neuer Versuch, sobald einer startet.
astro-samp-sent = { $count ->
        [one] An { $count } Anwendung gesendet.
       *[other] An { $count } Anwendungen gesendet.
    }
astro-samp-nobody = Keine Anwendung am Hub nimmt das entgegen.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } liegt nicht auf diesem Computer und wird daher nicht geöffnet.
astro-samp-pointed = Ausgerichtet auf { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } liegt nicht auf diesem Bild.
astro-samp-no-table = SAMP: Diese Tabelle ist hier nicht überlagert.
astro-samp-rows = { $count ->
        [one] { $count } Katalogquelle hervorgehoben.
       *[other] { $count } Katalogquellen hervorgehoben.
    }

astro-survey-off = Himmelsdurchmusterung aus.
astro-survey-on = Himmelsdurchmusterung: { $name }. { $credit }
astro-survey-failed = { $name } kann nicht angezeigt werden: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Kacheln der Himmelsdurchmusterung von { $host }
