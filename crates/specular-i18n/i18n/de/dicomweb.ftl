# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Deutsch.

palette-dicom-servers = DICOMweb-Server…

dicomweb-title = DICOMweb-Server
dicomweb-servers-intro = Studien auf einem Server werden nur auf Ihre Anforderung gesucht und geöffnet. Vorher wird nichts gesendet.
dicomweb-servers = Server
dicomweb-servers-empty = Noch keine Server. Fügen Sie einen mit seiner DICOMweb-Adresse hinzu.
dicomweb-session-only = Passwörter und Tokens werden nur aufbewahrt, bis Specular geschlossen wird.
dicomweb-add = Hinzufügen…
dicomweb-edit = Bearbeiten…
dicomweb-remove = Entfernen
dicomweb-close = Schließen
dicomweb-browse = Durchsuchen
dicomweb-back = Zurück
dicomweb-save = Speichern
dicomweb-search = Suchen
dicomweb-stop = Stopp
# $server ist der Name eines Servers.
dicomweb-removed = { $server } entfernt
# machine-drafted, needs native review
dicomweb-remove-ask = „{ $server }“ aus den gespeicherten Servern entfernen?
dicomweb-remove-detail = Auf dem Datenträger geht nichts verloren. Sein Passwort oder Token für diese Sitzung wird vergessen.
# Bei Servern ohne Verschlüsselung (http://).
dicomweb-not-encrypted = Nicht verschlüsselt

# Seite zum Hinzufügen oder Bearbeiten eines Servers.
dicomweb-add-title = Server hinzufügen
dicomweb-edit-title = Servereinstellungen
dicomweb-field-name = Name
dicomweb-hint-name = Optional
dicomweb-field-url = Adresse
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Anmeldung
dicomweb-auth-none = Keine Anmeldung
dicomweb-auth-basic = Benutzername und Passwort
dicomweb-auth-bearer = Zugriffstoken
dicomweb-field-user = Benutzername
dicomweb-field-password = Passwort
dicomweb-field-token = Zugriffstoken
dicomweb-hint-secret-kept = Leer lassen, um den bisherigen Wert zu behalten
dicomweb-secret-session = Das Passwort oder Token wird nur aufbewahrt, bis Specular geschlossen wird, und nie gespeichert.
dicomweb-secret-needed = Geben Sie das Passwort oder Token erneut ein: Es wird nur aufbewahrt, bis Specular geschlossen wird.
dicomweb-http-warning = Diese Adresse ist nicht verschlüsselt: Das Passwort oder Token würde im Klartext gesendet.
dicomweb-url-empty = Geben Sie die DICOMweb-Adresse des Servers ein.
dicomweb-url-scheme = Die Adresse muss mit https:// oder http:// beginnen.
dicomweb-url-host = Die Adresse enthält keinen Servernamen.
dicomweb-url-userinfo = Geben Sie Benutzername und Passwort in die Anmeldefelder ein, nicht in die Adresse.
# $reason ist der Grund, warum die Liste nicht gespeichert werden konnte.
dicomweb-save-failed = Die Serverliste konnte nicht gespeichert werden: { $reason }

# Suchseite.
dicomweb-search-title = Studien suchen
dicomweb-field-patient-name = Patientenname
dicomweb-field-patient-id = Patienten-ID
dicomweb-field-date-from = Von
dicomweb-field-date-to = Bis
dicomweb-hint-date = JJJJMMTT
dicomweb-field-modality = Modalität
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Auftragsnummer
dicomweb-show-patient = Patientendaten anzeigen
dicomweb-switch-on = Ein
dicomweb-switch-off = Aus
dicomweb-patient-shown = Patientendaten angezeigt
dicomweb-patient-hidden = Patientendaten ausgeblendet
dicomweb-studies = Studien
dicomweb-search-hint = Füllen Sie beliebige Felder aus und starten Sie die Suche.
dicomweb-no-studies = Keine passenden Studien.
dicomweb-searching = Suche läuft…
dicomweb-stopped = Gestoppt
dicomweb-load-more = Mehr laden
dicomweb-study-untitled = Studie ohne Beschreibung
dicomweb-series-untitled = Serie ohne Beschreibung
dicomweb-study-counts = { $series ->
    [one] Eine Serie
   *[other] { $series } Serien
}, { $images ->
    [one] ein Bild
   *[other] { $images } Bilder
}
dicomweb-series-images = { $count ->
    [one] Ein Bild
   *[other] { $count } Bilder
}
# $id ist eine Patienten-ID.
dicomweb-patient-id = ID { $id }
# $number ist eine Auftragsnummer.
dicomweb-accession = Auftrag { $number }
dicomweb-found = { $count ->
    [one] Eine Studie gefunden
   *[other] { $count } Studien gefunden
}
dicomweb-series-found = { $count ->
    [one] Eine Serie
   *[other] { $count } Serien
}

# Fehler bei der Kommunikation mit einem Server. $server ist der Name eines Servers.
dicomweb-error-offline = { $server } ist nicht erreichbar. Prüfen Sie das Netzwerk und die Adresse.
dicomweb-error-auth = Der Server hat Benutzername, Passwort oder Token abgelehnt.
dicomweb-error-not-found = Unter dieser Adresse gibt es keinen DICOMweb-Dienst.
# $reason ist ein technischer Grund.
dicomweb-error-other = Der Server konnte nicht antworten: { $reason }

# Eine in den Viewer gestreamte Serie. $series ist die Beschreibung einer Serie.
dicomweb-loading = { $series } wird geladen…
# $done und $total sind Bildanzahlen.
dicomweb-loading-progress = { $series } wird geladen: { $done } von { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] ein Bild geladen
   *[other] alle { $total } Bilder geladen
}
dicomweb-loaded-failed = { $failed } von { $total } Bildern konnten nicht geladen werden.
# $server ist der Name eines Servers.
dicomweb-offline-cached = { $server } ist nicht erreichbar: Die zuvor gespeicherten Bilder werden angezeigt.
# $reason ist ein technischer Grund.
dicomweb-cache-failed = Der Download-Ordner konnte nicht geöffnet werden: { $reason }
