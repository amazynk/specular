# machine-drafted, needs native review

settings-section-decoders = Optionale Decoder
settings-decoders-look-again = Erneut suchen
settings-decoders-look-again-help = Nach seither installierten Decodern suchen.
    { -app-name } sucht auch jedes Mal, wenn sein Fenster in den Vordergrund kommt.
settings-decoders-braw = Blackmagic RAW SDK verwenden
settings-decoders-braw-help = Blackmagic RAW-Clips werden damit geöffnet, wenn es installiert ist.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Leer lassen, um es dort zu finden, wo sein Installationsprogramm es ablegt.
settings-decoders-dwg = ODA File Converter verwenden
settings-decoders-dwg-help = DWG-Zeichnungen werden damit geöffnet, wenn er installiert ist.
    Eine Kopie der Zeichnung wird in einem privaten Ordner umgewandelt, der danach gelöscht wird.
settings-decoders-dwg-path = Programm ODA File Converter
settings-decoders-dwg-path-help = Leer lassen, um ihn dort zu finden, wo sein Installationsprogramm ihn ablegt.
settings-decoders-occt = OpenCASCADE verwenden
settings-decoders-occt-help = STEP- und IGES-Modelle werden damit geöffnet, wenn es installiert ist.
settings-decoders-occt-path = OpenCASCADE-Bibliotheken
settings-decoders-occt-path-help = Leer lassen, um sie dort zu finden, wo das System Bibliotheken ablegt.
# machine-drafted, needs native review
settings-decoders-czi = libCZI verwenden
settings-decoders-czi-help = Zeiss-CZI-Dateien werden damit geöffnet, wenn es installiert ist.
    Es läuft im abgeschotteten Hilfsprozess von { -app-name }, ohne Zugriff auf Ihre anderen Dateien.
settings-decoders-czi-path = Ordner der libCZI-Bibliothek
settings-decoders-czi-path-help = Leer lassen, um sie dort zu finden, wo das System Bibliotheken ablegt.

decoders-intro = Einige Formate öffnen sich mit kostenloser Software ihrer Hersteller.
    Einmal installieren: { -app-name } findet und verwendet sie. Es lädt sie nie herunter.
decoders-keywords = optionale Decoder Hersteller SDK Codec Plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI braucht Decoder
decoders-row = { $format } ({ $extensions })
decoders-state-found = Gefunden
decoders-state-off = Aus
decoders-state-not-found = Nicht installiert
decoders-state-broken = Nicht verwendbar
decoders-state-refused = Nicht möglich
decoders-state-planned = Noch nicht verfügbar
decoders-state-checking = Wird gesucht…
decoders-uses = Wird mit { $decoder } von { $vendor } geöffnet.
decoders-not-found = Kostenlos. Installieren Sie die Software und klicken Sie dann auf „Erneut suchen“.
decoders-off = Ausgeschaltet: Diese Dateien werden nicht geöffnet.
decoders-version = Version { $version }.
decoders-r3d-why = Die Lizenz von RED erlaubt nur Programmen, die mit seinem SDK unter REDs eigenen Bedingungen gebaut sind, R3D-Dateien zu lesen.
    { -app-name } kann diese Bedingungen nicht annehmen, daher werden R3D-Dateien nicht geöffnet.
decoders-ecw-why = Die Lizenz von Hexagon erlaubt das ECW SDK nur Programmen, die es mitliefern, nicht einer selbst installierten Kopie.
    Daher werden ECW-Dateien nicht geöffnet. JPEG-2000-Dateien aus dem ECW SDK schon.
decoders-mrsid-why = Die Lizenz des MrSID SDK erlaubt es nur Programmen, die es mitliefern, nicht einer selbst installierten Kopie.
    Daher werden MrSID-Dateien nicht geöffnet.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = Die SDK-Lizenz von DJI erlaubt das SDK nur Programmen, die es mitliefern, nicht einer selbst installierten Kopie. Foto und Metadaten werden trotzdem geöffnet.
decoders-occt-why = STEP- und IGES-Volumenkörper brauchen OpenCASCADE, das dieser Build von { -app-name } nicht verwenden kann.
decoders-get-row = Download-Seite des Herstellers
decoders-get = Herunterladen
decoders-locate-row = Woanders installiert?
decoders-locate = Auswählen…
decoders-forget-row = Von Hand ausgewählt.
decoders-forget = Vergessen…
decoders-forget-ask = Ihre Auswahl für { $decoder } vergessen?
decoders-forget-detail = { -app-name } sucht wieder selbst danach. Auf dem Datenträger wird nichts entfernt.
decoders-forget-ok = Vergessen
decoders-locate-title = { $decoder } auswählen
decoders-locate-wrong = Das ist nicht { $decoder }.
decoders-located = Ihre Auswahl für { $decoder } wird verwendet.
decoders-forgotten = { -app-name } sucht wieder selbst nach { $decoder }.
decoders-still-missing = { $decoder } wurde noch immer nicht gefunden.
decoders-now-found = { $decoder } gefunden.
decoders-looked = Erneut nach Decodern gesucht.

decoders-card-title = { $format } braucht den kostenlosen Decoder von { $vendor }
decoders-card-what = { $file } wird mit { $decoder } geöffnet. Er ist kostenlos.
decoders-card-how = Installieren Sie ihn und kommen Sie dann zurück: { -app-name } findet ihn selbst.
decoders-card-get = Herunterladen
decoders-card-installed = Ist installiert
decoders-card-locate = Auswählen…

gallery-badge-needs-decoder = Braucht Decoder
gallery-needs-decoder = Wird mit { $decoder } von { $vendor } geöffnet (kostenlos, nicht installiert).

error-reason-needs-decoder = Dateien im Format { $format } brauchen { $decoder } von { $vendor } (kostenlos, nicht installiert).
