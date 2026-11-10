# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Medicinske billeder
palette-window-level = Vindue/niveau-værktøj (venstretræk)
palette-window-default = Vindue: filens standard
palette-window-lung = Vindue: lunge (W 1500, L −600)
palette-window-mediastinum = Vindue: mediastinum (W 350, L 50)
palette-window-bone = Vindue: knogle (W 1800, L 400)
palette-window-brain = Vindue: hjerne (W 80, L 40)
palette-window-stroke = Vindue: apopleksi (W 40, L 40)
palette-window-soft-tissue = Vindue: bløddele (W 400, L 50)
palette-window-liver = Vindue: lever (W 150, L 30)
palette-window-subdural = Vindue: subduralt (W 200, L 75)
palette-window-full-range = Vindue: hele området
palette-invert-grey = Invertér gråtoner
palette-next-slice = Næste billede
palette-prev-slice = Forrige billede
disabled-not-medical = Kun for medicinske gråtonebilleder
disabled-one-frame = Filen har kun ét billede
settings-section-medical = Medicinske billeder
settings-medical-keep-window = Behold vindue/niveau til næste serie
settings-medical-keep-window-help = Behold det vindue, du har indstillet, når du skifter til en anden serie, i stedet for at starte fra dens standard. Billeder i én fil og snit i én serie deler det altid.
medical-window = C { $center }  W { $width }
medical-inverted = inverteret
medical-frame = Billede { $frame } af { $count }
medical-tool = Vindue/niveau-værktøj: træk
medical-tool-on = Vindue/niveau-værktøj til: træk for at justere
medical-tool-off = Vindue/niveau-værktøj fra
medical-preset-not-ct = CT-forvalg kræver værdier i Hounsfield-enheder
medical-file-window-none = Ingen vindue { $index } i denne fil
medical-file-window-unnamed = Filens vindue { $index } af { $count }
medical-file-window = Filens vindue { $index } af { $count }: { $name }
measure-templates-need-medical = Geometriskabeloner følger med profilen Medicin
medical-off = Ingen medicinske værktøjer til dette billede: { $reason }
medical-readout = Værdi og vindue

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Afspil billeder (cine)
palette-measure-length = Mål længde
palette-measure-angle = Mål vinkel
palette-measure-ellipse = Mål elliptisk område
palette-measure-rectangle = Mål rektangulært område
palette-clear-measurements = Ryd målinger
palette-measure-templates = Geometriskabeloner…
palette-overlays = Vis eller skjul overlejringer
medical-cine = Afspiller med { $rate } billeder/s
medical-overlays-shown = Overlejringer vist
medical-overlays-hidden = Overlejringer skjult
measure-hint-length = Længde: træk, eller klik på begge ender
measure-hint-angle = Vinkel: klik på tre punkter, toppunktet som det andet
measure-hint-ellipse = Ellipse: træk hen over området
measure-hint-rectangle = Rektangel: træk hen over området
measure-off = Måleværktøj slået fra
measure-cleared = Målinger ryddet
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Areal { $value } mm²
measure-area-px = Areal { $value } px²
measure-angle = { $value }°
measure-mean = Middel { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Maks { $max }  ({ $count } px)
measure-caveat-detector = Ved detektoren: ikke korrigeret for forstørrelse
measure-caveat-magnification = Anslået forstørrelseskorrektion
measure-caveat-uncalibrated = Ingen pixelafstand: i pixels
measure-caveat-outside-region = Uden for et kalibreret ultralydsområde: i pixels

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Vis orienteringsmærker
settings-medical-orientation-labels-help = Bogstaver ved billedets kanter for patientens retninger (R/L højre/venstre, A/P anterior/posterior, S/I hoved/fødder), hentet fra filen og fulgt med rotation og spejling. L slår dem til og fra i sessionen.
settings-medical-wheel-steps-frames = Musehjulet skifter mellem billeder
settings-medical-wheel-steps-frames-help = I en fil eller serie med flere billeder skifter hjulet mellem billederne i stedet for at zoome eller skifte fil. Ctrl+hjul zoomer stadig.
# machine-drafted, needs native review
settings-medical-display-function = Gråtonevisning
settings-medical-display-function-help = Hvordan gråtonerne i medicinske billeder vises. DICOM GSDF (standardvisningsfunktion for gråtoner) fordeler dem i trin med lige stor synlighed mellem skærmens sort- og hvidluminans (DICOM PS3.14), som på en kalibreret diagnostisk skærm. Skærmens farveprofil gælder stadig.
settings-medical-display-function-srgb = Standard (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Skærmens sortluminans
settings-medical-display-black-help = Luminansen af skærmens sort, inklusive omgivende lys, målt eller fra skærmens datablad. Bruges af DICOM GSDF.
settings-medical-display-white = Skærmens hvidluminans
settings-medical-display-white-help = Luminansen af skærmens hvid, målt eller fra skærmens datablad. Bruges af DICOM GSDF.
palette-orientation-labels = Vis eller skjul orienteringsmærker
palette-presentation-state = Næste præsentationstilstand
disabled-no-presentation-state = Ingen præsentationstilstand henviser til dette billede
medical-labels-shown = Orienteringsmærker vist
medical-labels-hidden = Orienteringsmærker skjult
medical-presentation = Præsentationstilstand: { $label } ({ $index } af { $count })
medical-presentation-none = Præsentationstilstand fra
medical-presentation-readout = Præsentation: { $label }
medical-preset-presentation = Præsentationstilstandens opslagstabel bestemmer gråtonerne

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientering: venstre { $left }, højre { $right }, top { $top }, bund { $bottom }
