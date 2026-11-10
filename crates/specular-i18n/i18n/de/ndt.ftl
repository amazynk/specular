# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Prüfen
ndt-probe = Prüfwerkzeug: { $name }
ndt-probe-profile = Linienprofil
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Doppeldraht-BPK
ndt-probe-wire-iqi = Einzeldraht-BPK
ndt-probe-crack = Rissbreite
ndt-probe-density = Dichtepunkt

ndt-hint-profile = Eine Linie ziehen, um die Grauwerte entlang der Linie darzustellen. [ und ] wechseln das Prüfwerkzeug.
ndt-hint-snr = Einen Bereich in einer gleichmäßigen Fläche aufziehen. Er wird auf mindestens 20 × 55 Pixel vergrößert.
ndt-hint-duplex-iqi = Eine Linie über alle Drahtpaare ziehen, entlang des BPK.
ndt-hint-wire-iqi = Eine kurze Linie quer über den dünnsten sichtbaren Draht ziehen.
ndt-hint-crack = Entlang des Risses klicken, dann Eingabe drücken. Die Breite wird an 10 Stellen gemessen.
ndt-hint-density = Auf einen Punkt klicken, um seinen Grauwert abzulesen. Seine bekannte Dichte eingeben, um zu kalibrieren.

ndt-refused-needs-scale = Legen Sie zuerst den Maßstab fest: Drahtgrößen brauchen Millimeter. Verwenden Sie Kalibrieren an einer bekannten Länge.
ndt-refused-nothing-found = Entlang dieser Linie hebt sich nichts vom Rauschen ab.
ndt-refused-too-small = Das Bild ist kleiner als der SNR-Bereich von 20 × 55 Pixeln.
ndt-refused-no-noise = Dieser Bereich hat kein messbares Rauschen (alle Pixel sind gleich).

ndt-reading = Wird gelesen…
ndt-no-pixels = Die Pixel konnten nicht gelesen werden.

ndt-profile-range = Grauwert { $min } bis { $max }
ndt-profile-mean = Mittlerer Grauwert { $mean }
ndt-profile-density = Dichte { $min } bis { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn braucht die SRb: Messen Sie einen Doppeldraht-BPK oder geben Sie den Wert ein.
ndt-snr-stats = Mittelwert { $mean }, σ { $sd }, Bereich { $w } × { $h } px

ndt-duplex-result = Erstes nicht aufgelöstes Paar { $pair }: SRb { $srb } µm, Unschärfe { $unsharpness }
ndt-duplex-all-resolved = Alle gefundenen Paare sind aufgelöst.
ndt-duplex-dips = Einsattelungen: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpoliert: iSRb { $isrb } µm, Unschärfe { $unsharpness }

ndt-wire-result = Draht { $wire }, { $diameter }
ndt-wire-measured = Gemessene Breite { $width } (FWHM), etwa { $diameter } Durchmesser

ndt-crack-mean = Mittlere Breite { $width }
ndt-crack-range = Schmalste Stelle { $min }, breiteste { $max }
ndt-crack-cuts = Messstellen: { $count }

ndt-density-value = Grauwert { $grey }, Dichte { $density }
ndt-density-grey = Grauwert { $grey }
ndt-density-uncalibrated = Noch keine Dichtekalibrierung: Geben Sie an zwei Punkten bekannte Dichten ein.

ndt-ask-density = Bekannte Dichte hier: { $value } (Eingabe übernimmt, Eingabe ohne Wert überspringt)
ndt-ask-srb = Basisortsauflösung SRb: { $value } µm (Eingabe übernimmt, Esc überspringt)
ndt-bad-density = Geben Sie eine Dichte von 0 bis 10 ein.
ndt-bad-srb = Geben Sie die SRb in Mikrometern ein, zum Beispiel 80.
ndt-srb-set = SRb für dieses Bild auf { $srb } µm gesetzt.
ndt-density-one-more = Dichtepunkt übernommen. Geben Sie die Dichte an einem weiteren Punkt ein.
ndt-density-calibrated = Dichte aus zwei Punkten kalibriert.
ndt-sidecar-failed = Die Prüfeinstellungen konnten nicht neben der Datei gespeichert werden: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m über dem Startpunkt
map-export-kml = Fotos als KMZ oder KML speichern…
map-export-kmz-full = Fotos als KMZ mit vollständigen Fotos speichern…
map-kml-heading = Ausrichtung { $degrees }°
map-kml-height = { $metres } m über dem Startpunkt
map-kml-nothing = Keine Fotos mit Position zum Speichern.
map-kml-saved = In { $name } gespeicherte Fotos: { $count }.
map-kml-failed = Die Kartendatei konnte nicht gespeichert werden: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Prüfbericht
inspection-report-default-title = Prüfbericht
inspection-report-folder = Die anderen Fotos in diesem Ordner hinzufügen
inspection-report-docx = Zusätzlich ein Word-Dokument (DOCX) speichern
inspection-report-kmz = Zusätzlich eine KMZ der Aufnahmeorte speichern
inspection-report-caption = Foto mit den Messungen
inspection-report-col-value = Wert
inspection-report-col-scale = Maßstab aus
inspection-report-docx-failed = Das Word-Dokument konnte nicht geschrieben werden.
inspection-report-unreadable = Keines der Fotos konnte gelesen werden.
