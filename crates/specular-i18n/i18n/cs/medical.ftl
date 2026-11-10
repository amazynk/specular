# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Lékařské snímky
palette-window-level = Nástroj okna/úrovně (tažení levým)
palette-window-default = Okno: výchozí ze souboru
palette-window-lung = Okno: plíce (W 1500, L −600)
palette-window-mediastinum = Okno: mediastinum (W 350, L 50)
palette-window-bone = Okno: kost (W 1800, L 400)
palette-window-brain = Okno: mozek (W 80, L 40)
palette-window-stroke = Okno: CMP (W 40, L 40)
palette-window-soft-tissue = Okno: měkké tkáně (W 400, L 50)
palette-window-liver = Okno: játra (W 150, L 30)
palette-window-subdural = Okno: subdurální (W 200, L 75)
palette-window-full-range = Okno: celý rozsah
palette-invert-grey = Invertovat stupně šedi
palette-next-slice = Další snímek
palette-prev-slice = Předchozí snímek
disabled-not-medical = Jen pro lékařské snímky ve stupních šedi
disabled-one-frame = Obrázek má jediný snímek
settings-section-medical = Lékařské snímky
settings-medical-keep-window = Zachovat okno/úroveň pro další sérii
settings-medical-keep-window-help = Zachová nastavené okno při přechodu na jinou sérii místo začátku z její výchozí hodnoty. Snímky jednoho souboru a řezy jedné série ho sdílejí vždy.
medical-window = C { $center }  W { $width }
medical-inverted = invertováno
medical-frame = Snímek { $frame } z { $count }
medical-tool = Nástroj okna/úrovně: táhněte
medical-tool-on = Nástroj okna/úrovně zapnut: tažením upravte
medical-tool-off = Nástroj okna/úrovně vypnut
medical-preset-not-ct = Předvolby CT vyžadují hodnoty v Hounsfieldových jednotkách
medical-file-window-none = Tento soubor nemá okno { $index }
medical-file-window-unnamed = Okno souboru { $index } z { $count }
medical-file-window = Okno souboru { $index } z { $count }: { $name }
measure-templates-need-medical = Geometrické šablony jsou součástí profilu Medicína
medical-off = Pro tento obrázek nejsou lékařské nástroje: { $reason }
medical-readout = Hodnota a okno

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Přehrát snímky (cine)
palette-measure-length = Změřit délku
palette-measure-angle = Změřit úhel
palette-measure-ellipse = Změřit eliptickou oblast
palette-measure-rectangle = Změřit obdélníkovou oblast
palette-clear-measurements = Vymazat měření
palette-measure-templates = Geometrické šablony…
palette-overlays = Zobrazit nebo skrýt překryvy
medical-cine = Přehrávání: { $rate } sn./s
medical-overlays-shown = Překryvy zobrazeny
medical-overlays-hidden = Překryvy skryty
measure-hint-length = Délka: táhněte, nebo klikněte na oba konce
measure-hint-angle = Úhel: klikněte na tři body, vrchol jako druhý
measure-hint-ellipse = Elipsa: táhněte přes oblast
measure-hint-rectangle = Obdélník: táhněte přes oblast
measure-off = Nástroj měření vypnut
measure-cleared = Měření vymazána
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Plocha { $value } mm²
measure-area-px = Plocha { $value } px²
measure-angle = { $value }°
measure-mean = Průměr { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Max { $max }  ({ $count } px)
measure-caveat-detector = Na detektoru: bez korekce zvětšení
measure-caveat-magnification = Odhadnutá korekce zvětšení
measure-caveat-uncalibrated = Bez rozteče pixelů: v pixelech
measure-caveat-outside-region = Mimo kalibrovanou ultrazvukovou oblast: v pixelech

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Zobrazit orientační popisky
settings-medical-orientation-labels-help = Písmena na okrajích obrazu udávající směry vůči pacientovi (R/L vpravo/vlevo, A/P anteriorně/posteriorně, S/I hlava/nohy), převzatá ze souboru a sledující otočení i převrácení. Klávesa L je přepíná pro aktuální relaci.
settings-medical-wheel-steps-frames = Kolečko myši prochází snímky
settings-medical-wheel-steps-frames-help = V souboru nebo sérii s více snímky přechází kolečko mezi snímky místo přibližování nebo změny obrázku. Ctrl+kolečko dál přibližuje.
# machine-drafted, needs native review
settings-medical-display-function = Zobrazení šedi
settings-medical-display-function-help = Jak se zobrazují odstíny šedi lékařských snímků. DICOM GSDF (standardní zobrazovací funkce stupňů šedi) je rozloží do kroků stejné viditelnosti mezi jasem černé a bílé displeje (DICOM PS3.14), jako na kalibrovaném diagnostickém displeji. Barevný profil displeje se dál uplatňuje.
settings-medical-display-function-srgb = Standardní (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Jas černé displeje
settings-medical-display-black-help = Jas černé displeje včetně okolního světla, změřený nebo z technického listu displeje. Používá ho DICOM GSDF.
settings-medical-display-white = Jas bílé displeje
settings-medical-display-white-help = Jas bílé displeje, změřený nebo z technického listu displeje. Používá ho DICOM GSDF.
palette-orientation-labels = Zobrazit nebo skrýt orientační popisky
palette-presentation-state = Další prezentační stav
disabled-no-presentation-state = Na tento obrázek neodkazuje žádný prezentační stav
medical-labels-shown = Orientační popisky zobrazeny
medical-labels-hidden = Orientační popisky skryty
medical-presentation = Prezentační stav: { $label } ({ $index } z { $count })
medical-presentation-none = Prezentační stav vypnut
medical-presentation-readout = Prezentace: { $label }
medical-preset-presentation = Úrovně šedi určuje převodní tabulka prezentačního stavu

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientace: vlevo { $left }, vpravo { $right }, nahoře { $top }, dole { $bottom }
