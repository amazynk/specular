# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Polski.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Jeden moduł tej edycji zaktualizowano do nowszej wersji. Wynik może wyglądać nieco inaczej.
    [few] { $count } moduły tej edycji zaktualizowano do nowszej wersji. Wynik może wyglądać nieco inaczej.
    [many] { $count } modułów tej edycji zaktualizowano do nowszej wersji. Wynik może wyglądać nieco inaczej.
   *[other] { $count } modułu tej edycji zaktualizowano do nowszej wersji. Wynik może wyglądać nieco inaczej.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Jeden moduł tej edycji wymaga nowszej wersji programu Specular. Zostaje zachowany, ale nie jest wyświetlany.
    [few] { $count } moduły tej edycji wymagają nowszej wersji programu Specular. Zostają zachowane, ale nie są wyświetlane.
    [many] { $count } modułów tej edycji wymaga nowszej wersji programu Specular. Zostają zachowane, ale nie są wyświetlane.
   *[other] { $count } modułu tej edycji wymaga nowszej wersji programu Specular. Zostają zachowane, ale nie są wyświetlane.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Punkt { $index } z { $count }: wejście { $input }, wyjście { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Strzałki przesuwają punkt. Page Up i Page Down wybierają inny. Insert dodaje punkt, Delete go usuwa.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutralny
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Barwa { $hue }°, siła { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Strzałki przesuwają kolor. Delete go zeruje.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Wywoływanie
# machine-drafted, needs native review
develop-undo = Cofnij
# machine-drafted, needs native review
develop-redo = Ponów
# machine-drafted, needs native review
develop-before-after = Przed i po
# machine-drafted, needs native review
develop-close = Opuść wywoływanie
# machine-drafted, needs native review
develop-history = Historia
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module }: włączone
develop-card-menu = { $module }: opcje
develop-card-changed = Zmieniono
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Taśma filmowa

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Ekspozycja
develop-module-light = Światło
develop-look-profile = Profil
develop-look-standard = Standardowy
develop-look-film = Film
develop-look-natural = Naturalny
develop-module-grading = Gradacja kolorów
develop-add-tool = Dodaj narzędzie
# machine-drafted, needs native review
develop-module-white-balance = Balans bieli
# machine-drafted, needs native review
develop-module-tone = Ton
# machine-drafted, needs native review
develop-module-contrast = Kontrast i zakres
# machine-drafted, needs native review
develop-module-colour = Kolor
# machine-drafted, needs native review
develop-module-curve = Krzywa
# machine-drafted, needs native review
develop-module-crop = Kadrowanie i prostowanie
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Inny moduł
# machine-drafted, needs native review
develop-exposure = Ekspozycja
# machine-drafted, needs native review
develop-black-level = Poziom czerni
# machine-drafted, needs native review
develop-temperature = Temperatura
# machine-drafted, needs native review
develop-tint = Odcień
# machine-drafted, needs native review
develop-highlights = Światła
# machine-drafted, needs native review
develop-shadows = Cienie
# machine-drafted, needs native review
develop-contrast = Kontrast
# machine-drafted, needs native review
develop-whites = Biele
# machine-drafted, needs native review
develop-blacks = Czernie
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = Limit HDR
# machine-drafted, needs native review
develop-hdr-preview = Pokaż
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Jaskrawość
# machine-drafted, needs native review
develop-saturation = Nasycenie
# machine-drafted, needs native review
develop-colour-wheels = Koła kolorów
# machine-drafted, needs native review
develop-curve = Krzywa tonalna
# machine-drafted, needs native review
develop-module-local-contrast = Kontrast lokalny
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Automatyczna ekspozycja
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Wygładzanie
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Tekstura
# machine-drafted, needs native review
develop-clarity = Klarowność
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Punkt czerni
# machine-drafted, needs native review
develop-levels-mid = Punkt środkowy
# machine-drafted, needs native review
develop-levels-white = Punkt bieli
# machine-drafted, needs native review
develop-module-detail = Szczegóły
# machine-drafted, needs native review
develop-module-denoise = Redukcja szumu
# machine-drafted, needs native review
develop-module-sharpen = Wyostrzanie
# machine-drafted, needs native review
develop-noise-luminance = Szum luminancji
# machine-drafted, needs native review
develop-noise-detail = Szczegóły szumu
# machine-drafted, needs native review
develop-noise-colour = Szum koloru
# machine-drafted, needs native review
develop-sharpen-amount = Siła wyostrzania
# machine-drafted, needs native review
develop-sharpen-radius = Promień wyostrzania
# machine-drafted, needs native review
develop-sharpen-detail = Szczegóły wyostrzania
# machine-drafted, needs native review
develop-sharpen-masking = Maskowanie
# machine-drafted, needs native review
develop-capture-amount = Wyostrzanie wejściowe
# machine-drafted, needs native review
develop-capture-radius = Promień wejściowy
# machine-drafted, needs native review
develop-straighten = Prostowanie
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Kadrowanie
# machine-drafted, needs native review
develop-crop-turn = Obróć
# machine-drafted, needs native review
develop-crop-flip = Odbij
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Korekcja obiektywu
develop-module-optics = Optyka
# machine-drafted, needs native review
develop-lens-distortion = Dystorsja
# machine-drafted, needs native review
develop-lens-vignette = Winietowanie
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Czerwona obwódka
# machine-drafted, needs native review
develop-lens-tca-blue = Niebieska obwódka
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Profil obiektywu
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automatycznie
# machine-drafted, needs native review
develop-lensfun-off = Brak
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Brak profilu obiektywu.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Brak profilu Lensfun dla { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Brak profilu Lensfun dla tego obiektywu.
# machine-drafted, needs native review
develop-lensfun-no-name = Plik nie podaje nazwy obiektywu. Wybierz go powyżej.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = Pasujące obiektywy: { $count }. Wybierz jeden powyżej.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Kadrowanie. Przeciągnij krawędzie albo Ctrl+przeciągnij wzdłuż linii, która ma być pozioma. Enter zatwierdza, Esc wraca.
# machine-drafted, needs native review
develop-crop-kept = Kadr zatwierdzony
# machine-drafted, needs native review
develop-crop-cancelled = Kadr cofnięty
# machine-drafted, needs native review
develop-crop-reset = Kadrowanie i prostowanie wyzerowane
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Wyprostowano o { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Proporcje { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = dowolne
# machine-drafted, needs native review
develop-crop-aspect-original = oryginalne
# machine-drafted, needs native review
develop-crop-turned-right = Obrócono w prawo
# machine-drafted, needs native review
develop-crop-turned-left = Obrócono w lewo
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Odbito w poziomie
# machine-drafted, needs native review
develop-crop-flipped-vertical = Odbito w pionie
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Proporcje { $aspect } · Linie pomocnicze: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A proporcje · X zamień · O linie · [ ] obróć · H V odbij · Enter zatwierdź · Esc wróć
# machine-drafted, needs native review
develop-crop-guide-thirds = trójpodział
# machine-drafted, needs native review
develop-crop-guide-golden = złoty podział
# machine-drafted, needs native review
develop-crop-guide-diagonal = przekątne
# machine-drafted, needs native review
develop-crop-guide-grid = siatka

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspektywa
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Pionowanie
# machine-drafted, needs native review
develop-persp-mode-off = Wył.
# machine-drafted, needs native review
develop-persp-mode-auto = Automatycznie
# machine-drafted, needs native review
develop-persp-mode-level = Poziom
# machine-drafted, needs native review
develop-persp-mode-vertical = Pion
# machine-drafted, needs native review
develop-persp-mode-full = Pełne
# machine-drafted, needs native review
develop-persp-mode-guided = Z liniami
# machine-drafted, needs native review
develop-persp-vertical = Pion
# machine-drafted, needs native review
develop-persp-horizontal = Poziom
# machine-drafted, needs native review
develop-persp-rotate = Obrót
# machine-drafted, needs native review
develop-persp-aspect = Proporcje
# machine-drafted, needs native review
develop-persp-scale = Skala
# machine-drafted, needs native review
develop-persp-offset-x = W bok
# machine-drafted, needs native review
develop-persp-offset-y = W górę lub w dół
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Bez pustych krawędzi
# machine-drafted, needs native review
develop-persp-guides = Linie pomocnicze
# machine-drafted, needs native review
develop-persp-focal = Ogniskowa
# machine-drafted, needs native review
develop-persp-draw-guides = Rysuj linie
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Rysuj linie ({ $n } z { $max })
# machine-drafted, needs native review
develop-persp-reset = Wyzeruj perspektywę
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Pionowanie: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Szukanie linii…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Wyzerować perspektywę? Linie pomocnicze ({ $n }) zostaną usunięte. Cofnij je przywróci.
# machine-drafted, needs native review
develop-persp-reset-ok = Wyzeruj
# machine-drafted, needs native review
develop-persp-reset-done = Perspektywa wyzerowana
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Usunąć linię pomocniczą { $n }? Cofnij ją przywróci.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Usuń
# machine-drafted, needs native review
develop-persp-guide-removed = Linia pomocnicza usunięta
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Pion { $vertical }, poziom { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Pionowanie nie powiodło się: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: pochylenie { $vertical }°, skręt { $horizontal }°, obrót { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = Tryb { $mode } potrzebuje co najmniej dwóch prostych linii, które da się rozróżnić. Spróbuj innego trybu albo narysuj linie pomocnicze.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Maksymalna liczba linii pomocniczych: { $n }. Przeciągnij koniec, aby przesunąć linię.
# machine-drafted, needs native review
develop-persp-guides-on = Linie pomocnicze. Przeciągnij wzdłuż linii, która ma być pionowa lub pozioma.
# machine-drafted, needs native review
develop-persp-guides-off = Kadrowanie
# machine-drafted, needs native review
develop-persp-guide-moved = Linia pomocnicza przesunięta
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Dodano linię pomocniczą { $n }
# machine-drafted, needs native review
develop-persp-guides-keys = Przeciągnij, aby dodać · Przeciągnij koniec, aby przesunąć · Delete usuwa · Shift+R kadrowanie
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Linie pomocnicze: { $n } z { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Cienie
# machine-drafted, needs native review
develop-wheel-midtones = Tony średnie
# machine-drafted, needs native review
develop-wheel-highlights = Światła
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminancja cieni
# machine-drafted, needs native review
develop-luminance-midtones = Luminancja tonów średnich
# machine-drafted, needs native review
develop-luminance-highlights = Luminancja świateł
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Całość
# machine-drafted, needs native review
develop-luminance-global = Luminancja całości
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Intensywność kolorów
# machine-drafted, needs native review
develop-brilliance = Jasność kolorów

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Mikser kolorów
# machine-drafted, needs native review
develop-colour-red = Czerwony
# machine-drafted, needs native review
develop-colour-orange = Pomarańczowy
# machine-drafted, needs native review
develop-colour-yellow = Żółty
# machine-drafted, needs native review
develop-colour-green = Zielony
# machine-drafted, needs native review
develop-colour-aqua = Akwamaryna
# machine-drafted, needs native review
develop-colour-blue = Niebieski
# machine-drafted, needs native review
develop-colour-purple = Fioletowy
# machine-drafted, needs native review
develop-colour-magenta = Purpurowy
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Barwa – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Nasycenie – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminancja – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Kolor punktowy
# machine-drafted, needs native review
develop-point-pick = Wybierz kolor
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Wybierz inny kolor
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Kliknij kolor do zmiany. Esc anuluje.
# machine-drafted, needs native review
develop-point-picked = Kolor wybrany
# machine-drafted, needs native review
develop-point-cancelled = Wybieranie anulowane
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Zakres
# machine-drafted, needs native review
develop-point-hue-range = Zakres odcienia
# machine-drafted, needs native review
develop-point-saturation-range = Zakres nasycenia
# machine-drafted, needs native review
develop-point-lightness-range = Zakres jasności
# machine-drafted, needs native review
develop-point-hue = Przesunięcie barwy
# machine-drafted, needs native review
develop-point-saturation = Przesunięcie nasycenia
# machine-drafted, needs native review
develop-point-lightness = Przesunięcie jasności

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Czarno-białe
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminancja – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Wybierz plik LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Plik: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Nie można odczytać { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Wybierz plik LUT
# machine-drafted, needs native review
develop-lut-kind = Pliki LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Przeznaczona dla
# machine-drafted, needs native review
develop-lut-space-linear = Liniowy (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES liniowy (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Zdjęcie (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Wideo (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Metoda
# machine-drafted, needs native review
develop-lut-tetrahedral = Dokładna
# machine-drafted, needs native review
develop-lut-trilinear = Prosta
# machine-drafted, needs native review
develop-lut-strength = Siła

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Oryginał
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module }: włączone
# machine-drafted, needs native review
develop-history-off = { $module }: wyłączone
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Powrót do oryginału
# machine-drafted, needs native review
develop-at-step = Powrót do: { $step }
# machine-drafted, needs native review
develop-undone = Cofnięto: { $step }
# machine-drafted, needs native review
develop-redone = Ponowiono: { $step }
# machine-drafted, needs native review
develop-entered = Wywoływanie. Edycja nigdy nie zmienia pliku.
# machine-drafted, needs native review
develop-left = Opuszczono wywoływanie
# machine-drafted, needs native review
develop-split-on = Przed i po obok siebie
# machine-drafted, needs native review
develop-split-off = Tylko edycja
# machine-drafted, needs native review
develop-showing-before = Widać oryginał
# machine-drafted, needs native review
develop-showing-after = Widać edycję
# machine-drafted, needs native review
# Labels over the image.
develop-before = Przed
# machine-drafted, needs native review
develop-after = Po
# machine-drafted, needs native review
develop-working = Wywoływanie…
# machine-drafted, needs native review
develop-preview-failed = Nie można pokazać edycji tego pliku

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Wywoływanie
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Szybka edycja: przytrzymaj, potem przewiń
# machine-drafted, needs native review
palette-develop = Wywoływanie
# machine-drafted, needs native review
palette-develop-help = Edytuj to zdjęcie bez zmiany pliku. Ponownie, aby wyjść.
# machine-drafted, needs native review
palette-develop-before-after = Przed i po
# machine-drafted, needs native review
palette-develop-before-after-help = Pokaż oryginał albo z powrotem edycję.
# machine-drafted, needs native review
palette-develop-split = Przed i po – podział
# machine-drafted, needs native review
palette-develop-split-help = Pokaż oryginał i edycję obok siebie.
# machine-drafted, needs native review
palette-develop-panel = Przejdź do panelu wywoływania
# machine-drafted, needs native review
palette-develop-panel-help = Przenieś klawiaturę do panelu. Esc wraca.
# machine-drafted, needs native review
palette-develop-speed-exposure = Szybka edycja ekspozycji
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Przytrzymaj, potem przewiń lub naciśnij strzałki, aby zmienić ekspozycję.
# machine-drafted, needs native review
palette-develop-speed-contrast = Szybka edycja kontrastu
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Przytrzymaj, potem przewiń lub naciśnij strzałki, aby zmienić kontrast.
# machine-drafted, needs native review
palette-develop-speed-highlights = Szybka edycja świateł
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Przytrzymaj, potem przewiń lub naciśnij strzałki, aby zmienić światła.
# machine-drafted, needs native review
palette-develop-speed-shadows = Szybka edycja cieni
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Przytrzymaj, potem przewiń lub naciśnij strzałki, aby zmienić cienie.
# machine-drafted, needs native review
palette-develop-speed-whites = Szybka edycja bieli
# machine-drafted, needs native review
palette-develop-speed-whites-help = Przytrzymaj, potem przewiń lub naciśnij strzałki, aby zmienić biele.
# machine-drafted, needs native review
palette-develop-speed-blacks = Szybka edycja czerni
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Przytrzymaj, potem przewiń lub naciśnij strzałki, aby zmienić czernie.
# machine-drafted, needs native review
palette-develop-speed-temperature = Szybka edycja temperatury
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Przytrzymaj, potem przewiń lub naciśnij strzałki, aby zmienić temperaturę balansu bieli.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Szybka edycja jaskrawości
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Przytrzymaj, potem przewiń lub naciśnij strzałki, aby zmienić jaskrawość.
# machine-drafted, needs native review
palette-develop-crop = Kadrowanie i prostowanie
# machine-drafted, needs native review
palette-develop-crop-help = Kadrowanie, prostowanie, obracanie lub odbijanie zdjęcia. Enter zatwierdza, Esc wraca.
# machine-drafted, needs native review
disabled-not-photo = Tylko dla zdjęć
# machine-drafted, needs native review
disabled-develop-off = Wywoływanie jest wyłączone w profilach Medycyna i Nauka
# machine-drafted, needs native review
develop-module-corrections = Korekty
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Wył.
# machine-drafted, needs native review
develop-on = Wł.
# machine-drafted, needs native review
develop-highlight-method = Przepalone światła
# machine-drafted, needs native review
develop-highlight-opposed = Odtwórz kolor
# machine-drafted, needs native review
develop-highlight-rebuild = Odtwórz duże obszary
# machine-drafted, needs native review
develop-highlight-clip = Przytnij do bieli
# machine-drafted, needs native review
develop-highlight-threshold = Poziom przepalenia
# machine-drafted, needs native review
develop-module-hot-pixels = Gorące piksele
# machine-drafted, needs native review
develop-hot-threshold = Próg gorących pikseli
# machine-drafted, needs native review
develop-module-ca = Aberracja chromatyczna
# machine-drafted, needs native review
develop-ca-mode = Obwódki
# machine-drafted, needs native review
develop-ca-auto = Automatycznie
# machine-drafted, needs native review
develop-ca-manual = Ręcznie
# machine-drafted, needs native review
develop-ca-red = Przesunięcie czerwieni
# machine-drafted, needs native review
develop-ca-blue = Przesunięcie błękitu
# machine-drafted, needs native review
develop-module-defringe = Usuwanie obwódek
# machine-drafted, needs native review
develop-defringe = Usuwanie obwódek
# machine-drafted, needs native review
develop-defringe-purple = Odcień fioletu
# machine-drafted, needs native review
develop-defringe-green = Odcień zieleni
# machine-drafted, needs native review
develop-defringe-range = Zakres odcienia
# machine-drafted, needs native review
develop-defringe-edge = Kontrast krawędzi
# machine-drafted, needs native review
develop-module-flat-field = Klatki kalibracyjne
# machine-drafted, needs native review
develop-calibration = Kalibracja
# machine-drafted, needs native review
develop-calibration-auto = Z folderu
# machine-drafted, needs native review
develop-calibration-chosen = Wybrane pliki
# machine-drafted, needs native review
develop-flat-choose = Wybierz klatkę flat…
# machine-drafted, needs native review
develop-dark-choose = Wybierz klatkę dark…
# machine-drafted, needs native review
develop-flat-file = Klatka flat: { $name }
# machine-drafted, needs native review
develop-dark-file = Klatka dark: { $name }
# machine-drafted, needs native review
develop-flat-use = Klatka flat
# machine-drafted, needs native review
develop-flat-use-full = Winietowanie i kurz
# machine-drafted, needs native review
develop-flat-use-vignetting = Tylko winietowanie
# machine-drafted, needs native review
develop-flat-dialog = Wybierz klatkę flat
# machine-drafted, needs native review
develop-dark-dialog = Wybierz klatkę dark
# machine-drafted, needs native review
develop-raw-kind = Pliki raw
# machine-drafted, needs native review
develop-flat-frame = Klatka flat
# machine-drafted, needs native review
develop-dark-frame = Klatka dark

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Dodaj maskę
# machine-drafted, needs native review
develop-mask-kind-ellipse = Elipsa
# machine-drafted, needs native review
develop-mask-kind-rectangle = Prostokąt
# machine-drafted, needs native review
develop-mask-kind-polygon = Wielokąt
# machine-drafted, needs native review
develop-mask-kind-linear = Gradient liniowy
# machine-drafted, needs native review
develop-mask-kind-radial = Gradient radialny
# machine-drafted, needs native review
develop-mask-kind-brush = Pędzel
# machine-drafted, needs native review
develop-mask-kind-luminance = Zakres luminancji
# machine-drafted, needs native review
develop-mask-kind-hue = Zakres odcienia
# machine-drafted, needs native review
develop-mask-kind-chroma = Zakres nasycenia
# machine-drafted, needs native review
develop-mask-kind-colour = Zakres koloru
# machine-drafted, needs native review
develop-mask-kind-other = Inna maska
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Mieszanie
# machine-drafted, needs native review
develop-blend-normal = Normalne
# machine-drafted, needs native review
develop-blend-multiply = Mnożenie
# machine-drafted, needs native review
develop-blend-screen = Ekran
# machine-drafted, needs native review
develop-blend-overlay = Nakładka
# machine-drafted, needs native review
develop-blend-soft-light = Łagodne światło
# machine-drafted, needs native review
develop-blend-lightness = Tylko jasność
# machine-drafted, needs native review
develop-blend-colour = Tylko kolor
# machine-drafted, needs native review
develop-mask-amount = Siła maski
# machine-drafted, needs native review
develop-mask-invert = Odwróć maskę
# machine-drafted, needs native review
develop-mask-off = Wył.
# machine-drafted, needs native review
develop-mask-on = Wł.
# machine-drafted, needs native review
develop-mask-refine = Popraw krawędzie
# machine-drafted, needs native review
develop-mask-show = Pokaż maskę
# machine-drafted, needs native review
develop-mask-show-red = Czerwona
# machine-drafted, needs native review
develop-mask-show-grey = Szara
# machine-drafted, needs native review
develop-mask-remove = Usuń maskę
# machine-drafted, needs native review
develop-mask-combine-add = Dodaj
# machine-drafted, needs native review
develop-mask-combine-subtract = Odejmij
# machine-drafted, needs native review
develop-mask-combine-intersect = Część wspólna
# machine-drafted, needs native review
develop-mask-row-opacity = Krycie
# machine-drafted, needs native review
develop-mask-row-invert = Odwróć
# machine-drafted, needs native review
develop-mask-feather = Wtapianie
# machine-drafted, needs native review
develop-mask-roundness = Zaokrąglenie
# machine-drafted, needs native review
develop-mask-brush-size = Rozmiar pędzla
# machine-drafted, needs native review
develop-mask-brush-feather = Miękkość pędzla
# machine-drafted, needs native review
develop-mask-brush-flow = Przepływ
# machine-drafted, needs native review
develop-mask-brush-mode = Pędzel
# machine-drafted, needs native review
develop-mask-brush-paint = Maluj
# machine-drafted, needs native review
develop-mask-brush-erase = Wymaż
# machine-drafted, needs native review
develop-mask-range-from = Od
# machine-drafted, needs native review
develop-mask-range-full-from = Pełne od
# machine-drafted, needs native review
develop-mask-range-full-to = Pełne do
# machine-drafted, needs native review
develop-mask-range-to = Do
# machine-drafted, needs native review
develop-mask-range-test = Sprawdzaj
# machine-drafted, needs native review
develop-mask-range-input = Wejście
# machine-drafted, needs native review
develop-mask-range-output = Wyjście
# machine-drafted, needs native review
develop-mask-colour-pick = Pobierz kolor
# machine-drafted, needs native review
develop-mask-colour-pick-more = Pobierz kolejny kolor
# machine-drafted, needs native review
develop-mask-colour-range = Zakres
# machine-drafted, needs native review
develop-mask-colour-falloff = Zanikanie
# machine-drafted, needs native review
develop-mask-edit = Edytuj na zdjęciu
# machine-drafted, needs native review
develop-mask-row-remove = Usuń { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Usunąć maskę modułu { $module }? Jej kształty znikną. Zdjęcie na dysku zostaje.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Usunąć { $name } z modułu { $module }? Pozostałe maski i zdjęcie na dysku zostają.
# machine-drafted, needs native review
develop-mask-remove-ok = Usuń
# machine-drafted, needs native review
develop-mask-removed = Maska usunięta.
# machine-drafted, needs native review
develop-mask-added = Dodano { $name }.
# machine-drafted, needs native review
develop-mask-full = Ta maska ma już tyle części, ile może pomieścić.
# machine-drafted, needs native review
develop-mask-editing = Edycja: { $name }. Przeciągaj uchwyty albo użyj Tab i strzałek.
# machine-drafted, needs native review
develop-mask-painting = Malowanie: { $name }. Przeciągaj po zdjęciu.
# machine-drafted, needs native review
develop-mask-picking = Kliknij zdjęcie, aby pobrać kolor.
# machine-drafted, needs native review
develop-mask-picked = Kolor pobrany.
# machine-drafted, needs native review
develop-mask-done = Koniec edycji maski.
# machine-drafted, needs native review
develop-mask-handles = Uchwyty maski
# machine-drafted, needs native review
develop-handle-centre = Środek
# machine-drafted, needs native review
develop-handle-width = Szerokość
# machine-drafted, needs native review
develop-handle-height = Wysokość
# machine-drafted, needs native review
develop-handle-rotate = Obrót
# machine-drafted, needs native review
develop-handle-feather = Wtapianie
# machine-drafted, needs native review
develop-handle-inner = Krawędź wewnętrzna
# machine-drafted, needs native review
develop-handle-start = Początek
# machine-drafted, needs native review
develop-handle-end = Koniec
# machine-drafted, needs native review
develop-handle-point = Punkt { $n }
# machine-drafted, needs native review
develop-instance = Warstwa
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Warstwa { $n }.
# machine-drafted, needs native review
develop-instance-new = Nowa warstwa
# machine-drafted, needs native review
develop-instance-duplicate = Duplikuj warstwę
# machine-drafted, needs native review
develop-instance-delete = Usuń warstwę
# machine-drafted, needs native review
develop-instance-earlier = Przesuń wcześniej
# machine-drafted, needs native review
develop-instance-later = Przesuń później
# machine-drafted, needs native review
develop-instance-delete-confirm = Usunąć warstwę { $n } modułu { $module }? Jej ustawienia i maski znikną. Zdjęcie na dysku zostaje.
# machine-drafted, needs native review
develop-instance-delete-ok = Usuń
# machine-drafted, needs native review
develop-instance-deleted = Warstwa usunięta.
# machine-drafted, needs native review
develop-history-mask = Maska: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Kształty maski: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Nowy: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Usunięty: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Kolejność: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Inne programy
# machine-drafted, needs native review
develop-darktable-edited = Edytowano w darktable:
# machine-drafted, needs native review
develop-darktable-open = Otwórz w darktable
# machine-drafted, needs native review
develop-darktable-opening = Otwieranie w darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable się nie uruchomił: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importuj z Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] Zaimportowano ustawienia Lightroom: 1. Wynik jest przybliżony.
    [few] Zaimportowano ustawienia Lightroom: { $count }. Wynik jest przybliżony.
    [many] Zaimportowano ustawienia Lightroom: { $count }. Wynik jest przybliżony.
   *[other] Zaimportowano ustawienia Lightroom: { $count }. Wynik jest przybliżony.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Nic do zaimportowania: te ustawienia niczego tu nie zmieniają.
# machine-drafted, needs native review
develop-history-lightroom = Zaimportowano z Lightroom (w przybliżeniu)
# machine-drafted, needs native review
develop-conflict-title = Edycja pliku { $file } zmieniła się poza Specular
# machine-drafted, needs native review
develop-conflict-file = Plik towarzyszący: zapisany { $time }
# machine-drafted, needs native review
develop-conflict-library = Biblioteka: zapisana { $time }
# machine-drafted, needs native review
develop-conflict-writer = Plik towarzyszący zapisał { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Zachowaj jedną. Druga zostanie zastąpiona.
# machine-drafted, needs native review
develop-conflict-use-file = Użyj z pliku
# machine-drafted, needs native review
develop-conflict-keep-library = Zachowaj z biblioteki
# machine-drafted, needs native review
develop-conflict-used-file = Używana jest edycja z pliku towarzyszącego
# machine-drafted, needs native review
develop-conflict-kept-library = Zachowano edycję z biblioteki
# machine-drafted, needs native review
toast-develop-edit-newer-process = Tę edycję wykonano w nowszym Specular. Tutaj może wyglądać inaczej.
# machine-drafted, needs native review
info-darktable-edited = Edytowano w darktable
# machine-drafted, needs native review
info-darktable-no-modules = Brak włączonych modułów
# machine-drafted, needs native review
info-darktable-group = Plik
# machine-drafted, needs native review
toast-develop-edit-partial = Tę edycję wykonano w nowszym Specular. Części, których ta wersja nie umie pokazać, pominięto. Zmiany tutaj nie są zapisywane.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Nie można odczytać pliku edycji obok zdjęcia. Zostaje bez zmian. Zmiany tutaj nie są zapisywane.
# machine-drafted, needs native review
develop-demosaic = Szczegóły raw
develop-demosaic-directional = Standardowy
develop-demosaic-dual = Gładkie jednolite obszary
develop-demosaic-fast = Szybkie
develop-dual-threshold = Kontrast jednolitych obszarów
