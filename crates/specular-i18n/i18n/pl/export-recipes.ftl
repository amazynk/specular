# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Eksportuj według przepisów…
palette-export-recipes-help = Eksportuj zaznaczenie według jednego lub kilku zapisanych przepisów.
palette-develop-export-recipes = Eksportuj według przepisów…
palette-develop-export-recipes-help = Eksportuj edytowane zdjęcie lub kilka zdjęć według zapisanych przepisów.

export-recipes-title = Eksport według przepisów
export-recipes-new = Nowy
export-recipes-edit = Edytuj…
export-recipes-duplicate = Duplikuj
export-recipes-delete = Usuń…
export-recipes-start = Eksportuj
export-recipes-save = Zapisz
export-recipes-open-folder = Otwórz folder
export-recipes-choose-image = Wybierz obraz…
export-recipes-no-images = Brak obrazów do eksportu.
export-recipes-none-checked = Zaznacz co najmniej jeden przepis.
export-recipes-checked = { $name }: wł.
export-recipes-unchecked = { $name }: wył.

## The starters.
export-recipes-starter-web = Internet, 2048 px
export-recipes-starter-print = Druk, pełny rozmiar
export-recipes-starter-email = E-mail, mały
export-recipes-unnamed = Przepis bez nazwy
export-recipes-new-name = Nowy przepis
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (kopia)

## The list's one-line summary.
export-recipes-summary-mark = znak wodny
export-recipes-size-full = Pełny rozmiar
export-recipes-size-long = { $px } px dłuższy bok
export-recipes-size-short = { $px } px krótszy bok
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Obok oryginału
export-recipes-folder-sub = Folder „{ $name }”
export-recipes-folder-ask = Pytaj za każdym razem

## The editor.
export-recipes-new-title = Nowy przepis
export-recipes-edit-title = Edytuj przepis
export-recipes-name = Nazwa przepisu
export-recipes-text = Własny tekst
export-recipes-pattern = Nazwa pliku
export-recipes-mark-text-field = Tekst znaku wodnego
export-recipes-subfolder = Nazwa folderu
export-recipes-format = Format
export-recipes-quality = Jakość
export-recipes-lossless = Bezstratnie
export-recipes-bits = Głębia bitowa
export-recipes-bits-8 = 8 bitów
export-recipes-bits-16 = 16 bitów
export-recipes-space = Przestrzeń kolorów
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profil
export-recipes-profile-embed = Osadź
export-recipes-profile-none = Pomiń
export-recipes-folder = Folder
export-recipes-folder-subfolder = Podfolder
export-recipes-collision = Nazwa zajęta
export-recipes-collision-unique = Dodaj numer
export-recipes-collision-overwrite = Zastąp (z pytaniem)
export-recipes-collision-skip = Pomiń
export-recipes-after = Po zakończeniu
export-recipes-after-nothing = Nic
export-recipes-after-open = Otwórz folder
export-recipes-resize = Rozmiar
export-recipes-resize-long = Dłuższy bok
export-recipes-resize-short = Krótszy bok
export-recipes-resize-mp = Megapiksele
export-recipes-resize-exact = Dokładny rozmiar
export-recipes-resize-percent = Procent
export-recipes-edge = Bok
export-recipes-width = Szerokość
export-recipes-height = Wysokość
export-recipes-megapixels = Megapiksele
export-recipes-percent = Procent
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Powiększaj
export-recipes-sharpen = Wyostrzanie dla
export-recipes-sharpen-off = Wył.
export-recipes-sharpen-screen = Ekran
export-recipes-sharpen-matte = Papier matowy
export-recipes-sharpen-glossy = Papier błyszczący
export-recipes-amount = Siła
export-recipes-amount-low = Niska
export-recipes-amount-standard = Standardowa
export-recipes-amount-high = Wysoka
export-recipes-metadata = Metadane
export-recipes-meta-all = Wszystkie
export-recipes-meta-copyright = Tylko prawa autorskie
export-recipes-meta-contact = Prawa autorskie i kontakt
export-recipes-meta-none = Brak
export-recipes-location = Lokalizacja
export-recipes-people = Widoczne osoby
export-recipes-keep = Zachowaj
export-recipes-remove = Usuń
export-recipes-mark = Znak wodny
export-recipes-mark-none = Brak
export-recipes-mark-text = Tekst
export-recipes-mark-image = Obraz (wybierz)
export-recipes-position = Położenie
export-recipes-pos-top-left = U góry po lewej
export-recipes-pos-top = U góry
export-recipes-pos-top-right = U góry po prawej
export-recipes-pos-left = Po lewej
export-recipes-pos-centre = Na środku
export-recipes-pos-right = Po prawej
export-recipes-pos-bottom-left = U dołu po lewej
export-recipes-pos-bottom = U dołu
export-recipes-pos-bottom-right = U dołu po prawej
export-recipes-opacity = Krycie
export-recipes-scale = Szerokość
export-recipes-yes = Tak
export-recipes-no = Nie
export-recipes-error-name = Nadaj przepisowi nazwę.
export-recipes-error-mark-text = Wpisz tekst znaku wodnego.
export-recipes-error-mark-image = Wybierz obraz znaku wodnego.
export-recipes-error-subfolder = Wpisz nazwę folderu.
export-recipes-saved = Zapisano przepis „{ $name }”.

## Delete.
export-recipes-delete-title = Usunąć przepis „{ $name }”?
export-recipes-delete-detail = Wyeksportowane pliki pozostaną na dysku.
export-recipes-delete-confirm = Usuń
export-recipes-delete-keep = Zachowaj
export-recipes-deleted = Usunięto przepis „{ $name }”.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Folder dla „{ $name }”
export-recipes-asking = Wybierz folder dla każdego przepisu, który o to pyta.
export-recipes-no-folder = Nie wybrano folderu. Nic nie wyeksportowano.
export-recipes-planning = Nadawanie nazw plikom…
export-recipes-plan-pattern = Przepis „{ $name }”: { $reason }
export-recipes-replace-title = { $count ->
    [one] Zastąpić 1 plik?
    [few] Zastąpić { $count } pliki?
    [many] Zastąpić { $count } plików?
   *[other] Zastąpić { $count } pliku?
}
export-recipes-replace-detail = Oryginały nigdy nie są zastępowane.
export-recipes-replace-confirm = Zastąp
export-recipes-replace-kept = Nic nie wyeksportowano.
export-recipes-running = Eksportowanie…
export-recipes-progress = { $done } z { $total } plików
export-recipes-done = { $count ->
    [one] Wyeksportowano 1 plik.
    [few] Wyeksportowano { $count } pliki.
    [many] Wyeksportowano { $count } plików.
   *[other] Wyeksportowano { $count } pliku.
}
export-recipes-stopped = { $count ->
    [one] Zatrzymano. Wyeksportowano 1 plik.
    [few] Zatrzymano. Wyeksportowano { $count } pliki.
    [many] Zatrzymano. Wyeksportowano { $count } plików.
   *[other] Zatrzymano. Wyeksportowano { $count } pliku.
}
export-recipes-done-skipped = { $count ->
    [one] Pominięto 1: nazwa była zajęta.
    [few] Pominięto { $count }: nazwy były zajęte.
    [many] Pominięto { $count }: nazwy były zajęte.
   *[other] Pominięto { $count }: nazwy były zajęte.
}
export-recipes-done-failed = { $count ->
    [one] Nieudane: 1
    [few] Nieudane: { $count }
    [many] Nieudane: { $count }
   *[other] Nieudane: { $count }
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
