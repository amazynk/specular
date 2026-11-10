# machine-drafted, needs native review
# Tarkastelutyökalut: leikkautumisvaroitukset, tarkennuskorostus, histogrammi, pikselitarkastin.

## Komentopaletti
palette-clipping = Näytä tai piilota leikkautumisvaroitukset
palette-focus-peaking = Näytä tai piilota tarkennuskorostus
palette-histogram = Näytä tai piilota histogrammi
palette-pixel-inspector = Näytä tai piilota pikselitarkastin
# machine-drafted, needs native review
palette-crosshair = Näppäimistön tähtäin
palette-announce-cursor = Lue osoittimen alla oleva arvo

## Histogrammipaneeli
histogram-title = Histogrammi
# Leikkautumisrajoilla tai niiden yli olevien näkyvien pikselien osuus; arvot ovat prosentteja.
histogram-clipped = Leikkautunut: kirkkaat { $high }, varjot { $low }
histogram-empty = Ei näkyvää kuvaa
# Ruudunlukijan yhteenveto. $mean on keskikirkkaus (0–255), $high/$low ovat prosentteja.
histogram-a11y = Näkyvän kuvan histogrammi: keskikirkkaus { $mean }, leikkautuneet kirkkaat sävyt { $high }, leikkautuneet varjot { $low }

## Pikselitarkastinpaneeli
inspector-title = Pikselitarkastin
inspector-position = Sijainti
inspector-source-rgb = Lähde RGB
inspector-source-rgba = Lähde RGBA
inspector-source-ycc = Lähde Y′CbCr
inspector-source-gray = Lähde harmaa
inspector-source-gray16 = Lähde harmaa (16-bittinen)
inspector-source-gray-alpha16 = Lähde harmaa + alfa (16-bittinen)
inspector-source-rgb16 = Lähde RGB (16-bittinen)
inspector-source-rgba16 = Lähde RGBA (16-bittinen)
inspector-source-pending = Lähde
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Lineaarinen
# Näytetään, kun arvot tulevat pienennetystä esikatselusta eivätkä täyden tarkkuuden pikseleistä.
inspector-approximate = Pienennetystä esikatselusta; lähennä saadaksesi tarkat arvot
inspector-outside = Osoita kuvaa tarkastellaksesi pikseliä
# Ruudunlukijan yhteenveto: pikselin sijainti ja sen sRGB-arvo.
inspector-a11y = Pikseli kohdassa { $x }, { $y }: sRGB { $value }

## Asetukset
settings-section-inspection = Tarkastelu
settings-inspection-clip-highlight = Kirkkaiden sävyjen leikkautumisraja
settings-inspection-clip-highlight-help = Pikselit, joiden jokin kanava on tässä 8-bittisessä arvossa tai sen yli, merkitään leikkautuneiksi kirkkaiksi sävyiksi.
settings-inspection-clip-shadow = Varjojen leikkautumisraja
settings-inspection-clip-shadow-help = Pikselit, joiden jokin kanava on tässä 8-bittisessä arvossa tai sen alle, merkitään leikkautuneiksi varjoiksi.
settings-inspection-highlight-color = Kirkkaiden sävyjen varoitusväri
settings-inspection-highlight-color-help = Väri, jolla leikkautuneet kirkkaat sävyt merkitään.
settings-inspection-shadow-color = Varjojen varoitusväri
settings-inspection-shadow-color-help = Väri, jolla leikkautuneet varjot merkitään.
settings-inspection-peaking-sensitivity = Tarkennuskorostuksen herkkyys
settings-inspection-peaking-sensitivity-help = Suuremmilla arvoilla myös pehmeämmät reunat merkitään tarkoiksi.
settings-inspection-peaking-color = Tarkennuskorostuksen väri
settings-inspection-peaking-color-help = Väri, jolla terävät reunat merkitään.
settings-inspection-histogram-channels = Histogrammin kanavat
settings-inspection-histogram-channels-help = Mitkä käyrät histogrammi näyttää.
settings-inspection-histogram-channels-colour = Punainen, vihreä ja sininen
settings-inspection-histogram-channels-luminance = Luminanssi
settings-inspection-histogram-channels-both = Väri ja luminanssi
settings-inspection-histogram-log = Logaritminen histogrammi
settings-inspection-histogram-log-help = Skaalaa histogrammi logaritmisesti, jotta pienetkin määrät näkyvät.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Näytä punainen kanava
palette-channel-green = Näytä vihreä kanava
palette-channel-blue = Näytä sininen kanava
palette-channel-alpha = Näytä alfakanava (uudelleen: shakkiruudukon päällä, sitten värin päällä)
palette-channel-luma = Näytä luminanssi
channel-normal = Kaikki kanavat
channel-red = Punainen kanava
channel-green = Vihreä kanava
channel-blue = Sininen kanava
channel-alpha = Alfakanava
channel-alpha-checker = Läpinäkyvyys shakkiruudukon päällä
channel-alpha-matte = Läpinäkyvyys värin päällä
channel-luma = Luminanssi
disabled-no-alpha = Kuvassa ei ole läpinäkyvyyttä
settings-inspection-alpha-matte-color = Alfan taustaväri
settings-inspection-alpha-matte-color-help = Yhtenäinen väri kuvan takana kanavanäkymässä ”Läpinäkyvyys värin päällä”.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Näytä tai piilota väärävärit
palette-next-colour-map = Seuraava värikartta
palette-channel-colours = Seuraavat kanavavärit (puna–vihreä, magenta–vihreä, magenta–keltainen–syaani)
palette-hilo = Näytä tai piilota HiLo (pikselit bittisyvyyden maksimissa ja minimissä)
palette-colour-vision = Seuraava värinäön simulointi
palette-colour-vision-off = Lopeta värinäön simulointi
palette-colour-vision-protanopia = Simuloi protanopiaa (ei punatappeja)
palette-colour-vision-deuteranopia = Simuloi deuteranopiaa (ei vihertappeja)
palette-colour-vision-tritanopia = Simuloi tritanopiaa (ei sinitappeja)
palette-colour-vision-achromatopsia = Simuloi akromatopsiaa (ei värejä)
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
colour-map-jet = Jet (ei värisokeusturvallinen)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = raaka-arvot
# Screen reader summary of the colour bar.
colour-bar-a11y = Väripalkki { $map }: { $low }–{ $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Väärävärit: { $map }
badge-hilo = HiLo: merkitään maksimin ja minimin pikselit
badge-colour-vision-protanopia = Simuloidaan protanopiaa
badge-colour-vision-deuteranopia = Simuloidaan deuteranopiaa
badge-colour-vision-tritanopia = Simuloidaan tritanopiaa
badge-colour-vision-achromatopsia = Simuloidaan akromatopsiaa
channel-colours-red-green = Kanavat: punainen, vihreä, sininen
channel-colours-magenta-green = Kanavat: magenta, vihreä
channel-colours-magenta-yellow-cyan = Kanavat: magenta, keltainen, syaani
settings-inspection-false-colour-map = Väärävärikartta
settings-inspection-false-colour-map-help = Värikartta vääräväreille ja niiden väripalkille. Kaikki paitsi jet ja phase pysyvät luettavina millä tahansa värinäöllä.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergoiva)
settings-inspection-false-colour-map-vik = Vik (divergoiva)
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
settings-inspection-false-colour-map-balance = Balance (divergoiva)
settings-inspection-false-colour-map-curl = Curl (divergoiva)
settings-inspection-false-colour-map-phase = Phase (syklinen)
settings-inspection-false-colour-map-jet = Jet (ei värisokeusturvallinen)
settings-inspection-colour-bar = Väripalkki
settings-inspection-colour-bar-help = Näytä väärävärien ollessa päällä väripalkki, jonka arvot ovat kuvan omissa yksiköissä.
settings-inspection-channel-colours = Kanavavärit
settings-inspection-channel-colours-help = Miten värikanavat näytetään. Magenta–vihreä ja magenta–keltainen–syaani erottuvat puna-vihervärisokeudessakin.
settings-inspection-channel-colours-red-green = Punainen, vihreä, sininen
settings-inspection-channel-colours-magenta-green = Magenta, vihreä
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, keltainen, syaani
settings-inspection-colour-vision = Värinäön simulointi
settings-inspection-colour-vision-help = Näytä koko ikkuna sellaisena kuin se näkyy värinäön puutoksen kanssa (Machado et al. 2009). Ei koskaan vaikuta vientiin.
settings-inspection-colour-vision-off = Pois
settings-inspection-colour-vision-protanopia = Protanopia
settings-inspection-colour-vision-deuteranopia = Deuteranopia
settings-inspection-colour-vision-tritanopia = Tritanopia
settings-inspection-colour-vision-achromatopsia = Akromatopsia
settings-inspection-auto-range = Korkeabittisen harmaasävyn automaattinen alue
settings-inspection-auto-range-help = Venytä 16-bittiset harmaasävykuvat (TIFF, PNG, FITS) tummimmasta kirkkaimpaan arvoon. DICOM-kuvat säilyttävät oman ikkunansa.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Tähtäin päällä. Nuolinäppäimet siirtävät sitä, Vaihto suurempiin askeliin. Enter napsauttaa. Esc lopettaa.
crosshair-off = Tähtäin pois päältä
announce-cursor-nothing = Osoittimen alla ei ole mitään
