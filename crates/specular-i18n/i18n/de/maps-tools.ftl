# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Gelände
cheatsheet-map-time = Zeit und Wetter
cheatsheet-map-online = Online-Dienste
cheatsheet-map-photos = Fotos und GPS
cheatsheet-map-output = Ausgabe

## Palette entries and their one-line descriptions.
palette-map-slope = Neigungsfarben
palette-map-slope-help = Ein Höhenmodell danach einfärben, wie steil es ist.
palette-map-aspect = Expositionsfarben
palette-map-aspect-help = Ein Höhenmodell nach der Richtung einfärben, in die seine Hänge zeigen.
palette-map-contours = Höhenlinien ein- oder ausblenden
palette-map-contours-help = Höhenlinien über einem Höhenmodell, alle paar Linien beschriftet.
palette-map-terrain-3d = Gelände in 3D
palette-map-terrain-3d-help = Das Höhenmodell als 3D-Fläche zeigen, mit seinen Farben darübergelegt.
palette-map-time = Zeitregler ein- oder ausblenden
palette-map-time-help = Durch die Zeiten eines Rasters oder entlang der Zeiten eines Tracks gehen.
palette-map-play = Zeit abspielen oder anhalten
palette-map-play-help = Den Zeitregler selbst laufen lassen, Schritt für Schritt.
palette-map-wind = Windpfeile ein- oder ausblenden
palette-map-wind-help = Pfeile für Windgeschwindigkeit und -richtung aus den u- und v-Komponenten des Rasters.
palette-map-weather-colours = Nächste Wetter-Farbskala
palette-map-weather-colours-help = Farbskalen für Temperatur, Regen, Wind und Luftdruck.
palette-map-services = Online-Kartendienste…
palette-map-services-help = Layer von WMS-, WMTS-, WFS-, OGC-API- oder STAC-Servern hinzufügen.
palette-map-photos-on-track = Fotos auf dem Track ein- oder ausblenden
palette-map-photos-on-track-help = Die Fotos des Ordners nach ihrer Aufnahmezeit entlang des GPS-Tracks platzieren.
palette-map-geotag = Fotos aus dem Track geotaggen…
palette-map-geotag-help = Die Positionen des Tracks in XMP-Filialdateien der Fotos des Ordners schreiben.
palette-map-export = Karte exportieren…
palette-map-export-help = Die Karte als PNG oder PDF in einem Maßstab speichern, mit Legende, Maßstabsleiste und Nordpfeil.
palette-map-print = Karte maßstabsgetreu drucken…
palette-map-print-help = Die Karte in einem gewählten Maßstab drucken, etwa 1:{ NUMBER(25000) }.
palette-map-copy-extent = Ausdehnung kopieren
palette-map-copy-extent-help = Den sichtbaren Bereich als Begrenzungsrahmen oder als WKT kopieren.

## Why a map command is unavailable.
disabled-no-time = Die Karte hat keine Zeiten
disabled-no-wind = Die Karte hat keine Windkomponenten (u und v)
disabled-no-track = Kein GPS-Track mit Zeiten
disabled-not-grid = Die Karte ist kein Werteraster

## Settings › Maps and Metadata.
settings-maps-services = Online-Kartendienste
settings-maps-services-help = Layer von WMS-, WMTS-, WFS-, OGC-API- und STAC-Servern hinzufügen. Aus: Es wird nichts geladen.
settings-maps-services-allowed = Erlaubte Kartenserver
settings-maps-services-allowed-help = Server, denen Sie nach der Erklärung, was sie sehen, zugestimmt haben.
settings-metadata-basemap-service = Ein Layer eines Kartendienstes
settings-metadata-basemap-service-help = Der Layer, der im Bereich der Online-Dienste der Karte gewählt ist.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NO
    [e] O
    [se] SO
    [s] S
    [sw] SW
    [w] W
    [nw] NW
   *[n] N
}
map-slope-aspect = Neigung { $slope }° · zeigt nach { $direction } ({ $aspect }°)
map-slope-flat = Neigung { $slope }° · eben
map-slope-on = Neigungsfarben
map-aspect-on = Expositionsfarben
map-relief-off = Höhenfarben
map-contours-on = Höhenlinien an
map-contours-off = Höhenlinien aus
map-legend-slope = Neigung
map-legend-aspect = Exposition: wohin die Hänge zeigen
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° und mehr
map-terrain-3d-on = Gelände in 3D. Esc kehrt zur Karte zurück.
map-terrain-3d-on-exaggerated = Gelände in 3D, Höhen × { $times }. Esc kehrt zur Karte zurück.
map-terrain-exaggeration = Höhen { $times }-fach überhöht.
map-terrain-drape = Kartenfarben

## Time, weather and wind.
map-time-grid = { $when } · { $step } von { $steps }
map-time-track = { $when } · nach { $elapsed }
map-time-speed = × { $times }
map-time-play = Abspielen
map-time-pause = Pause
map-weather-map = Farben: { $name }
map-weather-file = Farben aus der Datei
map-wind-on = Windpfeile an
map-wind-off = Windpfeile aus
map-wind-at = Wind { $speed } { $unit } aus { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Begrenzungsrahmen kopiert. Erneut kopieren für WKT.
map-extent-copied-wkt = Ausdehnung als WKT kopiert

## Online map services.
map-services-title = Online-Kartendienste
map-service-off = Online-Kartendienste sind aus.
map-service-off-why = Schalten Sie sie ein, um Layer von Servern im Internet hinzuzufügen.
map-service-turn-on = Einschalten
map-service-close = Schließen
map-service-back = Zurück
map-service-cancel = Abbrechen
map-service-connect = Verbinden
map-service-allow = Erlauben
map-service-remove = Entfernen
map-service-download = Herunterladen
map-service-list-hint = Eingabe öffnet einen Server. Entf entfernt ihn.
map-service-add = Server hinzufügen…
map-service-add-sub = WMS, WMTS, WFS, OGC API oder STAC
map-service-add-hint = Die Adresse des Servers. Eingabe verbindet.
map-service-bad-address = Keine Webadresse (http oder https, ohne Benutzernamen).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Online-Kartendienst
map-service-consent-server = Specular wird { $host } kontaktieren.
map-service-consent-sees = Dieser Server sieht Ihre Internetadresse und den angefragten Kartenbereich.
map-service-consent-private = Ihre Dateien und deren Namen werden nicht gesendet.
map-service-asking = Frage { $host } an…
map-service-downloading = Lade von { $host } herunter…
map-service-layers-hint = { $kind }-Layer. Eingabe verwendet einen.
map-service-no-layers = Der Server listet keine Layer.
map-service-offer-tiles = Kacheln: wird zur Grundkarte der Fotokarte
map-service-offer-features = Objekte: werden dieser Karte für den sichtbaren Bereich hinzugefügt
map-service-offer-search = Bilder: im sichtbaren Bereich suchen
map-service-refused-mercator = Nicht in Web Mercator (EPSG:3857) angeboten, den die Grundkarte braucht.
map-service-refused-format = Nicht als PNG- oder JPEG-Bilder angeboten.
map-service-refused-geojson = Nicht als GeoJSON angeboten, das Format, das hier gelesen wird.
map-service-items-hint = { $count ->
        [one] { $count } Bild gefunden. Eingabe lädt es herunter.
       *[other] { $count } Bilder gefunden. Eingabe lädt eines herunter.
    }
map-service-no-items = Hier keine Bilder gefunden.
map-service-item-cloud = { $when } · { PERCENT($cloud) } Wolken
map-service-item-no-cog = Kein Cloud-Optimized GeoTIFF
map-service-download-ask = { $size } von { $host } herunterladen?
map-service-download-ask-unknown = Dieses Bild von { $host } herunterladen? Seine Größe ist unbekannt.
map-service-download-cache = Es wird im Karten-Cache abgelegt und geöffnet.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = Das Bild ist { $size } groß, zu groß zum Herunterladen.
map-service-opened = Heruntergeladenes Bild wird geöffnet
map-service-basemap = { $layer } ist jetzt die Grundkarte der Fotokarte. Die Fotokarte fragt vor dem ersten Download.
map-service-status = Der Server antwortete mit { $status }.
map-service-unknown = Kein Kartendienst, den diese Version liest.
map-service-exception = Der Server meldet: { $message }
map-service-malformed = Die Antwort des Servers ist fehlerhaft: { $reason }
map-service-remove-ask = „{ $server }“ aus den gespeicherten Servern entfernen?
map-service-remove-detail = Auf dem Datenträger geht nichts verloren. Layer auf der Karte bleiben, bis sie geschlossen wird.
map-service-removed = { $server } entfernt
map-service-saved = Gespeicherte Kartenserver
map-service-saved-none = Noch keine. Fügen Sie einen im Bereich der Online-Dienste der Karte hinzu.
map-service-keywords = WMS WMTS WFS OGC API STAC Server Kartendienste

## Photos and geotagging.
map-geotag-title = Fotos aus dem Track geotaggen
map-photos-on = Fotos auf dem Track
map-photos-off = Fotos auf dem Track ausgeblendet
map-geotag-offset = Kamerauhr: { $offset } gegenüber GPS-Zeit. ← → ändern um eine Minute, mit Shift um eine Stunde, mit Alt um eine Sekunde.
map-geotag-reading = Fotos werden gelesen…
map-geotag-summary = { $count } von { $total } Fotos erhalten eine Position.
map-geotag-sidecars = Die Positionen kommen in XMP-Filialdateien neben den Fotos. Die Fotodateien werden nicht geändert.
map-geotag-writing = Wird geschrieben…
map-geotag-write = Filialdateien schreiben…
map-geotag-guess = Versatz schätzen
map-geotag-will-write = Erhält { $position }
map-geotag-has-position = Hat bereits eine Position: bleibt unverändert
map-geotag-no-time = Keine Aufnahmezeit: bleibt unverändert
map-geotag-outside = { $offset } außerhalb des Tracks aufgenommen: bleibt unverändert
map-geotag-gap = In einer Lücke des Tracks ({ $seconds } s von einem Fix): bleibt unverändert
map-geotag-nothing = Bei diesem Versatz erhält kein Foto eine Position.
map-geotag-confirm = Positionen in { $count ->
    [one] { $count } XMP-Filialdatei
   *[other] { $count } XMP-Filialdateien
} schreiben? Die Fotos selbst werden nicht geändert.
map-geotag-confirm-ok = Filialdateien schreiben
map-geotag-done = { $count ->
    [one] { $count } Filialdatei geschrieben
   *[other] { $count } Filialdateien geschrieben
}
map-geotag-done-failed = { $count ->
        [one] { $count } Filialdatei
       *[other] { $count } Filialdateien
    } geschrieben; { $failed } fehlgeschlagen: { $reason }
map-geotag-not-sidecar = Kein Name einer Filialdatei.

## Export and printing.
map-export-title = Karte exportieren
map-print-title = Karte maßstabsgetreu drucken
map-export-keys = ↑ ↓ wählen, ← → ändern, Eingabe startet.
map-export-format = Format
map-export-scale = Maßstab
map-export-scale-value = 1:{ $n }
map-export-resolution = Auflösung
map-export-dpi = { $dpi } dpi
map-export-paper = Papier
map-export-size = { $width } × { $height } px · { $across } × { $down } km im Gelände
map-export-save = Speichern…
map-export-print = Drucken…
map-export-too-large = Zu groß bei diesem Maßstab: { $width } × { $height } px. Wählen Sie einen kleineren Maßstab oder eine kleinere Auflösung.
map-export-no-scale = Diese Karte hat keinen Maßstab im Gelände.
map-export-no-gpu = Der Export braucht den Grafikprozessor.
map-export-subtitle = Maßstab { $scale } · { $crs }
map-export-data = Daten: { $sources }
map-export-saved = { $name } gespeichert
map-export-printing = Wird gedruckt…
map-export-printed = An den Drucker gesendet
map-export-print-failed = Nicht gedruckt: { $reason }
