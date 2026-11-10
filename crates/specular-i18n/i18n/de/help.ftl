# machine-drafted, needs native review
## Das Hilfefenster des Handbuchs, Absturzberichte, das Diagnosepaket, Update-Prüfung und Info. Deutsch.
## Stil: kurz, klar, ohne Füllwörter; ein Gedanke pro Satz.

## Palette

palette-help-manual = Hilfe
palette-help-manual-help = Das Handbuch auf der Seite zum aktuellen Bildschirm öffnen.
palette-save-diagnostics = Diagnosepaket speichern …
palette-save-diagnostics-help = Absturzberichte, Protokoll und Systemdetails als ZIP-Datei speichern. Es wird nichts gesendet.
# Paletteneintrag, der einen Abschnitt des Handbuchs öffnet; { $title } ist eine Seite oder „Seite › Abschnitt“.
palette-help-topic = Hilfe: { $title }
palette-help-topic-help = Diesen Abschnitt des Handbuchs öffnen.
# Unter einem nicht verfügbaren Befehl in der Palette.
palette-disabled-help = Gerade nicht verfügbar. Eingabetaste zeigt den Grund.

## Das Hilfefenster

help-title = Hilfe
help-search-placeholder = Handbuch durchsuchen
help-contents = Inhalt
help-back = Zurück
help-forward = Vorwärts
help-no-results = Nichts gefunden für „{ $query }“.
help-results = { $count ->
    [one] 1 Abschnitt gefunden
   *[other] { $count } Abschnitte gefunden
}
help-section-start = Erste Schritte
help-section-modes = Anzeige und Modi
help-section-tasks = Wie kann ich …
help-section-reference = Referenz
help-keys-unbound = Nicht belegt
# Ein im Handbuch genannter Befehl ohne Taste; er läuft über die Palette.
help-key-unbound = { $name } (Palette)
help-keys-command = Befehl
help-keys-keys = Tasten
help-commands = Jeder Befehl ist in der Befehlspalette ({ $key }). Einen Teil des Namens eingeben.
help-commands-no-key = Jeder Befehl ist in der Befehlspalette. Einen Teil des Namens eingeben.
help-link = Zu { $title }
help-open = Hilfe
# Name des Fensters für Bildschirmleser; { $page } ist die angezeigte Seite.
help-label = Hilfe: { $page }
help-disabled-why = Warum?

## Absturzbericht beim nächsten Start

crash-title = { -app-name } wurde unerwartet beendet
crash-body = Ein Bericht wurde auf diesem Computer gespeichert. Es wurde nichts gesendet.
crash-view = Bericht anzeigen
crash-hide = Bericht ausblenden
crash-copy = Kopieren
crash-copied = Bericht kopiert
crash-save-bundle = Diagnosepaket speichern …
crash-dismiss = Schließen

## Diagnosepaket

diag-title = Diagnosepaket speichern
diag-intro = Eine ZIP-Datei zum Anhängen an einen Fehlerbericht. Sie wird auf diesem Computer gespeichert. Es wird nichts gesendet.
diag-contents = Inhalt
diag-include-names = Dateinamen einschließen
diag-include-names-help = Aus: Namen werden durch file-1.jpg, dir-1 usw. ersetzt.
diag-privacy = Ihr Benutzerordner erscheint als ~ und Ihr Benutzername als <user>. Passwörter, Serveradressen und Bilder sind nie enthalten.
diag-where = Problem melden: { $issues } · { $email }
diag-save = Speichern …
diag-cancel = Abbrechen
diag-saved = Diagnosepaket gespeichert: { $path }
diag-failed = Diagnosepaket konnte nicht gespeichert werden: { $reason }
diag-entry-summary = Version, System, GPU und Treiber
diag-entry-crash = Letzter Absturzbericht
diag-entry-crashes = Frühere Absturzberichte ({ $count })
diag-entry-log = Letzte { $count } Protokollzeilen
diag-entry-settings = Einstellungen, ohne Passwörter und Adressen
diag-entry-gpu = GPU- und Anzeigedetails
diag-entry-readme = Was dieses Paket ist
diag-size = { $size }
# Die README.txt im Paket.
diag-readme =
    { -app-name } – Diagnosepaket.

    Es enthält, was bei der Fehlersuche hilft: Version und Systemdetails, Absturzberichte, das Ende des Protokolls und die Einstellungen.

    Ihr Benutzerordner erscheint als ~ und Ihr Benutzername als <user>. Sofern Sie sie nicht einschließen wollten, werden Datei- und Ordnernamen durch file-1, dir-1 usw. ersetzt. Passwörter und Serveradressen werden entfernt.

    Es wurde nichts gesendet. Sie entscheiden, ob Sie diese Datei weitergeben.

## Updates (Einstellungen)

settings-section-updates = Updates
settings-updates-check = Nach Updates suchen
settings-updates-check-help = Einmal am Tag fragen, ob es eine neue Version gibt. Die Anfrage enthält nur die Versionsnummer: keine Kennung, kein Cookie. Aus: { -app-name } verbindet sich nie für Updates.
settings-updates-channel = Kanal
settings-updates-channel-help = Stabil: nur Releases. Beta: auch Testversionen.
settings-updates-channel-stable = Stabil
settings-updates-channel-beta = Beta
settings-updates-check-now = Jetzt prüfen
settings-updates-check-now-help = Einmal jetzt prüfen.
update-available = Version { $version } ist verfügbar
update-notes = Versionshinweise
update-download = Herunterladen
update-none = { -app-name } ist auf dem neuesten Stand.
update-checking = Suche nach Updates …
update-failed = Suche nach Updates fehlgeschlagen: { $reason }
update-last-checked = Zuletzt geprüft: { $when }

## Info

about-build = Build { $commit } · { $profile } · { $target }
about-build-no-commit = Build { $profile } · { $target }
about-help = Hilfe
about-diagnostics = Diagnosepaket speichern …
# machine-drafted, needs native review
about-renderer = Rendererinfo …
about-licences-count = { $count ->
    [one] 1 Fremdkomponente
   *[other] { $count } Fremdkomponenten
}
about-licence-group = { $licence } ({ $count })
about-contact = Fehler: { $issues } · Support: { $email }
