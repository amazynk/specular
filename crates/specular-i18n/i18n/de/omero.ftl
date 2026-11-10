# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Deutsch. machine-drafted, needs native review

palette-omero-servers = OMERO-Server…

omero-title = OMERO-Server
omero-servers-intro = Projekte, Screens und Bilder eines Servers werden erst aufgelistet, wenn Sie ihn durchsuchen. Vorher wird nichts gesendet.
omero-servers = Server
omero-servers-empty = Noch keine Server. Fügen Sie einen mit seiner OMERO.web-Adresse hinzu.
omero-session-only = Passwörter werden nur bis zum Beenden von Specular aufbewahrt.
omero-add = Hinzufügen…
omero-edit = Bearbeiten…
omero-remove = Entfernen
omero-close = Schließen
omero-browse = Durchsuchen
omero-back = Zurück
omero-save = Speichern
omero-stop = Stoppen
omero-open = Öffnen
# $server a server's name.
omero-removed = { $server } entfernt
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Nicht verschlüsselt
# A server browsed without signing in.
omero-public = Öffentlich
# $user the user name a server is signed in with.
omero-signed-in-as = Als { $user }

# The page that adds or edits a server.
omero-add-title = Server hinzufügen
omero-edit-title = Servereinstellungen
omero-field-name = Name
omero-hint-name = Optional
omero-field-url = Adresse
omero-hint-url = https://omero.example.org
omero-field-user = Benutzername
omero-hint-user = Leer für öffentliche Daten
omero-field-password = Passwort
omero-hint-password-kept = Leer lassen, um das bisherige zu behalten
omero-password-session = Das Passwort wird nur bis zum Beenden von Specular aufbewahrt, nie auf der Festplatte.
omero-password-needed = Geben Sie das Passwort ein: Es wird nur bis zum Beenden von Specular aufbewahrt.
omero-http-warning = Diese Adresse ist nicht verschlüsselt: Das Passwort würde im Klartext gesendet.
omero-url-invalid = Geben Sie die OMERO.web-Adresse ein, beginnend mit https:// oder http://.
# $reason why the list could not be saved.
omero-save-failed = Die Serverliste konnte nicht gespeichert werden: { $reason }

# The browse page.
omero-browse-title = Durchsuchen
omero-top = Projekte und Screens
omero-datasets = Datasets
omero-plates = Platten
omero-wells = Wells
omero-fields = Felder
omero-images = Bilder
omero-kind-project = Projekt
omero-kind-dataset = Dataset
omero-kind-image = Bild
omero-kind-screen = Screen
omero-kind-plate = Platte
omero-kind-well = Well
# $well a well's row and column, as B7.
omero-well = Well { $well }
omero-well-fields = { $count ->
    [one] Ein Feld
   *[other] { $count } Felder
}
omero-children = { $count ->
    [one] Ein Eintrag
   *[other] { $count } Einträge
}
omero-load-more = Mehr laden
omero-loading = Wird geladen…
omero-empty = Hier ist nichts
omero-stopped = Gestoppt
omero-found = { $count ->
    [one] Ein Eintrag
   *[other] { $count } Einträge
}
# $reason why the image could not be opened.
omero-open-failed = Das Bild konnte nicht geöffnet werden: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Mindestens ein Kanal bleibt sichtbar
# $server a server's name.
omero-session-ended = Die Sitzung auf { $server } ist beendet. Geben Sie das Passwort ein, um fortzufahren.

# Errors.
# $server a server's name.
omero-error-offline = { $server } ist nicht erreichbar.
omero-error-sign-in = Für diesen Server ist eine Anmeldung nötig.
omero-error-expired = Die Sitzung ist beendet.
omero-error-login = Benutzername oder Passwort wurde abgelehnt.
omero-error-forbidden = Ihr Konto darf dies nicht sehen.
omero-error-not-found = Auf dem Server nicht gefunden (vielleicht wurde es entfernt).
# $code an HTTP status code.
omero-error-status = Der Server antwortete mit Fehler { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Die Antwort des Servers konnte nicht gelesen werden: { $reason }
