# machine-drafted, needs native review
# Granskningsverktyg: klippningsvarningar, fokusmarkering, histogram, pixelgranskare. Svenska.

## Kommandopalett
palette-clipping = Visa eller dölj klippningsvarningar
palette-focus-peaking = Visa eller dölj fokusmarkering
palette-histogram = Visa eller dölj histogram
palette-pixel-inspector = Visa eller dölj pixelgranskaren
# machine-drafted, needs native review
palette-crosshair = Tangentbordsmålkors
palette-announce-cursor = Läs upp värdet under markören

## Histogrampanel
histogram-title = Histogram
# Andel synliga pixlar vid eller bortom klippningsnivåerna; värdena är procentsatser.
histogram-clipped = Klippt: högdagrar { $high }, skuggor { $low }
histogram-empty = Ingen bild synlig
# Sammanfattning för skärmläsare. $mean är medelljusstyrkan (0–255), $high/$low är procentsatser.
histogram-a11y = Histogram över den synliga bilden: medelljusstyrka { $mean }, klippta högdagrar { $high }, klippta skuggor { $low }

## Pixelgranskare
inspector-title = Pixelgranskare
inspector-position = Position
inspector-source-rgb = Källa RGB
inspector-source-rgba = Källa RGBA
inspector-source-ycc = Källa Y′CbCr
inspector-source-gray = Källa grå
inspector-source-gray16 = Källa grå (16-bitars)
inspector-source-gray-alpha16 = Källa grå + alfa (16-bitars)
inspector-source-rgb16 = Källa RGB (16-bitars)
inspector-source-rgba16 = Källa RGBA (16-bitars)
inspector-source-pending = Källa
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Linjär
# Visas när värdena kommer från en förminskad förhandsvisning, inte pixlar i full upplösning.
inspector-approximate = Från en förminskad förhandsvisning; zooma in för exakta värden
inspector-outside = Peka på bilden för att granska en pixel
# Sammanfattning för skärmläsare: pixelns position och dess sRGB-värde.
inspector-a11y = Pixel vid { $x }, { $y }: sRGB { $value }

## Inställningar
settings-section-inspection = Granskning
settings-inspection-clip-highlight = Klippningsnivå för högdagrar
settings-inspection-clip-highlight-help = Pixlar med någon kanal på eller över detta 8-bitarsvärde markeras som klippta högdagrar.
settings-inspection-clip-shadow = Klippningsnivå för skuggor
settings-inspection-clip-shadow-help = Pixlar med någon kanal på eller under detta 8-bitarsvärde markeras som klippta skuggor.
settings-inspection-highlight-color = Varningsfärg för högdagrar
settings-inspection-highlight-color-help = Färgen som markerar klippta högdagrar.
settings-inspection-shadow-color = Varningsfärg för skuggor
settings-inspection-shadow-color-help = Färgen som markerar klippta skuggor.
settings-inspection-peaking-sensitivity = Känslighet för fokusmarkering
settings-inspection-peaking-sensitivity-help = Högre värden markerar mjukare kanter som skarpa.
settings-inspection-peaking-color = Färg för fokusmarkering
settings-inspection-peaking-color-help = Färgen som markerar skarpa kanter.
settings-inspection-histogram-channels = Histogramkanaler
settings-inspection-histogram-channels-help = Vilka kurvor histogrammet visar.
settings-inspection-histogram-channels-colour = Rött, grönt och blått
settings-inspection-histogram-channels-luminance = Luminans
settings-inspection-histogram-channels-both = Färg och luminans
settings-inspection-histogram-log = Logaritmiskt histogram
settings-inspection-histogram-log-help = Skala histogrammet logaritmiskt så att små antal förblir synliga.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Visa röd kanal
palette-channel-green = Visa grön kanal
palette-channel-blue = Visa blå kanal
palette-channel-alpha = Visa alfakanalen (igen: över schackrutor, sedan över en färg)
palette-channel-luma = Visa luminans
channel-normal = Alla kanaler
channel-red = Röd kanal
channel-green = Grön kanal
channel-blue = Blå kanal
channel-alpha = Alfakanal
channel-alpha-checker = Transparens över schackrutor
channel-alpha-matte = Transparens över färg
channel-luma = Luminans
disabled-no-alpha = Bilden har ingen transparens
settings-inspection-alpha-matte-color = Bakgrundsfärg för alfa
settings-inspection-alpha-matte-color-help = Enfärgad bakgrund bakom bilden i kanalvyn ”Transparens över färg”.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Visa eller dölj falskfärg
palette-next-colour-map = Nästa färgkarta
palette-channel-colours = Nästa kanalfärger (röd–grön, magenta–grön, magenta–gul–cyan)
palette-hilo = Visa eller dölj HiLo (pixlar vid bitdjupets max och min)
palette-colour-vision = Nästa färgseendesimulering
palette-colour-vision-off = Sluta simulera färgseende
palette-colour-vision-protanopia = Simulera protanopi (inga röda tappar)
palette-colour-vision-deuteranopia = Simulera deuteranopi (inga gröna tappar)
palette-colour-vision-tritanopia = Simulera tritanopi (inga blå tappar)
palette-colour-vision-achromatopsia = Simulera akromatopsi (ingen färg)
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
colour-map-jet = Jet (inte säker vid färgblindhet)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = råvärden
# Screen reader summary of the colour bar.
colour-bar-a11y = Färgskala { $map }: { $low } till { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Falskfärg: { $map }
badge-hilo = HiLo: markerar pixlar vid max och min
badge-colour-vision-protanopia = Simulerar protanopi
badge-colour-vision-deuteranopia = Simulerar deuteranopi
badge-colour-vision-tritanopia = Simulerar tritanopi
badge-colour-vision-achromatopsia = Simulerar akromatopsi
channel-colours-red-green = Kanaler: röd, grön, blå
channel-colours-magenta-green = Kanaler: magenta, grön
channel-colours-magenta-yellow-cyan = Kanaler: magenta, gul, cyan
settings-inspection-false-colour-map = Falskfärgskarta
settings-inspection-false-colour-map-help = Färgkarta för falskfärg och dess färgskala. Alla utom jet och phase förblir läsbara med alla typer av färgseende.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergerande)
settings-inspection-false-colour-map-vik = Vik (divergerande)
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
settings-inspection-false-colour-map-balance = Balance (divergerande)
settings-inspection-false-colour-map-curl = Curl (divergerande)
settings-inspection-false-colour-map-phase = Phase (cyklisk)
settings-inspection-false-colour-map-jet = Jet (inte säker vid färgblindhet)
settings-inspection-colour-bar = Färgskala
settings-inspection-colour-bar-help = Visa en färgskala med värden i bildens egna enheter när falskfärg är på.
settings-inspection-channel-colours = Kanalfärger
settings-inspection-channel-colours-help = Hur färgkanalerna visas. Magenta–grön och magenta–gul–cyan förblir åtskilda vid röd–grön färgblindhet.
settings-inspection-channel-colours-red-green = Röd, grön, blå
settings-inspection-channel-colours-magenta-green = Magenta, grön
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, gul, cyan
settings-inspection-colour-vision = Simulering av färgseende
settings-inspection-colour-vision-help = Visa hela fönstret så som det ser ut vid nedsatt färgseende (Machado et al. 2009). Exporter påverkas aldrig.
settings-inspection-colour-vision-off = Av
settings-inspection-colour-vision-protanopia = Protanopi
settings-inspection-colour-vision-deuteranopia = Deuteranopi
settings-inspection-colour-vision-tritanopia = Tritanopi
settings-inspection-colour-vision-achromatopsia = Akromatopsi
settings-inspection-auto-range = Autoomfång för gråskala med högt bitdjup
settings-inspection-auto-range-help = Sträck ut 16-bitars gråskalebilder (TIFF, PNG, FITS) från mörkaste till ljusaste värde. DICOM-bilder behåller sitt eget fönster.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Målkors på. Piltangenterna flyttar det, Skift för större steg. Retur klickar. Esc avslutar.
crosshair-off = Målkors av
announce-cursor-nothing = Inget under markören
