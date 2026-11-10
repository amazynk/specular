# machine-drafted, needs native review
# Inspektionsværktøjer: klipningsadvarsler, fokusmarkering, histogram, pixelinspektør. Dansk.

## Kommandopalet
palette-clipping = Vis eller skjul klipningsadvarsler
palette-focus-peaking = Vis eller skjul fokusmarkering
palette-histogram = Vis eller skjul histogram
palette-pixel-inspector = Vis eller skjul pixelinspektør
# machine-drafted, needs native review
palette-crosshair = Tastaturkrydshår
palette-announce-cursor = Læs værdien under markøren

## Histogrampanel
histogram-title = Histogram
# Andel af synlige pixel ved eller over klipningsniveauerne; værdierne er procentsatser.
histogram-clipped = Klippet: højlys { $high }, skygger { $low }
histogram-empty = Intet synligt billede
# Skærmlæseroversigt. $mean er den gennemsnitlige lysstyrke (0–255), $high/$low er procentsatser.
histogram-a11y = Histogram over det synlige billede: gennemsnitlig lysstyrke { $mean }, klippet højlys { $high }, klippede skygger { $low }

## Pixelinspektørpanel
inspector-title = Pixelinspektør
inspector-position = Position
inspector-source-rgb = Kilde-RGB
inspector-source-rgba = Kilde-RGBA
inspector-source-ycc = Kilde-Y′CbCr
inspector-source-gray = Kildegråtone
inspector-source-gray16 = Kildegråtone (16-bit)
inspector-source-gray-alpha16 = Kildegråtone + alfa (16-bit)
inspector-source-rgb16 = Kilde-RGB (16-bit)
inspector-source-rgba16 = Kilde-RGBA (16-bit)
inspector-source-pending = Kilde
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Lineær
# Vises, når værdierne kommer fra et formindsket eksempel og ikke fra pixel i fuld opløsning.
inspector-approximate = Fra et formindsket eksempel; zoom ind for at få nøjagtige værdier
inspector-outside = Peg på billedet for at inspicere en pixel
# Skærmlæseroversigt: pixelposition og dens sRGB-værdi.
inspector-a11y = Pixel ved { $x }, { $y }: sRGB { $value }

## Indstillinger
settings-section-inspection = Inspektion
settings-inspection-clip-highlight = Klipningsniveau for højlys
settings-inspection-clip-highlight-help = Pixel, hvor en kanal er på eller over denne 8-bit-værdi, markeres som klippet højlys.
settings-inspection-clip-shadow = Klipningsniveau for skygger
settings-inspection-clip-shadow-help = Pixel, hvor en kanal er på eller under denne 8-bit-værdi, markeres som klippede skygger.
settings-inspection-highlight-color = Advarselsfarve for højlys
settings-inspection-highlight-color-help = Farve, der markerer klippet højlys.
settings-inspection-shadow-color = Advarselsfarve for skygger
settings-inspection-shadow-color-help = Farve, der markerer klippede skygger.
settings-inspection-peaking-sensitivity = Følsomhed for fokusmarkering
settings-inspection-peaking-sensitivity-help = Højere værdier markerer blødere kanter som skarpe.
settings-inspection-peaking-color = Farve for fokusmarkering
settings-inspection-peaking-color-help = Farve, der markerer skarpe kanter.
settings-inspection-histogram-channels = Histogramkanaler
settings-inspection-histogram-channels-help = Hvilke kurver histogrammet viser.
settings-inspection-histogram-channels-colour = Rød, grøn og blå
settings-inspection-histogram-channels-luminance = Luminans
settings-inspection-histogram-channels-both = Farve og luminans
settings-inspection-histogram-log = Logaritmisk histogram
settings-inspection-histogram-log-help = Skaler histogrammet logaritmisk, så små antal forbliver synlige.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Vis rød kanal
palette-channel-green = Vis grøn kanal
palette-channel-blue = Vis blå kanal
palette-channel-alpha = Vis alfakanalen (igen: over skaktern, derefter over en farve)
palette-channel-luma = Vis luminans
channel-normal = Alle kanaler
channel-red = Rød kanal
channel-green = Grøn kanal
channel-blue = Blå kanal
channel-alpha = Alfakanal
channel-alpha-checker = Gennemsigtighed over skaktern
channel-alpha-matte = Gennemsigtighed over farve
channel-luma = Luminans
disabled-no-alpha = Billedet har ingen gennemsigtighed
settings-inspection-alpha-matte-color = Baggrundsfarve for alfa
settings-inspection-alpha-matte-color-help = Ensfarvet baggrund bag billedet i kanalvisningen “Gennemsigtighed over farve”.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Vis eller skjul falske farver
palette-next-colour-map = Næste farvekort
palette-channel-colours = Næste kanalfarver (rød–grøn, magenta–grøn, magenta–gul–cyan)
palette-hilo = Vis eller skjul HiLo (pixels ved bitdybdens maksimum og minimum)
palette-colour-vision = Næste farvesynssimulering
palette-colour-vision-off = Stop simulering af farvesyn
palette-colour-vision-protanopia = Simulér protanopi (ingen røde tapper)
palette-colour-vision-deuteranopia = Simulér deuteranopi (ingen grønne tapper)
palette-colour-vision-tritanopia = Simulér tritanopi (ingen blå tapper)
palette-colour-vision-achromatopsia = Simulér akromatopsi (ingen farver)
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
colour-map-jet = Jet (ikke farvesynssikker)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = rå værdier
# Screen reader summary of the colour bar.
colour-bar-a11y = Farveskala { $map }: { $low } til { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Falske farver: { $map }
badge-hilo = HiLo: markerer pixels ved maksimum og minimum
badge-colour-vision-protanopia = Simulerer protanopi
badge-colour-vision-deuteranopia = Simulerer deuteranopi
badge-colour-vision-tritanopia = Simulerer tritanopi
badge-colour-vision-achromatopsia = Simulerer akromatopsi
channel-colours-red-green = Kanaler: rød, grøn, blå
channel-colours-magenta-green = Kanaler: magenta, grøn
channel-colours-magenta-yellow-cyan = Kanaler: magenta, gul, cyan
settings-inspection-false-colour-map = Farvekort for falske farver
settings-inspection-false-colour-map-help = Farvekort til falske farver og deres farveskala. Alle undtagen jet og phase forbliver læselige med ethvert farvesyn.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergerende)
settings-inspection-false-colour-map-vik = Vik (divergerende)
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
settings-inspection-false-colour-map-balance = Balance (divergerende)
settings-inspection-false-colour-map-curl = Curl (divergerende)
settings-inspection-false-colour-map-phase = Phase (cyklisk)
settings-inspection-false-colour-map-jet = Jet (ikke farvesynssikker)
settings-inspection-colour-bar = Farveskala
settings-inspection-colour-bar-help = Vis en farveskala med værdier i billedets egne enheder, mens falske farver er slået til.
settings-inspection-channel-colours = Kanalfarver
settings-inspection-channel-colours-help = Hvordan farvekanalerne vises. Magenta–grøn og magenta–gul–cyan forbliver tydelige ved rød-grøn farveblindhed.
settings-inspection-channel-colours-red-green = Rød, grøn, blå
settings-inspection-channel-colours-magenta-green = Magenta, grøn
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, gul, cyan
settings-inspection-colour-vision = Farvesynssimulering
settings-inspection-colour-vision-help = Vis hele vinduet, som det ses med nedsat farvesyn (Machado et al. 2009). Eksporter påvirkes aldrig.
settings-inspection-colour-vision-off = Fra
settings-inspection-colour-vision-protanopia = Protanopi
settings-inspection-colour-vision-deuteranopia = Deuteranopi
settings-inspection-colour-vision-tritanopia = Tritanopi
settings-inspection-colour-vision-achromatopsia = Akromatopsi
settings-inspection-auto-range = Automatisk område for gråtoner med høj bitdybde
settings-inspection-auto-range-help = Stræk 16-bit gråtonebilleder (TIFF, PNG, FITS) fra deres mørkeste til deres lyseste værdi. DICOM-billeder beholder deres eget vindue.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Krydshår slået til. Piletasterne flytter det, Shift for større skridt. Enter klikker. Esc afslutter.
crosshair-off = Krydshår slået fra
announce-cursor-nothing = Intet under markøren
