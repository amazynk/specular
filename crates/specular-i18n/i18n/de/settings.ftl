# machine-drafted, needs native review
# Specular — Einstellungsfenster. Deutsch.

## Fensterelemente

settings-title = Einstellungen
settings-search-placeholder = Einstellungen durchsuchen
settings-search-no-results = Keine Einstellungen gefunden für { $query }
settings-reset = Zurücksetzen
settings-reset-section = Abschnitt zurücksetzen
settings-modified = Geändert
settings-default-value = Standard: { $value }
settings-restart-required = Wird nach einem Neustart wirksam
settings-restart-now = Jetzt neu starten
settings-undo = Rückgängig
settings-redo = Wiederholen
settings-value-on = Ein
settings-value-off = Aus
settings-value-auto = Automatisch
settings-value-system = Systemeinstellung verwenden
settings-value-system-language = Systemstandard
settings-value-follow-language = Wie Sprache der Benutzeroberfläche
settings-value-default-location = Standardort
settings-choose = Auswählen …
settings-clear = Leeren
settings-file-reloaded = settings.toml wurde außerhalb von { -app-name } geändert und neu geladen.
settings-warning-unknown-key = Unbekannte Einstellung { $key } in settings.toml. Sie bleibt in der Datei erhalten, wird aber ignoriert.
settings-warning-invalid-value = Ungültiger Wert für { $key }: { $reason } Stattdessen wird der Standardwert verwendet.
settings-warning-newer-version = settings.toml wurde von einer neueren Version von { -app-name } geschrieben. Einstellungen, die diese Version nicht kennt, bleiben erhalten.
settings-warning-syntax = settings.toml kann nicht gelesen werden (Zeile { $line }): { $reason } Bis der Fehler behoben ist, werden die Standardwerte verwendet.

## Validierungsfehler (werden auch von `specular --set` ausgegeben)

settings-error-unknown-key = Es gibt keine Einstellung namens { $key }.
settings-error-unknown-key-suggestion = Es gibt keine Einstellung namens { $key }. Meinten Sie { $suggestion }?
settings-error-not-bool = Erwartet wird on oder off (true oder false).
settings-error-not-integer = Erwartet wird eine ganze Zahl.
settings-error-not-number = Erwartet wird eine Zahl.
settings-error-out-of-range = Muss zwischen { $min } und { $max } liegen.
settings-error-out-of-range-auto = Muss 0 (automatisch) sein oder zwischen { $min } und { $max } liegen.
settings-error-unknown-option = Erwartet wird einer der folgenden Werte: { $options }.
settings-error-bad-colour = Erwartet wird eine Farbe wie #1E1E1E oder #1E1E1E80.
settings-error-bad-language = Erwartet wird ein Sprachcode wie de, pt-BR oder zh-Hans oder “system”.
settings-error-bad-path = Erwartet wird ein Datei- oder Ordnerpfad.
settings-error-not-settable = Dieser Eintrag ist ein Befehl und hat keinen Wert.
settings-error-syntax = Erwartet wird Schlüssel=Wert, zum Beispiel viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Darf keine Tabulatoren, Zeilenumbrüche oder anderen Steuerzeichen enthalten.
settings-error-invalid = { $reason }
settings-error-tile-url = Benötigt eine http- oder https-Adresse mit {"{"}z{"}"}, {"{"}x{"}"} und {"{"}y{"}"}.

## Einheiten

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } Bild
       *[other] { $value } Bilder
    }
unit-threads =
    { $value ->
        [one] { $value } Thread
       *[other] { $value } Threads
    }
unit-bytes =
    { $value ->
        [one] { $value } Byte
       *[other] { $value } Byte
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Abschnitte

settings-section-general = Allgemein
settings-section-viewing = Anzeige
settings-section-browsing = Blättern
settings-section-colour = Farbe
settings-section-metadata = Metadaten und Datenschutz
# machine-drafted, needs native review
settings-section-privacy = Datenschutz
settings-section-performance = Leistung
settings-section-accessibility = Bedienungshilfen
settings-section-keys = Tasten
settings-section-advanced = Erweitert

## Allgemein

settings-general-language = Sprache der Benutzeroberfläche
settings-general-language-help = Änderungen werden sofort wirksam. „Systemstandard“ verwendet die Sprache Ihres Betriebssystems.
settings-general-region-formats = Regionale Formate
settings-general-region-formats-help = Wie Datum, Uhrzeit, Zahlen und Größen geschrieben werden. Standardmäßig richten sie sich nach der Sprache der Benutzeroberfläche.
settings-general-theme = Erscheinungsbild
settings-general-theme-help = Helles oder dunkles Erscheinungsbild für Fenster und Bedienelemente. Bilder werden nie verändert.
settings-general-theme-system = Systemeinstellung verwenden
settings-general-theme-light = Hell
settings-general-theme-dark = Dunkel
# machine-drafted, needs native review
settings-general-on-start = Wenn { -app-name } ohne Datei öffnet
settings-general-on-start-help = Was angezeigt wird, wenn { -app-name } allein geöffnet wird. Eine Datei, die aus dem Dateimanager oder der Befehlszeile geöffnet wird, erscheint immer im Betrachter.
settings-general-on-start-start-screen = Der Startbildschirm
settings-general-on-start-gallery = Galerie des letzten Ordners
settings-general-on-start-library = Bibliothek
settings-general-start-tips = „Zum Ausprobieren“ auf dem Startbildschirm anzeigen
settings-general-start-tips-help = Eine Reihe Karten mit nützlichen Funktionen und ihren Tasten. „Willkommen anzeigen“ in der Befehlspalette holt sie zurück.
settings-general-tips = Tipps anzeigen
settings-general-tips-help = Ein kurzer Hinweis, einmalig, wenn eine Funktion zum ersten Mal helfen könnte, etwa die Taste, die einen Ordner als Galerie zeigt. Nie während einer Diashow.
settings-general-tips-shown = Bereits gezeigte Tipps
settings-general-tips-shown-help = Die bereits gegebenen einmaligen Hinweise, damit sie nicht erneut erscheinen.
# machine-drafted, needs native review
settings-general-your-name = Ihr Name
settings-general-your-name-help = Erscheint auf Stempeln und Markierungen, die Sie hinzufügen. Nur in den Markierungsdateien gespeichert, die Sie anlegen.
settings-general-single-instance = Dateien im vorhandenen Fenster öffnen
settings-general-single-instance-help = Wenn { -app-name } bereits läuft, wird ein weiteres Bild in diesem Fenster angezeigt, statt eine neue Instanz zu starten.
settings-general-file-associations = Dateizuordnungen …
settings-general-file-associations-help = { -app-name } als Standardbetrachter für die unterstützten Bildformate festlegen.

## Anzeige

settings-viewing-initial-zoom = Anfänglicher Zoom
settings-viewing-initial-zoom-help = Wie groß ein neu geöffnetes Bild angezeigt wird.
settings-viewing-initial-zoom-fit = An Fenster anpassen
settings-viewing-initial-zoom-fit-if-larger = Große Bilder anpassen, kleine mit { PERCENT(100) } anzeigen
settings-viewing-initial-zoom-actual-size = Originalgröße ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Fenster füllen
settings-viewing-wheel-behaviour = Mausrad
settings-viewing-wheel-behaviour-help = Was das Drehen des Mausrads bewirkt. Halten Sie Strg gedrückt für die jeweils andere Aktion.
settings-viewing-wheel-behaviour-help-macos = Was das Drehen des Mausrads bewirkt. Halten Sie ⌘ gedrückt für die jeweils andere Aktion.
settings-viewing-wheel-behaviour-zoom = Zoomen
settings-viewing-wheel-behaviour-navigate = Nächstes und vorheriges Bild
settings-viewing-keep-zoom = Zoom zwischen Bildern beibehalten
settings-viewing-keep-zoom-help = Eingezoomt öffnen sich das nächste und vorherige Bild mit demselben Zoom und zeigen denselben Bildausschnitt: praktisch, um die Schärfe einer Serie zu prüfen.
settings-viewing-zoom-animation = Zoomen animieren
settings-viewing-zoom-animation-help = Fließend zoomen statt zu springen. Wird automatisch deaktiviert, wenn „Bewegung reduzieren“ aktiviert ist.
settings-viewing-zoom-animation-speed = Geschwindigkeit der Zoomanimation
settings-viewing-zoom-animation-speed-help = Wie schnell animierte Zoomvorgänge zur Ruhe kommen.
settings-viewing-zoom-animation-speed-relaxed = Gemächlich
settings-viewing-zoom-animation-speed-normal = Normal
settings-viewing-zoom-animation-speed-fast = Schnell
settings-viewing-background = Hintergrund
settings-viewing-background-help = Die Farbe hinter dem Bild im Fenster.
settings-viewing-background-theme = Wie Erscheinungsbild
settings-viewing-background-black = Schwarz
settings-viewing-background-dark-grey = Dunkelgrau
settings-viewing-background-light-grey = Hellgrau
settings-viewing-background-white = Weiß
settings-viewing-background-custom = Eigene Farbe
settings-viewing-background-color = Eigene Hintergrundfarbe
settings-viewing-background-color-help = Wird verwendet, wenn „Hintergrund“ auf „Eigene Farbe“ gesetzt ist.
settings-viewing-fullscreen-background = Vollbild-Hintergrund
settings-viewing-fullscreen-background-help = Die Farbe hinter dem Bild im Vollbild.
settings-viewing-transparency = Transparente Bereiche
settings-viewing-transparency-help = Wie transparente Teile eines Bildes dargestellt werden.
settings-viewing-transparency-checkerboard = Schachbrettmuster
settings-viewing-transparency-background = Hintergrundfarbe
settings-viewing-transparency-black = Schwarz
settings-viewing-transparency-white = Weiß
settings-viewing-pixel-grid-threshold = Pixelraster anzeigen ab
settings-viewing-pixel-grid-threshold-help = Ab dieser Zoomstufe ein Raster zwischen den Pixeln zeichnen.
settings-viewing-smoothing-high-zoom = Vergrößerte Pixel
settings-viewing-smoothing-high-zoom-help = Wie Pixel aussehen, wenn das Bild größer als { PERCENT(100) } angezeigt wird.
settings-viewing-smoothing-high-zoom-auto = Geglättet, ab { PERCENT(400) } scharf
settings-viewing-smoothing-high-zoom-smooth = Immer geglättet
settings-viewing-smoothing-high-zoom-pixelated = Immer scharfe Quadrate
settings-viewing-double-click = Doppelklick auf das Bild
settings-viewing-double-click-help = Ein Doppelklick auf den leeren Hintergrund schaltet immer das Vollbild ein oder aus.
settings-viewing-double-click-toggle-zoom = Zwischen Einpassen und { PERCENT(100) } wechseln
settings-viewing-double-click-fullscreen = Vollbild ein/aus
settings-viewing-double-click-nothing = Nichts tun
settings-viewing-fullscreen-on-open = Bilder im Vollbild öffnen
settings-viewing-fullscreen-on-open-help = Im Vollbild starten, wenn ein Bild von außerhalb von { -app-name } geöffnet wird.
settings-viewing-fullscreen-mode = Vollbildmodus
settings-viewing-fullscreen-mode-help = Das native Vollbild verwendet einen eigenen Space; das sofortige Vollbild überdeckt den aktuellen ohne Animation.
settings-viewing-fullscreen-mode-native = Nativ (eigener Space)
settings-viewing-fullscreen-mode-borderless = Sofort
settings-viewing-toolbar-autohide = Symbolleiste automatisch ausblenden
settings-viewing-toolbar-autohide-help = Symbolleiste und Randpfeile werden ausgeblendet, wenn der Zeiger ruht, und erscheinen wieder, sobald er bewegt wird.
settings-viewing-toolbar-autohide-delay = Symbolleiste ausblenden nach
settings-viewing-toolbar-autohide-delay-help = Wie lange der Zeiger ruhen muss, bevor die Symbolleiste ausgeblendet wird.
settings-viewing-toolbar-items = Schaltflächen der Symbolleiste
settings-viewing-toolbar-items-help = Welche Schaltflächen die Symbolleiste in welcher Reihenfolge anzeigt.
settings-viewing-toolbar-drag = Symbolleiste durch Ziehen anordnen
settings-viewing-toolbar-drag-help = Ziehen Sie eine Schaltfläche entlang der Symbolleiste, um sie zu verschieben. Ein kurzer Klick löst sie weiterhin aus; Esc setzt eine gezogene Schaltfläche zurück.
settings-viewing-toolbar-items-open = Öffnen
settings-viewing-toolbar-items-prev = Vorheriges Bild
settings-viewing-toolbar-items-next = Nächstes Bild
settings-viewing-toolbar-items-zoom-out = Verkleinern
settings-viewing-toolbar-items-zoom-level = Zoomstufe
settings-viewing-toolbar-items-zoom-in = Vergrößern
settings-viewing-toolbar-items-fit = An Fenster anpassen
settings-viewing-toolbar-items-rotate-left = Gegen den Uhrzeigersinn drehen
settings-viewing-toolbar-items-rotate-right = Im Uhrzeigersinn drehen
settings-viewing-toolbar-items-info = Informationen
settings-viewing-toolbar-items-copy = Bild kopieren
settings-viewing-toolbar-items-delete = In den Papierkorb verschieben
settings-viewing-toolbar-items-palette = Befehlspalette
settings-viewing-toolbar-items-settings = Einstellungen
settings-viewing-toolbar-items-fullscreen = Vollbild
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Flexibler Abstand
settings-viewing-toolbar-items-separator = Trennlinie
settings-toolbar-preview = Vorschau der Symbolleiste
settings-toolbar-hidden = Ausgeblendete Schaltflächen
settings-toolbar-add-spacer = Flexiblen Abstand hinzufügen
settings-toolbar-add-separator = Trennlinie hinzufügen
settings-toolbar-reset = Auf Standard zurücksetzen
settings-toolbar-remove = { $item } entfernen
settings-toolbar-hint = Ziehen Sie eine Zeile, um sie zu verschieben, oder wählen Sie sie aus und drücken Sie Alt+↑ oder Alt+↓. Tab wechselt zwischen Suchfeld und Liste.
settings-toolbar-move-up = Nach oben
settings-toolbar-move-down = Nach unten
settings-toolbar-move-start = An den Anfang
settings-toolbar-move-end = Ans Ende
settings-toolbar-menu-hide = Ausblenden
settings-toolbar-menu-show = Einblenden
settings-toolbar-menu-remove = Entfernen
settings-toolbar-row-menu = Weitere Aktionen für { $item }
settings-toolbar-position = Position { $index } von { $count }
settings-toolbar-hidden-state = Ausgeblendet

## Blättern

settings-browsing-sort-order = Bilder sortieren nach
settings-browsing-sort-order-help = Die Reihenfolge beim Blättern durch einen Ordner. Die Sortierung nach Name stellt „img2“ vor „img10“ und folgt dem Alphabet Ihrer Sprache.
settings-browsing-sort-order-natural = Name
settings-browsing-sort-order-date-taken = Aufnahmedatum
settings-browsing-sort-order-modified = Änderungsdatum
settings-browsing-sort-order-size = Dateigröße
settings-browsing-sort-order-type = Dateityp
settings-browsing-sort-descending = Umgekehrte Reihenfolge
settings-browsing-sort-descending-help = Vom letzten zum ersten sortieren.
settings-browsing-loop-folder = Am Ende des Ordners von vorn beginnen
settings-browsing-loop-folder-help = Nach dem letzten Bild mit dem ersten fortfahren und umgekehrt.
settings-browsing-include-subfolders = Unterordner einbeziehen
settings-browsing-include-subfolders-help = Auch durch Bilder in Unterordnern blättern.
settings-browsing-hold-acceleration = Beschleunigung beim Gedrückthalten von Weiter oder Zurück
settings-browsing-hold-acceleration-help = Wie stark das Blättern beschleunigt, während eine Taste oder Schaltfläche gedrückt gehalten wird.
settings-browsing-hold-acceleration-off = Keine Beschleunigung
settings-browsing-hold-acceleration-gentle = Sanft
settings-browsing-hold-acceleration-normal = Normal
settings-browsing-hold-acceleration-fast = Schnell
settings-browsing-confirm-trash = Vor dem Verschieben in den Papierkorb nachfragen
settings-browsing-confirm-trash-help = Das Verschieben in den Papierkorb lässt sich immer rückgängig machen, daher fragt { -app-name } standardmäßig nicht nach.
settings-browsing-arrows-follow-reading-direction = Pfeiltasten folgen der Leserichtung
settings-browsing-arrows-follow-reading-direction-help = In Sprachen mit Schreibrichtung von rechts nach links führt die linke Pfeiltaste zum nächsten Bild.

## Farbe

settings-colour-management = Farbmanagement
settings-colour-management-help = Farben so anzeigen, wie sie gedacht sind, anhand des Farbprofils des Bildes und des Profils Ihres Bildschirms.
settings-colour-display-profile = Bildschirmprofil
settings-colour-display-profile-help = Das Farbprofil Ihres Bildschirms. „Automatisch“ verwendet das vom Betriebssystem gemeldete Profil.
settings-colour-rendering-intent = Rendering-Priorität
settings-colour-rendering-intent-help = Wie Farben, die der Bildschirm nicht darstellen kann, in seinen Farbumfang gebracht werden.
settings-colour-rendering-intent-perceptual = Perzeptiv
settings-colour-rendering-intent-relative-colorimetric = Relativ farbmetrisch
settings-colour-rendering-intent-saturation = Sättigung
settings-colour-rendering-intent-absolute-colorimetric = Absolut farbmetrisch
settings-colour-black-point-compensation = Tiefenkompensierung
settings-colour-black-point-compensation-help = Details in den Schatten erhalten, wenn das Schwarz des Bildschirms heller ist als das des Bildes.
settings-colour-untagged-images = Bilder ohne Farbprofil
settings-colour-untagged-images-help = Wie Bilder angezeigt werden, die keine Angabe zu ihrem Farbraum enthalten.
settings-colour-untagged-images-srgb = Als sRGB behandeln
settings-colour-untagged-images-display = Unverändert anzeigen
settings-colour-dithering = Dithering
settings-colour-dithering-help = Beim Verringern der Farbtiefe feines Blaurauschen hinzufügen, um Streifenbildung in Himmeln und Verläufen zu verhindern.
settings-colour-hdr-output = HDR-Ausgabe
settings-colour-hdr-output-help = HDR-Bilder auf geeigneten Bildschirmen mit voller Helligkeit zeigen. Automatisch nutzt HDR, wenn der Bildschirm zusätzliche Helligkeit meldet; sonst werden helle Bereiche sanft angepasst.
settings-colour-hdr-output-auto = Automatisch
settings-colour-hdr-output-on = Ein
settings-colour-hdr-output-off = Aus
# machine-drafted, needs native review
settings-colour-ocio-config = OCIO-Konfiguration
settings-colour-ocio-config-help = Eine OpenColorIO-Konfiguration (.ocio) für Bilder in EXR, Radiance HDR und PFM. Leer verwendet die in $OCIO angegebene.
settings-colour-ocio-display = OCIO-Anzeige
settings-colour-ocio-display-help = Die Anzeige der OCIO-Konfiguration, die Ihrem Bildschirm entspricht.
settings-colour-ocio-display-default = Erste der Konfiguration
settings-colour-ocio-view = OCIO-Ansicht
settings-colour-ocio-view-help = Wie szenenlineare Bilder auf dieser Anzeige dargestellt werden, z. B. mit einer Tone-Mapping-Ansicht.
settings-colour-ocio-view-default = Erste der Anzeige
settings-colour-ocio-input = OCIO-Eingangsfarbraum
settings-colour-ocio-input-help = Der Farbraum der Bildwerte.
settings-colour-ocio-input-default = Rolle „scene-linear“ der Konfiguration
# machine-drafted, needs native review
settings-colour-proof-profile = Softproof-Profil
settings-colour-proof-profile-help = Das Druckerprofil, das der Softproof in Entwickeln zuletzt nutzte.
settings-colour-proof-intent = Softproof-Rendering-Intent
settings-colour-proof-intent-help = Wie Farben, die der Drucker nicht drucken kann, angepasst werden.
settings-colour-proof-intent-perceptual = Perzeptiv
settings-colour-proof-intent-relative = Relativ farbmetrisch
settings-colour-proof-bpc = Softproof-Tiefenkompensierung
settings-colour-proof-bpc-help = Schwarz auf das Schwarz des Druckers abbilden.
settings-colour-proof-paper = Softproof-Papierweiß
settings-colour-proof-paper-help = Die eigene Farbe des Papiers zeigen.
settings-colour-proof-ink = Softproof-Tintenschwarz
settings-colour-proof-ink-help = Zeigen, wie dunkel das Schwarz der Tinte ist.

## Metadaten und Datenschutz

settings-metadata-show-info-on-open = Informationsbereich beim Öffnen anzeigen
settings-metadata-show-info-on-open-help = Den Bereich mit Kamera-, Datums- und Ortsangaben bei jedem geöffneten Bild einblenden.
settings-metadata-info-detail = Informationsbereich zeigt
settings-metadata-info-detail-help = Die Zusammenfassung erklärt die wichtigsten Angaben in verständlicher Sprache; „Alle Felder“ listet alles auf, was in der Datei gespeichert ist.
settings-metadata-info-detail-summary = Zusammenfassung
settings-metadata-info-detail-all = Alle Felder
settings-metadata-reverse-geocoding = Ortsnamen für GPS-Positionen anzeigen
settings-metadata-reverse-geocoding-help = Den nächstgelegenen Ort anhand integrierter Daten ermitteln. Dies funktioniert offline: Positionen verlassen nie Ihren Computer.
settings-metadata-hide-gps = GPS-Positionen auf dem Bildschirm ausblenden
settings-metadata-hide-gps-help = Keine Koordinaten oder Orte anzeigen, zum Beispiel während Sie Ihren Bildschirm teilen. Dateien werden nicht verändert.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Entwicklungseinstellungen neben dem Foto speichern
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Schreibt IMG_1234.CR2.specular.xmp neben die Datei, damit die Bearbeitungen mitwandern. Aus: Bearbeitungen bleiben nur in der Bibliothek.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Lightroom-Einstellungen in Entwickeln übernehmen
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Ein Foto mit Lightroom-Einstellungen und ohne Bearbeitung öffnet ungefähr mit ihnen. Aus: Entwickeln bietet eine Schaltfläche.
settings-metadata-units = Einheiten
settings-metadata-units-help = Einheiten für Höhe, Entfernungen und Geschwindigkeiten.
settings-metadata-units-auto = Wie regionale Formate
settings-metadata-units-metric = Metrisch
settings-metadata-units-imperial = Angloamerikanisch
# machine-drafted, needs native review
settings-metadata-basemap = Grundkarte
settings-metadata-basemap-help = Die Karte unter den Fotoorten. Online-Karten laden Kacheln für das angezeigte Gebiet.
settings-metadata-basemap-off = Nur integrierte Karte (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Anderer Kachelserver (online)
settings-metadata-basemap-file = Eigene Kartendatei
settings-metadata-basemap-url = Adresse des Kachelservers
settings-metadata-basemap-url-help = Eine Adresse mit {"{"}z{"}"}, {"{"}x{"}"} und {"{"}y{"}"}, zum Beispiel https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Kartendatei
settings-metadata-basemap-path-help = Eine MBTiles- oder PMTiles-Datei. Sie funktioniert offline.
settings-metadata-basemap-cache = Cache für Kartenkacheln
settings-metadata-basemap-cache-help = Speicherplatz für geladene Kacheln. Die ältesten werden zuerst entfernt.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Erlaubte Kachelserver
settings-metadata-basemap-allowed-help = Kartenserver, von denen Sie dem Laden von Kacheln zugestimmt haben.

## Leistung

settings-performance-ram-cache = Arbeitsspeicher-Cache
settings-performance-ram-cache-help = Arbeitsspeicher, in dem decodierte Bilder bereitgehalten werden. „Automatisch“ richtet sich danach, wie viel Arbeitsspeicher Ihr Computer hat.
settings-performance-vram-cache = GPU-Speicher-Cache
settings-performance-vram-cache-help = GPU-Speicher für Bilder und Kacheln. „Automatisch“ richtet sich nach Ihrer GPU.
settings-performance-prefetch-depth = Im Voraus vorzubereitende Bilder
settings-performance-prefetch-depth-help = Wie viele Bilder in Blätterrichtung im Voraus decodiert werden.
settings-performance-decoder-threads = Decoder-Threads
settings-performance-decoder-threads-help = CPU-Threads zum Decodieren von Bildern. „Automatisch“ verwendet alle Leistungskerne und passt sich der Datei an.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Welche GPU verwendet wird, wenn der Computer mehrere hat.
settings-performance-gpu-adapter-high-performance = Höchste Leistung
settings-performance-gpu-adapter-low-power = Geringster Stromverbrauch
settings-performance-processing = Wo Bilder verarbeitet werden
settings-performance-processing-help = Manche Arbeit kann auf der CPU, der GPU oder einem Hardware-Decoder laufen. Automatisch misst diesen Computer und wählt für das angezeigte Bild den schnellsten Weg und für Arbeit im Hintergrund den sparsamsten. Was auf diesem Computer gewählt wurde und warum, steht weiter unten.
settings-performance-processing-automatic = Automatisch
settings-performance-processing-prefer-gpu = GPU bevorzugen
settings-performance-processing-cpu-only = Nur CPU
settings-performance-drag-preview = Vorschau beim Ziehen
settings-performance-drag-preview-help = Wie Entwickeln das Foto zeigt, während Sie einen Regler ziehen. Automatisch nimmt nur dann ein kleineres Bild, wenn der Computer nicht mithält.
settings-performance-drag-preview-automatic = Automatisch
settings-performance-drag-preview-sharp = Immer scharf
settings-performance-drag-preview-fast = Immer schnell

## Settings → Performance → How { -app-name } runs on this computer (DESIGN.md §12.3.1)
# machine-drafted, needs native review
settings-paths-title = So arbeitet { -app-name } auf diesem Computer
settings-paths-intro = { -app-name } misst jeden Weg für diese Aufgaben auf diesem Computer und nutzt den besten. Die Zeiten gelten für ein Foto mit { $mpx } Megapixeln.
settings-paths-intro-measuring = Noch nicht fertig gemessen: { -app-name } misst, während der Computer nichts zu tun hat.
settings-paths-intro-battery = Im Akkubetrieb: Die Auswahl bevorzugt geringeren Energieverbrauch.
settings-paths-keywords = GPU CPU NPU Grafikkarte Hardware Beschleunigung Decoder Benchmark Geschwindigkeit
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, alle Kerne
paths-engine-gpu = GPU
paths-engine-hw-decoder = Hardware-Decoder
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (Ihre Einstellung)
settings-paths-measured-faster = Auf diesem Computer gemessen: { $ratio }-mal so schnell wie mit { $other }.
settings-paths-measured-similar = Auf diesem Computer gemessen: etwa so schnell wie mit { $other }.
settings-paths-measured-power = Im Akkubetrieb verbraucht dies am wenigsten Energie (auf diesem Computer gemessen).
settings-paths-not-measured = Noch nicht gemessen: die übliche Wahl für diese Art von Computer.
settings-paths-set-by = Durch Ihre Einstellung „{ $setting }: { $option }“ gewählt.
settings-paths-blocked-check = { $engine }: am { $date } abgeschaltet, weil das Ergebnis nicht mit dem der CPU übereinstimmte.
settings-paths-blocked-error = { $engine }: am { $date } nach einem Fehler abgeschaltet.
settings-paths-blocked-lost = { $engine }: am { $date } abgeschaltet, weil keine Antwort mehr kam.
settings-paths-blocked-undated = { $engine }: nach einem Fehler abgeschaltet.
settings-paths-checking = { $engine }: wird mit der CPU verglichen, bevor das angezeigte Bild damit verarbeitet wird.
settings-paths-exploring = Wird gerade im Hintergrund ausprobiert: { $engine }.
settings-paths-unavailable = Auf diesem Computer dafür nicht vorhanden: { $engine }.
settings-paths-background-cheaper = Im Hintergrund: { $engine }, gemessen { $percent } weniger CPU-Zeit.
settings-paths-background-power = Im Hintergrund: { $engine }, um Energie zu sparen.
settings-paths-background = Im Hintergrund: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms CPU-Zeit, { $runs ->
    [one] { $runs } Messung
   *[other] { $runs } Messungen
} ({ $confidence })
settings-paths-detail-guess = { $engine }: etwa { $time } ms (geschätzt)
settings-paths-confidence-low = geringe Sicherheit
settings-paths-confidence-medium = mittlere Sicherheit
settings-paths-confidence-high = hohe Sicherheit
settings-paths-machine = Gemessen für { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Erneut versuchen
settings-paths-measure-again = Neu messen
settings-paths-measure-again-question = Alles, was auf diesem Computer gemessen wurde, samt der abgeschalteten Wege vergessen und neu messen, sobald der Computer nichts zu tun hat?
settings-paths-measure-again-confirm = Messungen vergessen

settings-performance-display-cache = Anzeige-Cache auf der Festplatte speichern
settings-performance-display-cache-help = Bildschirmgroße Kopien von Bildern speichern, damit bereits angesehene Ordner sofort geöffnet werden.
settings-performance-display-cache-size = Größe des Anzeige-Cache
settings-performance-display-cache-size-help = Wenn der Cache voll ist, werden die ältesten Einträge entfernt.
settings-performance-display-cache-location = Speicherort des Anzeige-Cache
settings-performance-display-cache-location-help = Ein Ordner auf einem schnellen Laufwerk ist am besten geeignet.
settings-performance-fast-simd = SIMD-Befehle verwenden
settings-performance-fast-simd-help = Die Vektorbefehle der CPU zum Decodieren verwenden. Nur zur Fehlerdiagnose deaktivieren.
settings-performance-fast-dct-scaling = In Bildschirmgröße decodieren
settings-performance-fast-dct-scaling-help = Große JPEGs direkt in der angezeigten Größe decodieren. Nur zur Fehlerdiagnose deaktivieren.
settings-performance-fast-parallel-decode = Auf mehreren Kernen decodieren
settings-performance-fast-parallel-decode-help = Große Bilder auf mehrere CPU-Kerne aufteilen. Nur zur Fehlerdiagnose deaktivieren.
settings-performance-fast-restart-index = Große JPEGs zum Zoomen indizieren
settings-performance-fast-restart-index-help = Speichern, wo jeder Teil eines JPEGs beginnt, damit vergrößerte Bereiche schnell decodiert werden. Nur zur Fehlerdiagnose deaktivieren.
settings-performance-fast-yuv-upload = Farben auf der GPU umrechnen
settings-performance-fast-yuv-upload-help = JPEG-Farben auf der GPU oder der CPU umrechnen. Automatisch wählt, was auf diesem Computer schneller ist. Nur zur Fehlerdiagnose ausschalten.
settings-performance-fast-yuv-upload-auto = Automatisch
settings-performance-fast-yuv-upload-on = Ein
settings-performance-fast-yuv-upload-off = Aus
settings-performance-fast-async-io = Asynchrones Lesen von Dateien
settings-performance-fast-async-io-help = Mit der schnellsten Dateischnittstelle des Betriebssystems vorauslesen. Nur zur Fehlerdiagnose deaktivieren.
settings-performance-fast-mmap = Speicherabgebildete Dateien
settings-performance-fast-mmap-help = Dateien lesen, ohne sie zu kopieren. Nur zur Fehlerdiagnose deaktivieren.
settings-performance-hardware-decode = Hardware-Dekodierung
settings-performance-hardware-decode-help = JPEG-Fotos mit dem eingebauten Decoder der GPU öffnen. Automatisch nutzt ihn, wo er hilft, meist für im Hintergrund vorbereitete Bilder. Er wird zuerst mit dem normalen Decoder verglichen, und was er nicht unterstützt, wird normal geöffnet.
settings-performance-hardware-decode-auto = Automatisch
settings-performance-hardware-decode-on = Ein
settings-performance-hardware-decode-off = Aus
settings-performance-frame-time-hud = Frame-Time-HUD
settings-performance-frame-time-hud-help = Decodier- und Frame-Zeiten sowie die verwendeten schnellen Verarbeitungswege anzeigen.

## Bedienungshilfen

settings-accessibility-reduce-motion = Bewegung reduzieren
settings-accessibility-reduce-motion-help = Zoom-, Schiebe- und Überblendanimationen durch sofortige Wechsel ersetzen.
settings-accessibility-reduce-motion-system = Systemeinstellung verwenden
settings-accessibility-reduce-motion-on = Ein
settings-accessibility-reduce-motion-off = Aus
settings-accessibility-increase-contrast = Kontrast erhöhen
settings-accessibility-increase-contrast-help = Kräftigere Rahmen und Texte und weniger Transparenz.
settings-accessibility-increase-contrast-system = Systemeinstellung verwenden
settings-accessibility-increase-contrast-on = Ein
settings-accessibility-increase-contrast-off = Aus
settings-accessibility-text-scale = Textgröße
settings-accessibility-text-scale-help = Größe des Textes im Verhältnis zur Standardgröße.
settings-accessibility-ui-scale = Skalierung der Benutzeroberfläche
settings-accessibility-ui-scale-help = Größe der gesamten Benutzeroberfläche, zusätzlich zur Anzeigeskalierung Ihres Betriebssystems.
settings-accessibility-focus-ring-always = Fokusrahmen immer anzeigen
settings-accessibility-focus-ring-always-help = Auch bei Verwendung der Maus anzeigen, welches Bedienelement den Tastaturfokus hat.
settings-accessibility-screen-reader-verbosity = Ausführlichkeit des Bildschirmlesers
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Wie viel ein Bildschirmleser ansagt, wenn Sie zwischen Bildern wechseln, und wie viel der Anzeige unter dem Zeiger er vorliest.
settings-accessibility-screen-reader-verbosity-concise = Knapp
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Ausführlich
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Wert unter dem Zeiger vorlesen
settings-accessibility-announce-cursor-help = Ein Bildschirmleser liest den Pixelwert, HU, die Temperatur oder die Position unter dem Zeiger oder Tastatur-Fadenkreuz vor, sobald es sich nicht mehr bewegt.

## Datenschutz

# machine-drafted, needs native review
settings-privacy-no-network = Kein Netzwerk
# machine-drafted, needs native review
settings-privacy-no-network-help = Aus: Nur diese verbinden sich, jeweils erst, wenn Sie sie einschalten oder etwas damit öffnen: Suche nach Updates, Downloads von Modellen der lokalen KI, Online-Karten, Bilder aus einer Webadresse oder einem Cloud-Speicher und Server, die Sie hinzufügen (DICOMweb, OMERO, Karten). An: Nichts verbindet sich.

## Erweitert

settings-advanced-portable-mode = Portabler Modus
settings-advanced-portable-mode-help = Einstellungen und Caches im Ordner neben dem Programm statt in Ihrem Benutzerprofil speichern.
settings-advanced-log-level = Protokollumfang
settings-advanced-log-level-help = Wie viel { -app-name } in seine Protokolldatei schreibt.
settings-advanced-log-level-error = Nur Fehler
settings-advanced-log-level-warn = Warnungen
settings-advanced-log-level-info = Informationen
settings-advanced-log-level-debug = Debugging
settings-advanced-log-level-trace = Alles
settings-advanced-developer-mode = Entwicklermodus
settings-advanced-developer-mode-help = Zeigt Werkzeuge zum Entwickeln und Testen von { -app-name }, etwa die Pseudosprachen in der Sprachliste.
# machine-drafted, needs native review
settings-advanced-audit-log = Lokales Prüfprotokoll
# machine-drafted, needs native review
settings-advanced-audit-log-help = Geöffnete medizinische Bilder, Exporte, Anonymisierungen, Ausdrucke und Sendungen an ein PACS in audit.jsonl im Datenordner aufzeichnen. Es bleibt auf diesem Computer.
settings-advanced-hw-report = Hardwarebericht …
settings-advanced-hw-report-help = Anzeigen, welche CPU- und GPU-Funktionen { -app-name } erkannt hat und welche schnellen Verarbeitungswege verwendet werden.
# machine-drafted, needs native review
settings-advanced-renderer-info = Rendererinformationen …
settings-advanced-renderer-info-help = Grafikadapter, Treiber, Anzeigeformate und Speichergrenzen anzeigen, die { -app-name } verwendet, bereit zum Kopieren in einen Fehlerbericht.
settings-advanced-edit-settings-file = settings.toml bearbeiten
settings-advanced-edit-settings-file-help = Die Einstellungsdatei im eigenen Editor von { -app-name } bearbeiten. Er prüft die Datei beim Tippen und speichert sie nur, wenn sie fehlerfrei ist.
settings-advanced-import = Einstellungen importieren …
settings-advanced-import-help = Einstellungen aus einer Datei laden, die aus { -app-name } exportiert wurde.
settings-advanced-export = Einstellungen exportieren …
settings-advanced-export-help = Alle Einstellungen in einer Datei speichern, zur Sicherung oder für einen anderen Computer.
settings-advanced-reset-all = Alle Einstellungen zurücksetzen …
settings-advanced-reset-all-help = Alle Einstellungen auf ihre Standardwerte zurücksetzen. Dies kann rückgängig gemacht werden.

# Stage 1 merge (machine-drafted, needs native review)
settings-performance-stall-warning = Hängende Ladevorgänge melden nach
settings-performance-stall-warning-help = Wenn ein Bild nach dieser Zeit noch lädt, werden sein Ladezustand und die ausstehenden Aufträge ins Protokoll geschrieben (Standardfehlerausgabe oder SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Leistungshinweise
settings-performance-insights-help = Wenn Bilder wiederholt aus demselben Grund langsam öffnen, „Warum ist das langsam?“ in der Ecke anzeigen, mit dem, was helfen würde. Was dabei gelernt wird, bleibt auf diesem Computer.
settings-performance-insights-storage = Hinweise zu langsamem Speicher
settings-performance-insights-storage-help = Hinweise anzeigen, wenn das Lesen von Dateien (von einer Netzwerkfreigabe, einem langsamen Laufwerk oder aus Archiven) das Öffnen von Bildern wiederholt aufhält.
settings-performance-insights-decoder = Hinweise zum Dekodieren
settings-performance-insights-decoder-help = Hinweise anzeigen, wenn Bilder wiederholt auf der CPU dekodiert werden, obwohl ein Hardware-Decoder helfen könnte.
# machine-drafted, needs native review
settings-performance-insights-restore = Ausgeblendete Leistungshinweise wieder anzeigen
settings-performance-insights-restore-help = Holt die Leistungshinweise zurück, die Sie mit × bei „Warum ist das langsam?“ ausgeblendet haben – für das Öffnen von Bildern und für Miniaturen. Nicht verfügbar, solange keine ausgeblendet sind.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Speichern
config-editor-revert = Zurücksetzen
config-editor-close = Schließen
config-editor-modified = Geändert
config-editor-text = Einstellungsdatei
config-editor-position = Zeile { $line }, Spalte { $column }
config-editor-saved = settings.toml gespeichert und übernommen.
config-editor-cant-save =
    { $count ->
        [one] Speichern nicht möglich: 1 Problem. Bitte zuerst beheben.
       *[other] Speichern nicht möglich: { $count } Probleme. Bitte zuerst beheben.
    }
config-editor-save-failed = settings.toml kann nicht gespeichert werden: { $reason }
config-editor-conflict = settings.toml wurde anderswo geändert.
config-editor-reload = Neu laden
config-editor-keep-mine = Meine behalten
config-editor-discard-question = Ungespeicherte Änderungen an settings.toml verwerfen?
config-editor-discard = Verwerfen
config-editor-keep-editing = Weiter bearbeiten
config-editor-find-placeholder = In Datei suchen
config-editor-find-count = { $current } von { $total }
config-editor-find-none = Keine Treffer
config-editor-find-previous = Vorheriger Treffer
config-editor-find-next = Nächster Treffer
config-editor-find-close = Suche schließen
config-editor-problems = Probleme
config-editor-problems-count =
    { $count ->
        [0] Keine Probleme
        [one] 1 Problem
       *[other] { $count } Probleme
    }
config-editor-no-problems = Keine Probleme. Die Datei kann gespeichert werden.
config-editor-problem-at = Zeile { $line }: { $message }
config-editor-severity-error = Fehler
config-editor-severity-warning = Warnung
config-editor-severity-info = Hinweis
config-editor-problem-syntax = Kein gültiges TOML: { $reason }
config-editor-problem-duplicate = { $key } ist in derselben Tabelle zweimal gesetzt. Entfernen Sie einen Eintrag.
config-editor-problem-unknown-section = Es gibt keinen Einstellungsabschnitt [{ $key }].
config-editor-problem-unknown-section-suggestion = Es gibt keinen Einstellungsabschnitt [{ $key }]. Meinten Sie [{ $suggestion }]?
config-editor-problem-unknown-key = Es gibt keine Einstellung { $key }.
config-editor-problem-unknown-key-suggestion = Es gibt keine Einstellung { $key }. Meinten Sie { $suggestion }?
config-editor-problem-invalid = Ungültiger Wert für { $key }: { $reason }
config-editor-problem-migrated = { $key } ist so geschrieben, wie eine ältere Version von { -app-name } es schrieb. Es wird beim Lesen umgewandelt.
config-editor-problem-newer-version = Diese Datei wurde von einer neueren Version von { -app-name } geschrieben (Format { $found }; diese Version liest { $supported }). Unbekannte Einstellungen bleiben erhalten.
config-editor-problem-command = Befehl { $index } in [[commands]]: { $reason }
config-editor-problem-hook = Hook { $index } im Abschnitt [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Tastenbelegungsdatei
config-editor-keymap-saved = keymap.toml gespeichert und angewendet.
config-editor-keymap-save-failed = keymap.toml kann nicht gespeichert werden: { $reason }
config-editor-keymap-conflict = keymap.toml wurde anderswo geändert.
config-editor-keymap-discard-question = Ungespeicherte Änderungen an keymap.toml verwerfen?
config-editor-new-file = neue Datei, wird beim Speichern angelegt
config-editor-no-folder = es gibt keinen Einstellungsordner, in dem sie gespeichert werden könnte.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Alle Einstellungen auf ihre Standardwerte zurücksetzen?
settings-reset-all-confirm-detail = Das betrifft alle Bereiche der Einstellungen. Es lässt sich danach rückgängig machen.
settings-reset-all-confirm-ok = Alle zurücksetzen
settings-reset-all-done = Alle Einstellungen wurden auf ihre Standardwerte zurückgesetzt.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Hardware-Encoder

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = { $engine }: wird vor der Verwendung mit der CPU verglichen.
settings-paths-unit-thumbnail = Die Zeiten gelten für eine Miniatur mit { $size }
settings-paths-unit-frame = Die Zeiten gelten je Videobild mit { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Ordner für Kalibrierbilder
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flat- und Darkframes für Entwickeln. Datei- oder Ordnernamen enthalten „flat“ oder „dark“.

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Weiß
settings-colour-light-grey = Hellgrau
settings-colour-grey = Grau
settings-colour-dark-grey = Dunkelgrau
settings-colour-black = Schwarz
settings-colour-yellow = Gelb
settings-colour-orange = Orange
settings-colour-red = Rot
settings-colour-magenta = Magenta
settings-colour-blue = Blau
settings-colour-cyan = Cyan
settings-colour-green = Grün
