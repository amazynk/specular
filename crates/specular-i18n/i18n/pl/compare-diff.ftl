# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Porównaj z…
palette-compare-with-help = Porównaj ten obraz z innym, który wybierzesz.
palette-compare-side = Porównaj obok siebie
palette-compare-side-help = Pokaż porównywane obrazy obok siebie.
palette-compare-split = Porównaj z podziałem
palette-compare-split-help = Pokaż drugi obraz na pierwszym, przycięty linią podziału, którą można przeciągać.
palette-compare-split-direction = Obróć linię podziału
palette-compare-split-direction-help = Ustaw linię podziału poziomo zamiast pionowo, lub z powrotem.
palette-compare-overlay = Porównaj jako nakładkę
palette-compare-overlay-help = Pokaż drugi obraz na pierwszym, z przenikaniem.
palette-compare-difference = Porównaj różnicę
palette-compare-difference-help = Pokaż, gdzie obrazy się różnią; ponownie dla następnego rodzaju.
palette-compare-flicker = Migaj między porównywanymi obrazami
palette-compare-flicker-help = Od razu pokaż drugi obraz.
palette-compare-auto-flicker = Migaj automatycznie
palette-compare-auto-flicker-help = Samoczynnie przełączaj między dwoma obrazami, lub zatrzymaj.
palette-compare-align = Wyrównaj porównywane obrazy
palette-compare-align-help = Dopasuj obrazy o różnych rozmiarach: do lewego górnego rogu, do środka, ze skalowaniem lub automatycznie.
palette-compare-metric-space = Przełącz przestrzeń pomiaru
palette-compare-metric-space-help = Mierz w świetle liniowym lub w kodowanym sRGB.
palette-compare-whole-image = Mierz cały obraz
palette-compare-whole-image-help = Mierz cały obraz zamiast widocznej części, lub z powrotem.
palette-compare-probe = Sonda pikseli porównania
palette-compare-probe-help = Pokaż wartości obu obrazów pod wskaźnikiem.
palette-compare-link = Połącz lub rozłącz porównywane obrazy
palette-compare-link-help = Powiększaj i przesuwaj każdy z dwóch obrazów osobno, lub znów razem.
palette-compare-close = Zamknij porównanie
palette-compare-close-help = Wróć do galerii.
disabled-compare-closed = Najpierw otwórz porównanie
disabled-compare-not-two = Działa z dwoma obrazami

## Cheat sheet
cheatsheet-compare-split = Podział lub obok siebie
cheatsheet-compare-split-direction = Linia podziału pozioma lub pionowa
cheatsheet-compare-auto-flicker = Migaj automatycznie
cheatsheet-compare-align = Następne wyrównanie
cheatsheet-compare-space = Przestrzeń pomiaru
cheatsheet-compare-whole = Cały obraz lub widoczna część
cheatsheet-compare-probe = Sonda pikseli
cheatsheet-compare-link = Połącz lub rozłącz dwa obrazy

## Settings
settings-gallery-compare-metric-space = Przestrzeń pomiaru porównania
settings-gallery-compare-metric-space-help = Gdzie porównanie mierzy MAE, MSE, PSNR i SSIM. Światło liniowe to to, co pokazuje ekran. sRGB zgadza się z innymi narzędziami dla plików 8-bit.
settings-gallery-compare-metric-space-linear = Światło liniowe
settings-gallery-compare-metric-space-srgb = Kodowane sRGB
settings-gallery-compare-flicker-limit = Najszybsze automatyczne miganie
settings-gallery-compare-flicker-limit-help = Najwyżej tyle zmian obrazu na sekundę. Powyżej 3 miganie może wywołać napady.

## Toolbar and captions
compare-split = Podział
compare-split-down = Linia podziału pionowa
compare-split-across = Linia podziału pozioma
compare-link = Połączone
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } zmian na sekundę
compare-gain = Wzmocnienie ×{ $gain }
compare-probe = Sonda
compare-kind-signed = Błąd ze znakiem
compare-kind-on-image = Błąd na obrazie
compare-align-top-left = Lewy górny róg
compare-align-centre = Do środka
compare-align-scaled = B przeskalowany do A
compare-align-auto = Automatyczne wyrównanie
compare-space-linear = Światło liniowe
compare-space-srgb = sRGB
compare-space-help-linear = Mierzone w świetle liniowym, jak wyświetlane po mapowaniu tonów. PSNR nie jest tu zwykłym PSNR dla 8-bit sRGB.
compare-space-help-srgb = Mierzone na kodowanych wartościach sRGB, jak robią to inne narzędzia dla plików 8-bit. HDR powyżej bieli SDR liczy się jako biel.
compare-space-help-common = ΔE2000 jest zawsze w CIELAB (D65), biel SDR jako L* 100, HDR obcięte. Obrazy większe niż { $size } pikseli są mierzone na kopii o { $size } pikselach.
compare-region-view = Widoczna część
compare-region-whole = Cały obraz
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM wymaga 11 pikseli
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, maks. { $max })
compare-delta-e-hdr = HDR obcięte dla ΔE
compare-aligned-centre = Do środka, porównano { $width } × { $height }
compare-aligned-scaled = B przeskalowany { $scale }×, porównano { $width } × { $height }
compare-aligned-auto = Wyrównano { $angle }° { $scale }×, porównano { $width } × { $height }
compare-aligning = Wyrównywanie…
compare-align-failed = Nie udało się wyrównać tych obrazów. Powrót do lewego górnego rogu.
compare-split-label = { $b } na { $a }, linia podziału na { PERCENT($percent) }
compare-showing-which = Wyświetlane { $which }: { $name }
compare-auto-flicker-on = Miganie, { $rate } zmian na sekundę
compare-auto-flicker-off = Miganie zatrzymane
compare-linked = Obrazy połączone
compare-unlinked = Obrazy rozłączone: każdy przesuwa się osobno
compare-with-title = Porównaj z
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Linia podziału
compare-a11y-space = Przestrzeń pomiaru
compare-a11y-align = Wyrównanie
