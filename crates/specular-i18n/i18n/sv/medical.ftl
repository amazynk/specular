# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Medicinska bilder
palette-window-level = Fönster/nivå-verktyg (vänsterdra)
palette-window-default = Fönster: filens standard
palette-window-lung = Fönster: lunga (W 1500, L −600)
palette-window-mediastinum = Fönster: mediastinum (W 350, L 50)
palette-window-bone = Fönster: skelett (W 1800, L 400)
palette-window-brain = Fönster: hjärna (W 80, L 40)
palette-window-stroke = Fönster: stroke (W 40, L 40)
palette-window-soft-tissue = Fönster: mjukdelar (W 400, L 50)
palette-window-liver = Fönster: lever (W 150, L 30)
palette-window-subdural = Fönster: subduralt (W 200, L 75)
palette-window-full-range = Fönster: hela intervallet
palette-invert-grey = Invertera gråskala
palette-next-slice = Nästa bildruta
palette-prev-slice = Föregående bildruta
disabled-not-medical = Endast för medicinska gråskalebilder
disabled-one-frame = Bilden har en bildruta
settings-section-medical = Medicinska bilder
settings-medical-keep-window = Behåll fönster/nivå för nästa serie
settings-medical-keep-window-help = Behåll fönstret du ställt in när du går till en annan serie, i stället för att börja från dess standard. Bildrutor i en fil och snitt i en serie delar det alltid.
medical-window = C { $center }  W { $width }
medical-inverted = inverterad
medical-frame = Bildruta { $frame } av { $count }
medical-tool = Fönster/nivå-verktyg: dra
medical-tool-on = Fönster/nivå-verktyg på: dra för att justera
medical-tool-off = Fönster/nivå-verktyg av
medical-preset-not-ct = CT-förval kräver värden i Hounsfield-enheter
medical-file-window-none = Inget fönster { $index } i den här filen
medical-file-window-unnamed = Filens fönster { $index } av { $count }
medical-file-window = Filens fönster { $index } av { $count }: { $name }
measure-templates-need-medical = Geometrimallar ingår i profilen Medicin
medical-off = Inga medicinska verktyg för den här bilden: { $reason }
medical-readout = Värde och fönster

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Spela upp bildrutor (cine)
palette-measure-length = Mät längd
palette-measure-angle = Mät vinkel
palette-measure-ellipse = Mät elliptiskt område
palette-measure-rectangle = Mät rektangulärt område
palette-clear-measurements = Rensa mätningar
palette-measure-templates = Geometrimallar…
palette-overlays = Visa eller dölj överlägg
medical-cine = Spelar upp med { $rate } bildrutor/s
medical-overlays-shown = Överlägg visas
medical-overlays-hidden = Överlägg dolda
measure-hint-length = Längd: dra eller klicka på båda ändarna
measure-hint-angle = Vinkel: klicka på tre punkter, spetsen som andra
measure-hint-ellipse = Ellips: dra över området
measure-hint-rectangle = Rektangel: dra över området
measure-off = Mätverktyget av
measure-cleared = Mätningar rensade
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Area { $value } mm²
measure-area-px = Area { $value } px²
measure-angle = { $value }°
measure-mean = Medel { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Max { $max }  ({ $count } px)
measure-caveat-detector = Vid detektorn: inte korrigerat för förstoring
measure-caveat-magnification = Uppskattad förstoringskorrigering
measure-caveat-uncalibrated = Inget pixelavstånd: i pixlar
measure-caveat-outside-region = Utanför ett kalibrerat ultraljudsområde: i pixlar

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Visa orienteringsetiketter
settings-medical-orientation-labels-help = Bokstäver vid bildens kanter för patientens riktningar (R/L höger/vänster, A/P anterior/posterior, S/I huvud/fötter), hämtade från filen och följer rotation och spegling. L slår på eller av dem under sessionen.
settings-medical-wheel-steps-frames = Mushjulet stegar genom bildrutor
settings-medical-wheel-steps-frames-help = I en fil eller serie med flera bildrutor flyttar hjulet mellan bildrutorna i stället för att zooma eller byta bild. Ctrl+hjul zoomar fortfarande.
# machine-drafted, needs native review
settings-medical-display-function = Gråskalevisning
settings-medical-display-function-help = Hur gråtonerna i medicinska bilder visas. DICOM GSDF (standardvisningsfunktion för gråskala) fördelar dem i steg med lika synlighet mellan skärmens svart- och vitluminans (DICOM PS3.14), som på en kalibrerad diagnostisk skärm. Skärmens färgprofil gäller fortfarande.
settings-medical-display-function-srgb = Standard (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Skärmens svartluminans
settings-medical-display-black-help = Luminansen hos skärmens svart, inklusive omgivande ljus, uppmätt eller från skärmens datablad. Används av DICOM GSDF.
settings-medical-display-white = Skärmens vitluminans
settings-medical-display-white-help = Luminansen hos skärmens vitt, uppmätt eller från skärmens datablad. Används av DICOM GSDF.
palette-orientation-labels = Visa eller dölj orienteringsetiketter
palette-presentation-state = Nästa presentationstillstånd
disabled-no-presentation-state = Inget presentationstillstånd hänvisar till den här bilden
medical-labels-shown = Orienteringsetiketter visas
medical-labels-hidden = Orienteringsetiketter dolda
medical-presentation = Presentationstillstånd: { $label } ({ $index } av { $count })
medical-presentation-none = Presentationstillstånd av
medical-presentation-readout = Presentation: { $label }
medical-preset-presentation = Presentationstillståndets uppslagstabell bestämmer gråskalan

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientering: vänster { $left }, höger { $right }, överkant { $top }, nederkant { $bottom }
