# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Limit kopii lokalnych
settings-library-local-copies-limit-help = Ile miejsca kopie lokalne mogą najwyżej zająć w folderze danych Speculara.

local-keep-menu = Zachowaj kopię lokalną…
local-remove-menu = Usuń kopię lokalną…
local-keep-location-menu = Zachowaj kopię lokalną tej lokalizacji…
local-remove-location-menu = Usuń kopię lokalną tej lokalizacji…
local-keep-album-menu = Zachowaj kopię lokalną tego albumu…
local-remove-album-menu = Usuń kopię lokalną tego albumu…
local-remove-held-elsewhere = Te kopie należą do zachowanej lokalizacji lub albumu. Usuń je tam.

local-keep-ask = Zachować kopię lokalną: { $item }?
local-keep-detail = { $count ->
    [one] { $count } zdjęcie.
    [few] { $count } zdjęcia.
    [many] { $count } zdjęć.
   *[other] { $count } zdjęcia.
}
    Oryginały: pliki takie, jakie są.
    Proxy: pliki raw w 2560 pikselach, wystarczy do wywołania. Inne pliki są kopiowane w całości.
    Kopie trafiają do folderu danych Speculara. Oryginały się nie zmieniają.
local-keep-originals = Zachowaj oryginały
local-keep-proxies = Zachowaj proxy
local-keeping = Kopiowanie: { $item }…
local-kept = { $item }: { $count ->
    [one] { $count } kopia lokalna
    [few] { $count } kopie lokalne
    [many] { $count } kopii lokalnych
   *[other] { $count } kopii lokalnej
}, { $size }.
local-kept-full = { $count ->
    [one] { $count } zdjęcie nie zmieściło się w limicie rozmiaru.
    [few] { $count } zdjęcia nie zmieściły się w limicie rozmiaru.
    [many] { $count } zdjęć nie zmieściło się w limicie rozmiaru.
   *[other] { $count } zdjęcia nie zmieściło się w limicie rozmiaru.
}
local-kept-unreachable = { $count ->
    [one] Nie udało się odczytać { $count } zdjęcia.
    [few] Nie udało się odczytać { $count } zdjęć.
    [many] Nie udało się odczytać { $count } zdjęć.
   *[other] Nie udało się odczytać { $count } zdjęcia.
}

local-remove-ask = Usunąć kopię lokalną: { $item }?
local-remove-detail = Kopie w folderze danych Speculara zostaną skasowane. Oryginały pozostają nietknięte. Oczekujące zmiany zostają.
local-remove-ok = Usuń kopię lokalną
local-removed = Usunięto kopię lokalną: { $item }, zwolniono { $size }.

local-banner-offline = { $location } jest offline. Pracujesz na kopiach lokalnych.
local-banner-offline-many = Lokalizacje offline: { $count }. Pracujesz na kopiach lokalnych.
local-banner-waiting = { $count ->
    [one] { $count } zmiana czeka na oryginał.
    [few] { $count } zmiany czekają na oryginały.
    [many] { $count } zmian czeka na oryginały.
   *[other] { $count } zmiany czeka na oryginały.
}
local-banner-conflicts = { $count ->
    [one] { $count } plik zmienił się, gdy byłeś offline.
    [few] { $count } pliki zmieniły się, gdy byłeś offline.
    [many] { $count } plików zmieniło się, gdy byłeś offline.
   *[other] { $count } pliku zmieniło się, gdy byłeś offline.
}
local-review = Przejrzyj…
local-sidebar-originals = kopia lokalna
local-sidebar-proxies = lokalne proxy

local-synced = { $count ->
    [one] { $count } zmiana zapisana w oryginale.
    [few] { $count } zmiany zapisane w oryginałach.
    [many] { $count } zmian zapisanych w oryginałach.
   *[other] { $count } zmiany zapisanej w oryginałach.
}
local-sync-failed = { $count ->
    [one] Nie udało się zapisać { $count } zmiany. Nadal czeka.
    [few] Nie udało się zapisać { $count } zmian. Nadal czekają.
    [many] Nie udało się zapisać { $count } zmian. Nadal czekają.
   *[other] Nie udało się zapisać { $count } zmiany. Nadal czekają.
}

local-conflict-title = { $file } zmienił się, gdy byłeś offline. Którą wersję zachować: { $what }?
local-conflict-yours = Twoja: { $text }
local-conflict-theirs = Na dysku: { $text }
local-conflict-later = Anuluj zostawia decyzję na później. Nic nie zostanie nadpisane.
local-keep-mine = Zachowaj twoją
local-keep-theirs = Zachowaj wersję pliku
local-what-marks = ocena
local-what-keywords = słowa kluczowe
local-what-fields = pola IPTC
local-what-edit = edycja
local-stars = { $count ->
    [0] Bez gwiazdek
    [one] { $count } gwiazdka
    [few] { $count } gwiazdki
    [many] { $count } gwiazdek
   *[other] { $count } gwiazdki
}
local-label = { $label ->
    [1] Czerwona
    [2] Żółta
    [3] Zielona
   *[4] Niebieska
}
local-pick = Wybrane
local-reject = Odrzucone
local-no-keywords = brak słów kluczowych
local-changed-elsewhere = zmieniono gdzie indziej
local-photo-changed = zmieniło się samo zdjęcie
local-edit-here = edytowano tutaj, { $when }
local-edit-elsewhere = edytowano gdzie indziej, { $when }
local-edit-none = brak edycji

local-export-ask = { $count ->
    [one] { $count } zdjęcie jest offline. Wyeksportować je z proxy?
    [few] { $count } zdjęcia są offline. Wyeksportować je z proxy?
    [many] { $count } zdjęć jest offline. Wyeksportować je z proxy?
   *[other] { $count } zdjęcia jest offline. Wyeksportować je z proxy?
}
local-export-detail = Proxy mają 2560 pikseli na dłuższym boku. Oryginały dają pełny rozmiar.
local-export-proxy = Eksportuj z proxy
local-export-wait = Czekaj na oryginały
local-export-waiting = Eksport zacznie się, gdy oryginały wrócą.

local-panel-title = Kopie lokalne
local-panel-used = Zajęte { $used } z { $limit }.
local-panel-originals = { $count ->
    [one] { $count } oryginał
    [few] { $count } oryginały
    [many] { $count } oryginałów
   *[other] { $count } oryginału
}
local-panel-proxies = { $count ->
    [one] { $count } zdjęcie, raw jako proxy
    [few] { $count } zdjęcia, raw jako proxy
    [many] { $count } zdjęć, raw jako proxy
   *[other] { $count } zdjęcia, raw jako proxy
}
local-panel-remove = Usuń…
local-panel-footer = Przechowywane w { $folder }. Oryginały nigdy się nie zmieniają.
local-panel-keywords = offline kopie lokalne proxy miejsce
