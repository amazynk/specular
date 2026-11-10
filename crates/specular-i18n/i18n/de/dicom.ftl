# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = DICOM-Tags anzeigen
palette-next-series = Nächste Serie
palette-prev-series = Vorherige Serie
disabled-not-dicom = Das Bild ist keine DICOM-Datei
disabled-no-other-series = In diesem Ordner gibt es keine weitere Serie
dicom-tags-title = DICOM-Tags
dicom-tags-title-file = DICOM-Tags: { $file }
dicom-tags-filter = Tags filtern
dicom-tags-show-patient = Patientendaten anzeigen
dicom-tags-col-tag = Tag
dicom-tags-col-keyword = Schlüsselwort
dicom-tags-col-vr = VR
dicom-tags-col-length = Länge
dicom-tags-col-value = Wert
dicom-tags-hidden = Ausgeblendet
dicom-tags-hidden-count = { $count ->
    [one] { $count } Patientenwert ausgeblendet
   *[other] { $count } Patientenwerte ausgeblendet
}
dicom-tags-no-match = Keine passenden Tags
dicom-tags-reading = Tags werden gelesen…
dicom-tags-error = Die Tags konnten nicht gelesen werden: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } Element
   *[other] { $count } Elemente
}
dicom-tags-fragments = { $count ->
    [one] { $count } Fragment
   *[other] { $count } Fragmente
}
dicom-tags-item = Element { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } Byte
   *[other] { $count } Byte
}
dicom-tags-empty = (leer)
dicom-tags-omitted = { $count ->
    [one] { $count } weiteres Element nicht aufgeführt
   *[other] { $count } weitere Elemente nicht aufgeführt
}
dicom-series-group = Serie
dicom-series-slice = Schicht
dicom-series-slice-value = { $index } von { $count }
dicom-series-frames = { $frames ->
        [one] { $frames } Einzelbild
       *[other] { $frames } Einzelbilder
    }
dicom-series-location = Schichtposition
dicom-series-mm = { $mm } mm
dicom-series-spacing = Schichtabstand
dicom-series-which = Serie
dicom-series-which-value = { $index } von { $count }
dicom-series-order = Reihenfolge
dicom-series-order-position = Nach Position
dicom-series-order-instance = Nach Instanznummer
dicom-series-order-time = Nach Aufnahmezeit
dicom-series-order-name = Nach Dateiname
dicom-series-gaps = Fehlende Schichten
dicom-series-gaps-value = { $count ->
    [one] { $count } Schicht fehlt
   *[other] { $count } Schichten fehlen
}
dicom-series-gaps-where = Nach Schicht { $slices }
dicom-series-irregular = Abstand ist unregelmäßig
dicom-series-tilt = Gantry-Neigung
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Duplikate übersprungen
dicom-series-toast = { $series ->
    [one] Serie – Schichten: { $count }
   *[other] Serie { $index } von { $series } – Schichten: { $count }
}
dicom-series-capped = Nur die ersten { $count } Dateien des Ordners wurden gelesen

studies-title = Studien
studies-title-medium = Studien: { $medium }
studies-show-patient = Patientendaten anzeigen
studies-patient-hidden = Ausgeblendet
studies-patient-hidden-note = Patientennamen und Geburtsdaten sind ausgeblendet
studies-none = Keine Studien gefunden
studies-reading = Datenträger wird gelesen…
studies-reading-count = Datenträger wird gelesen: { $done } von { $total ->
        [one] { $total } Datei
       *[other] { $total } Dateien
    }
studies-opening = Serie wird geöffnet…
studies-error = Datenträger konnte nicht gelesen werden: { $reason }
studies-missing = { $count ->
    [one] { $count } auf dem Datenträger verzeichnete Datei fehlt
   *[other] { $count } auf dem Datenträger verzeichnete Dateien fehlen
}
studies-capped = Nur die ersten { $count } Dateien wurden gelesen
# machine-drafted, needs native review
studies-stopped = Lesen nach { $done } von { $total ->
        [one] { $total } Datei
       *[other] { $total } Dateien
    } angehalten: Die Liste ist unvollständig
studies-stopped-early = Lesen angehalten: Die Liste ist unvollständig
studies-cancel-description = Beendet das Lesen des Datenträgers. Was bisher gelesen wurde, bleibt aufgeführt.
studies-patient-n = Patient { $index }
studies-patient-unknown = Unbekannter Patient
studies-born = Geboren { $date }
studies-no-description = Keine Beschreibung
studies-series = Serie { $number }: { $description }
studies-series-number = Serie { $number }
studies-images = { $count ->
    [one] { $count } Bild
   *[other] { $count } Bilder
}
studies-images-missing = { $count } fehlen
studies-others = { $count ->
    [one] { $count } weiteres Objekt
   *[other] { $count } weitere Objekte
}
studies-no-images = Diese Serie hat keine Bilder auf dem Datenträger
studies-medium-inserted = Ein Datenträger mit medizinischen Studien wurde eingelegt
studies-medium-show = Studien anzeigen
palette-studies = Studien auf diesem Datenträger anzeigen
