# machine-drafted, needs native review
# Specular — Tryb selekcji (DESIGN.md §6.3).

palette-cull = Włącz lub wyłącz tryb selekcji
cheatsheet-cull = Tryb selekcji
settings-section-cull = Tryb selekcji
settings-cull-auto-enter = Selekcjonuj foldery RAW
settings-cull-auto-enter-help = Otwieraj folder zawierający głównie pliki RAW od razu w trybie selekcji.
settings-cull-auto-advance = Następne zdjęcie po oznaczeniu
settings-cull-auto-advance-help = Przejdź dalej po wybraniu, odrzuceniu lub ocenie gwiazdkami. Etykiety i usunięcie flagi zostają na zdjęciu.
settings-cull-show = Pokazuj dla par RAW+JPEG
settings-cull-show-help = Plik RAW i JPEG o tej samej nazwie są selekcjonowane jako jedno zdjęcie, a oznaczenia dotyczą obu. JPEG otwiera się szybciej.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Grupuj serie
settings-cull-bursts-help = Zdjęcia zrobione szybko po sobie jednym aparatem są pokazywane jako jeden stos. S otwiera lub zamyka stos, B zachowuje wyświetlany obraz i odrzuca resztę stosu.
settings-cull-burst-gap = Przerwa w serii
settings-cull-burst-gap-help = Najdłuższa przerwa między dwoma zdjęciami jednej serii.
settings-cull-watch = Pokazuj nowe pliki
settings-cull-watch-help = Pliki, które pojawią się w folderze podczas selekcji (podłączony aparat, import), są dodawane na końcu, gdy zostaną w pełni zapisane.
settings-cull-follow-newest = Podążaj za najnowszym plikiem
settings-cull-follow-newest-help = Przechodź do każdego nowego pliku, gdy się pojawi. T przełącza to podczas selekcji.
settings-cull-slot-1 = Folder 1
settings-cull-slot-1-help = Alt+1 kopiuje obraz (z jego plikiem RAW lub JPEG i plikami towarzyszącymi) do tego folderu, Alt+Shift+1 przenosi go tam.
settings-cull-slot-1-unset = Nie ustawiono
settings-cull-slot-2 = Folder 2
settings-cull-slot-2-help = Alt+2 kopiuje obraz (z jego plikiem RAW lub JPEG i plikami towarzyszącymi) do tego folderu, Alt+Shift+2 przenosi go tam.
settings-cull-slot-2-unset = Nie ustawiono
settings-cull-slot-3 = Folder 3
settings-cull-slot-3-help = Alt+3 kopiuje obraz (z jego plikiem RAW lub JPEG i plikami towarzyszącymi) do tego folderu, Alt+Shift+3 przenosi go tam.
settings-cull-slot-3-unset = Nie ustawiono
settings-cull-collisions = Gdy nazwa jest zajęta
settings-cull-collisions-help = Pliki w folderach nigdy nie są zastępowane.
settings-cull-collisions-suffix = Dodaj numer
settings-cull-collisions-skip = Pomiń obraz
# machine-drafted, needs native review
# Wybrano tryb selekcji bez otwartego folderu ani obrazów galerii.
cull-nothing = Nie ma czego selekcjonować — najpierw otwórz folder ze zdjęciami
cull-slot-unset = Nie ustawiono folderu { $slot } — wybierz go w Ustawienia → Tryb selekcji
cull-slot-missing = Folder { $slot } jest niedostępny: { $reason }
cull-copied = Skopiowano do { $folder }
cull-moved = Przeniesiono do { $folder }
cull-slot-renamed = Zapisano jako { $name } w { $folder }
cull-slot-skipped = { $folder } zawiera już { $name } — pominięto
cull-slot-same = Obraz jest już w { $folder }
cull-slot-failed = { $op ->
    [move] Nie można przenieść { $file }
   *[copy] Nie można skopiować { $file }
}: { $reason }
cull-copy-kept = Zachowane kopie zmienione w międzyczasie: { $count }
cull-undo-failed = Nie można cofnąć dla { $file }: { $reason }
cull-burst-open = Seria { $index } z { $count }
cull-burst-closed = Seria: { $count }
cull-burst-toggle = Otwórz lub zamknij serię
cull-burst-best = Zachowano 1, odrzucono: { $count }
cull-burst-none = Nie należy do serii
cull-follow-on = Podążanie za nowymi plikami
cull-follow-off = Bez podążania za nowymi plikami
cull-new-files = Nowe pliki: { $count }
cull-key-copy = Kopiuj do folderu 1–3
cull-key-move = Przenieś do folderu 1–3
cull-key-burst = Otwórz lub zamknij serię
cull-key-best = Zachowaj ten, odrzuć resztę serii
cull-key-follow = Podążaj za nowymi plikami
cull-key-subframes = Pokaż lub ukryj tabelę klatek
cull-key-sort = Sortuj według następnej kolumny
cull-key-sort-reverse = Odwróć kolejność
cull-key-metric = Następna miara na wykresie
cull-key-sigma-down = Automatyczne odrzucanie: zmniejsz k
cull-key-sigma-up = Automatyczne odrzucanie: zwiększ k
cull-key-auto-reject = Odrzuć sugerowane klatki
cull-key-move-rejects = Przenieś odrzucone klatki do podfolderu
cull-key-same-stretch = To samo rozciągnięcie dla każdej klatki

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Otwórz serię
cull-tip-burst-close = Zamknij serię
cull-tip-exit = Opuść tryb selekcji
cull-tip-label = Etykieta: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Klatki
cull-astro-title = { $count ->
    [one] 1 klatka
    [few] { $count } klatki
    [many] { $count } klatek
   *[other] { $count } klatki
}
cull-astro-pending = mierzenie: { $count }
cull-astro-col-name = Klatka
cull-astro-col-filter = Filtr
cull-astro-col-exposure = Ekspoz.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Mimośr.
cull-astro-col-stars = Gwiazdy
cull-astro-col-bg = Tło
cull-astro-col-snr = SNR
cull-astro-col-noise = Szum
cull-astro-same-stretch = To samo rozciągnięcie
cull-astro-own-stretch = Własne rozciągnięcie
cull-astro-suggest-high = { $metric } powyżej { $value } (mediana + { $k }σ): { $count ->
    [one] 1 klatka
    [few] { $count } klatki
    [many] { $count } klatek
   *[other] { $count } klatki
}
cull-astro-suggest-low = { $metric } poniżej { $value } (mediana − { $k }σ): { $count ->
    [one] 1 klatka
    [few] { $count } klatki
    [many] { $count } klatek
   *[other] { $count } klatki
}
cull-astro-suggest-none = Żadna klatka nie wykracza poza { $k }σ
cull-astro-suggest-few = Automatyczne odrzucanie wymaga 3 zmierzonych klatek
cull-astro-suggested = sugerowana do odrzucenia
cull-astro-k = k { $k }
cull-astro-reject = Odrzuć { $count }
cull-astro-move = Przenieś odrzucone…
cull-astro-chart = { $metric }, klatki: { $count }, mediana { $median }.
cull-astro-sort-by = Sortuj według: { $column }
cull-astro-ascending = rosnąco
cull-astro-descending = malejąco
cull-astro-sorted = Posortowano według: { $column }
cull-astro-reversed = Odwrócono kolejność
cull-astro-metric = Wykres: { $metric }
cull-astro-k-set = Automatyczne odrzucanie przy { $k }σ
cull-astro-none = Brak klatek
cull-astro-same-on = To samo rozciągnięcie dla każdej klatki
cull-astro-same-off = Każda klatka rozciągnięta osobno
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = mimośród { $value }
cull-astro-line-stars = { $count ->
    [one] 1 gwiazda
    [few] { $count } gwiazdy
    [many] { $count } gwiazd
   *[other] { $count } gwiazdy
}
cull-astro-measuring = mierzenie gwiazd…
cull-astro-reject-confirm = { $count ->
    [one] Odrzucić sugerowaną klatkę?
    [few] Odrzucić { $count } sugerowane klatki?
    [many] Odrzucić { $count } sugerowanych klatek?
   *[other] Odrzucić sugerowane klatki ({ $count })?
}
cull-astro-reject-detail = { $rule }. Cofnięcie przywraca oznaczenia.
cull-astro-reject-ok = Odrzuć
cull-astro-nothing-to-reject = Brak klatek do odrzucenia
cull-astro-rejected = { $count ->
    [one] Odrzucono 1 klatkę
    [few] Odrzucono { $count } klatki
    [many] Odrzucono { $count } klatek
   *[other] Odrzucono { $count } klatki
}
cull-move-rejects-confirm = { $count ->
    [one] Przenieść odrzuconą klatkę do folderu „{ $folder }”?
    [few] Przenieść { $count } odrzucone klatki do folderu „{ $folder }”?
    [many] Przenieść { $count } odrzuconych klatek do folderu „{ $folder }”?
   *[other] Przenieść odrzucone klatki ({ $count }) do folderu „{ $folder }”?
}
cull-move-rejects-detail = Pliki są przenoszone, nie usuwane. Cofnięcie przywraca je na miejsce.
cull-move-rejects-ok = Przenieś
cull-move-rejects-none = Brak odrzuconych klatek do przeniesienia
cull-moved-rejects = { $count ->
    [one] Przeniesiono 1 klatkę do „{ $folder }”
    [few] Przeniesiono { $count } klatki do „{ $folder }”
    [many] Przeniesiono { $count } klatek do „{ $folder }”
   *[other] Przeniesiono { $count } klatki do „{ $folder }”
}
