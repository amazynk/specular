# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Medische beelden
palette-window-level = Venster/niveau-gereedschap (links slepen)
palette-window-default = Venster: standaard van bestand
palette-window-lung = Venster: longen (W 1500, L −600)
palette-window-mediastinum = Venster: mediastinum (W 350, L 50)
palette-window-bone = Venster: bot (W 1800, L 400)
palette-window-brain = Venster: hersenen (W 80, L 40)
palette-window-stroke = Venster: beroerte (W 40, L 40)
palette-window-soft-tissue = Venster: weke delen (W 400, L 50)
palette-window-liver = Venster: lever (W 150, L 30)
palette-window-subdural = Venster: subduraal (W 200, L 75)
palette-window-full-range = Venster: volledig bereik
palette-invert-grey = Grijswaarden omkeren
palette-next-slice = Volgend frame
palette-prev-slice = Vorig frame
disabled-not-medical = Alleen voor medische grijswaardenbeelden
disabled-one-frame = Het beeld heeft één frame
settings-section-medical = Medische beelden
settings-medical-keep-window = Venster/niveau behouden voor de volgende serie
settings-medical-keep-window-help = Het ingestelde venster behouden bij het wisselen naar een andere serie, in plaats van te beginnen bij de standaard daarvan. Frames van één bestand en plakken van één serie delen het altijd.
medical-window = C { $center }  W { $width }
medical-inverted = omgekeerd
medical-frame = Frame { $frame } van { $count }
medical-tool = Venster/niveau-gereedschap: slepen
medical-tool-on = Venster/niveau-gereedschap aan: sleep om aan te passen
medical-tool-off = Venster/niveau-gereedschap uit
medical-preset-not-ct = CT-voorinstellingen vereisen waarden in Hounsfield-eenheden
medical-file-window-none = Geen venster { $index } in dit bestand
medical-file-window-unnamed = Venster van het bestand { $index } van { $count }
medical-file-window = Venster van het bestand { $index } van { $count }: { $name }
measure-templates-need-medical = Geometriesjablonen horen bij het profiel Medisch
medical-off = Geen medische hulpmiddelen voor deze afbeelding: { $reason }
medical-readout = Waarde en venster

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Beelden afspelen (cine)
palette-measure-length = Lengte meten
palette-measure-angle = Hoek meten
palette-measure-ellipse = Elliptisch gebied meten
palette-measure-rectangle = Rechthoekig gebied meten
palette-clear-measurements = Metingen wissen
palette-measure-templates = Geometriesjablonen…
palette-overlays = Overlays tonen of verbergen
medical-cine = Afspelen met { $rate } beelden/s
medical-overlays-shown = Overlays getoond
medical-overlays-hidden = Overlays verborgen
measure-hint-length = Lengte: sleep of klik op beide uiteinden
measure-hint-angle = Hoek: klik op drie punten, het hoekpunt als tweede
measure-hint-ellipse = Ellips: sleep over het gebied
measure-hint-rectangle = Rechthoek: sleep over het gebied
measure-off = Meetgereedschap uit
measure-cleared = Metingen gewist
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Oppervlakte { $value } mm²
measure-area-px = Oppervlakte { $value } px²
measure-angle = { $value }°
measure-mean = Gemiddelde { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Max { $max }  ({ $count } px)
measure-caveat-detector = Bij de detector: niet gecorrigeerd voor vergroting
measure-caveat-magnification = Geschatte vergrotingscorrectie
measure-caveat-uncalibrated = Geen pixelafstand: in pixels
measure-caveat-outside-region = Buiten een gekalibreerd echogebied: in pixels

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Oriëntatielabels tonen
settings-medical-orientation-labels-help = Letters aan de beeldranden voor de richtingen van de patiënt (R/L rechts/links, A/P anterieur/posterieur, S/I hoofd/voeten), uit het bestand gehaald en meedraaiend met rotatie en spiegeling. L zet ze aan of uit voor de sessie.
settings-medical-wheel-steps-frames = Muiswiel bladert door frames
settings-medical-wheel-steps-frames-help = In een bestand of serie met meerdere frames gaat het wiel van frame naar frame in plaats van te zoomen of van afbeelding te wisselen. Ctrl+wiel zoomt nog steeds.
# machine-drafted, needs native review
settings-medical-display-function = Grijsweergave
settings-medical-display-function-help = Hoe de grijstinten van medische afbeeldingen worden weergegeven. DICOM GSDF (Grayscale Standard Display Function) verdeelt ze in stappen van gelijke zichtbaarheid tussen de zwart- en witluminantie van het beeldscherm (DICOM PS3.14), zoals op een gekalibreerd diagnostisch beeldscherm. Het kleurprofiel van het beeldscherm blijft van toepassing.
settings-medical-display-function-srgb = Standaard (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Zwartluminantie van het beeldscherm
settings-medical-display-black-help = De luminantie van het zwart van het beeldscherm, inclusief omgevingslicht, gemeten of uit het gegevensblad van het beeldscherm. Gebruikt door DICOM GSDF.
settings-medical-display-white = Witluminantie van het beeldscherm
settings-medical-display-white-help = De luminantie van het wit van het beeldscherm, gemeten of uit het gegevensblad van het beeldscherm. Gebruikt door DICOM GSDF.
palette-orientation-labels = Oriëntatielabels tonen of verbergen
palette-presentation-state = Volgende presentatiestatus
disabled-no-presentation-state = Geen presentatiestatus verwijst naar dit beeld
medical-labels-shown = Oriëntatielabels getoond
medical-labels-hidden = Oriëntatielabels verborgen
medical-presentation = Presentatiestatus: { $label } ({ $index } van { $count })
medical-presentation-none = Presentatiestatus uit
medical-presentation-readout = Presentatie: { $label }
medical-preset-presentation = De opzoektabel van de presentatiestatus bepaalt de grijswaarden

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Oriëntatie: links { $left }, rechts { $right }, boven { $top }, onder { $bottom }
