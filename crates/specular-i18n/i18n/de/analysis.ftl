# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Deutsch. machine-drafted, needs native review

palette-segments = Segmentierungen ein-/ausblenden
palette-segment-opacity = Nächste Füllungsdeckkraft der Segmentierung
palette-fusion = Serien desselben Bezugsrahmens fusionieren
palette-fusion-colormap = Nächste Farbskala der Fusion
palette-fusion-blend = Nächste Fusionsdeckkraft
palette-fusion-nudge = Fusionierte Serie von Hand verschieben
palette-tmtv = TMTV-Werkzeug (Kugel, Quader, aus)
palette-tmtv-threshold = TMTV-Schwelle (SUV { NUMBER(2.5) } oder { PERCENT(41) } von SUVmax)
palette-time-curve = Zeit-Intensitäts-Kurve der letzten Region
palette-export-curve = Zeit-Intensitäts-Kurve als CSV speichern

analysis-searching = Suche noch nach Segmentierungen und Serien zum Fusionieren
segments-none = Keine Segmentierung dieses Bezugsrahmens gefunden
segments-shown = Segmentierungen eingeblendet
segments-hidden = Segmentierungen ausgeblendet
segments-fill = Segmentierungsfüllung { PERCENT($percent) }
segments-failed = Segmentierung nicht angezeigt: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = SUVbw-Faktor { $factor }, { $minutes } min nach Injektion
suv-refused = Kein SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = kein PET-Bild
suv-refused-units = die Pixeleinheit ist nicht Bq/mL
suv-refused-corrected = nicht schwächungs- und zerfallskorrigiert
suv-refused-decay = nicht unterstützte Zerfallskorrektur
suv-refused-weight = kein Patientengewicht
suv-refused-dose = keine injizierte Dosis
suv-refused-half-life = keine Halbwertszeit des Radionuklids
suv-refused-injection = keine Injektionszeit
suv-refused-scan-time = keine Aufnahmezeit
suv-refused-delay = die Injektion liegt nach der Aufnahme
suv-refused-scale = kein Philips-SUV-Skalierungsfaktor
suv-refused-height = keine Patientengröße
suv-refused-sex = kein Patientengeschlecht
suv-refused-lbm = fettfreie Körpermasse außerhalb des Bereichs

colormap-hot-iron = Hot Iron
colormap-rainbow = Regenbogen
colormap-viridis = Viridis
fusion-none = Keine PET-, SPECT-, CT- oder MR-Serie dieses Bezugsrahmens zum Fusionieren
fusion-needed = Zuerst eine Serie fusionieren (Shift+U)
fusion-opening = { $modality } wird fusioniert
fusion-off = Fusion aus
fusion-refused = Fusion nicht möglich: { $reason }
fusion-on = { $modality } fusioniert: { $colormap }, { PERCENT($blend) }
fusion-hidden = Fusioniertes { $modality } ausgeblendet
fusion-window-suv = Fusionsfenster SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Fusionsdeckkraft { PERCENT($percent) }
fusion-moved = Von Hand verschoben: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Pfeiltasten verschieben die fusionierte Serie um 1 mm, Shift+Up/Down entlang der Ansicht, [ und ] drehen sie
fusion-nudge-off = Fusionierte Serie fixiert

tmtv-hint-sphere = TMTV: Kugel vom Mittelpunkt aus ziehen
tmtv-hint-box = TMTV: Quader ziehen
tmtv-off = TMTV aus
tmtv-threshold-suv = Schwelle SUV { $suv }
tmtv-threshold-share = Schwelle { PERCENT($share) } von SUVmax
tmtv-result = TMTV { $ml } mL in { $count ->
        [one] { $count } Region
       *[other] { $count } Regionen
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = Die Region liegt außerhalb des Volumens
tmtv-no-pet = TMTV braucht ein PET-Volumen mit SUV

analysis-mpr-only = Fusion und TMTV funktionieren in der MPR (Shift+R)
curve-not-dynamic = Kein Mehrbild und keine dynamische Serie
curve-not-mpr = Zeit-Intensitäts-Kurven entstehen auf dem Bild, außerhalb der MPR
curve-no-roi = Zuerst eine Ellipse oder ein Rechteck zeichnen
curve-loading = { $count ->
        [one] { $count } Zeitpunkt wird gelesen…
       *[other] { $count } Zeitpunkte werden gelesen…
    }
curve-title = Zeit-Intensitäts-Kurve
curve-mean = Mittelwert
curve-max = Max.
curve-seconds = s
curve-saved = Kurve gespeichert
curve-none = Keine Kurve zum Speichern
curve-failed = Kurve nicht erstellt: { $reason }
