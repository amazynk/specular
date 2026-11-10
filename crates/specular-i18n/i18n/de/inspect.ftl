# machine-drafted, needs native review
# Prüfwerkzeuge: Clipping-Warnungen, Fokus-Peaking, Histogramm, Pixelinspektor.

## Befehlspalette
palette-clipping = Clipping-Warnungen ein-/ausblenden
palette-focus-peaking = Fokus-Peaking ein-/ausblenden
palette-histogram = Histogramm ein-/ausblenden
palette-pixel-inspector = Pixelinspektor ein-/ausblenden
# machine-drafted, needs native review
palette-crosshair = Tastatur-Fadenkreuz
palette-announce-cursor = Wert unter dem Zeiger vorlesen

## Histogramm
histogram-title = Histogramm
histogram-clipped = Beschnitten: Lichter { $high }, Schatten { $low }
histogram-empty = Kein Bild sichtbar
histogram-a11y = Histogramm des sichtbaren Bildes: mittlere Helligkeit { $mean }, beschnittene Lichter { $high }, beschnittene Schatten { $low }

## Pixelinspektor
inspector-title = Pixelinspektor
inspector-position = Position
inspector-source-rgb = Quelle RGB
inspector-source-rgba = Quelle RGBA
inspector-source-ycc = Quelle Y′CbCr
inspector-source-gray = Quelle Grau
inspector-source-gray16 = Quelle Grau (16 Bit)
inspector-source-gray-alpha16 = Quelle Grau + Alpha (16 Bit)
inspector-source-rgb16 = Quelle RGB (16 Bit)
inspector-source-rgba16 = Quelle RGBA (16 Bit)
inspector-source-pending = Quelle
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Linear
inspector-approximate = Aus einer verkleinerten Vorschau; für exakte Werte hineinzoomen
inspector-outside = Auf das Bild zeigen, um ein Pixel zu prüfen
inspector-a11y = Pixel bei { $x }, { $y }: sRGB { $value }

## Einstellungen
settings-section-inspection = Prüfen
settings-inspection-clip-highlight = Clipping-Grenze Lichter
settings-inspection-clip-highlight-help = Pixel, bei denen ein Kanal diesen 8-Bit-Wert erreicht oder überschreitet, werden als beschnittene Lichter markiert.
settings-inspection-clip-shadow = Clipping-Grenze Schatten
settings-inspection-clip-shadow-help = Pixel, bei denen ein Kanal diesen 8-Bit-Wert erreicht oder unterschreitet, werden als beschnittene Schatten markiert.
settings-inspection-highlight-color = Warnfarbe Lichter
settings-inspection-highlight-color-help = Farbe, die beschnittene Lichter markiert.
settings-inspection-shadow-color = Warnfarbe Schatten
settings-inspection-shadow-color-help = Farbe, die beschnittene Schatten markiert.
settings-inspection-peaking-sensitivity = Empfindlichkeit des Fokus-Peakings
settings-inspection-peaking-sensitivity-help = Höhere Werte markieren auch weichere Kanten als scharf.
settings-inspection-peaking-color = Farbe des Fokus-Peakings
settings-inspection-peaking-color-help = Farbe, die scharfe Kanten markiert.
settings-inspection-histogram-channels = Histogrammkanäle
settings-inspection-histogram-channels-help = Welche Kurven das Histogramm zeigt.
settings-inspection-histogram-channels-colour = Rot, Grün und Blau
settings-inspection-histogram-channels-luminance = Luminanz
settings-inspection-histogram-channels-both = Farbe und Luminanz
settings-inspection-histogram-log = Logarithmisches Histogramm
settings-inspection-histogram-log-help = Das Histogramm logarithmisch skalieren, damit kleine Anzahlen sichtbar bleiben.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Rotkanal anzeigen
palette-channel-green = Grünkanal anzeigen
palette-channel-blue = Blaukanal anzeigen
palette-channel-alpha = Alphakanal anzeigen (erneut: über Schachbrett, dann über Farbe)
palette-channel-luma = Luminanz anzeigen
channel-normal = Alle Kanäle
channel-red = Rotkanal
channel-green = Grünkanal
channel-blue = Blaukanal
channel-alpha = Alphakanal
channel-alpha-checker = Transparenz über Schachbrett
channel-alpha-matte = Transparenz über Farbe
channel-luma = Luminanz
disabled-no-alpha = Dieses Bild hat keine Transparenz
settings-inspection-alpha-matte-color = Hintergrundfarbe für Alpha
settings-inspection-alpha-matte-color-help = Einfarbiger Hintergrund hinter dem Bild in der Kanalansicht „Transparenz über Farbe“.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Falschfarben ein-/ausblenden
palette-next-colour-map = Nächste Farbskala
palette-channel-colours = Nächste Kanalfarben (Rot–Grün, Magenta–Grün, Magenta–Gelb–Cyan)
palette-hilo = HiLo ein-/ausblenden (Pixel am Maximum und Minimum der Bittiefe)
palette-colour-vision = Nächste Farbsehsimulation
palette-colour-vision-off = Farbsehsimulation beenden
palette-colour-vision-protanopia = Protanopie simulieren (keine Rotzapfen)
palette-colour-vision-deuteranopia = Deuteranopie simulieren (keine Grünzapfen)
palette-colour-vision-tritanopia = Tritanopie simulieren (keine Blauzapfen)
palette-colour-vision-achromatopsia = Achromatopsie simulieren (keine Farbe)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (nicht für Farbfehlsichtige)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = Rohwerte
# Screen reader summary of the colour bar.
colour-bar-a11y = Farbbalken { $map }: { $low } bis { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Falschfarben: { $map }
badge-hilo = HiLo: Pixel am Maximum und Minimum markiert
badge-colour-vision-protanopia = Protanopie wird simuliert
badge-colour-vision-deuteranopia = Deuteranopie wird simuliert
badge-colour-vision-tritanopia = Tritanopie wird simuliert
badge-colour-vision-achromatopsia = Achromatopsie wird simuliert
channel-colours-red-green = Kanäle: Rot, Grün, Blau
channel-colours-magenta-green = Kanäle: Magenta, Grün
channel-colours-magenta-yellow-cyan = Kanäle: Magenta, Gelb, Cyan
settings-inspection-false-colour-map = Falschfarben-Skala
settings-inspection-false-colour-map-help = Farbskala für Falschfarben und ihren Farbbalken. Alle außer Jet und Phase bleiben bei jedem Farbsehen lesbar.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergierend)
settings-inspection-false-colour-map-vik = Vik (divergierend)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (divergierend)
settings-inspection-false-colour-map-curl = Curl (divergierend)
settings-inspection-false-colour-map-phase = Phase (zyklisch)
settings-inspection-false-colour-map-jet = Jet (nicht für Farbfehlsichtige)
settings-inspection-colour-bar = Farbbalken
settings-inspection-colour-bar-help = Bei aktiven Falschfarben einen Farbbalken mit Werten in den Einheiten des Bildes zeigen.
settings-inspection-channel-colours = Kanalfarben
settings-inspection-channel-colours-help = Wie die Farbkanäle gezeigt werden. Magenta–Grün und Magenta–Gelb–Cyan bleiben bei Rot-Grün-Schwäche unterscheidbar.
settings-inspection-channel-colours-red-green = Rot, Grün, Blau
settings-inspection-channel-colours-magenta-green = Magenta, Grün
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, Gelb, Cyan
settings-inspection-colour-vision = Farbsehsimulation
settings-inspection-colour-vision-help = Das ganze Fenster so zeigen, wie es mit einer Farbsehschwäche erscheint (Machado et al. 2009). Exporte sind nie betroffen.
settings-inspection-colour-vision-off = Aus
settings-inspection-colour-vision-protanopia = Protanopie
settings-inspection-colour-vision-deuteranopia = Deuteranopie
settings-inspection-colour-vision-tritanopia = Tritanopie
settings-inspection-colour-vision-achromatopsia = Achromatopsie
settings-inspection-auto-range = Hochbit-Graustufen automatisch strecken
settings-inspection-auto-range-help = 16-Bit-Graustufenbilder (TIFF, PNG, FITS) vom dunkelsten bis zum hellsten Wert strecken. DICOM-Bilder behalten ihr eigenes Fenster.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Fadenkreuz ein. Pfeiltasten bewegen es, Umschalt für größere Schritte. Eingabe klickt. Esc beendet.
crosshair-off = Fadenkreuz aus
announce-cursor-nothing = Nichts unter dem Zeiger
