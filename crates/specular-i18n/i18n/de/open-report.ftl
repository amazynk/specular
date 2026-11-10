# machine-drafted, needs native review
# Specular — Öffnungsbericht: warum das Öffnen eines Bildes so lange dauerte (Informationsbereich,
# Abschnitt „Leistung“; DESIGN.md §12.7).

open-report-title = Leistung

## Headlines
open-report-fast = In { $ms } ms geöffnet
open-report-display-cache = Aus dem Anzeige-Cache angezeigt
open-report-slow-read = Langsamer Speicher: { $percent } der Zeit entfielen auf das Lesen der Datei ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Langsamer Speicher: { $percent } der Zeit entfielen auf das Lesen der Datei (kleine Datei: vor allem Warten auf die Antwort des Speichers)
open-report-archive-read = { $percent } der Zeit entfielen auf das Entpacken des Archivs
open-report-startup = { $percent } der Zeit entfielen auf den Start von { -app-name }
open-report-checked = Zweimal dekodiert: das erste Ergebnis des Hardware-Decoders wurde mit dem der CPU verglichen ({ $percent } der Zeit)
open-report-hardware = Vom Hardware-Decoder dekodiert, der { $percent } der Zeit brauchte
open-report-decode = Großes Bild: das Dekodieren von { $mpx } Megapixeln brauchte { $percent } der Zeit
open-report-upload = { $percent } der Zeit entfielen auf die Übertragung an die GPU
open-report-waiting = { $percent } der Zeit entfielen auf das Warten auf andere Arbeit
open-report-stage = { $stage } brauchte { $percent } der Zeit
open-report-memory-short = Der Arbeitsspeicher war knapp, das System musste eventuell auslagern

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = In Software dekodiert: der Hardware-Decoder wurde noch erkannt
open-report-sw-no-device = In Software dekodiert: { $api } ist auf diesem Computer nicht verfügbar
open-report-sw-no-codec = In Software dekodiert: { $api } hat auf dieser GPU keinen { $codec }-Decoder
open-report-sw-blocked-mismatch = In Software dekodiert: der Hardware-Decoder wurde abgeschaltet, weil sein Ergebnis nicht mit dem der CPU übereinstimmte
open-report-sw-blocked-error = In Software dekodiert: der Hardware-Decoder wurde nach einem Fehler abgeschaltet
open-report-sw-blocked-lost = In Software dekodiert: der Hardware-Decoder wurde abgeschaltet, weil er nicht mehr reagierte
open-report-sw-failed = In Software dekodiert: der Hardware-Decoder ist bei diesem Bild gescheitert
open-report-sw-busy = In Software dekodiert: der Hardware-Decoder war ausgelastet
open-report-sw-unsupported = In Software dekodiert: der Hardware-Decoder kann dieses Bild nicht verarbeiten
open-report-sw-unavailable = In Software dekodiert: Hardware-Dekodierung ist bis zum Neustart von { -app-name } aus
open-report-sw-cpu-only = In Software dekodiert: die Verarbeitung ist auf „Nur CPU“ gestellt
open-report-sw-setting-off = In Software dekodiert: Hardware-Dekodierung ist in den Einstellungen ausgeschaltet
open-report-sw-unchecked = In Software dekodiert: der Hardware-Decoder ist noch nicht geprüft (er wird zuerst an einem im Hintergrund vorbereiteten Bild ausprobiert)
open-report-sw-lower-power = In Software dekodiert: im Akkubetrieb braucht die CPU dafür weniger Energie
open-report-sw-faster = In Software dekodiert: auf diesem Computer schneller (etwa { $cpu } ms gegenüber { $hw } ms mit dem Hardware-Decoder)
open-report-sw-scaled = In Software dekodiert: der Hardware-Decoder dekodiert Bilder nur in voller Größe

## Hints
open-report-hint-copy-local = Kopieren Sie die Datei auf ein lokales Laufwerk, damit sie schneller öffnet.
open-report-hint-extract = Entpacken Sie das Archiv in einen Ordner, damit seine Seiten schneller öffnen.
open-report-hint-try-again = Um den Hardware-Decoder erneut zu versuchen: Einstellungen → Leistung → Erneut versuchen.
open-report-hint-automatic = Stellen Sie „Wo Bilder verarbeitet werden“ unter Einstellungen → Leistung auf Automatisch.
open-report-hint-turn-on = Stellen Sie „Hardware-Dekodierung“ unter Einstellungen → Leistung auf Automatisch.
open-report-hint-free-memory = Schließen Sie andere Programme, um Speicher freizugeben.
open-report-hint-once-only = Nur das erste Bild wird so geprüft.

## Rows
open-report-total = Bis scharf
open-report-first-pixels = Erste Pixel
# machine-drafted, needs native review
open-report-whole = Ganzes Bild sichtbar
open-report-stage-startup = Programmstart
open-report-stage-read = Datei lesen
open-report-stage-parse = Kopfdaten lesen
open-report-stage-colour = Farbmanagement vorbereiten
open-report-stage-decode = Dekodieren
open-report-stage-upload = An die GPU senden
open-report-stage-waiting = Warten auf andere Arbeit
open-report-decoder = Dekodiert von
open-report-decoder-checked = { $engine }, mit der CPU verglichen
open-report-decoder-cache = Anzeige-Cache
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, in 1/{ $scale } der Größe
open-report-decoder-scaled-full = { $engine }, in voller Größe, dann auf 1/{ $scale } verkleinert
open-report-decoder-scaled-no-index = { $engine }, in 1/{ $scale } der Größe, ohne den Schnellstart-Index
open-report-colour-conversion = Farben umgerechnet von

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Wiederkehrend
open-report-advice-change = Würde helfen
open-report-advice-gain = Geschätzter Gewinn
open-report-advice-software = { $count } der letzten { $total } langsamen { $format }-Bilder { $count ->
    [one] wurde
   *[other] wurden
} aus diesem Grund in Software dekodiert
open-report-advice-slow-storage = { $count } der letzten { $total } langsamen Bilder { $count ->
    [one] wurde
   *[other] wurden
} beim Lesen der Datei aufgehalten (etwa { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } der letzten { $total } langsamen Bilder { $count ->
    [one] wurde
   *[other] wurden
} beim Lesen der Datei aufgehalten (kleine Dateien: vor allem Warten auf die Antwort des Speichers)
open-report-advice-archive = { $count } der letzten { $total } langsamen Bilder { $count ->
    [one] wurde
   *[other] wurden
} beim Entpacken des Archivs aufgehalten
open-report-advice-hardware-decoder = Eine GPU mit { $codec }-Hardware-Decoder
open-report-advice-gain-range = Etwa { $low }- bis { $high }-mal schneller
open-report-advice-gain-one = Etwa { $low }-mal schneller

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Leistungshinweise
open-report-insights-indicator = Warum ist das langsam?
open-report-insights-announce = Leistungshinweis: Bilder öffnen wiederholt aus demselben Grund langsam. Öffnen Sie „Warum ist das langsam?“, um zu sehen, was helfen würde.
open-report-insights-hide = Ausblenden
open-report-insights-last = Letztes langsames Bild: { $total } bis scharf
open-report-insights-now = Was Sie jetzt tun können
open-report-insights-hardware = Hardware, die helfen würde
open-report-insights-none = Keine wiederkehrende Verlangsamung
open-report-insights-none-detail = Hinweise erscheinen, wenn mehrere Bilder aus demselben Grund langsam öffnen. Um sie zu finden, behält { -app-name } langsame Öffnungsvorgänge einer Woche auf diesem Computer; nichts wird irgendwohin gesendet.
open-report-insights-software = { $format }-Bilder werden auf der CPU dekodiert
open-report-insights-storage = Das Lesen der Dateien hält die Bilder auf
open-report-insights-archive = Das Entpacken von Archiven hält die Bilder auf
open-report-insights-nothing-now = Keine Einstellung in { -app-name } ändert daran etwas.
open-report-insights-display-cache = Schalten Sie den Anzeige-Cache ein: Bereits geöffnete Bilder öffnen dann aus einer schnellen Kopie auf diesem Computer.
open-report-insights-prefetch = Halten Sie mehr benachbarte Bilder bereit, damit die nächsten vorbereitet werden, während Sie schauen.
open-report-insights-hw-ssd = Ein schnelles internes Laufwerk (eine NVMe-SSD) für diese Dateien
open-report-insights-action-turn-on = Einschalten
open-report-insights-action-try-again = Erneut versuchen
open-report-insights-action-automatic = Auf Automatisch stellen
open-report-insights-dismiss-storage = Für Speicher nicht mehr anzeigen
open-report-insights-dismiss-decoder = Für Dekodierung nicht mehr anzeigen
open-report-insights-off = Leistungshinweise ausschalten
open-report-insights-on = Leistungshinweise einschalten
open-report-insights-restore = Ausgeblendete Leistungshinweise wieder anzeigen
open-report-insights-power = Dieser Computer spart Energie (im Akkubetrieb oder in einem Energiesparmodus), was das Dekodieren auf der CPU verlangsamt. Am Netzteil öffnen Bilder womöglich schneller.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Leistungshinweis: Miniaturen erscheinen wiederholt aus demselben Grund langsam. Öffnen Sie „Warum ist das langsam?“, um zu sehen, was helfen würde.
open-report-insights-last-thumbnail = Letzte langsame Miniatur: { $total } zum Erstellen
open-report-insights-thumbnails-software = Miniaturen von { $format }-Bildern werden auf der CPU dekodiert
open-report-insights-thumbnails-storage = Das Lesen der Dateien hält die Miniaturen auf
open-report-insights-thumbnails-archive = Das Entpacken von Archiven hält die Miniaturen auf
open-report-insights-thumbnail-cache = Miniaturen können auf diesem Computer nicht aufbewahrt werden (der Miniatur-Cache ist nicht verfügbar), daher werden sie bei jeder Anzeige des Ordners neu erstellt.
open-report-advice-thumbnails-software = { $count } der letzten { $total } langsamen { $format }-Miniaturen { $count ->
    [one] wurde
   *[other] wurden
} aus diesem Grund in Software dekodiert
open-report-advice-thumbnails-slow-storage = { $count } der letzten { $total } langsamen Miniaturen { $count ->
    [one] wurde
   *[other] wurden
} beim Lesen der Datei aufgehalten (etwa { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } der letzten { $total } langsamen Miniaturen { $count ->
    [one] wurde
   *[other] wurden
} beim Lesen der Datei aufgehalten (kleine Dateien: vor allem Warten auf die Antwort des Speichers)
open-report-advice-thumbnails-archive = { $count } der letzten { $total } langsamen Miniaturen { $count ->
    [one] wurde
   *[other] wurden
} beim Entpacken des Archivs aufgehalten
