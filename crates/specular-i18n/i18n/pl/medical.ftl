# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Obrazy medyczne
palette-window-level = Narzędzie okna/poziomu (przeciąganie lewym)
palette-window-default = Okno: domyślne z pliku
palette-window-lung = Okno: płuca (W 1500, L −600)
palette-window-mediastinum = Okno: śródpiersie (W 350, L 50)
palette-window-bone = Okno: kości (W 1800, L 400)
palette-window-brain = Okno: mózg (W 80, L 40)
palette-window-stroke = Okno: udar (W 40, L 40)
palette-window-soft-tissue = Okno: tkanki miękkie (W 400, L 50)
palette-window-liver = Okno: wątroba (W 150, L 30)
palette-window-subdural = Okno: podtwardówkowe (W 200, L 75)
palette-window-full-range = Okno: pełny zakres
palette-invert-grey = Odwróć skalę szarości
palette-next-slice = Następna klatka
palette-prev-slice = Poprzednia klatka
disabled-not-medical = Tylko dla medycznych obrazów w skali szarości
disabled-one-frame = Obraz ma jedną klatkę
settings-section-medical = Obrazy medyczne
settings-medical-keep-window = Zachowaj okno/poziom dla następnej serii
settings-medical-keep-window-help = Zachowuje ustawione okno przy przejściu do innej serii zamiast zaczynać od jej wartości domyślnej. Klatki jednego pliku i warstwy jednej serii zawsze je współdzielą.
medical-window = C { $center }  W { $width }
medical-inverted = odwrócone
medical-frame = Klatka { $frame } z { $count }
medical-tool = Narzędzie okna/poziomu: przeciągnij
medical-tool-on = Narzędzie okna/poziomu włączone: przeciągnij, aby dostosować
medical-tool-off = Narzędzie okna/poziomu wyłączone
medical-preset-not-ct = Ustawienia TK wymagają wartości w jednostkach Hounsfielda
medical-file-window-none = Brak okna { $index } w tym pliku
medical-file-window-unnamed = Okno z pliku { $index } z { $count }
medical-file-window = Okno z pliku { $index } z { $count }: { $name }
measure-templates-need-medical = Szablony geometryczne są dostępne w profilu Medycyna
medical-off = Brak narzędzi medycznych dla tego obrazu: { $reason }
medical-readout = Wartość i okno

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Odtwarzaj klatki (cine)
palette-measure-length = Zmierz długość
palette-measure-angle = Zmierz kąt
palette-measure-ellipse = Zmierz obszar eliptyczny
palette-measure-rectangle = Zmierz obszar prostokątny
palette-clear-measurements = Wyczyść pomiary
palette-measure-templates = Szablony geometryczne…
palette-overlays = Pokaż lub ukryj nakładki
medical-cine = Odtwarzanie: { $rate } kl./s
medical-overlays-shown = Nakładki widoczne
medical-overlays-hidden = Nakładki ukryte
measure-hint-length = Długość: przeciągnij lub kliknij oba końce
measure-hint-angle = Kąt: kliknij trzy punkty, wierzchołek jako drugi
measure-hint-ellipse = Elipsa: przeciągnij przez obszar
measure-hint-rectangle = Prostokąt: przeciągnij przez obszar
measure-off = Narzędzie pomiaru wyłączone
measure-cleared = Pomiary wyczyszczone
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Pole { $value } mm²
measure-area-px = Pole { $value } px²
measure-angle = { $value }°
measure-mean = Średnia { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Maks { $max }  ({ $count } px)
measure-caveat-detector = Na detektorze: bez korekcji powiększenia
measure-caveat-magnification = Szacowana korekcja powiększenia
measure-caveat-uncalibrated = Brak odstępu pikseli: w pikselach
measure-caveat-outside-region = Poza skalibrowanym obszarem USG: w pikselach

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Pokaż oznaczenia orientacji
settings-medical-orientation-labels-help = Litery przy krawędziach obrazu oznaczające kierunki względem pacjenta (R/L prawo/lewo, A/P przód/tył, S/I głowa/stopy), pobrane z pliku i uwzględniające obrót oraz odbicie. Klawisz L przełącza je na czas sesji.
settings-medical-wheel-steps-frames = Kółko myszy przewija klatki
settings-medical-wheel-steps-frames-help = W pliku lub serii z wieloma klatkami kółko przechodzi między klatkami zamiast powiększać lub zmieniać obraz. Ctrl+kółko nadal powiększa.
# machine-drafted, needs native review
settings-medical-display-function = Wyświetlanie szarości
settings-medical-display-function-help = Sposób wyświetlania odcieni szarości obrazów medycznych. DICOM GSDF (standardowa funkcja wyświetlania skali szarości) rozkłada je w krokach jednakowej widoczności między luminancją czerni i bieli wyświetlacza (DICOM PS3.14), jak na skalibrowanym monitorze diagnostycznym. Profil kolorów wyświetlacza nadal obowiązuje.
settings-medical-display-function-srgb = Standardowe (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Luminancja czerni wyświetlacza
settings-medical-display-black-help = Luminancja czerni wyświetlacza, łącznie ze światłem otoczenia, z pomiaru lub karty katalogowej wyświetlacza. Używana przez DICOM GSDF.
settings-medical-display-white = Luminancja bieli wyświetlacza
settings-medical-display-white-help = Luminancja bieli wyświetlacza, z pomiaru lub karty katalogowej wyświetlacza. Używana przez DICOM GSDF.
palette-orientation-labels = Pokaż lub ukryj oznaczenia orientacji
palette-presentation-state = Następny stan prezentacji
disabled-no-presentation-state = Żaden stan prezentacji nie odnosi się do tego obrazu
medical-labels-shown = Oznaczenia orientacji widoczne
medical-labels-hidden = Oznaczenia orientacji ukryte
medical-presentation = Stan prezentacji: { $label } ({ $index } z { $count })
medical-presentation-none = Stan prezentacji wyłączony
medical-presentation-readout = Prezentacja: { $label }
medical-preset-presentation = Poziomy szarości ustala tablica przekodowań stanu prezentacji

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientacja: lewo { $left }, prawo { $right }, góra { $top }, dół { $bottom }
