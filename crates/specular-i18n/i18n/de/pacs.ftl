# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Art
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Host
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Port
dicomweb-hint-port = 104 oder 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE-Titel des Servers
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Unser AE-Titel
# machine-drafted, needs native review
dicomweb-field-tls = Sichere Verbindung (TLS)
dicomweb-tls-on = Sichere Verbindung ein
dicomweb-tls-off = Sichere Verbindung aus
dicomweb-problem-host = Geben Sie den Hostnamen oder die Adresse des Servers ein.
dicomweb-problem-port = Der Port ist eine Zahl von 1 bis 65535.
dicomweb-problem-ae = Ein AE-Titel hat 1 bis 16 Zeichen, ohne Backslash.
# machine-drafted, needs native review
dicomweb-dimse-note-open = DICOM-Server (DIMSE) lassen sich durchsuchen und öffnen, und Sie können an sie senden.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Abrufen mit
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Empfangen an Adresse
# machine-drafted, needs native review
pacs-hint-receive-address = Adresse dieses Computers, etwa 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Empfangen an Port
# machine-drafted, needs native review
pacs-hint-receive-port = Etwa 11113
# machine-drafted, needs native review
pacs-move-note = Für C-MOVE muss der Server unseren AE-Titel { $ae } mit dieser Adresse und diesem Port kennen.
    Specular nimmt dort nur Verbindungen an, solange eine Serie eingeht.
# machine-drafted, needs native review
pacs-problem-receive-address = Geben Sie die Adresse dieses Computers ein (etwa 192.168.1.20), an der empfangen werden soll.
# machine-drafted, needs native review
pacs-problem-receive-port = Geben Sie einen Port für den Empfang ein. C-MOVE benötigt einen.
dicomweb-verify = Prüfen
dicomweb-verifying = Server wird geprüft…
dicomweb-verify-ok = { $server } hat geantwortet.

## The Studies browser and the palette.

studies-send = An PACS senden…
studies-send-description = Sendet diese Serie, ihre Studie oder alles auf dem Datenträger an einen Server.
palette-send-to-pacs = An PACS senden…
palette-send-to-pacs-help = Die aktuelle Serie oder Studie an einen PACS-Server senden.

## The Send page.

pacs-title = An PACS senden
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } Studie
   *[other] { $studies } Studien
} · { $files ->
    [one] { $files } Datei
   *[other] { $files } Dateien
}
pacs-what = Senden
pacs-scope-series = Diese Serie ({ $files ->
    [one] { $files } Datei
   *[other] { $files } Dateien
})
pacs-scope-study = Diese Studie ({ $files ->
    [one] { $files } Datei
   *[other] { $files } Dateien
})
pacs-scope-all = Alles Eingelesene ({ $studies ->
    [one] { $studies } Studie
   *[other] { $studies } Studien
}, { $files ->
    [one] { $files } Datei
   *[other] { $files } Dateien
})
pacs-anonymise = Anonymisieren
pacs-anonymise-note = Patientendaten werden aus den gesendeten Kopien entfernt. Ihre Dateien bleiben unverändert.
pacs-identified-note = Die Dateien werden unverändert gesendet, mit Patientendaten.
# The list of servers to choose from.
pacs-to = An
pacs-no-servers = Noch keine Server. Fügen Sie einen unter DICOMweb-Server hinzu.
pacs-send = Senden…
pacs-go-back = Zurück
pacs-confirm-send = Senden
pacs-confirm = { $studies ->
    [one] { $studies } Studie
   *[other] { $studies } Studien
} ({ $files ->
    [one] { $files } Datei
   *[other] { $files } Dateien
}) an { $server } senden?
pacs-busy = Es läuft noch ein Sendevorgang.
pacs-started = Senden läuft.
pacs-stopping = Wird gestoppt…
pacs-sending = { $done } von { $total } werden an { $server } gesendet…
pacs-sent-all = { $total ->
    [one] { $total } Datei wurde an { $server } gesendet.
   *[other] Alle { $total } Dateien wurden an { $server } gesendet.
}
pacs-sent-some = { $sent } von { $total } Dateien wurden an { $server } gesendet.
pacs-stopped = Gestoppt nach { $sent } von { $total ->
        [one] { $total } Datei
       *[other] { $total } Dateien
    }.
pacs-stopped-because = Gestoppt nach { $sent } von { $total ->
        [one] { $total } Datei
       *[other] { $total } Dateien
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } Datei wurde mit einer Warnung gespeichert.
   *[other] { $count } Dateien wurden mit einer Warnung gespeichert.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = Und { $count } weitere.

## Why one file was not sent.

pacs-reason-unreadable = Konnte nicht gelesen werden: { $reason }
pacs-reason-not-dicom = Keine DICOM-Datei
pacs-reason-burned-in = Enthält ins Bild eingebrannte Patientendaten und wird daher nicht anonymisiert gesendet
pacs-reason-document = Ein Dokument, das nicht anonymisiert werden kann
pacs-reason-anonymise = Konnte nicht anonymisiert werden: { $reason }
pacs-reason-not-accepted = Der Server nimmt { $class } nicht an
pacs-reason-convert = Konnte nicht für den Server umgewandelt werden: { $reason }
pacs-reason-resources = Dem Server fehlen Speicherplatz oder Ressourcen
pacs-reason-mismatch = Laut Server passen die Daten nicht zu ihrem Typ
pacs-reason-understand = Der Server konnte sie nicht lesen
pacs-reason-class = Der Server unterstützt diese Art von Objekt nicht
pacs-reason-not-authorised = Der Server erlaubt es nicht
pacs-reason-status = Der Server hat sie abgelehnt (Status { $code })
pacs-reason-refused = Der Server hat sie abgelehnt
pacs-reason-too-large = Zu groß für den Server
pacs-reason-http = Der Server hat sie abgelehnt (Fehler { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Das Netzwerk ist ausgeschaltet.
pacs-fatal-unknown-host = { $host } ist kein bekannter Hostname.
pacs-fatal-connect = Unter { $host }, Port { $port }, antwortet nichts.
pacs-fatal-unreachable = { $host } ist nicht erreichbar.
pacs-fatal-timeout = { $host } hat nicht rechtzeitig geantwortet.
pacs-fatal-cancelled = Gestoppt.
pacs-fatal-called-ae = { $host } antwortet nicht auf den AE-Titel { $ae }. Prüfen Sie den AE-Titel des Servers.
pacs-fatal-calling-ae = { $host } kennt unseren AE-Titel { $ae } nicht. Fügen Sie ihn auf dem Server hinzu.
pacs-fatal-busy = { $host } ist ausgelastet. Versuchen Sie es später erneut.
pacs-fatal-rejected = { $host } hat die Verbindung abgelehnt.
pacs-fatal-aborted = { $host } hat die Verbindung beendet.
pacs-fatal-nothing-accepted = { $host } nimmt keine dieser Objektarten an.
pacs-fatal-protocol = { $host } hat auf eine Weise geantwortet, die Specular nicht versteht.
pacs-fatal-auth = { $host } hat die Anmeldung nicht angenommen.
pacs-fatal-http = { $host } hat mit Fehler { $status } geantwortet.

## Teaching quiz.

palette-teaching-quiz = Lehrquiz
palette-teaching-quiz-help = Die Schlüsselbilder oder einen Lehrfall durchgehen, die Befunde verborgen, bis Sie sie aufdecken.
# "Case 2 of 5".
quiz-case = Fall { $n } von { $total }
quiz-hint-hidden = Leertaste zeigt die Befunde · → nächster Fall · Esc beendet
quiz-hint-revealed = Leertaste verbirgt sie wieder · → nächster Fall · Esc beendet
quiz-hidden = Befunde verborgen.
quiz-no-findings = Keine Befunde erfasst.
quiz-notes = Fallnotizen: { $notes }
quiz-ended = Quiz beendet.
quiz-empty = Es gibt keine Fälle zum Anzeigen. Markieren Sie Schlüsselbilder mit K oder wählen Sie einen Lehrfall-Ordner.
quiz-not-a-case = Dieser Ordner ist kein von Specular exportierter Lehrfall.
quiz-pick-title = Lehrfall-Ordner wählen

## Teaching case folder (Export key images).

figure-keys-case = Lehrfall-Ordner
figure-case-notes = Fallnotizen
figure-case-findings = Befunde
figure-case-none = Keine erfasst
figure-case-readme = Von Specular exportierter Lehrfall. Patientendaten sind entfernt.
    images: die Schlüsselbilder, eingebrannter Text geschwärzt.
    DICOM: anonymisierte Kopien und ein Key-Object-Selection-Manifest „For Teaching“.
    case.txt: die Fallnotizen und die Befunde jedes Bildes. case.json: dasselbe für Skripte.
    Für Radiopaedia: Laden Sie den DICOM-Ordner oder die Bilder hoch und kopieren Sie die Notizen aus case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } beantwortet keine Suchanfragen (C-FIND).
pacs-find-status = { $host } hat die Suche abgelehnt (Status { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } sendet keine Serien per C-GET.
    Wählen Sie C-MOVE in den Einstellungen des Servers.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } nimmt keine C-MOVE-Anfragen an.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } kennt unseren AE-Titel { $ae } nicht.
    Fügen Sie ihn auf dem Server mit der Adresse dieses Computers und Port { $port } hinzu.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } hat keine Ressourcen frei. Versuchen Sie es später erneut.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } hat das Senden der Serie abgelehnt (Status { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } hat keine Bilder zu dieser Serie.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } hat es abgelehnt, Bilder über diese Verbindung zu senden.
    Wählen Sie C-MOVE in den Einstellungen des Servers.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } konnte die Bilder nicht an diesen Computer senden.
    Prüfen Sie Adresse, Port und Firewall und ob der Server unseren AE-Titel { $ae } kennt.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } konnte { $count ->
    [one] das Bild nicht
   *[other] keines der { $count } Bilder
} in einer Form senden, die Specular lesen kann.
# machine-drafted, needs native review
pacs-retrieve-listen = Empfang an { $address }, Port { $port } nicht möglich: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Das Zertifikat von { $host } ist nicht vertrauenswürdig.
    Sein Aussteller muss zu den vertrauenswürdigen Zertifikaten des Systems gehören.
pacs-fatal-tls = Keine sichere Verbindung zu { $host }.
    Prüfen Sie, ob Port { $port } DICOM über TLS spricht.
