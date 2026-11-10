# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Deutsch.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Ein Modul dieser Bearbeitung wurde auf eine neuere Version aktualisiert. Das Ergebnis kann etwas anders aussehen.
   *[other] { $count } Module dieser Bearbeitung wurden auf eine neuere Version aktualisiert. Das Ergebnis kann etwas anders aussehen.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Ein Modul dieser Bearbeitung erfordert eine neuere Version von Specular. Es bleibt erhalten, wird aber nicht angezeigt.
   *[other] { $count } Module dieser Bearbeitung erfordern eine neuere Version von Specular. Sie bleiben erhalten, werden aber nicht angezeigt.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Punkt { $index } von { $count }: Eingabe { $input }, Ausgabe { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Pfeiltasten bewegen den Punkt. Bild auf und Bild ab wählen einen anderen. Einfg fügt einen Punkt hinzu, Entf entfernt ihn.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutral
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Farbton { $hue }°, Stärke { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Pfeiltasten verschieben die Farbe. Entf setzt sie zurück.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Entwickeln
# machine-drafted, needs native review
develop-undo = Rückgängig
# machine-drafted, needs native review
develop-redo = Wiederholen
# machine-drafted, needs native review
develop-before-after = Vorher und nachher
# machine-drafted, needs native review
develop-close = Entwickeln verlassen
# machine-drafted, needs native review
develop-history = Protokoll
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } ein
develop-card-menu = Optionen für { $module }
develop-card-changed = Geändert
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Filmstreifen

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Belichtung
develop-module-light = Licht
develop-look-profile = Profil
develop-look-standard = Standard
develop-look-film = Film
develop-look-natural = Natürlich
develop-module-grading = Color Grading
develop-add-tool = Werkzeug hinzufügen
# machine-drafted, needs native review
develop-module-white-balance = Weißabgleich
# machine-drafted, needs native review
develop-module-tone = Tonwerte
# machine-drafted, needs native review
develop-module-contrast = Kontrast und Umfang
# machine-drafted, needs native review
develop-module-colour = Farbe
# machine-drafted, needs native review
develop-module-curve = Kurve
# machine-drafted, needs native review
develop-module-crop = Freistellen und gerade ausrichten
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Anderes Modul
# machine-drafted, needs native review
develop-exposure = Belichtung
# machine-drafted, needs native review
develop-black-level = Schwarzwert
# machine-drafted, needs native review
develop-temperature = Temperatur
# machine-drafted, needs native review
develop-tint = Tönung
# machine-drafted, needs native review
develop-highlights = Lichter
# machine-drafted, needs native review
develop-shadows = Tiefen
# machine-drafted, needs native review
develop-contrast = Kontrast
# machine-drafted, needs native review
develop-whites = Weiß
# machine-drafted, needs native review
develop-blacks = Schwarz
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = HDR-Grenze
# machine-drafted, needs native review
develop-hdr-preview = Anzeigen
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Dynamik
# machine-drafted, needs native review
develop-saturation = Sättigung
# machine-drafted, needs native review
develop-colour-wheels = Farbräder
# machine-drafted, needs native review
develop-curve = Gradationskurve
# machine-drafted, needs native review
develop-module-local-contrast = Lokaler Kontrast
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Automatische Belichtung
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Glättung
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Struktur
# machine-drafted, needs native review
develop-clarity = Klarheit
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Schwarzpunkt
# machine-drafted, needs native review
develop-levels-mid = Mittelpunkt
# machine-drafted, needs native review
develop-levels-white = Weißpunkt
# machine-drafted, needs native review
develop-module-detail = Details
# machine-drafted, needs native review
develop-module-denoise = Rauschreduzierung
# machine-drafted, needs native review
develop-module-sharpen = Schärfen
# machine-drafted, needs native review
develop-noise-luminance = Luminanzrauschen
# machine-drafted, needs native review
develop-noise-detail = Rauschdetails
# machine-drafted, needs native review
develop-noise-colour = Farbrauschen
# machine-drafted, needs native review
develop-sharpen-amount = Schärfestärke
# machine-drafted, needs native review
develop-sharpen-radius = Schärferadius
# machine-drafted, needs native review
develop-sharpen-detail = Schärfedetails
# machine-drafted, needs native review
develop-sharpen-masking = Maskieren
# machine-drafted, needs native review
develop-capture-amount = Aufnahmeschärfung
# machine-drafted, needs native review
develop-capture-radius = Aufnahmeradius
# machine-drafted, needs native review
develop-straighten = Gerade ausrichten
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Freistellen
# machine-drafted, needs native review
develop-crop-turn = Drehen
# machine-drafted, needs native review
develop-crop-flip = Spiegeln
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Objektivkorrekturen
develop-module-optics = Optik
# machine-drafted, needs native review
develop-lens-distortion = Verzeichnung
# machine-drafted, needs native review
develop-lens-vignette = Vignettierung
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Roter Farbsaum
# machine-drafted, needs native review
develop-lens-tca-blue = Blauer Farbsaum
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Objektivprofil
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automatisch
# machine-drafted, needs native review
develop-lensfun-off = Keines
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Kein Objektivprofil.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Kein Lensfun-Profil für { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Kein Lensfun-Profil für dieses Objektiv.
# machine-drafted, needs native review
develop-lensfun-no-name = Die Datei nennt das Objektiv nicht. Wählen Sie es oben aus.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } Objektive passen. Wählen Sie oben eines aus.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Freistellen. Ziehen Sie die Kanten, oder ziehen Sie mit Strg entlang einer Linie, die waagerecht sein soll. Eingabe übernimmt, Esc geht zurück.
# machine-drafted, needs native review
develop-crop-kept = Freistellung übernommen
# machine-drafted, needs native review
develop-crop-cancelled = Freistellung verworfen
# machine-drafted, needs native review
develop-crop-reset = Freistellen und Ausrichten zurückgesetzt
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Ausgerichtet auf { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Seitenverhältnis { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = frei
# machine-drafted, needs native review
develop-crop-aspect-original = Original
# machine-drafted, needs native review
develop-crop-turned-right = Nach rechts gedreht
# machine-drafted, needs native review
develop-crop-turned-left = Nach links gedreht
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Horizontal gespiegelt
# machine-drafted, needs native review
develop-crop-flipped-vertical = Vertikal gespiegelt
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Seitenverhältnis { $aspect } · Hilfslinien: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A Seitenverhältnis · X tauschen · O Hilfslinien · [ ] drehen · H V spiegeln · Eingabe übernehmen · Esc zurück
# machine-drafted, needs native review
develop-crop-guide-thirds = Drittelregel
# machine-drafted, needs native review
develop-crop-guide-golden = Goldener Schnitt
# machine-drafted, needs native review
develop-crop-guide-diagonal = Diagonalen
# machine-drafted, needs native review
develop-crop-guide-grid = Raster

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspektive
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Aufrichten
# machine-drafted, needs native review
develop-persp-mode-off = Aus
# machine-drafted, needs native review
develop-persp-mode-auto = Automatisch
# machine-drafted, needs native review
develop-persp-mode-level = Horizont
# machine-drafted, needs native review
develop-persp-mode-vertical = Vertikal
# machine-drafted, needs native review
develop-persp-mode-full = Vollständig
# machine-drafted, needs native review
develop-persp-mode-guided = Geführt
# machine-drafted, needs native review
develop-persp-vertical = Senkrecht
# machine-drafted, needs native review
develop-persp-horizontal = Waagerecht
# machine-drafted, needs native review
develop-persp-rotate = Drehen
# machine-drafted, needs native review
develop-persp-aspect = Seitenverhältnis
# machine-drafted, needs native review
develop-persp-scale = Skalieren
# machine-drafted, needs native review
develop-persp-offset-x = Seitlich
# machine-drafted, needs native review
develop-persp-offset-y = Hoch oder runter
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Leere Ränder vermeiden
# machine-drafted, needs native review
develop-persp-guides = Hilfslinien
# machine-drafted, needs native review
develop-persp-focal = Brennweite
# machine-drafted, needs native review
develop-persp-draw-guides = Hilfslinien zeichnen
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Hilfslinien zeichnen ({ $n } von { $max })
# machine-drafted, needs native review
develop-persp-reset = Perspektive zurücksetzen
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Aufrichten: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Linien werden gesucht…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Perspektive zurücksetzen? Ihre { $n } Hilfslinien werden entfernt. Rückgängig holt sie zurück.
# machine-drafted, needs native review
develop-persp-reset-ok = Zurücksetzen
# machine-drafted, needs native review
develop-persp-reset-done = Perspektive zurückgesetzt
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Hilfslinie { $n } entfernen? Rückgängig holt sie zurück.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Entfernen
# machine-drafted, needs native review
develop-persp-guide-removed = Hilfslinie entfernt
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Senkrecht { $vertical }, waagerecht { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Aufrichten fehlgeschlagen: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: Neigung { $vertical }°, Schwenk { $horizontal }°, Drehung { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } braucht mindestens zwei gerade Linien, die es unterscheiden kann. Versuchen Sie einen anderen Modus, oder zeichnen Sie Hilfslinien.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Höchstens { $n } Hilfslinien. Ziehen Sie ein Ende, um eine zu verschieben.
# machine-drafted, needs native review
develop-persp-guides-on = Hilfslinien. Ziehen Sie entlang einer Linie, die senkrecht oder waagerecht sein soll.
# machine-drafted, needs native review
develop-persp-guides-off = Freistellen
# machine-drafted, needs native review
develop-persp-guide-moved = Hilfslinie verschoben
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Hilfslinie { $n } hinzugefügt
# machine-drafted, needs native review
develop-persp-guides-keys = Ziehen fügt hinzu · Ende ziehen verschiebt · Entf entfernt · Shift+R Freistellen
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Hilfslinien: { $n } von { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Tiefen
# machine-drafted, needs native review
develop-wheel-midtones = Mitteltöne
# machine-drafted, needs native review
develop-wheel-highlights = Lichter
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminanz Tiefen
# machine-drafted, needs native review
develop-luminance-midtones = Luminanz Mitteltöne
# machine-drafted, needs native review
develop-luminance-highlights = Luminanz Lichter
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Global
# machine-drafted, needs native review
develop-luminance-global = Luminanz Global
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Farbintensität
# machine-drafted, needs native review
develop-brilliance = Farbhelligkeit

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Farbmischer
# machine-drafted, needs native review
develop-colour-red = Rot
# machine-drafted, needs native review
develop-colour-orange = Orange
# machine-drafted, needs native review
develop-colour-yellow = Gelb
# machine-drafted, needs native review
develop-colour-green = Grün
# machine-drafted, needs native review
develop-colour-aqua = Aquamarin
# machine-drafted, needs native review
develop-colour-blue = Blau
# machine-drafted, needs native review
develop-colour-purple = Lila
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Farbton { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Sättigung { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminanz { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Punktfarbe
# machine-drafted, needs native review
develop-point-pick = Farbe aufnehmen
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Andere Farbe aufnehmen
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Klicken Sie auf die Farbe, die Sie ändern möchten. Esc bricht ab.
# machine-drafted, needs native review
develop-point-picked = Farbe aufgenommen
# machine-drafted, needs native review
develop-point-cancelled = Aufnehmen abgebrochen
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Bereich
# machine-drafted, needs native review
develop-point-hue-range = Farbtonbereich
# machine-drafted, needs native review
develop-point-saturation-range = Sättigungsbereich
# machine-drafted, needs native review
develop-point-lightness-range = Helligkeitsbereich
# machine-drafted, needs native review
develop-point-hue = Farbtonverschiebung
# machine-drafted, needs native review
develop-point-saturation = Sättigungsverschiebung
# machine-drafted, needs native review
develop-point-lightness = Helligkeitsverschiebung

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Schwarzweiß
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminanz { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = LUT-Datei wählen…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Datei: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = { $name } kann nicht gelesen werden
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = LUT-Datei wählen
# machine-drafted, needs native review
develop-lut-kind = LUT-Dateien
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Erstellt für
# machine-drafted, needs native review
develop-lut-space-linear = Linear (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES linear (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES Log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Foto (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Video (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Methode
# machine-drafted, needs native review
develop-lut-tetrahedral = Genau
# machine-drafted, needs native review
develop-lut-trilinear = Einfach
# machine-drafted, needs native review
develop-lut-strength = Stärke

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Original
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } ein
# machine-drafted, needs native review
develop-history-off = { $module } aus
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Zurück zum Original
# machine-drafted, needs native review
develop-at-step = Zurück zu: { $step }
# machine-drafted, needs native review
develop-undone = Rückgängig gemacht: { $step }
# machine-drafted, needs native review
develop-redone = Wiederholt: { $step }
# machine-drafted, needs native review
develop-entered = Entwickeln. Bearbeitungen ändern die Datei nie.
# machine-drafted, needs native review
develop-left = Entwickeln verlassen
# machine-drafted, needs native review
develop-split-on = Vorher und nachher nebeneinander
# machine-drafted, needs native review
develop-split-off = Nur Bearbeitung
# machine-drafted, needs native review
develop-showing-before = Original wird angezeigt
# machine-drafted, needs native review
develop-showing-after = Bearbeitung wird angezeigt
# machine-drafted, needs native review
# Labels over the image.
develop-before = Vorher
# machine-drafted, needs native review
develop-after = Nachher
# machine-drafted, needs native review
develop-working = Wird entwickelt…
# machine-drafted, needs native review
develop-preview-failed = Die Bearbeitung kann für diese Datei nicht angezeigt werden

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Entwickeln
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Schnellbearbeitung: gedrückt halten, dann scrollen
# machine-drafted, needs native review
palette-develop = Entwickeln
# machine-drafted, needs native review
palette-develop-help = Dieses Foto bearbeiten, ohne die Datei zu ändern. Erneut ausführen zum Verlassen.
# machine-drafted, needs native review
palette-develop-before-after = Vorher und nachher
# machine-drafted, needs native review
palette-develop-before-after-help = Das Original zeigen oder wieder die Bearbeitung.
# machine-drafted, needs native review
palette-develop-split = Vorher und nachher geteilt
# machine-drafted, needs native review
palette-develop-split-help = Original und Bearbeitung nebeneinander zeigen.
# machine-drafted, needs native review
palette-develop-panel = Zum Entwickeln-Bedienfeld
# machine-drafted, needs native review
palette-develop-panel-help = Die Tastatur ins Bedienfeld verlegen. Esc kehrt zurück.
# machine-drafted, needs native review
palette-develop-speed-exposure = Schnellbearbeitung Belichtung
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Gedrückt halten, dann scrollen oder Pfeiltasten drücken, um die Belichtung zu ändern.
# machine-drafted, needs native review
palette-develop-speed-contrast = Schnellbearbeitung Kontrast
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Gedrückt halten, dann scrollen oder Pfeiltasten drücken, um den Kontrast zu ändern.
# machine-drafted, needs native review
palette-develop-speed-highlights = Schnellbearbeitung Lichter
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Gedrückt halten, dann scrollen oder Pfeiltasten drücken, um die Lichter zu ändern.
# machine-drafted, needs native review
palette-develop-speed-shadows = Schnellbearbeitung Tiefen
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Gedrückt halten, dann scrollen oder Pfeiltasten drücken, um die Tiefen zu ändern.
# machine-drafted, needs native review
palette-develop-speed-whites = Schnellbearbeitung Weiß
# machine-drafted, needs native review
palette-develop-speed-whites-help = Gedrückt halten, dann scrollen oder Pfeiltasten drücken, um Weiß zu ändern.
# machine-drafted, needs native review
palette-develop-speed-blacks = Schnellbearbeitung Schwarz
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Gedrückt halten, dann scrollen oder Pfeiltasten drücken, um Schwarz zu ändern.
# machine-drafted, needs native review
palette-develop-speed-temperature = Schnellbearbeitung Temperatur
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Gedrückt halten, dann scrollen oder Pfeiltasten drücken, um die Farbtemperatur des Weißabgleichs zu ändern.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Schnellbearbeitung Dynamik
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Gedrückt halten, dann scrollen oder Pfeiltasten drücken, um die Dynamik zu ändern.
# machine-drafted, needs native review
palette-develop-crop = Freistellen und gerade ausrichten
# machine-drafted, needs native review
palette-develop-crop-help = Foto freistellen, gerade ausrichten, drehen oder spiegeln. Eingabe übernimmt, Esc geht zurück.
# machine-drafted, needs native review
disabled-not-photo = Nur für Fotos
# machine-drafted, needs native review
disabled-develop-off = Entwickeln ist in den Profilen Medizin und Wissenschaft ausgeschaltet
# machine-drafted, needs native review
develop-module-corrections = Korrekturen
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Aus
# machine-drafted, needs native review
develop-on = An
# machine-drafted, needs native review
develop-highlight-method = Ausgefressene Lichter
# machine-drafted, needs native review
develop-highlight-opposed = Farbe rekonstruieren
# machine-drafted, needs native review
develop-highlight-rebuild = Große Flächen rekonstruieren
# machine-drafted, needs native review
develop-highlight-clip = Auf Weiß beschneiden
# machine-drafted, needs native review
develop-highlight-threshold = Clipping-Grenze
# machine-drafted, needs native review
develop-module-hot-pixels = Hotpixel
# machine-drafted, needs native review
develop-hot-threshold = Hotpixel-Schwelle
# machine-drafted, needs native review
develop-module-ca = Chromatische Aberration
# machine-drafted, needs native review
develop-ca-mode = Farbsäume
# machine-drafted, needs native review
develop-ca-auto = Automatisch
# machine-drafted, needs native review
develop-ca-manual = Manuell
# machine-drafted, needs native review
develop-ca-red = Rot-Verschiebung
# machine-drafted, needs native review
develop-ca-blue = Blau-Verschiebung
# machine-drafted, needs native review
develop-module-defringe = Farbsäume entfernen
# machine-drafted, needs native review
develop-defringe = Farbsäume entfernen
# machine-drafted, needs native review
develop-defringe-purple = Violett-Farbton
# machine-drafted, needs native review
develop-defringe-green = Grün-Farbton
# machine-drafted, needs native review
develop-defringe-range = Farbtonbereich
# machine-drafted, needs native review
develop-defringe-edge = Kantenkontrast
# machine-drafted, needs native review
develop-module-flat-field = Kalibrierbilder
# machine-drafted, needs native review
develop-calibration = Kalibrierung
# machine-drafted, needs native review
develop-calibration-auto = Aus Ordner
# machine-drafted, needs native review
develop-calibration-chosen = Gewählte Dateien
# machine-drafted, needs native review
develop-flat-choose = Flatfield wählen…
# machine-drafted, needs native review
develop-dark-choose = Darkframe wählen…
# machine-drafted, needs native review
develop-flat-file = Flatfield: { $name }
# machine-drafted, needs native review
develop-dark-file = Darkframe: { $name }
# machine-drafted, needs native review
develop-flat-use = Flatfield
# machine-drafted, needs native review
develop-flat-use-full = Vignettierung und Staub
# machine-drafted, needs native review
develop-flat-use-vignetting = Nur Vignettierung
# machine-drafted, needs native review
develop-flat-dialog = Flatfield wählen
# machine-drafted, needs native review
develop-dark-dialog = Darkframe wählen
# machine-drafted, needs native review
develop-raw-kind = Raw-Dateien
# machine-drafted, needs native review
develop-flat-frame = Flatfield
# machine-drafted, needs native review
develop-dark-frame = Darkframe

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Maske hinzufügen
# machine-drafted, needs native review
develop-mask-kind-ellipse = Ellipse
# machine-drafted, needs native review
develop-mask-kind-rectangle = Rechteck
# machine-drafted, needs native review
develop-mask-kind-polygon = Polygon
# machine-drafted, needs native review
develop-mask-kind-linear = Linearer Verlauf
# machine-drafted, needs native review
develop-mask-kind-radial = Radialer Verlauf
# machine-drafted, needs native review
develop-mask-kind-brush = Pinsel
# machine-drafted, needs native review
develop-mask-kind-luminance = Luminanzbereich
# machine-drafted, needs native review
develop-mask-kind-hue = Farbtonbereich
# machine-drafted, needs native review
develop-mask-kind-chroma = Sättigungsbereich
# machine-drafted, needs native review
develop-mask-kind-colour = Farbbereich
# machine-drafted, needs native review
develop-mask-kind-other = Andere Maske
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Mischen
# machine-drafted, needs native review
develop-blend-normal = Normal
# machine-drafted, needs native review
develop-blend-multiply = Multiplizieren
# machine-drafted, needs native review
develop-blend-screen = Negativ multiplizieren
# machine-drafted, needs native review
develop-blend-overlay = Ineinanderkopieren
# machine-drafted, needs native review
develop-blend-soft-light = Weiches Licht
# machine-drafted, needs native review
develop-blend-lightness = Nur Helligkeit
# machine-drafted, needs native review
develop-blend-colour = Nur Farbe
# machine-drafted, needs native review
develop-mask-amount = Maskenstärke
# machine-drafted, needs native review
develop-mask-invert = Maske umkehren
# machine-drafted, needs native review
develop-mask-off = Aus
# machine-drafted, needs native review
develop-mask-on = Ein
# machine-drafted, needs native review
develop-mask-refine = Kanten verfeinern
# machine-drafted, needs native review
develop-mask-show = Maske zeigen
# machine-drafted, needs native review
develop-mask-show-red = Rot
# machine-drafted, needs native review
develop-mask-show-grey = Grau
# machine-drafted, needs native review
develop-mask-remove = Maske entfernen
# machine-drafted, needs native review
develop-mask-combine-add = Hinzufügen
# machine-drafted, needs native review
develop-mask-combine-subtract = Abziehen
# machine-drafted, needs native review
develop-mask-combine-intersect = Schnittmenge
# machine-drafted, needs native review
develop-mask-row-opacity = Deckkraft
# machine-drafted, needs native review
develop-mask-row-invert = Umkehren
# machine-drafted, needs native review
develop-mask-feather = Weiche Kante
# machine-drafted, needs native review
develop-mask-roundness = Rundung
# machine-drafted, needs native review
develop-mask-brush-size = Pinselgröße
# machine-drafted, needs native review
develop-mask-brush-feather = Pinselkante
# machine-drafted, needs native review
develop-mask-brush-flow = Fluss
# machine-drafted, needs native review
develop-mask-brush-mode = Pinsel
# machine-drafted, needs native review
develop-mask-brush-paint = Malen
# machine-drafted, needs native review
develop-mask-brush-erase = Radieren
# machine-drafted, needs native review
develop-mask-range-from = Von
# machine-drafted, needs native review
develop-mask-range-full-from = Voll ab
# machine-drafted, needs native review
develop-mask-range-full-to = Voll bis
# machine-drafted, needs native review
develop-mask-range-to = Bis
# machine-drafted, needs native review
develop-mask-range-test = Prüfen
# machine-drafted, needs native review
develop-mask-range-input = Eingang
# machine-drafted, needs native review
develop-mask-range-output = Ausgang
# machine-drafted, needs native review
develop-mask-colour-pick = Farbe aufnehmen
# machine-drafted, needs native review
develop-mask-colour-pick-more = Weitere Farbe aufnehmen
# machine-drafted, needs native review
develop-mask-colour-range = Bereich
# machine-drafted, needs native review
develop-mask-colour-falloff = Abfall
# machine-drafted, needs native review
develop-mask-edit = Im Bild bearbeiten
# machine-drafted, needs native review
develop-mask-row-remove = { $name } entfernen
# machine-drafted, needs native review
develop-mask-remove-confirm = Die Maske von { $module } entfernen? Ihre Formen gehen. Das Foto auf der Festplatte bleibt.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = { $name } aus { $module } entfernen? Die anderen Masken und das Foto auf der Festplatte bleiben.
# machine-drafted, needs native review
develop-mask-remove-ok = Entfernen
# machine-drafted, needs native review
develop-mask-removed = Maske entfernt.
# machine-drafted, needs native review
develop-mask-added = { $name } hinzugefügt.
# machine-drafted, needs native review
develop-mask-full = Diese Maske hat so viele Teile, wie sie fassen kann.
# machine-drafted, needs native review
develop-mask-editing = { $name } bearbeiten. Ziehen Sie die Griffe, oder drücken Sie Tab und die Pfeiltasten.
# machine-drafted, needs native review
develop-mask-painting = { $name } malen. Ziehen Sie im Bild.
# machine-drafted, needs native review
develop-mask-picking = Klicken Sie ins Bild, um eine Farbe aufzunehmen.
# machine-drafted, needs native review
develop-mask-picked = Farbe aufgenommen.
# machine-drafted, needs native review
develop-mask-done = Maskenbearbeitung beendet.
# machine-drafted, needs native review
develop-mask-handles = Maskengriffe
# machine-drafted, needs native review
develop-handle-centre = Mitte
# machine-drafted, needs native review
develop-handle-width = Breite
# machine-drafted, needs native review
develop-handle-height = Höhe
# machine-drafted, needs native review
develop-handle-rotate = Drehen
# machine-drafted, needs native review
develop-handle-feather = Weiche Kante
# machine-drafted, needs native review
develop-handle-inner = Innenkante
# machine-drafted, needs native review
develop-handle-start = Anfang
# machine-drafted, needs native review
develop-handle-end = Ende
# machine-drafted, needs native review
develop-handle-point = Punkt { $n }
# machine-drafted, needs native review
develop-instance = Ebene
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Ebene { $n }.
# machine-drafted, needs native review
develop-instance-new = Neue Ebene
# machine-drafted, needs native review
develop-instance-duplicate = Ebene duplizieren
# machine-drafted, needs native review
develop-instance-delete = Ebene löschen
# machine-drafted, needs native review
develop-instance-earlier = Früher
# machine-drafted, needs native review
develop-instance-later = Später
# machine-drafted, needs native review
develop-instance-delete-confirm = Ebene { $n } von { $module } löschen? Ihre Einstellungen und Masken gehen. Das Foto auf der Festplatte bleibt.
# machine-drafted, needs native review
develop-instance-delete-ok = Löschen
# machine-drafted, needs native review
develop-instance-deleted = Ebene gelöscht.
# machine-drafted, needs native review
develop-history-mask = Maske: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Maskenformen: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Neu: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Gelöscht: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Reihenfolge: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Andere Programme
# machine-drafted, needs native review
develop-darktable-edited = Bearbeitet in darktable:
# machine-drafted, needs native review
develop-darktable-open = In darktable öffnen
# machine-drafted, needs native review
develop-darktable-opening = Wird in darktable geöffnet
# machine-drafted, needs native review
develop-darktable-failed = darktable startete nicht: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Lightroom-Bearbeitung übernehmen
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] 1 Lightroom-Einstellung übernommen. Das Ergebnis ist ungefähr.
   *[other] { $count } Lightroom-Einstellungen übernommen. Das Ergebnis ist ungefähr.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Nichts zu übernehmen: Diese Lightroom-Einstellungen ändern hier nichts.
# machine-drafted, needs native review
develop-history-lightroom = Aus Lightroom übernommen (ungefähr)
# machine-drafted, needs native review
develop-conflict-title = Die Bearbeitung von { $file } wurde außerhalb von Specular geändert
# machine-drafted, needs native review
develop-conflict-file = Begleitdatei: gespeichert { $time }
# machine-drafted, needs native review
develop-conflict-library = Bibliothek: gespeichert { $time }
# machine-drafted, needs native review
develop-conflict-writer = Die Begleitdatei schrieb { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Eine behalten. Die andere wird ersetzt.
# machine-drafted, needs native review
develop-conflict-use-file = Die der Datei verwenden
# machine-drafted, needs native review
develop-conflict-keep-library = Die der Bibliothek behalten
# machine-drafted, needs native review
develop-conflict-used-file = Die Bearbeitung aus der Begleitdatei wird verwendet
# machine-drafted, needs native review
develop-conflict-kept-library = Die Bearbeitung der Bibliothek bleibt
# machine-drafted, needs native review
toast-develop-edit-newer-process = Diese Bearbeitung stammt aus einem neueren Specular. Sie kann hier anders aussehen.
# machine-drafted, needs native review
info-darktable-edited = Bearbeitet in darktable
# machine-drafted, needs native review
info-darktable-no-modules = Keine Module aktiv
# machine-drafted, needs native review
info-darktable-group = Datei
# machine-drafted, needs native review
toast-develop-edit-partial = Diese Bearbeitung stammt aus einem neueren Specular. Teile, die diese Version nicht zeigen kann, fehlen. Änderungen hier werden nicht gespeichert.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Die Bearbeitungsdatei neben diesem Foto ist nicht lesbar. Sie bleibt unverändert. Änderungen hier werden nicht gespeichert.
# machine-drafted, needs native review
develop-demosaic = Raw-Details
develop-demosaic-directional = Standard
develop-demosaic-dual = Glatte Flächen
develop-demosaic-fast = Schnell
develop-dual-threshold = Flächenkontrast
