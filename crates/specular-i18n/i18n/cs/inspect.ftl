# machine-drafted, needs native review
# Specular — nástroje kontroly: varování před oříznutím, zvýraznění zaostření, histogram, kontrola pixelů. Čeština.

## Paleta příkazů
palette-clipping = Zobrazit nebo skrýt varování před oříznutím
palette-focus-peaking = Zobrazit nebo skrýt zvýraznění zaostření
palette-histogram = Zobrazit nebo skrýt histogram
palette-pixel-inspector = Zobrazit nebo skrýt kontrolu pixelů
# machine-drafted, needs native review
palette-crosshair = Klávesový zaměřovač
palette-announce-cursor = Číst hodnotu pod ukazatelem

## Panel histogramu
histogram-title = Histogram
# Podíl viditelných pixelů na úrovni oříznutí nebo za ní; hodnoty jsou procenta.
histogram-clipped = Oříznuto: světla { $high }, stíny { $low }
histogram-empty = Není viditelný žádný obrázek
# Souhrn pro čtečku obrazovky. $mean je průměrný jas (0–255), $high/$low jsou procenta.
histogram-a11y = Histogram viditelného obrázku: průměrný jas { $mean }, oříznutá světla { $high }, oříznuté stíny { $low }

## Panel kontroly pixelů
inspector-title = Kontrola pixelů
inspector-position = Pozice
inspector-source-rgb = Zdrojové RGB
inspector-source-rgba = Zdrojové RGBA
inspector-source-ycc = Zdrojové Y′CbCr
inspector-source-gray = Zdrojová šedá
inspector-source-gray16 = Zdrojová šedá (16 bitů)
inspector-source-gray-alpha16 = Zdrojová šedá + alfa (16 bitů)
inspector-source-rgb16 = Zdrojové RGB (16 bitů)
inspector-source-rgba16 = Zdrojové RGBA (16 bitů)
inspector-source-pending = Zdroj
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Lineární
# Zobrazí se, když hodnoty pocházejí ze zmenšeného náhledu, ne z pixelů v plném rozlišení.
inspector-approximate = Ze zmenšeného náhledu; pro přesné hodnoty přibližte
inspector-outside = Ukažte na obrázek a zobrazí se údaje o pixelu
# Souhrn pro čtečku obrazovky: pozice pixelu a jeho hodnota sRGB.
inspector-a11y = Pixel na { $x }, { $y }: sRGB { $value }

## Nastavení
settings-section-inspection = Kontrola
settings-inspection-clip-highlight = Úroveň oříznutí světel
settings-inspection-clip-highlight-help = Pixely, jejichž některý kanál dosahuje této 8bitové hodnoty nebo ji překračuje, se označí jako oříznutá světla.
settings-inspection-clip-shadow = Úroveň oříznutí stínů
settings-inspection-clip-shadow-help = Pixely, jejichž některý kanál je na této 8bitové hodnotě nebo pod ní, se označí jako oříznuté stíny.
settings-inspection-highlight-color = Barva varování ve světlech
settings-inspection-highlight-color-help = Barva, kterou se označují oříznutá světla.
settings-inspection-shadow-color = Barva varování ve stínech
settings-inspection-shadow-color-help = Barva, kterou se označují oříznuté stíny.
settings-inspection-peaking-sensitivity = Citlivost zvýraznění zaostření
settings-inspection-peaking-sensitivity-help = Vyšší hodnoty označí jako zaostřené i měkčí hrany.
settings-inspection-peaking-color = Barva zvýraznění zaostření
settings-inspection-peaking-color-help = Barva, kterou se označují ostré hrany.
settings-inspection-histogram-channels = Kanály histogramu
settings-inspection-histogram-channels-help = Které křivky histogram zobrazuje.
settings-inspection-histogram-channels-colour = Červená, zelená a modrá
settings-inspection-histogram-channels-luminance = Jas
settings-inspection-histogram-channels-both = Barvy a jas
settings-inspection-histogram-log = Logaritmický histogram
settings-inspection-histogram-log-help = Logaritmické měřítko histogramu, aby zůstaly vidět i malé počty.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Zobrazit červený kanál
palette-channel-green = Zobrazit zelený kanál
palette-channel-blue = Zobrazit modrý kanál
palette-channel-alpha = Zobrazit alfa kanál (znovu: na šachovnici, pak na barvě)
palette-channel-luma = Zobrazit jas
channel-normal = Všechny kanály
channel-red = Červený kanál
channel-green = Zelený kanál
channel-blue = Modrý kanál
channel-alpha = Alfa kanál
channel-alpha-checker = Průhlednost na šachovnici
channel-alpha-matte = Průhlednost na barvě
channel-luma = Jas
disabled-no-alpha = Tento obrázek nemá průhlednost
settings-inspection-alpha-matte-color = Barva podkladu pro alfu
settings-inspection-alpha-matte-color-help = Jednolitá barva za obrázkem v zobrazení kanálu „Průhlednost na barvě“.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Zobrazit nebo skrýt nepravé barvy
palette-next-colour-map = Další barevná mapa
palette-channel-colours = Další barvy kanálů (červená–zelená, purpurová–zelená, purpurová–žlutá–azurová)
palette-hilo = Zobrazit nebo skrýt HiLo (pixely na maximu a minimu bitové hloubky)
palette-colour-vision = Další simulace barevného vidění
palette-colour-vision-off = Ukončit simulaci barevného vidění
palette-colour-vision-protanopia = Simulovat protanopii (bez červených čípků)
palette-colour-vision-deuteranopia = Simulovat deuteranopii (bez zelených čípků)
palette-colour-vision-tritanopia = Simulovat tritanopii (bez modrých čípků)
palette-colour-vision-achromatopsia = Simulovat achromatopsii (bez barev)
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
colour-map-jet = Jet (nevhodná pro poruchy barvocitu)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = surové hodnoty
# Screen reader summary of the colour bar.
colour-bar-a11y = Barevná škála { $map }: { $low } až { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Nepravé barvy: { $map }
badge-hilo = HiLo: označuje pixely na maximu a minimu
badge-colour-vision-protanopia = Simulace protanopie
badge-colour-vision-deuteranopia = Simulace deuteranopie
badge-colour-vision-tritanopia = Simulace tritanopie
badge-colour-vision-achromatopsia = Simulace achromatopsie
channel-colours-red-green = Kanály: červená, zelená, modrá
channel-colours-magenta-green = Kanály: purpurová, zelená
channel-colours-magenta-yellow-cyan = Kanály: purpurová, žlutá, azurová
settings-inspection-false-colour-map = Mapa nepravých barev
settings-inspection-false-colour-map-help = Barevná mapa pro nepravé barvy a jejich barevnou škálu. Všechny kromě jet a phase zůstanou čitelné při jakémkoli barevném vidění.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergentní)
settings-inspection-false-colour-map-vik = Vik (divergentní)
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
settings-inspection-false-colour-map-balance = Balance (divergentní)
settings-inspection-false-colour-map-curl = Curl (divergentní)
settings-inspection-false-colour-map-phase = Phase (cyklická)
settings-inspection-false-colour-map-jet = Jet (nevhodná pro poruchy barvocitu)
settings-inspection-colour-bar = Barevná škála
settings-inspection-colour-bar-help = Při zapnutých nepravých barvách zobrazit barevnou škálu s hodnotami ve vlastních jednotkách obrázku.
settings-inspection-channel-colours = Barvy kanálů
settings-inspection-channel-colours-help = Jak se zobrazují barevné kanály. Purpurová–zelená a purpurová–žlutá–azurová zůstanou rozlišitelné i při červeno-zelené barvosleposti.
settings-inspection-channel-colours-red-green = Červená, zelená, modrá
settings-inspection-channel-colours-magenta-green = Purpurová, zelená
settings-inspection-channel-colours-magenta-yellow-cyan = Purpurová, žlutá, azurová
settings-inspection-colour-vision = Simulace barevného vidění
settings-inspection-colour-vision-help = Zobrazí celé okno tak, jak jej vidí člověk s poruchou barvocitu (Machado et al. 2009). Exporty nejsou nikdy ovlivněny.
settings-inspection-colour-vision-off = Vypnuto
settings-inspection-colour-vision-protanopia = Protanopie
settings-inspection-colour-vision-deuteranopia = Deuteranopie
settings-inspection-colour-vision-tritanopia = Tritanopie
settings-inspection-colour-vision-achromatopsia = Achromatopsie
settings-inspection-auto-range = Automatický rozsah šedi s vysokou bitovou hloubkou
settings-inspection-auto-range-help = Roztáhne 16bitové obrázky ve stupních šedi (TIFF, PNG, FITS) od nejtmavší po nejsvětlejší hodnotu. Snímky DICOM si ponechají vlastní okno.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Zaměřovač zapnut. Šipky ho posouvají, Shift pro větší kroky. Enter klikne. Esc ukončí.
crosshair-off = Zaměřovač vypnut
announce-cursor-nothing = Pod ukazatelem nic není
