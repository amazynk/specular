# machine-drafted, needs native review
## Przegląd modeli 3D: spacer i lot, płaszczyzny przekroju i prostopadłościan przekroju, zapisane
## widoki, tematy BCF (tryb 3D). Podane klawisze są domyślne.

cheatsheet-review = Przegląd

palette-mesh-walk = Spacer
palette-mesh-fly = Lot
palette-mesh-views = Zapisane widoki
palette-mesh-save-view = Zapisz widok
palette-mesh-section-plane = Dodaj płaszczyznę przekroju
palette-mesh-section-box = Prostopadłościan przekroju
palette-mesh-section-flip = Odwróć płaszczyznę przekroju
palette-mesh-section-clear = Usuń wszystkie przekroje
palette-mesh-measure-cut = Mierz na przekroju
palette-mesh-bcf-topics = Tematy BCF
# machine-drafted, needs native review
palette-mesh-model-checks = Kontrole modelu
# machine-drafted, needs native review
palette-mesh-models = Modele w scenie
# machine-drafted, needs native review
palette-mesh-add-model = Dodaj model…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Dodaj modele z folderu…

# Spacer i lot: powiadomienia i wiersze odczytu, według $what. $eye: wysokość oka nad podłogą,
# z jednostką (tylko walk-line).
mesh-nav = { $what ->
    [walk-toast] Spacer: W A S D ruch, przeciągnij, by się rozejrzeć, Esc kończy
    [fly-toast] Lot: W A S D ruch, Q E w dół i w górę, Esc kończy
    [orbit] Orbita
    [walk-line] Spacer · oko { $eye } nad podłogą
    [walk-keys] W A S D ruch · przeciąganie rozgląda · Shift szybciej · Esc kończy
    [preparing] Ściany jeszcze niegotowe: przechodzisz przez nie
    [fly-keys] W A S D ruch · Q E w dół, w górę · przeciąganie rozgląda · Shift szybciej · Esc kończy
   *[fly-line] Lot
}

# Krótkie teksty przekrojów, pomiaru i zapisanych widoków, według $what.
bim-note = { $what ->
    [plane-added] Dodano płaszczyznę przekroju: przeciągnij uchwyt, aby ją przesunąć
    [box-on] Prostopadłościan przekroju włączony: przeciągnij uchwyt ściany, aby ją przesunąć
    [box-off] Prostopadłościan przekroju wyłączony
    [cleared] Usunięto przekroje
    [measure-on] Kliknij dwa punkty na przekroju
    [measure-needs-cut] Najpierw dodaj płaszczyznę przekroju lub prostopadłościan
    [measure-off-cut] Kliknij na przekroju
    [measure-hint] Pomiar: kliknij dwa punkty na przekroju
    [ask-view-name] Nazwa widoku
    [ask-rename] Nowa nazwa
    [views-empty] Brak zapisanych widoków. Naciśnij N, aby zapisać ten.
    [views-hints] Enter przejdź · N zapisz · F2 zmień nazwę · Delete usuń · P zapisz jako PNG
    [delete-detail] Usunięty zostanie tylko zapisany widok. Model się nie zmieni.
    [delete-ok] Usuń
    [not-over-model] Wybierz inny plik: ten zastąpiłby model.
   *[view-name] Widok
}

# Krótkie teksty BCF (BIM Collaboration Format), według $what.
bcf-note = { $what ->
    [title-none] Tematy BCF
    [new-file] nowy
    [empty] Brak tematów. Naciśnij O, aby otworzyć plik BCF, lub N, aby dodać temat.
    [hints] Enter przejdź · ← → punkty widzenia · N temat · C komentarz · S status · E tytuł · O otwórz · Ctrl+S eksportuj
    [ask-title] Tytuł tematu
    [ask-comment] Komentarz
    [untitled] Bez tytułu
    [topic-added] Dodano temat z tym widokiem
   *[comment-added] Dodano komentarz z tym widokiem
}

# $planes: liczba płaszczyzn przekroju.
bim-sections-line = { $planes ->
    [0] Brak płaszczyzn przekroju
    [one] 1 płaszczyzna przekroju
    [few] { $planes } płaszczyzny przekroju
    [many] { $planes } płaszczyzn przekroju
   *[other] { $planes } płaszczyzny przekroju
}
# $size: rozmiar prostopadłościanu przekroju z jednostką.
bim-box-line = prostopadłościan { $size }
# $count: największa możliwa liczba płaszczyzn.
bim-planes-full = Najwyżej { $count } płaszczyzn przekroju
# Uchwyt do przeciągania, dla czytników ekranu. $n: jego numer.
bim-handle = { $kind ->
    [box] Ściana prostopadłościanu przekroju { $n }
   *[plane] Płaszczyzna przekroju { $n }
}
# $length: długość z jednostką.
bim-measure-line = Długość na przekroju: { $length }
bim-views-title = { $count ->
    [one] 1 zapisany widok
    [few] { $count } zapisane widoki
    [many] { $count } zapisanych widoków
   *[other] { $count } zapisanego widoku
}
# $name: nazwa widoku.
bim-view-saved = Zapisano „{ $name }”
bim-view-delete-title = Usunąć widok „{ $name }”?
# $file: zapisany plik.
bim-saved-file = Zapisano { $file }
# $reason: przyczyna niepowodzenia.
bim-failed = { $what ->
    [open] Nie można otworzyć pliku BCF: { $reason }
    [views] Widoki zachowane tylko w tej sesji: { $reason }
   *[save] Nie można zapisać: { $reason }
}
# $count: tematy; $file: nazwa pliku archiwum.
bcf-title = { $count ->
    [one] 1 temat
    [few] { $count } tematy
    [many] { $count } tematów
   *[other] { $count } tematu
} · { $file }
bcf-comments = { $count ->
    [one] 1 komentarz
    [few] { $count } komentarze
    [many] { $count } komentarzy
   *[other] { $count } komentarza
}
# $at: pokazany punkt widzenia; $count: punkty widzenia tematu.
bcf-viewpoint-of = Punkt widzenia { $at } z { $count }
bcf-opened = { $count ->
    [one] Otwarto 1 temat
    [few] Otwarto { $count } tematy
    [many] Otwarto { $count } tematów
   *[other] Otwarto { $count } tematu
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modele: 1
    [few] Modele: { $count }
    [many] Modele: { $count }
   *[other] Modele: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Ten plik nie jest modelem IFC.
    [not-ifc-open] Najpierw otwórz model IFC. Łączyć można tylko modele IFC.
    [this-model] Ten model
    [hints] A dodaj model · Shift+A dodaj folder · Space pokaż lub ukryj · C kolor według modelu · Enter przejdź · Delete usuń
    [origin-ok] Umieść w początku układu
    [first-stays] Otwarty model pozostaje. Aby go zmienić, otwórz inny plik.
    [remove-detail] Zniknie ze sceny. Plik na dysku pozostaje.
   *[remove-ok] Usuń
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 element
    [few] { $elements } elementy
    [many] { $elements } elementów
   *[other] { $elements } elementu
} · { $shown ->
    [hidden] ukryty
   *[shown] widoczny
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] otwarty model
    [map-crs] według współrzędnych mapy ({ $crs })
    [map] według współrzędnych mapy
    [site] według szerokości i długości geograficznej działki (w przybliżeniu)
    [project] według wspólnych współrzędnych projektu
   *[origin] w początku układu (niewyrównany)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Modele używają różnych układów współrzędnych: { $first } i { $this }.
    [only-first] Otwarty model ma georeferencję, ten nie.
    [only-this] Ten model ma georeferencję, otwarty nie.
   *[broken] Nie można użyć konwersji mapy tego modelu (jej skala wynosi zero).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Przesunięto o { $distance }, obrócono o { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Wczytywanie 1 modelu…
    [few] Wczytywanie { $count } modeli…
    [many] Wczytywanie { $count } modeli…
   *[other] Wczytywanie { $count } modelu…
}
# machine-drafted, needs native review
fed-ask-origin = Nie można wyrównać { $file }. Umieścić w początku układu?
# machine-drafted, needs native review
fed-no-files = Brak plików IFC w { $folder }.
# machine-drafted, needs native review
fed-not-added = Nie dodano { $file }: { $reason }
# machine-drafted, needs native review
fed-added = Dodano { $file }. Modele w scenie: { $count }.
# machine-drafted, needs native review
fed-remove-title = Usunąć { $file } ze sceny?
# machine-drafted, needs native review
fed-removed = Usunięto { $file } ze sceny.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] ukryty
   *[shown] widoczny
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Kolor według modelu włączony
   *[off] Kolor według modelu wyłączony
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI i zapisane widoki
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 adnotacja
    [few] { $count } adnotacje
    [many] { $count } adnotacji
   *[other] { $count } adnotacji
}
# machine-drafted, needs native review
pmi-view = Zapisany widok
# machine-drafted, needs native review
pmi-view-all = pokazuje wszystkie adnotacje
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] pokazuje 1 adnotację
    [few] pokazuje { $count } adnotacje
    [many] pokazuje { $count } adnotacji
   *[other] pokazuje { $count } adnotacji
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Pokazano widok „{ $name }”. A pokazuje wszystko.
# machine-drafted, needs native review
pmi-has-faces = powierzchnie podświetlone
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Brak PMI w tym modelu.
    [hints] ↑ ↓ wybierz · Enter przejdź · A wszystko · D jak narysowano · Shift+A ukryj
    [hidden] Adnotacje ukryte
    [shown] Adnotacje pokazane
    [as-drawn] Adnotacje tak, jak rysuje je plik
    [as-labels] Adnotacje jako etykiety
   *[all-shown] Wszystkie adnotacje pokazane
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Wymiar
    [tolerance] Tolerancja geometryczna
    [datum] Baza
    [target] Baza częściowa
   *[graphic] Adnotacja rysunkowa
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Odległość
    [angular] Kąt
    [diameter] Średnica
    [radius] Promień
    [length] Długość
    [thickness] Grubość
   *[other] Wymiar
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Prostoliniowość
    [flatness] Płaskość
    [circularity] Okrągłość
    [cylindricity] Walcowość
    [line-profile] Kształt linii
    [surface-profile] Kształt powierzchni
    [angularity] Nachylenie
    [perpendicularity] Prostopadłość
    [parallelism] Równoległość
    [position] Pozycja
    [concentricity] Współśrodkowość
    [coaxiality] Współosiowość
    [symmetry] Symetria
    [circular-runout] Bicie
   *[total-runout] Bicie całkowite
}
