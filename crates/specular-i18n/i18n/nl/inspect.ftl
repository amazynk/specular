# machine-drafted, needs native review
# Inspectiehulpmiddelen: clippingwaarschuwingen, focuspeaking, histogram, pixelinspector. Nederlands.

## Opdrachtenpalet
palette-clipping = Clippingwaarschuwingen tonen of verbergen
palette-focus-peaking = Focuspeaking tonen of verbergen
palette-histogram = Histogram tonen of verbergen
palette-pixel-inspector = Pixelinspector tonen of verbergen
# machine-drafted, needs native review
palette-crosshair = Toetsenbordkruis
palette-announce-cursor = Waarde onder de aanwijzer voorlezen

## Histogrampaneel
histogram-title = Histogram
# Aandeel zichtbare pixels op of voorbij de clippingniveaus; waarden zijn percentages.
histogram-clipped = Geclipt: hoge lichten { $high }, schaduwen { $low }
histogram-empty = Geen afbeelding zichtbaar
# Samenvatting voor schermlezers. $mean is de gemiddelde helderheid (0–255), $high/$low zijn percentages.
histogram-a11y = Histogram van de zichtbare afbeelding: gemiddelde helderheid { $mean }, geclipte hoge lichten { $high }, geclipte schaduwen { $low }

## Pixelinspectorpaneel
inspector-title = Pixelinspector
inspector-position = Positie
inspector-source-rgb = Bron-RGB
inspector-source-rgba = Bron-RGBA
inspector-source-ycc = Bron-Y′CbCr
inspector-source-gray = Bron-grijs
inspector-source-gray16 = Bron-grijs (16-bits)
inspector-source-gray-alpha16 = Bron-grijs + alfa (16-bits)
inspector-source-rgb16 = Bron-RGB (16-bits)
inspector-source-rgba16 = Bron-RGBA (16-bits)
inspector-source-pending = Bron
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Lineair
# Getoond als de waarden uit een verkleind voorbeeld komen, niet uit pixels op volledige resolutie.
inspector-approximate = Uit een verkleind voorbeeld; zoom in voor exacte waarden
inspector-outside = Wijs de afbeelding aan om een pixel te inspecteren
# Samenvatting voor schermlezers: pixelpositie en de sRGB-waarde.
inspector-a11y = Pixel op { $x }, { $y }: sRGB { $value }

## Instellingen
settings-section-inspection = Inspectie
settings-inspection-clip-highlight = Clippingniveau hoge lichten
settings-inspection-clip-highlight-help = Pixels met een kanaal op of boven deze 8-bitswaarde worden gemarkeerd als geclipte hoge lichten.
settings-inspection-clip-shadow = Clippingniveau schaduwen
settings-inspection-clip-shadow-help = Pixels met een kanaal op of onder deze 8-bitswaarde worden gemarkeerd als geclipte schaduwen.
settings-inspection-highlight-color = Waarschuwingskleur hoge lichten
settings-inspection-highlight-color-help = Kleur die geclipte hoge lichten markeert.
settings-inspection-shadow-color = Waarschuwingskleur schaduwen
settings-inspection-shadow-color-help = Kleur die geclipte schaduwen markeert.
settings-inspection-peaking-sensitivity = Gevoeligheid focuspeaking
settings-inspection-peaking-sensitivity-help = Bij hogere waarden worden ook zachtere randen als scherp gemarkeerd.
settings-inspection-peaking-color = Kleur focuspeaking
settings-inspection-peaking-color-help = Kleur die scherpe randen markeert.
settings-inspection-histogram-channels = Histogramkanalen
settings-inspection-histogram-channels-help = Welke curven het histogram toont.
settings-inspection-histogram-channels-colour = Rood, groen en blauw
settings-inspection-histogram-channels-luminance = Luminantie
settings-inspection-histogram-channels-both = Kleur en luminantie
settings-inspection-histogram-log = Logaritmisch histogram
settings-inspection-histogram-log-help = Schaal het histogram logaritmisch, zodat kleine aantallen zichtbaar blijven.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Rode kanaal tonen
palette-channel-green = Groene kanaal tonen
palette-channel-blue = Blauwe kanaal tonen
palette-channel-alpha = Alfakanaal tonen (nogmaals: over dambord, daarna over een kleur)
palette-channel-luma = Luminantie tonen
channel-normal = Alle kanalen
channel-red = Rood kanaal
channel-green = Groen kanaal
channel-blue = Blauw kanaal
channel-alpha = Alfakanaal
channel-alpha-checker = Transparantie over dambord
channel-alpha-matte = Transparantie over kleur
channel-luma = Luminantie
disabled-no-alpha = Deze afbeelding heeft geen transparantie
settings-inspection-alpha-matte-color = Achtergrondkleur voor alfa
settings-inspection-alpha-matte-color-help = Effen kleur achter de afbeelding in de kanaalweergave ‘Transparantie over kleur’.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Valse kleuren tonen of verbergen
palette-next-colour-map = Volgende kleurenkaart
palette-channel-colours = Volgende kanaalkleuren (rood–groen, magenta–groen, magenta–geel–cyaan)
palette-hilo = HiLo tonen of verbergen (pixels op het maximum en minimum van de bitdiepte)
palette-colour-vision = Volgende kleurenzichtsimulatie
palette-colour-vision-off = Kleurenzicht niet meer simuleren
palette-colour-vision-protanopia = Protanopie simuleren (geen rode kegeltjes)
palette-colour-vision-deuteranopia = Deuteranopie simuleren (geen groene kegeltjes)
palette-colour-vision-tritanopia = Tritanopie simuleren (geen blauwe kegeltjes)
palette-colour-vision-achromatopsia = Achromatopsie simuleren (geen kleur)
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
colour-map-jet = Jet (niet kleurenblindveilig)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = ruwe waarden
# Screen reader summary of the colour bar.
colour-bar-a11y = Kleurenbalk { $map }: { $low } tot { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Valse kleuren: { $map }
badge-hilo = HiLo: pixels op het maximum en minimum gemarkeerd
badge-colour-vision-protanopia = Protanopie gesimuleerd
badge-colour-vision-deuteranopia = Deuteranopie gesimuleerd
badge-colour-vision-tritanopia = Tritanopie gesimuleerd
badge-colour-vision-achromatopsia = Achromatopsie gesimuleerd
channel-colours-red-green = Kanalen: rood, groen, blauw
channel-colours-magenta-green = Kanalen: magenta, groen
channel-colours-magenta-yellow-cyan = Kanalen: magenta, geel, cyaan
settings-inspection-false-colour-map = Kleurenkaart voor valse kleuren
settings-inspection-false-colour-map-help = Kleurenkaart voor valse kleuren en de kleurenbalk. Alle behalve jet en phase blijven leesbaar bij elk kleurenzicht.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergerend)
settings-inspection-false-colour-map-vik = Vik (divergerend)
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
settings-inspection-false-colour-map-balance = Balance (divergerend)
settings-inspection-false-colour-map-curl = Curl (divergerend)
settings-inspection-false-colour-map-phase = Phase (cyclisch)
settings-inspection-false-colour-map-jet = Jet (niet kleurenblindveilig)
settings-inspection-colour-bar = Kleurenbalk
settings-inspection-colour-bar-help = Toon een kleurenbalk met waarden in de eigen eenheden van de afbeelding zolang valse kleuren aan staan.
settings-inspection-channel-colours = Kanaalkleuren
settings-inspection-channel-colours-help = Hoe de kleurkanalen worden getoond. Magenta–groen en magenta–geel–cyaan blijven onderscheidbaar bij rood-groenkleurenblindheid.
settings-inspection-channel-colours-red-green = Rood, groen, blauw
settings-inspection-channel-colours-magenta-green = Magenta, groen
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, geel, cyaan
settings-inspection-colour-vision = Kleurenzichtsimulatie
settings-inspection-colour-vision-help = Toon het hele venster zoals gezien met een kleurenzichtafwijking (Machado et al. 2009). Exports veranderen nooit.
settings-inspection-colour-vision-off = Uit
settings-inspection-colour-vision-protanopia = Protanopie
settings-inspection-colour-vision-deuteranopia = Deuteranopie
settings-inspection-colour-vision-tritanopia = Tritanopie
settings-inspection-colour-vision-achromatopsia = Achromatopsie
settings-inspection-auto-range = Automatisch bereik voor grijswaarden met hoge bitdiepte
settings-inspection-auto-range-help = Rek 16-bits grijswaardenafbeeldingen (TIFF, PNG, FITS) op van hun donkerste tot hun helderste waarde. DICOM-afbeeldingen houden hun eigen venster.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Kruis aan. Pijltjestoetsen verplaatsen het, Shift voor grotere stappen. Enter klikt. Esc stopt.
crosshair-off = Kruis uit
announce-cursor-nothing = Niets onder de aanwijzer
