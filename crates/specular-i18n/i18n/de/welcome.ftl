# machine-drafted, needs native review
# Specular — Der Startbildschirm (wenn Specular ohne Datei öffnet und wenn das letzte Bild
# geschlossen wird), seine Karten „Zum Ausprobieren“ und die einmaligen Hinweise. Deutsch.

## Startbildschirm

welcome-title = { -app-name }
welcome-tagline = Ein schneller, originalgetreuer Betrachter für Fotos und für technische, wissenschaftliche und medizinische Bilder.
welcome-open = Datei öffnen …
welcome-open-about = Ein Bild, ein Dokument oder eine Datendatei.
welcome-browse = Ordner durchsuchen …
welcome-browse-about = Einen Ordner wählen und als Galerie ansehen.
welcome-pictures = Ihre Bilder
welcome-pictures-about = Alle Fotos Ihrer Bibliothek, nach Datum und Ort.
welcome-pictures-setup-title = Bibliothek einrichten …
welcome-pictures-setup = Ausgehend vom Ordner { $folder }.
welcome-pictures-ask = Ihren Ordner { $folder } zur Bibliothek hinzufügen?
welcome-pictures-add = Ordner hinzufügen
welcome-drop = Sie können Dateien oder Ordner auch an beliebiger Stelle in dieses Fenster ziehen.
welcome-recent = Zuletzt verwendete Ordner
welcome-recent-name = { $name }, in { $parent }
welcome-tips = Zum Ausprobieren
welcome-tips-hide = Diese Tipps ausblenden
welcome-tips-hidden = Tipps ausgeblendet. „Willkommen anzeigen“ in der Befehlspalette holt sie zurück.

## Zum Ausprobieren: je ein Name und eine Zeile. Die Taste neben dem Namen ist die eigene
## Belegung des Benutzers.

welcome-tip-gallery = Galerie
welcome-tip-gallery-about = Einen ganzen Ordner auf einen Blick sehen.
welcome-tip-compare = Vergleichen
welcome-tip-compare-about = Zwei bis vier Bilder in der Galerie auswählen, um sie nebeneinander zu sehen.
welcome-tip-cull = Cull-Modus
welcome-tip-cull-about = Fotos einer Serie mit je einer Taste auswählen, verwerfen und bewerten.
welcome-tip-slideshow = Diashow
welcome-tip-slideshow-about = Einen Ordner im Vollbild zeigen, ein Bild nach dem anderen.
welcome-tip-palette = Befehlspalette
welcome-tip-palette-about = Jeden Befehl und jede Einstellung nach Namen finden.
welcome-tip-studies = Studien
welcome-tip-studies-about = Patienten, Studien und Serien einer DICOM-CD oder eines DICOM-Ordners.
welcome-tip-dicom-servers = DICOMweb-Server
welcome-tip-dicom-servers-about = Studien aus einem PACS durchsuchen und öffnen.
welcome-tip-dicom-tags = DICOM-Tags
welcome-tip-dicom-tags-about = Alle Tags eines Bildes, durchsuchbar.
welcome-tip-window-level = Fensterung
welcome-tip-window-level-about = Ziehen, um den Kontrast eines DICOM-Bildes einzustellen.
welcome-tip-omero-servers = OMERO-Server
welcome-tip-omero-servers-about = Mikroskopie-Projekte auf OMERO und IDR durchsuchen.
welcome-tip-stretch = Streckung
welcome-tip-stretch-about = Schwache Details in astronomischen und wissenschaftlichen Daten hervorheben.
welcome-tip-drawing-layers = Zeichnungs-Layer
welcome-tip-drawing-layers-about = Die Layer einer technischen Zeichnung ein- und ausblenden.
welcome-tip-needs-image = Öffnen Sie zuerst ein Bild und drücken Sie dann { $key }.
welcome-tip-needs-gallery = Wählen Sie in der Galerie zwei bis vier Bilder aus und drücken Sie dann { $key }.
welcome-tip-key = Tastenkürzel: { $key }

## Einmalige Hinweise: je einmal, unaufdringlich, höchstens einer pro Start. Einstellungen ›
## Allgemein › Tipps anzeigen schaltet sie alle aus.

hint-gallery = Drücken Sie { $key }, um diesen Ordner als Galerie zu sehen.
hint-gallery-first = Wählen Sie zwei bis vier Bilder aus und drücken Sie { $compare } zum Vergleichen oder { $cull } zum Aussortieren des Ordners.
hint-raw = Ein Ordner mit Raw-Fotos: Drücken Sie { $key }, um ihn auszusortieren (auswählen, verwerfen, bewerten).
hint-pages =
    { $count ->
        [one] Diese Datei hat { $count } Seite. { $next } und { $prev } blättern um.
       *[other] Diese Datei hat { $count } Seiten. { $next } und { $prev } blättern um.
    }
hint-turn-off = Tipps ausschalten
hint-turned-off = Tipps ausgeschaltet. Einstellungen › Allgemein › Tipps anzeigen schaltet sie wieder ein.

## Befehlspalette

palette-show-welcome = Willkommen anzeigen
palette-show-welcome-help = Der Startbildschirm mit Öffnen, Durchsuchen, Ihren Bildern und Tipps zum Ausprobieren.
palette-close = Schließen
palette-close-help = Bild oder Ordner schließen und zum Startbildschirm zurückkehren. Auf dem Startbildschirm: beenden.

## Ersteinrichtung (unter der Profilfrage)

profile-on-start = Wenn { -app-name } ohne Datei öffnet:
profile-skip = Überspringen
