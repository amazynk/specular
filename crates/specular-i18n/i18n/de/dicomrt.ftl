# DICOM-Befundberichte (SR), Kurven, RT-Dosis und ophthalmologische Tomographie. machine-drafted, needs native review

## Inhaltsbaum des strukturierten Befunds (der Tag-Browser im Befundmodus)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Eintrag
sr-col-concept = Konzept
sr-col-type = Typ
sr-col-relationship = Beziehung
sr-col-value = Wert
sr-show-image = Bild anzeigen
sr-jump-missing = Das referenzierte Bild liegt nicht in diesem Ordner
sr-jump-frame = Kein Bild dieses Bezugsrahmens enthält diesen Punkt

## Diagramme (Dosis-Volumen-Histogramme, EKG-Blätter)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Zum Messen über ein Diagramm ziehen
plot-adjust-hint = + und − ändern die Verstärkung, [ und ] die Geschwindigkeit

## EKG-Blatt
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Eine weitere Kanalgruppe wird nicht angezeigt
   *[other] { $count } weitere Kanalgruppen werden nicht angezeigt
}

## RT-Dosis über der MPR (Isodosenlinien, Farbkarte, Legende, Messwert)
dose-value = Dosis { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dosis ausgeblendet (Shift+U blendet sie ein)
dose-on-rx = Dosis: { $dose } { $unit } verschrieben · max. { $max } { $unit }
dose-on-max = Dosis: Prozent des Maximums, { $max } { $unit }
dose-colour = Dosis in { $colormap } mit { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: das Dosis-Volumen-Histogramm einer Struktur
   *[other] Shift+K: die Dosis-Volumen-Histogramme von { $count } Strukturen
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dosis als Isodosenlinien
dose-mode-colour = Dosis als Farbkarte

## Dosis-Volumen-Histogramme (das Diagrammblatt)
dvh-title = Dosis-Volumen-Histogramme
dvh-col-roi = Struktur
dvh-col-volume = Volumen
dvh-axis-dose = Dosis ({ $unit })
dvh-axis-volume = Volumen (%)
dvh-prescription = Verschreibung { $dose } Gy · kumulativ
dvh-max = Maximaldosis { $dose } { $unit } · kumulativ
dvh-none = Kein Strukturset dieses Bezugsrahmens, über das die Dosis als Histogramm ausgewertet werden kann

## Ophthalmologische Tomographie (der B-Scan-Schieberegler; das Übersichtsbild in einer Ecke)
oct-bscan = B-Scan
oct-depth = { $depth } µm tief
