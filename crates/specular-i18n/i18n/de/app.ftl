# machine-drafted, needs native review
# Specular — Bildbetrachter-Oberfläche, Befehlspalette, Fehlermeldungen, Info-Fenster. Deutsch.

-app-name = Specular

## Tooltips und Bedienungshilfen-Beschriftungen der Symbolleiste

toolbar-open = Bild öffnen …
toolbar-prev = Vorheriges Bild
toolbar-next = Nächstes Bild
toolbar-back-gallery = Zurück zur Galerie
toolbar-zoom-out = Verkleinern
toolbar-zoom-in = Vergrößern
toolbar-zoom-level = Zoomstufe und Vorgaben
toolbar-zoom-fit = An Fenster anpassen
toolbar-zoom-100 = Originalgröße ({ PERCENT(100) })
toolbar-rotate-left = Gegen den Uhrzeigersinn drehen
toolbar-rotate-right = Im Uhrzeigersinn drehen
toolbar-fullscreen-enter = Vollbild aktivieren
toolbar-fullscreen-exit = Vollbild beenden
toolbar-info = Bildinformationen
toolbar-settings = Einstellungen
toolbar-delete = In den Papierkorb verschieben
toolbar-copy = Bild kopieren
toolbar-palette = Befehlspalette
toolbar-go-to = Gehe zu Bild…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Symbolleiste anpassen…
toolbar-more = Mehr
toolbar-announce-moved = { $item }: Position { $index } von { $count }
toolbar-announce-added = { $item } hinzugefügt: Position { $index } von { $count }
toolbar-announce-removed = { $item } aus der Symbolleiste entfernt
toolbar-announce-reset = Symbolleiste auf Standard zurückgesetzt
tooltip-with-shortcut = { $label } ({ $shortcut })

## Anzeigen

readout-position = { $index } / { $count }
readout-position-a11y = Bild { $index } von { $count }
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Angepasst ({ $zoom })
zoom-preset-fit = An Fenster anpassen
zoom-preset-fill = Fenster füllen
zoom-preset-actual = Originalgröße
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Vollbild
zoom-preset-percent = { $zoom }
readout-dimensions = { $width } × { $height }
status-image-count =
    { $count ->
        [one] { $count } Bild
       *[other] { $count } Bilder
    }

## Befehlspalette

palette-placeholder = Befehl oder Einstellung eingeben …
palette-no-results = Keine passenden Befehle
palette-set-setting = Festlegen: { $setting } → { $value }
palette-next = Nächstes Bild
palette-prev = Vorheriges Bild
palette-first = Erstes Bild
palette-last = Letztes Bild
palette-go-to = Gehe zu Bild…
palette-zoom-in = Vergrößern
palette-zoom-out = Verkleinern
palette-zoom-fit = Einpassen
palette-zoom-fill = Füllen
# machine-drafted, needs native review
palette-zoom-fit-width = Breite einpassen
palette-zoom-fit-height = Höhe einpassen
palette-zoom-100 = Zoom auf { PERCENT(100) }
palette-zoom-200 = Zoom auf { PERCENT(200) }
palette-zoom-300 = Zoom auf { PERCENT(300) }
palette-zoom-400 = Zoom auf { PERCENT(400) }
palette-zoom-rectangle = Auf Auswahl zoomen
palette-rotate-left = Gegen den Uhrzeigersinn drehen
palette-rotate-right = Im Uhrzeigersinn drehen
palette-flip-horizontal = Horizontal spiegeln
palette-flip-vertical = Vertikal spiegeln
palette-toggle-fullscreen = Vollbild ein/aus
palette-navigator = Navigator ein-/ausblenden
palette-loupe = Lupe
palette-pixel-grid = Pixelraster ein-/ausblenden
palette-info-panel = Informationsbereich ein-/ausblenden
palette-copy-image = Bild kopieren
# machine-drafted, needs native review
palette-read-codes = Codes lesen
# machine-drafted, needs native review
palette-find-text = Text finden
# machine-drafted, needs native review
palette-face-boxes = Gesichter anzeigen
palette-copy-path = Dateipfad kopieren
palette-move-to-trash = In den Papierkorb verschieben
palette-undo-trash = Verschieben in den Papierkorb rückgängig machen
palette-open = Bild öffnen …
# machine-drafted, needs native review
palette-open-address = Adresse öffnen …
palette-settings = Einstellungen
# machine-drafted, needs native review
palette-customise-toolbar = Symbolleiste anpassen…
palette-cheat-sheet = Tastaturkurzbefehle
palette-hud = Frame-Time-HUD ein-/ausblenden
palette-language = Sprache
palette-theme = Erscheinungsbild
# machine-drafted, needs native review
palette-interface-larger = Oberfläche größer
palette-interface-smaller = Oberfläche kleiner
palette-interface-reset = Oberfläche auf { PERCENT(100) }
palette-quit = { -app-name } beenden

## Übersicht der Tastaturkurzbefehle

cheatsheet-title = Tastaturkurzbefehle
cheatsheet-navigation = Navigation
cheatsheet-zoom-pan = Zoomen und Verschieben
cheatsheet-view = Ansicht
cheatsheet-file = Datei
cheatsheet-window = Fenster
cheatsheet-help = Hilfe
cheatsheet-hold = { $key } gedrückt halten
cheatsheet-close-hint = Zum Schließen { $key } oder Esc drücken

## Fehler

# machine-drafted, needs native review
viewer-no-preview = Keine Vorschau in dieser Datei.
error-cannot-open = { $file } kann nicht geöffnet werden
error-cannot-open-reason = { $file } kann nicht geöffnet werden: { $reason }
error-unsupported-format = { $file } hat ein Format, das { -app-name } noch nicht lesen kann.
error-file-truncated = { $file } ist unvollständig. Der lesbare Teil wird angezeigt.
error-file-too-large = { $file } ist zu groß zum Öffnen ({ $megapixels } Megapixel; die Grenze liegt bei { $limit } Megapixel).
error-not-found = { $file } existiert nicht mehr.
error-permission-denied = Sie haben keine Berechtigung, { $file } zu lesen.
error-details = Details
crash-last-time = { -app-name } wurde beim letzten Mal unerwartet beendet; ein Bericht wurde unter { $path } gespeichert
error-trash-failed = { $file } konnte nicht in den Papierkorb verschoben werden: { $reason }
# machine-drafted, needs native review
# Gründe, angezeigt nach „FILE kann nicht geöffnet werden:“ und anderen „… konnte nicht …:“-Meldungen,
# daher beginnen sie klein. Sie sagen, was passiert ist, und wo es hilft, was zu tun ist.
error-reason-not-found = sie ist nicht mehr vorhanden. Sie wurde möglicherweise verschoben, umbenannt oder gelöscht.
error-reason-permission = Sie haben keine Berechtigung, sie zu lesen. Prüfen Sie Eigentümer und Berechtigungen.
error-reason-disk-full = der Datenträger ist voll. Geben Sie Speicherplatz frei und versuchen Sie es erneut.
error-reason-read-only = der Datenträger ist schreibgeschützt.
error-reason-truncated = die Datei ist unvollständig. Möglicherweise wird sie noch kopiert oder heruntergeladen.
error-reason-is-folder = es ist ein Ordner, keine Datei.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = der Server hat den Zugriff verweigert. Die Daten sind möglicherweise privat; richten Sie für einen S3-Bucket zuerst AWS-Zugangsdaten ein.
error-reason-remote-missing = unter dieser Adresse gibt es keine Zarr-Daten.
# $status is an HTTP status code ("500").
error-reason-remote-status = der Server antwortete mit Fehler { $status }.
error-reason-offline = der Server ist nicht erreichbar. Prüfen Sie die Adresse und die Verbindung.
# machine-drafted, needs native review
error-reason-network-off = das Netzwerk ist in Einstellungen › Datenschutz ausgeschaltet.
error-reason-timed-out = das Laufwerk oder der Server hat nicht rechtzeitig geantwortet. Prüfen Sie die Verbindung und versuchen Sie es erneut.
error-reason-memory = es ist nicht genug Arbeitsspeicher frei. Schließen Sie andere Bilder oder Anwendungen und versuchen Sie es erneut.
error-reason-unsupported = sie hat ein Format, das { -app-name } noch nicht lesen kann.
# $detail sind die eigenen Worte des Decoders, technisch und auf Englisch („12-bit lossless JPEG“).
error-reason-unsupported-detail = sie verwendet eine Funktion, die { -app-name } noch nicht lesen kann ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = sie ist keine der .{ $ext }-Dateien, die { -app-name } liest ({ $formats }).
error-reason-damaged = die Datei ist beschädigt oder nicht das, was ihr Name sagt ({ $detail }).
error-reason-too-large = sie ist zu groß zum Öffnen.
# machine-drafted, needs native review
error-reason-needs-memory = sie benötigt { $needed } freien Arbeitsspeicher (RAM), aber gerade sind nur { $free } frei. Schließen Sie andere Apps und versuchen Sie es erneut, oder öffnen Sie sie auf einem Computer mit mehr Arbeitsspeicher.
error-needs-memory = Dieses Bild benötigt { $needed } freien Arbeitsspeicher (RAM), aber gerade sind nur { $free } frei. Schließen Sie andere Apps und versuchen Sie es erneut, oder öffnen Sie es auf einem Computer mit mehr Arbeitsspeicher.
error-needs-memory-short = Benötigt { $needed } freien RAM
error-reason-cancelled = das Öffnen wurde abgebrochen.
error-reason-trash-access = sie ist nicht mehr vorhanden, oder Sie haben keine Berechtigung, sie zu ändern.
error-reason-restore-collision = in ihrem Ordner gibt es bereits eine Datei mit demselben Namen.
error-reason-trash-root = ein ganzes Laufwerk kann nicht in den Papierkorb verschoben werden.
error-reason-not-in-trash = sie ist nicht mehr im Papierkorb.
error-reason-program-missing = das Programm ist nicht installiert oder wurde verschoben. Wählen Sie es erneut aus oder installieren Sie es neu.
error-reason-program-not-allowed = das Programm darf nicht ausgeführt werden. Prüfen Sie, ob es ausführbar ist.
# machine-drafted, needs native review
error-reason-permission-write = Sie haben keine Berechtigung, sie zu ändern. Prüfen Sie Eigentümer und Berechtigungen.
error-reason-file-has-errors = sie enthält Fehler. Beheben Sie diese zuerst, damit nichts darin verloren geht.
error-reason-not-dicom = sie ist keine DICOM-Datei.
error-reason-no-image = sie enthält kein Bild.
error-reason-needs-layout = es ist ein Volumen ohne Header. Zum Öffnen werden Größe, Sampletyp und Bytereihenfolge benötigt.
error-reason-not-archive = sie ist kein Archiv, das { -app-name } lesen kann.
error-reason-needs-sandbox = diese Art von Archiv wird nur vom geschützten Leser von { -app-name } geöffnet, der nicht verfügbar ist.
error-reason-encrypted = sie ist verschlüsselt und benötigt ihr Passwort.
error-reason-wrong-password = das Passwort ist falsch.
error-reason-checksum = die Datei ist beschädigt: Ihr Inhalt stimmt nicht mit der Prüfsumme überein.
error-reason-missing-entry = eine darin enthaltene Datei fehlt.
error-reason-busy = ein anderes Programm verwendet sie. Schließen Sie sie dort und versuchen Sie es erneut.
error-reason-catalogue-damaged = die Katalogdatei ist beschädigt.
error-reason-cannot-open = ihre Datei kann nicht geöffnet werden. Prüfen Sie, ob ihr Ordner existiert und Sie darin schreiben dürfen.
error-reason-newer-version = sie wurde von einer neueren Version von { -app-name } geschrieben. Aktualisieren Sie { -app-name }, um sie zu ändern.
error-reason-internal = in { -app-name } ist ein interner Fehler aufgetreten. Das Protokoll enthält die Details.
error-reason-volume-too-few = { $count ->
    [one] eine Schicht ist zu wenig, um ein Volumen aufzubauen.
   *[other] { $count } Schichten sind zu wenige, um ein Volumen aufzubauen.
}
error-reason-volume-no-geometry = eine Schicht gibt nicht an, wo sie liegt oder wohin sie zeigt.
error-reason-volume-not-parallel = die Schichten sind nicht parallel.
error-reason-volume-shared-position = mehrere Bilder teilen sich jede von { $count ->
        [one] { $count } Position
       *[other] { $count } Positionen
    } (Phasen, Echos oder eine Zeitreihe).
error-reason-volume-missing = { $count ->
        [one] etwa { $count } Schicht fehlt.
       *[other] etwa { $count } Schichten fehlen.
    }
error-reason-volume-mismatch = die Schichten unterscheiden sich in Größe oder Pixelabstand.
error-reason-volume-not-grey = die Schichten sind farbig, nicht in Graustufen.
error-reason-volume-skewed = die Achsen des Volumens stehen nicht rechtwinklig zueinander.
error-reason-volume-too-large = es benötigt { $need } MiB, erlaubt sind nur { $budget } MiB.
error-reason-url-not-specular = es ist kein { -app-name }-Link.
error-reason-url-too-long = er ist zu lang.
error-reason-url-damaged = er ist beschädigt. Möglicherweise wurde er beim Kopieren gekürzt oder verändert.
error-reason-url-unknown = er verlangt etwas, das diese Version von { -app-name } nicht kennt. Eine Aktualisierung von { -app-name } kann helfen.
error-reason-url-not-absolute = seine Dateipfade müssen vollständige Pfade sein.
error-reason-url-no-paths = er nennt keine Datei.
error-reason-url-too-many = er nennt zu viele Dateien.

## Hinweise

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Nicht abgeschlossen: { $reason }
toast-moved-to-trash = In den Papierkorb verschoben
toast-moved-to-trash-named = { $file } in den Papierkorb verschoben
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } Bild in den Papierkorb verschoben
       *[other] { $count } Bilder in den Papierkorb verschoben
    }
toast-undo = Rückgängig
toast-restored = { $file } wiederhergestellt
toast-copied-image = Bild kopiert
toast-copied-path = Pfad kopiert
toast-copied-report = Bericht kopiert
toast-copied-value = Wert kopiert
toast-loop-start = Zurück zum ersten Bild
toast-loop-end = Zum letzten Bild gesprungen
# machine-drafted, needs native review
toast-interface-size = Oberflächengröße { PERCENT($percent) }
toast-language-changed = Sprache geändert in { $language }

## Leere Zustände

empty-drop = Bild hierher ziehen oder { $shortcut } drücken
empty-open-button = Bild öffnen …
empty-drop-release = Zum Öffnen loslassen
empty-folder = Dieser Ordner enthält keine Bilder, die { -app-name } öffnen kann.
# machine-drafted, needs native review
# Unter „Bild öffnen …“ im leeren Fenster: wo alles andere zu finden ist.
empty-hint-keys = Drücken Sie { $cheatsheet } für Tastaturkurzbefehle oder { $palette }, um einen Befehl zu finden.
empty-hint-cheatsheet = Drücken Sie { $cheatsheet } für Tastaturkurzbefehle.
empty-hint-palette = Drücken Sie { $palette }, um einen Befehl zu finden.
# Unter dem Ladekreis, wenn das Öffnen einer Datei länger als einen Moment dauert.
viewer-opening = { $file } wird geöffnet …
fullscreen-exit-hint = Zum Beenden des Vollbilds Esc drücken

## Sprachauswahl

language-picker-title = Sprache
# machine-drafted, needs native review
language-picker-search = Sprachen durchsuchen
language-completeness = { $percent } übersetzt
language-system-default = Systemstandard ({ $language })
language-pseudo-accented = Pseudo-Locale: mit Akzenten und länger
language-pseudo-rtl = Pseudo-Locale: gespiegelt von rechts nach links

## Info-Fenster

about-title = Über { -app-name }
about-version = Version { $version }
about-tagline = Ein schneller, farbtreuer Bildbetrachter.
about-copyright = © { $year } Die Autoren von { -app-name }
about-license = Lizenziert unter der MIT License / Apache License 2.0.
about-geonames = Ortsnamen von GeoNames (geonames.org), lizenziert unter Creative Commons Attribution 4.0.
about-unicode = Locale-Daten aus dem Unicode CLDR über ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Objektivprofile aus der Lensfun-Datenbank (lensfun.github.io), lizenziert unter Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Lizenzen von Drittanbietern
about-copy-info = Systeminformationen kopieren
about-close = Schließen

## Bildbetrachter (Stufe 1)
# machine-drafted, needs native review
app-close = Schließen
app-truncated-badge = Unvollständige Datei
app-info-summary = Übersicht
app-info-all-fields = Alle Felder
app-info-copy-value = Wert kopieren
app-info-copy-field = Wert von { $field } kopieren
app-info-loading = Metadaten werden gelesen …
# machine-drafted, needs native review
# Alle Felder, für eine Datei, die außer ihren Pixeln keine Metadaten speichert.
app-info-no-fields = Diese Datei hat keine Metadatenfelder.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR-Gain-Map
app-info-gain-map-headroom = Bis zu { $headroom }× SDR-Helligkeit
app-copy-failed = Kopieren nicht möglich: { $reason }
app-restore-failed = { $file } konnte nicht wiederhergestellt werden: { $reason }
app-open-dialog-title = Bild öffnen
app-open-dialog-filter = Unterstützte Dateien
app-open-dialog-all = Alle Dateien
app-open-group-images = Bilder
app-open-group-raw = Kamera-Rohdaten
app-open-group-video = Video
app-open-group-sound = Ton
app-open-group-documents = Dokumente
app-open-group-drawings = Zeichnungen und CAD
app-open-group-models = 3D-Modelle
app-open-group-maps = Karten und Tracks
app-open-group-science = Wissenschaft und Medizin
app-open-group-fonts = Schriften
app-open-group-other = Sonstige

## Rendererinformationen (Über, Einstellungen › Erweitert)

# machine-drafted, needs native review
renderer-title = Rendererinformationen
renderer-section-adapter = Grafikadapter
renderer-section-limits = Grenzen und Funktionen
renderer-section-display = Anzeige
renderer-section-memory = Speicher
renderer-section-decoders = Optionale Decoder
renderer-unknown = Unbekannt
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = Hersteller- und Geräte-ID
# Wayland, X11, Windows or macOS.
renderer-display-server = Anzeigeserver
renderer-features = Verwendete Funktionen
renderer-surface-format = Fensterformat
renderer-surface-formats = Angebotene Formate
renderer-present-mode = Präsentationsmodus
renderer-present-modes = Angebotene Modi
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = An ({ $headroom }× Spielraum)
renderer-hdr-off = Aus (die Anzeige kann HDR darstellen)
renderer-hdr-unavailable = Auf dieser Anzeige nicht verfügbar
renderer-decode-budget = Dekodieren darf nutzen
renderer-decode-budget-detail = { PERCENT(80) } des freien Speichers, geteilt von allen Bildern, die gerade geöffnet werden.
renderer-display-cache = Anzeige-Cache

## Leistungs-HUD (F12)
# machine-drafted, needs native review
app-hud-scale = Skalierung
app-hud-level = Stufe
app-hud-level-thumbnail = Eingebettete Vorschau
app-hud-level-full = Volle Auflösung
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Öffnen
app-hud-decode = Dekodieren
app-hud-upload = Hochladen
app-hud-frame = Frame
app-hud-threads = Threads
app-hud-threads-value = { $threads } ({ $queued } wartend)
app-hud-tiles = Kacheln
app-hud-tiles-value = { $tiles } von { $capacity }
app-hud-ram = Arbeitsspeicher
app-hud-vram = GPU-Speicher
app-hud-gpu = GPU

# Trash confirmation (machine-drafted, needs native review)
app-confirm-trash-title = { $file } in den Papierkorb verschieben?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Die Begleitdateien kommen mit in den Papierkorb: { $files }
app-cancel = Abbrechen

# Stage 1 merge (machine-drafted, needs native review)
disabled-no-image = Kein Bild geöffnet
disabled-still-loading = Das Bild wird noch geladen
disabled-decode-failed = Dieses Bild konnte nicht decodiert werden
disabled-no-other-images = Keine weiteren Bilder in diesem Ordner
tooltip-unavailable = { $label } (nicht verfügbar: { $reason })
error-decoder-stopped = der Decoder wurde beendet, ohne ein Bild zu liefern
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = dafür wird eine Gehirnoberfläche (eine .surf.gii-Datei mit { $vertices } Eckpunkten) im selben Ordner benötigt
fullscreen-back-hint = Esc drücken, um den Zoom zurückzusetzen oder den Vollbildmodus zu beenden
action-back = Bereich schließen (in einem Dialog zuerst einen Schritt zurück), Zoom zurücksetzen, Vollbild beenden; in der Galerie eine Ordnerebene höher
about-third-party-file = { -app-name } enthält Software und Daten Dritter unter deren eigenen Lizenzen; siehe THIRD-PARTY.md, das mit { -app-name } installiert wird.
toast-low-memory = Dieses Bild benötigt möglicherweise etwa { $need } Arbeitsspeicher, verfügbar sind aber nur { $available }. Es lädt eventuell langsam oder gar nicht.

hw-report-title = Hardwarebericht
hw-report-copy = Bericht kopieren
hw-report-version = Version
hw-report-cpu = CPU
hw-report-cores = Kerne
hw-report-cores-value = { $logical } logisch, { $physical } physisch
hw-report-performance-cores = Leistungskerne
hw-report-cpu-features = CPU-Beschleunigung
hw-report-none = Keine
hw-report-memory = Arbeitsspeicher
hw-report-memory-value = { $total } gesamt, { $available } verfügbar
hw-report-graphics-api = Grafik-API
hw-report-driver = Treiber
hw-report-max-texture = Größte Textur
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Dekodier-Threads
hw-report-ram-cache = Bildspeicher-Cache
hw-report-vram-cache = GPU-Speicher-Cache
hw-report-processing = Verarbeitung
hw-report-path-jpeg-decode = JPEG-Dekodierung
hw-report-path-ycbcr-to-rgb = Farbumrechnung
hw-report-path-png-unfilter = PNG-Dekodierung
hw-report-path-heic-decode = HEIC-Dekodierung
hw-report-path-inference = Bildanalyse
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, parallel
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = Hardware-Decoder
hw-report-engine-npu = NPU
# machine-drafted, needs native review
hw-report-build-debug = Debug-Build
hw-report-build-release = Release-Build

## Accessibility (screen readers)
# The image as a screen reader announces it: file name and size in pixels.
a11y-image = { $name }, { $width } × { $height } Pixel
# The region where short notifications (toasts) appear.
a11y-notifications = Benachrichtigungen
# machine-drafted, needs native review
a11y-transport-playback = Wiedergabe
a11y-transport-pages = Seiten
a11y-settings-sections = Bereiche

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Codes in diesem Bild
codes-looking = Codes werden gesucht …
codes-none = Keine Codes in diesem Bild gefunden.
codes-failed = Das Bild konnte nicht gelesen werden: { $reason }
codes-copy = „{ $text }“ kopieren
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Link auf { $host } öffnen …
codes-network-off = Kein Netzwerk ist eingeschaltet, daher öffnen sich keine Links.
codes-open-title = Einen Link zu { $host } öffnen?
codes-open-detail = Er öffnet sich in Ihrem Webbrowser. Öffnen Sie nur Links, denen Sie vertrauen.
codes-open = Im Browser öffnen
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Text in diesem Bild
# machine-drafted, needs native review
ocr-looking = Text wird gelesen …
# machine-drafted, needs native review
ocr-none = Kein Text in diesem Bild gefunden.
# machine-drafted, needs native review
ocr-failed = Der Text konnte nicht gelesen werden: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = „Text finden“ benötigt die Textmodelle des lokalen KI-Pakets (Einstellungen › Lokale KI).
# machine-drafted, needs native review
ocr-loading = Textmodelle werden geladen …
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = „{ $text }“ kopieren
# machine-drafted, needs native review
ocr-copy-all = Gesamten Text kopieren
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = { $percent } sicher

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = System
hw-report-section-gpu = GPU
hw-report-section-decoding = Dekodierung
hw-report-operating-system = Betriebssystem
hw-report-adapter = Adapter
hw-report-gpu-type = Typ
hw-report-gpu-init = Startzeit
hw-report-max-layers = Texturebenen
hw-report-compression = Texturkomprimierung
hw-report-unified-memory = Speicher mit der CPU geteilt
hw-report-display = Bildschirm
hw-report-hw-decode = Hardware-Dekodierung
hw-report-core-cluster = Kerngruppe
hw-report-power = Stromversorgung
hw-report-calibration = Geschwindigkeitsmessungen
hw-report-codec-others = Andere
hw-report-performance-cores-value = { $count } von { $total }
hw-report-cluster-value = { $name }: { $physical } physisch, { $logical } logisch
hw-report-thermal-nominal = Normale Temperatur
hw-report-thermal-fair = Warm
hw-report-thermal-serious = Heiß, wird gedrosselt
hw-report-thermal-critical = Überhitzt
hw-report-power-battery = Akkubetrieb ({ $percent })
hw-report-power-battery-unknown = Akkubetrieb
hw-report-power-mains = Am Netz
hw-report-power-mains-percent = Am Netz ({ $percent })
hw-report-power-saving = Energiesparmodus an
hw-report-gpu-discrete = Dediziert
hw-report-gpu-integrated = Integriert
hw-report-gpu-virtual = Virtuell
hw-report-gpu-software = Software, auf der CPU
hw-report-gpu-other = Andere
hw-report-gpu-unavailable = Nicht verfügbar
hw-report-ms = { $ms } ms
hw-report-yes = Ja
hw-report-no = Nein
hw-report-unified-platform-differs = Das System meldet etwas anderes
hw-report-display-value = { $fps } Hz, HDR-Spielraum jetzt { $now }, bis zu { $max }
hw-report-hw-decode-value = { $api } auf { $device }
hw-report-hw-decode-checking = Wird noch geprüft…
hw-report-codec-up-to = Bis { $width } × { $height }
hw-report-codec-none = Nicht verfügbar
hw-report-codec-h264-encode = H.264-Kodierung
hw-report-supported = unterstützt: { $list }
hw-report-unsupported = nicht unterstützt: { $list }
hw-report-calibration-done = Auf diesem Computer gemessen
hw-report-calibration-pending = Noch nicht gemessen
hw-report-paths-power-saving = Energiesparen: Wege mit geringerem Energieverbrauch werden bevorzugt
hw-report-paths-caption = Wege für ein Bild mit { $mpx } Megapixeln
hw-report-col-task = Aufgabe
hw-report-col-path = Weg
hw-report-col-time = Zeit
hw-report-col-cpu = CPU-Zeit (alle Threads)
hw-report-col-runs = Messungen
hw-report-col-use = Verwendet für
hw-report-runs-untested = Nicht getestet
hw-report-use-viewing = Anzeige ({ $reason })
hw-report-use-background = Hintergrund ({ $reason })
hw-report-reason-only = einzige Möglichkeit
hw-report-reason-faster = am schnellsten
hw-report-reason-cheaper = wenigste CPU-Zeit
hw-report-reason-lower-power = geringster Energieverbrauch
hw-report-reason-explore = wird ausprobiert
hw-report-reason-forced = in den Einstellungen festgelegt
hw-report-reason-prefer-gpu = GPU bevorzugen
hw-report-reason-cpu-only = Nur CPU
hw-report-reason-calibrate = wird gemessen
hw-report-reason-scaled = verkleinert
hw-report-trust-unverified = Noch nicht geprüft
hw-report-trust-blocked = Nach einem Fehler abgeschaltet
hw-report-trust-absent = Auf diesem Computer nicht vorhanden

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galerie
cheatsheet-mode-compare = Vergleichen
cheatsheet-gallery-browse = Durchsuchen
cheatsheet-gallery-select = Auswählen
cheatsheet-gallery-mark = Bewerten und markieren
cheatsheet-gallery-move = Zwischen Bildern wechseln
cheatsheet-gallery-page = Seite nach oben oder unten
cheatsheet-gallery-ends = Erstes oder letztes Bild
cheatsheet-gallery-up = Hoch zum übergeordneten Ordner
cheatsheet-gallery-history-back = Zurück zum vorherigen Ordner
cheatsheet-gallery-history-forward = Vor zum nächsten Ordner
cheatsheet-gallery-back = Obere Ansicht schließen, dann zurück
cheatsheet-gallery-viewer = Zurück zum Betrachter
cheatsheet-gallery-album-add = Zum letzten Album hinzufügen
cheatsheet-gallery-select-one = Auswählen oder abwählen
cheatsheet-gallery-quick-look = Schnellansicht
cheatsheet-gallery-filter = Filtern
cheatsheet-gallery-bigger = Größere Miniaturen
cheatsheet-gallery-smaller = Kleinere Miniaturen
cheatsheet-gallery-default-size = Standardgröße der Miniaturen
cheatsheet-gallery-refresh = Ordner aktualisieren
cheatsheet-gallery-menu = Kontextmenü
cheatsheet-compare-views = Vergleichen
cheatsheet-compare-overlay = Überlagerung oder nebeneinander
cheatsheet-compare-difference = Differenz (erneut: nächste Art)
cheatsheet-compare-step-down = Weniger Überblenden oder Differenzverstärkung
cheatsheet-compare-step-up = Mehr Überblenden oder Differenzverstärkung
cheatsheet-compare-pick = Bild, auf das die Tasten wirken
cheatsheet-compare-close = Vergleich schließen
cheatsheet-channels = Kanäle
cheatsheet-medical-window = Fensterung
cheatsheet-medical-measure = Messen und beschriften
cheatsheet-medical-frames = Bilder und Serien
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D und Rekonstruktion
cheatsheet-medical-analysis = Segmentierung, Fusion und PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Studien und Weitergabe
cheatsheet-palette-only-title = In der Befehlspalette ({ $key })
cheatsheet-palette-only-title-nokey = In der Befehlspalette
cheatsheet-palette-only-tag = Palette
cheatsheet-palette-only-line = { $action }: keine Taste; in der Befehlspalette ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: keine Taste; in der Befehlspalette
# Esc in der Tastaturübersicht (die lange Beschreibung aus der Palette passt nicht).
cheatsheet-back = Schließen oder zurück

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Videoexport
hw-report-engine-hw-encoder = Hardware-Encoder

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (je Miniatur mit { $size })
hw-report-task-frame = { $task } (je Einzelbild mit { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Nur Software

about-natural-earth = Kartendaten von Natural Earth (naturalearthdata.com), gemeinfrei.
app-info-show-on-map = Auf Karte zeigen
