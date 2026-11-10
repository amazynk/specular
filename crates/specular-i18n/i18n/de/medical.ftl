# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Medizinische Bilder
palette-window-level = Fensterungswerkzeug (Linksziehen)
palette-window-default = Fenster: Dateistandard
palette-window-lung = Fenster: Lunge (W 1500, L −600)
palette-window-mediastinum = Fenster: Mediastinum (W 350, L 50)
palette-window-bone = Fenster: Knochen (W 1800, L 400)
palette-window-brain = Fenster: Gehirn (W 80, L 40)
palette-window-stroke = Fenster: Schlaganfall (W 40, L 40)
palette-window-soft-tissue = Fenster: Weichteile (W 400, L 50)
palette-window-liver = Fenster: Leber (W 150, L 30)
palette-window-subdural = Fenster: Subdural (W 200, L 75)
palette-window-full-range = Fenster: Gesamter Bereich
palette-invert-grey = Graustufen umkehren
palette-next-slice = Nächstes Bild
palette-prev-slice = Vorheriges Bild
disabled-not-medical = Nur für medizinische Graustufenbilder
disabled-one-frame = Die Datei hat nur ein Einzelbild
settings-section-medical = Medizinische Bilder
settings-medical-keep-window = Fensterung für die nächste Serie beibehalten
settings-medical-keep-window-help = Das eingestellte Fenster beim Wechsel zu einer anderen Serie beibehalten, statt mit deren Standard zu beginnen. Die Bilder einer Datei und die Schichten einer Serie teilen es immer.
medical-window = C { $center }  W { $width }
medical-inverted = invertiert
medical-frame = Bild { $frame } von { $count }
medical-tool = Fensterungswerkzeug: ziehen
medical-tool-on = Fensterungswerkzeug an: zum Anpassen ziehen
medical-tool-off = Fensterungswerkzeug aus
medical-preset-not-ct = CT-Voreinstellungen brauchen Werte in Hounsfield-Einheiten
medical-file-window-none = Kein Fenster { $index } in dieser Datei
medical-file-window-unnamed = Fenster der Datei { $index } von { $count }
medical-file-window = Fenster der Datei { $index } von { $count }: { $name }
measure-templates-need-medical = Geometrievorlagen gibt es mit dem Profil Medizin
medical-off = Keine medizinischen Werkzeuge für dieses Bild: { $reason }
medical-readout = Wert und Fenster

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Bilder abspielen (Cine)
palette-measure-length = Länge messen
palette-measure-angle = Winkel messen
palette-measure-ellipse = Elliptische Region messen
palette-measure-rectangle = Rechteckige Region messen
palette-clear-measurements = Messungen löschen
palette-measure-templates = Geometrievorlagen…
palette-overlays = Overlays ein- oder ausblenden
medical-cine = Wiedergabe mit { $rate } Bildern/s
medical-overlays-shown = Overlays eingeblendet
medical-overlays-hidden = Overlays ausgeblendet
measure-hint-length = Länge: ziehen oder beide Enden anklicken
measure-hint-angle = Winkel: drei Punkte anklicken, den Scheitel als zweiten
measure-hint-ellipse = Ellipse: über die Region ziehen
measure-hint-rectangle = Rechteck: über die Region ziehen
measure-off = Messwerkzeug aus
measure-cleared = Messungen gelöscht
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Fläche { $value } mm²
measure-area-px = Fläche { $value } px²
measure-angle = { $value }°
measure-mean = Mittelwert { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Max { $max }  ({ $count } px)
measure-caveat-detector = Am Detektor: nicht um die Vergrößerung korrigiert
measure-caveat-magnification = Geschätzte Vergrößerungskorrektur
measure-caveat-uncalibrated = Kein Pixelabstand: in Pixeln
measure-caveat-outside-region = Außerhalb einer kalibrierten Ultraschallregion: in Pixeln

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Orientierungsbeschriftung anzeigen
settings-medical-orientation-labels-help = Buchstaben an den Bildrändern für die Richtungen am Patienten (R/L rechts/links, A/P anterior/posterior, S/I Kopf/Fuß), aus der Datei übernommen und mit Drehung und Spiegelung mitgeführt. L schaltet sie für die Sitzung um.
settings-medical-wheel-steps-frames = Mausrad blättert durch Einzelbilder
settings-medical-wheel-steps-frames-help = In einer Datei oder Serie mit mehreren Einzelbildern wechselt das Mausrad zwischen den Bildern, statt zu zoomen oder die Datei zu wechseln. Strg+Mausrad zoomt weiterhin.
# machine-drafted, needs native review
settings-medical-display-function = Graustufendarstellung
settings-medical-display-function-help = Wie die Graustufen medizinischer Bilder dargestellt werden. DICOM GSDF (Graustufen-Standardanzeigefunktion) verteilt sie in Schritten gleicher Wahrnehmbarkeit zwischen der Schwarz- und der Weißleuchtdichte des Bildschirms (DICOM PS3.14), wie auf einem kalibrierten Befundungsmonitor. Das Farbprofil des Bildschirms gilt weiterhin.
settings-medical-display-function-srgb = Standard (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Schwarzleuchtdichte des Bildschirms
settings-medical-display-black-help = Die Leuchtdichte des Schwarz des Bildschirms einschließlich Umgebungslicht, gemessen oder aus dem Datenblatt des Bildschirms. Wird von DICOM GSDF verwendet.
settings-medical-display-white = Weißleuchtdichte des Bildschirms
settings-medical-display-white-help = Die Leuchtdichte des Weiß des Bildschirms, gemessen oder aus dem Datenblatt des Bildschirms. Wird von DICOM GSDF verwendet.
palette-orientation-labels = Orientierungsbeschriftung ein- oder ausblenden
palette-presentation-state = Nächster Präsentationszustand
disabled-no-presentation-state = Kein Präsentationszustand verweist auf dieses Bild
medical-labels-shown = Orientierungsbeschriftung eingeblendet
medical-labels-hidden = Orientierungsbeschriftung ausgeblendet
medical-presentation = Präsentationszustand: { $label } ({ $index } von { $count })
medical-presentation-none = Präsentationszustand aus
medical-presentation-readout = Präsentation: { $label }
medical-preset-presentation = Die Lookup-Tabelle des Präsentationszustands bestimmt die Graustufen

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Ausrichtung: links { $left }, rechts { $right }, oben { $top }, unten { $bottom }
