# machine-drafted, needs native review
# Specular — Import z aparatów, telefonów i kart pamięci. Polski.
# Karta jest tylko odczytywana; pliki są z niej usuwane tylko na żądanie, po sprawdzonej kopii.

palette-import = Importuj zdjęcia…
disabled-no-import-source = Nie podłączono karty pamięci, aparatu ani telefonu

## Ustawienia → Import
settings-section-import = Import
settings-import-on-card = Po włożeniu karty
settings-import-on-card-help = Co się dzieje po podłączeniu karty pamięci, aparatu lub telefonu ze zdjęciami.
settings-import-on-card-ask = Pytaj
settings-import-on-card-open = Otwórz Import
settings-import-on-card-nothing = Nic nie rób
settings-import-destination = Importuj do
settings-import-destination-help = Folder na importowane zdjęcia i filmy. Bez ustawienia trafiają do folderu Obrazy.
settings-import-destination-unset = Folder Obrazy
settings-import-folder-pattern = Foldery
settings-import-folder-pattern-help = Foldery tworzone w nim: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} i {"{"}event{"}"}, oddzielone znakiem /.
settings-import-rename-pattern = Zmień nazwy
settings-import-rename-pattern-help = Nowe nazwy plików, jak w Konwertuj: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Puste zachowuje nazwy z aparatu.
settings-import-keep-pairs = Trzymaj RAW i JPEG razem
settings-import-keep-pairs-help = Plik RAW i zrobiony z nim JPEG trafiają do jednego folderu pod jedną nazwą, także po zmianie nazwy.
settings-import-low-res-videos = Importuj kopie filmów w niskiej rozdzielczości
settings-import-low-res-videos-help = Niektóre aparaty nagrywają małą kopię podglądu każdego filmu (.LRV). Wyłączone: zostają na karcie.
settings-import-eject = Wysuń po zakończeniu
settings-import-eject-help = Wysuwa kartę, gdy wszystko jest skopiowane, aby można ją było wyjąć.

## Propozycja po włożeniu karty
import-offer = Importować z { $name }?
import-offer-button = Importuj

## Widok importu
import-title = Import
import-from = z { $name }
import-source-next = Następne źródło
import-close = Zamknij
import-scanning = Szukanie nowych zdjęć… { $done } z { $total }
import-scanning-start = Szukanie nowych zdjęć…
import-none-new = Wszystko z { $name } zostało już zaimportowane.
import-empty = Na { $name } nie ma zdjęć ani filmów.
import-no-source = Nie podłączono karty pamięci, aparatu ani telefonu.
import-summary = { $count ->
    [one] { $count } nowy element
    [few] { $count } nowe elementy
    [many] { $count } nowych elementów
   *[other] { $count } nowego elementu
} · { $size }
import-already = Wcześniej zaimportowane: { $count }
import-unreadable = { $count ->
    [one] Nie można odczytać { $count } pliku
    [few] Nie można odczytać { $count } plików
    [many] Nie można odczytać { $count } plików
   *[other] Nie można odczytać { $count } pliku
}
import-selected = Zaznaczono { $count } z { $total }
import-select-all = Zaznacz wszystko
import-select-none = Odznacz wszystko
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Film

import-destination = Importuj do
import-choose = Wybierz…
import-folder-pattern = Foldery
import-event = Wydarzenie
import-event-hint = Opcjonalne, dla {"{"}event{"}"}
import-rename = Zmień nazwy
import-rename-hint = Puste zachowuje nazwy z aparatu
import-preview = Pierwszy plik: { $path }
import-pattern-error = { $error }
import-keep-pairs = Trzymaj RAW i JPEG razem
import-eject = Wysuń po zakończeniu
import-delete = Usuń z karty po imporcie
import-free = Do skopiowania: { $size } · wolne miejsce: { $free }
import-free-unknown = Do skopiowania: { $size }
import-no-space = Za mało wolnego miejsca w { $folder }: ten import wymaga { $need }, a wolne jest tylko { $free }. Zwolnij miejsce lub wybierz inny folder.
import-start = { $count ->
    [one] Importuj { $count } element
    [few] Importuj { $count } elementy
    [many] Importuj { $count } elementów
   *[other] Importuj { $count } elementu
}
import-nothing-selected = Zaznacz zdjęcia do importu

import-confirm-delete = { $count ->
    [one] Po skopiowaniu i sprawdzeniu { $count } plik zostanie usunięty z { $name }. Tego nie można cofnąć.
    [few] Po skopiowaniu i sprawdzeniu { $count } pliki zostaną usunięte z { $name }. Tego nie można cofnąć.
    [many] Po skopiowaniu i sprawdzeniu { $count } plików zostanie usuniętych z { $name }. Tego nie można cofnąć.
   *[other] Po skopiowaniu i sprawdzeniu { $count } pliku zostanie usunięte z { $name }. Tego nie można cofnąć.
}
import-confirm-yes = Importuj i usuń
import-confirm-no = Wróć

import-progress = Importowanie { $done } z { $total } · { $size }
import-progress-eta = Importowanie { $done } z { $total } · { $size } · { $time }
import-paused = Wstrzymano · { $done } z { $total }
import-pause = Wstrzymaj
import-resume = Wznów
import-cancel = Anuluj
import-hide = Ukryj
import-stopping = Zatrzymywanie…
import-running = Import już trwa.

import-done = { $count ->
    [one] Zaimportowano { $count } plik
    [few] Zaimportowano { $count } pliki
    [many] Zaimportowano { $count } plików
   *[other] Zaimportowano { $count } pliku
} do { $folder }
import-done-existed = Już istniało: { $count }
import-done-deleted = Usunięto z karty: { $count }
import-done-failed = { $count ->
    [one] Nie można zaimportować { $count } pliku
    [few] Nie można zaimportować { $count } plików
    [many] Nie można zaimportować { $count } plików
   *[other] Nie można zaimportować { $count } pliku
}
import-done-cancelled = Import anulowano po { $count ->
        [one] { $count } pliku
        [few] { $count } plikach
        [many] { $count } plikach
       *[other] { $count } pliku
    }. Ponowny import wznowi go tam, gdzie się zatrzymał.
import-done-nothing = Nie zaimportowano niczego nowego.
import-ejected = { $name } można wyjąć.
import-eject-failed = Nie można wysunąć { $name }: { $error }
import-failed-destination = Nie można użyć folderu { $folder }: { $error }
import-ledger-failed = Nie można odczytać listy zaimportowanych plików ({ $error }); wcześniej zaimportowane pliki mogą zostać zaproponowane ponownie.

## Czytnik ekranu
import-a11y-group = Import
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, zaznaczono
