# machine-drafted, needs native review
# Narzędzia analizy: ostrzeżenia o przepaleniach, focus peaking, histogram, inspektor pikseli. Polski.

## Paleta poleceń
palette-clipping = Pokaż lub ukryj ostrzeżenia o przycięciu
palette-focus-peaking = Pokaż lub ukryj focus peaking
palette-histogram = Pokaż lub ukryj histogram
palette-pixel-inspector = Pokaż lub ukryj inspektor pikseli
# machine-drafted, needs native review
palette-crosshair = Celownik klawiatury
palette-announce-cursor = Odczytaj wartość pod wskaźnikiem

## Panel histogramu
histogram-title = Histogram
# Udział widocznych pikseli na poziomach przycięcia lub poza nimi; wartości są procentami.
histogram-clipped = Przycięte: światła { $high }, cienie { $low }
histogram-empty = Brak widocznego obrazu
# Podsumowanie dla czytnika ekranu. $mean to średnia jasność (0–255), $high/$low to procenty.
histogram-a11y = Histogram widocznego obrazu: średnia jasność { $mean }, przycięte światła { $high }, przycięte cienie { $low }

## Panel inspektora pikseli
inspector-title = Inspektor pikseli
inspector-position = Pozycja
inspector-source-rgb = RGB źródła
inspector-source-rgba = RGBA źródła
inspector-source-ycc = Y′CbCr źródła
inspector-source-gray = Szarość źródła
inspector-source-gray16 = Szarość źródła (16-bitowe)
inspector-source-gray-alpha16 = Szarość źródła + alfa (16-bitowe)
inspector-source-rgb16 = RGB źródła (16-bitowe)
inspector-source-rgba16 = RGBA źródła (16-bitowe)
inspector-source-pending = Źródło
inspector-srgb = sRGB
inspector-hex = Hex
inspector-linear = Liniowe
# Gdy wartości pochodzą ze zmniejszonego podglądu, a nie z pikseli w pełnej rozdzielczości.
inspector-approximate = Ze zmniejszonego podglądu; powiększ, aby uzyskać dokładne wartości
inspector-outside = Wskaż obraz, aby zbadać piksel
# Podsumowanie dla czytnika ekranu: pozycja piksela i jego wartość sRGB.
inspector-a11y = Piksel { $x }, { $y }: sRGB { $value }

## Ustawienia
settings-section-inspection = Analiza
settings-inspection-clip-highlight = Próg przycięcia świateł
settings-inspection-clip-highlight-help = Piksele, których dowolny kanał ma 8-bitową wartość równą lub wyższą, są oznaczane jako przycięte światła.
settings-inspection-clip-shadow = Próg przycięcia cieni
settings-inspection-clip-shadow-help = Piksele, których dowolny kanał ma 8-bitową wartość równą lub niższą, są oznaczane jako przycięte cienie.
settings-inspection-highlight-color = Kolor ostrzeżenia o światłach
settings-inspection-highlight-color-help = Kolor oznaczający przycięte światła.
settings-inspection-shadow-color = Kolor ostrzeżenia o cieniach
settings-inspection-shadow-color-help = Kolor oznaczający przycięte cienie.
settings-inspection-peaking-sensitivity = Czułość focus peakingu
settings-inspection-peaking-sensitivity-help = Wyższe wartości oznaczają jako ostre także łagodniejsze krawędzie.
settings-inspection-peaking-color = Kolor focus peakingu
settings-inspection-peaking-color-help = Kolor oznaczający ostre krawędzie.
settings-inspection-histogram-channels = Kanały histogramu
settings-inspection-histogram-channels-help = Które krzywe pokazuje histogram.
settings-inspection-histogram-channels-colour = Czerwony, zielony i niebieski
settings-inspection-histogram-channels-luminance = Luminancja
settings-inspection-histogram-channels-both = Kolor i luminancja
settings-inspection-histogram-log = Histogram logarytmiczny
settings-inspection-histogram-log-help = Skaluj histogram logarytmicznie, aby małe wartości pozostały widoczne.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Pokaż kanał czerwony
palette-channel-green = Pokaż kanał zielony
palette-channel-blue = Pokaż kanał niebieski
palette-channel-alpha = Pokaż kanał alfa (ponownie: na szachownicy, potem na kolorze)
palette-channel-luma = Pokaż luminancję
channel-normal = Wszystkie kanały
channel-red = Kanał czerwony
channel-green = Kanał zielony
channel-blue = Kanał niebieski
channel-alpha = Kanał alfa
channel-alpha-checker = Przezroczystość na szachownicy
channel-alpha-matte = Przezroczystość na kolorze
channel-luma = Luminancja
disabled-no-alpha = Ten obraz nie ma przezroczystości
settings-inspection-alpha-matte-color = Kolor tła dla alfy
settings-inspection-alpha-matte-color-help = Jednolity kolor za obrazem w widoku kanału „Przezroczystość na kolorze”.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Pokaż lub ukryj fałszywe kolory
palette-next-colour-map = Następna mapa kolorów
palette-channel-colours = Następne kolory kanałów (czerwony–zielony, magenta–zielony, magenta–żółty–cyjan)
palette-hilo = Pokaż lub ukryj HiLo (piksele na maksimum i minimum głębi bitowej)
palette-colour-vision = Następna symulacja widzenia barw
palette-colour-vision-off = Wyłącz symulację widzenia barw
palette-colour-vision-protanopia = Symuluj protanopię (brak czopków czerwonych)
palette-colour-vision-deuteranopia = Symuluj deuteranopię (brak czopków zielonych)
palette-colour-vision-tritanopia = Symuluj tritanopię (brak czopków niebieskich)
palette-colour-vision-achromatopsia = Symuluj achromatopsję (brak barw)
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
colour-map-jet = Jet (nieprzyjazna dla daltonistów)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = surowe wartości
# Screen reader summary of the colour bar.
colour-bar-a11y = Pasek kolorów { $map }: od { $low } do { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Fałszywe kolory: { $map }
badge-hilo = HiLo: zaznaczanie pikseli na maksimum i minimum
badge-colour-vision-protanopia = Symulacja protanopii
badge-colour-vision-deuteranopia = Symulacja deuteranopii
badge-colour-vision-tritanopia = Symulacja tritanopii
badge-colour-vision-achromatopsia = Symulacja achromatopsji
channel-colours-red-green = Kanały: czerwony, zielony, niebieski
channel-colours-magenta-green = Kanały: magenta, zielony
channel-colours-magenta-yellow-cyan = Kanały: magenta, żółty, cyjan
settings-inspection-false-colour-map = Mapa fałszywych kolorów
settings-inspection-false-colour-map-help = Mapa kolorów dla fałszywych kolorów i ich paska. Wszystkie oprócz jet i phase pozostają czytelne przy każdym widzeniu barw.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (rozbieżna)
settings-inspection-false-colour-map-vik = Vik (rozbieżna)
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
settings-inspection-false-colour-map-balance = Balance (rozbieżna)
settings-inspection-false-colour-map-curl = Curl (rozbieżna)
settings-inspection-false-colour-map-phase = Phase (cykliczna)
settings-inspection-false-colour-map-jet = Jet (nieprzyjazna dla daltonistów)
settings-inspection-colour-bar = Pasek kolorów
settings-inspection-colour-bar-help = Pokazuj pasek kolorów z wartościami w jednostkach obrazu, gdy fałszywe kolory są włączone.
settings-inspection-channel-colours = Kolory kanałów
settings-inspection-channel-colours-help = Jak są pokazywane kanały kolorów. Magenta–zielony i magenta–żółty–cyjan pozostają rozróżnialne przy daltonizmie czerwono-zielonym.
settings-inspection-channel-colours-red-green = Czerwony, zielony, niebieski
settings-inspection-channel-colours-magenta-green = Magenta, zielony
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, żółty, cyjan
settings-inspection-colour-vision = Symulacja widzenia barw
settings-inspection-colour-vision-help = Pokazuj całe okno tak, jak widzi je osoba z zaburzeniem widzenia barw (Machado et al. 2009). Nie wpływa na eksport.
settings-inspection-colour-vision-off = Wył.
settings-inspection-colour-vision-protanopia = Protanopia
settings-inspection-colour-vision-deuteranopia = Deuteranopia
settings-inspection-colour-vision-tritanopia = Tritanopia
settings-inspection-colour-vision-achromatopsia = Achromatopsja
settings-inspection-auto-range = Automatyczny zakres dla szarości o wysokiej głębi
settings-inspection-auto-range-help = Rozciągaj 16-bitowe obrazy w skali szarości (TIFF, PNG, FITS) od najciemniejszej do najjaśniejszej wartości. Obrazy DICOM zachowują własne okno.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Celownik włączony. Strzałki go przesuwają, Shift daje większe kroki. Enter klika. Esc kończy.
crosshair-off = Celownik wyłączony
announce-cursor-nothing = Nic pod wskaźnikiem
