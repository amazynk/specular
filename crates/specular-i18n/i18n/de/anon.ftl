# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

palette-export-anonymised = Anonymisierte Kopie exportieren…
palette-export-patient-disc = Patienten-CD-Ordner exportieren…
palette-mask-burned-in-text = Eingebrannten Text maskieren
anon-title = Anonymisierte Kopie exportieren
anon-disc-title = Patienten-CD-Ordner exportieren
anon-images = Bilder
anon-images-current = Dieses Bild
anon-images-series = { $count ->
    [one] Diese Serie ({ $count } Datei)
   *[other] Diese Serie ({ $count } Dateien)
}
anon-images-study = { $count ->
    [one] Diese Studie ({ $count } Datei)
   *[other] Diese Studie ({ $count } Dateien)
}
anon-images-reading = Ordner wird gelesen…
anon-anonymise = Anonymisieren
anon-dates = Datum und Uhrzeit
anon-dates-remove = Entfernen
anon-dates-keep = Behalten
anon-dates-shift = Verschieben
anon-patient = Patientenmerkmale
anon-device = Geräteidentität
anon-institution = Identität der Einrichtung
anon-descriptions = Beschreibungen
anon-descriptions-remove = Entfernen
anon-descriptions-clean = Bereinigen
anon-structured = Befundinhalt
anon-uids = UIDs
anon-uids-replace = Ersetzen
anon-uids-keep = Behalten
anon-overlays = Overlays und Kurven
anon-keep = Behalten
anon-remove = Entfernen
anon-on = Ein
anon-off = Aus
anon-burned-in = Eingebrannter Text
anon-masks = { $count ->
    [one] { $count } Bereich maskiert
   *[other] { $count } Bereiche maskiert
}
anon-masks-none = Keine Masken
anon-folder = Ordner
anon-folder-choose = Auswählen…
anon-export = Exportieren
anon-risk-present = Diese Bilder enthalten laut Angabe Patiententext: vor dem Export maskieren.
anon-risk-possible = Bilder dieser Art zeigen oft Patiententext: vor dem Export prüfen.
anon-progress = Exportiere { $done } von { $total }…
anon-stopping = Wird angehalten…
anon-mask-title = Eingebrannten Text maskieren
anon-mask-hint = Ziehen Sie über Text, um ihn zu maskieren. Maskierte Bereiche werden in jedem Frame schwarz.
anon-mask-suggest = Vorschlagen
anon-mask-remove = Entfernen
anon-mask-clear = Leeren
anon-mask-done = Fertig
anon-mask-area = Bereich { $n }, { $width } × { $height } Pixel
anon-mask-suggested = { $count ->
    [0] Kein Text nahe den Rändern gefunden
    [one] { $count } Textbereich vorgeschlagen
   *[other] { $count } Textbereiche vorgeschlagen
}
anon-done = { $count ->
    [one] { $count } anonymisierte Datei in { $folder } geschrieben
   *[other] { $count } anonymisierte Dateien in { $folder } geschrieben
}
anon-disc-done = { $count ->
    [one] Patienten-CD-Ordner mit { $count } Bild in { $folder } geschrieben
   *[other] Patienten-CD-Ordner mit { $count } Bildern in { $folder } geschrieben
}
anon-skipped-burned-in = { $count ->
    [one] { $count } Datei mit eingebranntem Text ohne Masken wurde übersprungen
   *[other] { $count } Dateien mit eingebranntem Text ohne Masken wurden übersprungen
}
anon-skipped-failed = { $count ->
    [one] { $count } Datei konnte nicht exportiert werden
   *[other] { $count } Dateien konnten nicht exportiert werden
}
anon-stopped = Export angehalten
anon-failed = Export fehlgeschlagen: { $reason }
anon-not-empty = Der Ordner enthält bereits eine Patienten-CD
