# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Lääketieteelliset kuvat
palette-window-level = Ikkunointityökalu (vedä vasemmalla)
palette-window-default = Ikkuna: tiedoston oletus
palette-window-lung = Ikkuna: keuhko (W 1500, L −600)
palette-window-mediastinum = Ikkuna: välikarsina (W 350, L 50)
palette-window-bone = Ikkuna: luu (W 1800, L 400)
palette-window-brain = Ikkuna: aivot (W 80, L 40)
palette-window-stroke = Ikkuna: aivoinfarkti (W 40, L 40)
palette-window-soft-tissue = Ikkuna: pehmytkudos (W 400, L 50)
palette-window-liver = Ikkuna: maksa (W 150, L 30)
palette-window-subdural = Ikkuna: subduraali (W 200, L 75)
palette-window-full-range = Ikkuna: koko alue
palette-invert-grey = Käännä harmaasävyt
palette-next-slice = Seuraava kehys
palette-prev-slice = Edellinen kehys
disabled-not-medical = Vain harmaasävyisille lääketieteellisille kuville
disabled-one-frame = Kuvassa on yksi kehys
settings-section-medical = Lääketieteelliset kuvat
settings-medical-keep-window = Säilytä ikkunointi seuraavalle sarjalle
settings-medical-keep-window-help = Säilytä asettamasi ikkuna siirryttäessä toiseen sarjaan sen oletuksen sijaan. Saman tiedoston kehykset ja saman sarjan leikkeet jakavat sen aina.
medical-window = C { $center }  W { $width }
medical-inverted = käännetty
medical-frame = Kehys { $frame }/{ $count }
medical-tool = Ikkunointityökalu: vedä
medical-tool-on = Ikkunointityökalu päällä: säädä vetämällä
medical-tool-off = Ikkunointityökalu pois
medical-preset-not-ct = TT-esiasetukset vaativat arvot Hounsfield-yksiköissä
medical-file-window-none = Tiedostossa ei ole ikkunaa { $index }
medical-file-window-unnamed = Tiedoston ikkuna { $index }/{ $count }
medical-file-window = Tiedoston ikkuna { $index }/{ $count }: { $name }
measure-templates-need-medical = Geometriamallit tulevat Lääketiede-profiilin mukana
medical-off = Tälle kuvalle ei ole lääketieteellisiä työkaluja: { $reason }
medical-readout = Arvo ja ikkuna

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Toista kuvat (cine)
palette-measure-length = Mittaa pituus
palette-measure-angle = Mittaa kulma
palette-measure-ellipse = Mittaa elliptinen alue
palette-measure-rectangle = Mittaa suorakulmainen alue
palette-clear-measurements = Tyhjennä mittaukset
palette-measure-templates = Geometriamallit…
palette-overlays = Näytä tai piilota peitteet
medical-cine = Toistetaan nopeudella { $rate } kuvaa/s
medical-overlays-shown = Peitteet näkyvissä
medical-overlays-hidden = Peitteet piilotettu
measure-hint-length = Pituus: vedä tai napsauta molempia päitä
measure-hint-angle = Kulma: napsauta kolmea pistettä, kärkeä toisena
measure-hint-ellipse = Ellipsi: vedä alueen yli
measure-hint-rectangle = Suorakulmio: vedä alueen yli
measure-off = Mittaustyökalu pois
measure-cleared = Mittaukset tyhjennetty
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Pinta-ala { $value } mm²
measure-area-px = Pinta-ala { $value } px²
measure-angle = { $value }°
measure-mean = Keskiarvo { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Maks { $max }  ({ $count } px)
measure-caveat-detector = Ilmaisimen kohdalla: suurennusta ei korjattu
measure-caveat-magnification = Arvioitu suurennuskorjaus
measure-caveat-uncalibrated = Ei pikseliväliä: pikseleinä
measure-caveat-outside-region = Kalibroidun ultraäänialueen ulkopuolella: pikseleinä

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Näytä suuntamerkinnät
settings-medical-orientation-labels-help = Kuvan reunoilla olevat kirjaimet potilaan suunnista (R/L oikea/vasen, A/P anteriorinen/posteriorinen, S/I pää/jalat), luettu tiedostosta ja kiertoa ja peilausta seuraten. L näyttää tai piilottaa ne istunnon ajaksi.
settings-medical-wheel-steps-frames = Hiiren rulla selaa kehyksiä
settings-medical-wheel-steps-frames-help = Useita kehyksiä sisältävässä tiedostossa tai sarjassa rulla siirtyy kehyksestä toiseen zoomaamisen tai kuvan vaihtamisen sijaan. Ctrl+rulla zoomaa edelleen.
# machine-drafted, needs native review
settings-medical-display-function = Harmaasävyjen näyttö
settings-medical-display-function-help = Miten lääketieteellisten kuvien harmaasävyt näytetään. DICOM GSDF (harmaasävyjen standardinäyttöfunktio) jakaa ne näkyvyydeltään tasaisiin portaisiin näytön mustan ja valkoisen luminanssin välille (DICOM PS3.14), kuten kalibroidulla diagnostisella näytöllä. Näytön väriprofiilia käytetään edelleen.
settings-medical-display-function-srgb = Vakio (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Näytön mustan luminanssi
settings-medical-display-black-help = Näytön mustan luminanssi ympäristön valo mukaan lukien, mitattuna tai näytön tietolehdestä. DICOM GSDF käyttää tätä.
settings-medical-display-white = Näytön valkoisen luminanssi
settings-medical-display-white-help = Näytön valkoisen luminanssi, mitattuna tai näytön tietolehdestä. DICOM GSDF käyttää tätä.
palette-orientation-labels = Näytä tai piilota suuntamerkinnät
palette-presentation-state = Seuraava esitystila
disabled-no-presentation-state = Mikään esitystila ei viittaa tähän kuvaan
medical-labels-shown = Suuntamerkinnät näkyvissä
medical-labels-hidden = Suuntamerkinnät piilotettu
medical-presentation = Esitystila: { $label } ({ $index }/{ $count })
medical-presentation-none = Esitystila pois
medical-presentation-readout = Esitys: { $label }
medical-preset-presentation = Esitystilan hakutaulukko määrää harmaasävyt

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Suunta: vasen { $left }, oikea { $right }, ylä { $top }, ala { $bottom }
