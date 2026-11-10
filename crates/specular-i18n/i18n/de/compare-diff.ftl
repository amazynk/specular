# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Vergleichen mit…
palette-compare-with-help = Dieses Bild mit einem anderen Ihrer Wahl vergleichen.
palette-compare-side = Nebeneinander vergleichen
palette-compare-side-help = Die verglichenen Bilder nebeneinander zeigen.
palette-compare-split = Mit Trennlinie vergleichen
palette-compare-split-help = Das zweite Bild über dem ersten zeigen, geteilt an einer verschiebbaren Trennlinie.
palette-compare-split-direction = Trennlinie drehen
palette-compare-split-direction-help = Die Trennlinie waagerecht statt senkrecht legen, oder zurück.
palette-compare-overlay = Als Überlagerung vergleichen
palette-compare-overlay-help = Das zweite Bild über dem ersten zeigen, überblendet.
palette-compare-difference = Differenz vergleichen
palette-compare-difference-help = Zeigen, wo sich die Bilder unterscheiden; erneut für die nächste Art.
palette-compare-flicker = Zwischen verglichenen Bildern flackern
palette-compare-flicker-help = Sofort das andere Bild zeigen.
palette-compare-auto-flicker = Automatisch flackern
palette-compare-auto-flicker-help = Selbsttätig zwischen den beiden Bildern wechseln, oder anhalten.
palette-compare-align = Verglichene Bilder ausrichten
palette-compare-align-help = Bilder verschiedener Größe ausrichten: oben links, zentriert, skaliert oder automatisch gefunden.
palette-compare-metric-space = Messraum wechseln
palette-compare-metric-space-help = In linearem Licht oder in kodiertem sRGB messen.
palette-compare-whole-image = Ganzes Bild messen
palette-compare-whole-image-help = Das ganze Bild statt des sichtbaren Ausschnitts messen, oder zurück.
palette-compare-probe = Pixelsonde im Vergleich
palette-compare-probe-help = Die Werte beider Bilder unter dem Zeiger zeigen.
palette-compare-link = Verglichene Bilder koppeln oder entkoppeln
palette-compare-link-help = Zwei Bilder einzeln zoomen und verschieben, oder wieder gemeinsam.
palette-compare-close = Vergleich schließen
palette-compare-close-help = Zurück zur Galerie.
disabled-compare-closed = Öffnen Sie zuerst den Vergleich
disabled-compare-not-two = Funktioniert mit zwei Bildern

## Cheat sheet
cheatsheet-compare-split = Trennlinie oder nebeneinander
cheatsheet-compare-split-direction = Trennlinie waagerecht oder senkrecht
cheatsheet-compare-auto-flicker = Automatisch flackern
cheatsheet-compare-align = Nächste Ausrichtung
cheatsheet-compare-space = Messraum
cheatsheet-compare-whole = Ganzes Bild oder Ausschnitt
cheatsheet-compare-probe = Pixelsonde
cheatsheet-compare-link = Zwei Bilder koppeln oder entkoppeln

## Settings
settings-gallery-compare-metric-space = Messraum beim Vergleich
settings-gallery-compare-metric-space-help = Worin der Vergleich MAE, MSE, PSNR und SSIM misst. Lineares Licht entspricht der Anzeige. sRGB stimmt bei 8-bit-Dateien mit anderen Werkzeugen überein.
settings-gallery-compare-metric-space-linear = Lineares Licht
settings-gallery-compare-metric-space-srgb = Kodiertes sRGB
settings-gallery-compare-flicker-limit = Schnellstes automatisches Flackern
settings-gallery-compare-flicker-limit-help = Höchstens so viele Bildwechsel pro Sekunde. Über 3 kann Flackern Anfälle auslösen.

## Toolbar and captions
compare-split = Trennlinie
compare-split-down = Trennlinie senkrecht
compare-split-across = Trennlinie waagerecht
compare-link = Gekoppelt
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } Wechsel pro Sekunde
compare-gain = Verstärkung ×{ $gain }
compare-probe = Sonde
compare-kind-signed = Vorzeichenbehafteter Fehler
compare-kind-on-image = Fehler über dem Bild
compare-align-top-left = Oben links
compare-align-centre = Zentriert
compare-align-scaled = B auf A skaliert
compare-align-auto = Automatisch ausrichten
compare-space-linear = Lineares Licht
compare-space-srgb = sRGB
compare-space-help-linear = In linearem Licht gemessen, wie nach dem Tone Mapping angezeigt. PSNR ist hier nicht das übliche 8-bit-sRGB-PSNR.
compare-space-help-srgb = An kodierten sRGB-Werten gemessen, wie andere Werkzeuge es bei 8-bit-Dateien tun. HDR über SDR-Weiß zählt als Weiß.
compare-space-help-common = ΔE2000 ist immer CIELAB (D65), SDR-Weiß als L* 100, HDR beschnitten. Bilder über { $size } Pixel werden an einer Kopie mit { $size } Pixeln gemessen.
compare-region-view = Ausschnitt
compare-region-whole = Ganzes Bild
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM braucht 11 Pixel
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, max. { $max })
compare-delta-e-hdr = HDR für ΔE beschnitten
compare-aligned-centre = Zentriert, { $width } × { $height } verglichen
compare-aligned-scaled = B { $scale }× skaliert, { $width } × { $height } verglichen
compare-aligned-auto = Ausgerichtet { $angle }° { $scale }×, { $width } × { $height } verglichen
compare-aligning = Wird ausgerichtet…
compare-align-failed = Diese Bilder ließen sich nicht ausrichten. Zurück zu oben links.
compare-split-label = { $b } über { $a }, Trennlinie bei { PERCENT($percent) }
compare-showing-which = Zeigt { $which }: { $name }
compare-auto-flicker-on = Flackert, { $rate } Wechsel pro Sekunde
compare-auto-flicker-off = Flackern angehalten
compare-linked = Bilder gekoppelt
compare-unlinked = Bilder entkoppelt: jedes wird einzeln verschoben
compare-with-title = Vergleichen mit
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Trennlinie
compare-a11y-space = Messraum
compare-a11y-align = Ausrichtung
