# machine-drafted, needs native review
# Operacje na plikach: zmiana nazwy, zmiana nazw wielu plików, kopiowanie i przenoszenie do
# folderu, obrót zapisany w pliku, cofanie i ponawianie, tapeta, udostępnianie, wysyłanie
# e-mailem, eksport i import ustawień.

## Paleta, ściągawka i menu

palette-undo = Cofnij
palette-undo-help = Cofnij ostatnią zmianę plików: przeniesienie do kosza, zmianę nazwy, kopiowanie lub przeniesienie, obrót.
palette-redo = Ponów
palette-redo-help = Wykonaj ponownie to, co cofnęło ostatnie cofnięcie.
palette-rename = Zmień nazwę…
palette-rename-help = Nadaj plikowi nową nazwę; pliki sidecar i para RAW+JPEG idą za nim.
palette-batch-rename = Zmień nazwy plików…
palette-batch-rename-help = Zmień nazwy zaznaczonych plików lub całego folderu według wzorca, z podglądem.
palette-copy-to-folder = Kopiuj do folderu…
palette-copy-to-folder-help = Skopiuj plik lub zaznaczenie do innego folderu.
palette-move-to-folder = Przenieś do folderu…
palette-move-to-folder-help = Przenieś plik lub zaznaczenie do innego folderu.
palette-rotate-file-left = Obróć plik w lewo
palette-rotate-file-left-help = Obróć sam plik o ćwierć obrotu w lewo, bezstratnie, jeśli format na to pozwala.
palette-rotate-file-right = Obróć plik w prawo
palette-rotate-file-right-help = Obróć sam plik o ćwierć obrotu w prawo, bezstratnie, jeśli format na to pozwala.
palette-set-wallpaper-help = Ustaw obraz jako tło pulpitu.
palette-share = Udostępnij…
palette-share-help = Wyślij plik do innej aplikacji przez systemowe menu udostępniania.
palette-send-by-email = Wyślij e-mailem…
palette-send-by-email-help = Utwórz nową wiadomość w programie pocztowym z plikiem w załączniku.
# machine-drafted, needs native review
palette-share-with-zynk = Udostępnij w Zynk…
palette-share-with-zynk-help = Utwórz link internetowy do pliku w aplikacji Zynk.
palette-send-with-zynk = Wyślij przez Zynk…
palette-send-with-zynk-help = Wyślij plik do kontaktu lub na jedno z twoich urządzeń przez Zynk.
palette-export-settings = Eksportuj ustawienia…
palette-export-settings-help = Zapisz wszystkie ustawienia i skróty klawiszowe w pliku.
palette-import-settings = Importuj ustawienia…
palette-import-settings-help = Wczytaj ustawienia z pliku po pokazaniu, co by się zmieniło.

disabled-in-archive-file = Ta strona jest w archiwum i nie ma własnego pliku
disabled-not-rotatable = Ten rodzaj pliku nie może zapisać obrotu
disabled-not-on-this-desktop = Twój pulpit tego nie oferuje

## Ustawienia

settings-browsing-rotate-trim-edges = Przycinaj krawędzie JPEG, by obracać bezstratnie
settings-browsing-rotate-trim-edges-help = JPEG, którego wymiary nie są wielokrotnością 8 lub 16 pikseli, można obrócić bezstratnie tylko po odcięciu kilku ostatnich pikseli. Bez tej opcji zmieniany jest znacznik orientacji, a przycięcie jest proponowane za każdym razem.
settings-browsing-rotate-keep-modified = Zachowuj datę modyfikacji przy obracaniu
settings-browsing-rotate-keep-modified-help = Obrócony plik zachowuje datę ostatniej zmiany, więc przy sortowaniu według daty zostaje na swoim miejscu.
settings-browsing-recent-folders = Ostatnie foldery
settings-browsing-recent-folders-help = Foldery ostatnio użyte do kopiowania i przenoszenia.

## Okna

fileop-cancel = Anuluj
rename-title = Zmień nazwę
rename-field-label = Nazwa
rename-button = Zmień nazwę
rename-sidecars-follow = Zmieniono też: { $files }
rename-extension-changed = Zmiana rozszerzenia nie konwertuje pliku; inne aplikacje mogą go nie otworzyć.
rename-problem-empty = Wpisz nazwę.
rename-problem-invalid-char = Nazwy nie mogą zawierać „{ $char }”.
rename-problem-reserved = „{ $name }” to nazwa zarezerwowana przez Windows dla urządzeń.
rename-problem-too-long = Ta nazwa jest za długa; nazwy mogą mieć najwyżej 255 bajtów.
rename-problem-dot = „.” i „..” nie mogą być nazwami.
rename-problem-trailing = Nazwy nie mogą kończyć się spacją ani kropką.
rename-problem-exists = Inny plik ma już tę nazwę.
rename-problem-read-only = Tego folderu nie można zmieniać.
rename-problem-duplicate = Dwa pliki dostałyby tę nazwę.

batch-rename-title =
    { $count ->
        [one] Zmień nazwę { $count } pliku
        [few] Zmień nazwy { $count } plików
        [many] Zmień nazwy { $count } plików
       *[other] Zmień nazwy { $count } pliku
    }
batch-rename-field-label = Wzorzec
batch-rename-tokens = Użyj { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } i { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } plik zmieni nazwę
        [few] { $count } pliki zmienią nazwę
        [many] { $count } plików zmieni nazwę
       *[other] { $count } pliku zmieni nazwę
    }{ $problems ->
        [0] .
        [one] ; { $problems } nie może i zostaje bez zmian.
        [few] ; { $problems } nie mogą i zostają bez zmian.
        [many] ; { $problems } nie może i zostaje bez zmian.
       *[other] ; { $problems } nie może i zostaje bez zmian.
    }
batch-rename-reading = Odczyt dat i aparatów…
batch-rename-preview = Podgląd
batch-rename-unchanged = zachowuje nazwę
batch-rename-more =
    { $count ->
        [one] Jeszcze { $count } poniżej
        [few] Jeszcze { $count } poniżej
        [many] Jeszcze { $count } poniżej
       *[other] Jeszcze { $count } poniżej
    }

transfer-copy-title-one = Kopiuj „{ $file }” do folderu
transfer-copy-title =
    { $count ->
        [one] Kopiuj { $count } plik do folderu
        [few] Kopiuj { $count } pliki do folderu
        [many] Kopiuj { $count } plików do folderu
       *[other] Kopiuj { $count } pliku do folderu
    }
transfer-move-title-one = Przenieś „{ $file }” do folderu
transfer-move-title =
    { $count ->
        [one] Przenieś { $count } plik do folderu
        [few] Przenieś { $count } pliki do folderu
        [many] Przenieś { $count } plików do folderu
       *[other] Przenieś { $count } pliku do folderu
    }
transfer-recent = Ostatnie foldery
transfer-folder = Folder
transfer-choose-folder = Wybierz folder…
transfer-copy-dialog = Kopiuj do folderu
transfer-move-dialog = Przenieś do folderu
transfer-sidecars =
    { $count ->
        [one] { $count } plik sidecar idzie razem z nimi.
        [few] { $count } pliki sidecar idą razem z nimi.
        [many] { $count } plików sidecar idzie razem z nimi.
       *[other] { $count } pliku sidecar idzie razem z nimi.
    }
transfer-taken =
    { $count ->
        [one] { $count } nazwa jest już używana w { $folder }.
        [few] { $count } nazwy są już używane w { $folder }.
        [many] { $count } nazw jest już używanych w { $folder }.
       *[other] { $count } nazwy jest już używane w { $folder }.
    }
transfer-when-taken = Gdy nazwa jest już używana
transfer-skip = Pomiń
transfer-keep-both = Zachowaj oba
transfer-replace-option = Zastąp
transfer-copy = Kopiuj
transfer-move = Przenieś
transfer-replace-title =
    { $count ->
        [one] Zastąpić { $count } plik w { $folder }?
        [few] Zastąpić { $count } pliki w { $folder }?
        [many] Zastąpić { $count } plików w { $folder }?
       *[other] Zastąpić { $count } pliku w { $folder }?
    }
transfer-replace-note = Obecne tam pliki trafią do kosza. Cofnij je przywróci.
transfer-replace = Zastąp
transfer-back = Wstecz
transfer-stop = Zatrzymaj

settings-file-import-title = Importuj ustawienia
settings-file-import-dialog = Importuj ustawienia
settings-file-export-dialog = Eksportuj ustawienia
settings-file-import-changes = Zmiany
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = brak klawisza
settings-file-import-unknown = Nie jest ustawieniem tej wersji; pominięto.
settings-file-import-keymap = Skróty klawiszowe
settings-file-import-nothing = Nic by się nie zmieniło: te ustawienia już obowiązują.
settings-file-import-summary =
    { $count ->
        [one] { $count } zmiana. Import można cofnąć w Ustawieniach.
        [few] { $count } zmiany. Import można cofnąć w Ustawieniach.
        [many] { $count } zmian. Import można cofnąć w Ustawieniach.
       *[other] { $count } zmiany. Import można cofnąć w Ustawieniach.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } wpis wygląda na hasło lub klucz i nigdy nie jest importowany.
        [few] { $count } wpisy wyglądają na hasła lub klucze i nigdy nie są importowane.
        [many] { $count } wpisów wygląda na hasła lub klucze i nigdy nie jest importowanych.
       *[other] { $count } wpisu wygląda na hasła lub klucze i nigdy nie jest importowane.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } polecenie użytkownika nie jest importowane: polecenia uruchamiają programy. Jeśli mu ufasz, skopiuj je sam do pliku ustawień.
        [few] { $count } polecenia użytkownika nie są importowane: polecenia uruchamiają programy. Jeśli im ufasz, skopiuj je sam do pliku ustawień.
        [many] { $count } poleceń użytkownika nie jest importowanych: polecenia uruchamiają programy. Jeśli im ufasz, skopiuj je sam do pliku ustawień.
       *[other] { $count } polecenia użytkownika nie jest importowane: polecenia uruchamiają programy. Jeśli im ufasz, skopiuj je sam do pliku ustawień.
    }
settings-file-import-button = Importuj

## Wyniki

fileop-renamed = Zmieniono nazwę na { $file }
fileop-renamed-count =
    { $count ->
        [one] Zmieniono nazwę { $count } pliku
        [few] Zmieniono nazwy { $count } plików
        [many] Zmieniono nazwy { $count } plików
       *[other] Zmieniono nazwy { $count } pliku
    }
fileop-copied =
    { $count ->
        [one] Skopiowano { $count } plik do { $folder }
        [few] Skopiowano { $count } pliki do { $folder }
        [many] Skopiowano { $count } plików do { $folder }
       *[other] Skopiowano { $count } pliku do { $folder }
    }
fileop-moved =
    { $count ->
        [one] Przeniesiono { $count } plik do { $folder }
        [few] Przeniesiono { $count } pliki do { $folder }
        [many] Przeniesiono { $count } plików do { $folder }
       *[other] Przeniesiono { $count } pliku do { $folder }
    }
fileop-skipped =
    { $count ->
        [one] Pominięto { $count }: nazwa jest tam już używana.
        [few] Pominięto { $count }: nazwy są tam już używane.
        [many] Pominięto { $count }: nazwy są tam już używane.
       *[other] Pominięto { $count }: nazwy są tam już używane.
    }
fileop-stopped = Zatrzymano; to, co zrobiono, zostaje.
fileop-failed-file =
    { $count ->
        [one] Nie można skopiować ani przenieść { $file }: { $reason }
        [few] Nie można skopiować ani przenieść { $count } plików; pierwszy, { $file }: { $reason }
        [many] Nie można skopiować ani przenieść { $count } plików; pierwszy, { $file }: { $reason }
       *[other] Nie można skopiować ani przenieść { $count } pliku; pierwszy, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Kopiowanie { $count } pliku… { PERCENT($percent) }
        [few] Kopiowanie { $count } plików… { PERCENT($percent) }
        [many] Kopiowanie { $count } plików… { PERCENT($percent) }
       *[other] Kopiowanie { $count } pliku… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Przenoszenie { $count } pliku… { PERCENT($percent) }
        [few] Przenoszenie { $count } plików… { PERCENT($percent) }
        [many] Przenoszenie { $count } plików… { PERCENT($percent) }
       *[other] Przenoszenie { $count } pliku… { PERCENT($percent) }
    }
fileop-done-status = Gotowe
fileop-busy = Ostatnia zmiana plików wciąż trwa; spróbuj za chwilę.
fileop-nothing-to-redo = Nic do ponowienia
fileop-undone = Cofnięto: { $what }
fileop-redone = Ponowiono: { $what }
fileop-kind-rename = zmiana nazwy
fileop-kind-batch-rename = zmiana nazw wielu plików
fileop-kind-move = przeniesienie do folderu
fileop-kind-copy = kopiowanie do folderu
fileop-kind-rotate = obrót
rename-failed = Nie można zmienić nazwy: { $reason }

undo-problem-changed = Nie można cofnąć: { $file } zmieniono w międzyczasie, a cofnięcie utraciłoby tę zmianę.
undo-problem-missing = Nie można cofnąć: { $file } już nie istnieje.
undo-problem-taken = Nie można cofnąć: inny plik nazywa się teraz { $file }.
undo-problem-io = Nie można cofnąć zmiany w { $file }: { $reason }

rotate-done-lossless = Obrócono { $file } bezstratnie.
rotate-done-lossless-trimmed = Obrócono { $file } bezstratnie; krawędź przycięto do { $width } × { $height }.
rotate-done-tag = Obrócono { $file } przez zmianę znacznika orientacji.
rotate-done-sidecar = Zapisano obrót { $file } w { $sidecar }; sam plik RAW nigdy nie jest zmieniany.
rotate-tag-unreadable = Nie udało się odczytać danych obrazu na tyle, by go obrócić.
rotate-tag-multi-picture = Zawiera więcej niż jeden obraz, a obrót pierwszego uszkodziłby pozostałe.
rotate-tag-trailer = Za obrazem są dane, które obrót by przesunął.
rotate-tag-not-aligned = Bezstratny obrót pikseli przyciąłby go do { $width } × { $height }.
rotate-trim-offer = Przytnij i obróć
rotate-problem-unsupported = { $file } to rodzaj pliku, który nie może zapisać obrotu.
rotate-problem-too-large = { $file } jest za duży, by go obrócić.
rotate-problem-damaged = { $file } jest uszkodzony w miejscu zapisu orientacji: { $reason }
rotate-problem-io = Nie można obrócić { $file }: { $reason }

share-failed = Nie można udostępnić: { $reason }
email-failed = Nie można uruchomić programu pocztowego: { $reason }
# machine-drafted, needs native review
zynk-opening = Otwieranie aplikacji Zynk…
zynk-failed = Nie można połączyć się z Zynk: { $reason }
zynk-get-title-send = „Wyślij przez Zynk” wymaga aplikacji Zynk
zynk-get-title-share = „Udostępnij w Zynk” wymaga aplikacji Zynk
zynk-get-about = Zynk to darmowa aplikacja na Maca, Windows, Linuksa, iPhone’a i Androida. Wysyła zdjęcia, filmy i całe foldery prosto na twoje inne urządzenia i do innych osób, bez względu na rozmiar. Transfery są szyfrowane end-to-end i po zerwaniu połączenia wznawiają się tam, gdzie się zatrzymały. Odbiorcy, który nie ma Zynk, tworzy link internetowy, z którego pobierze pliki w przeglądarce.
zynk-get-macos = Pobierz ją z Mac App Store. Po instalacji { -app-name } przekaże jej twoje pliki bezpośrednio.
zynk-get-windows = Pobierz ją z Microsoft Store. Po instalacji { -app-name } przekaże jej twoje pliki bezpośrednio.
zynk-get-linux = Zynk dla Linuksa to plik AppImage, niczego nie trzeba instalować: pobierz go, umieść w ~/Applications i zezwól na jego uruchamianie (we właściwościach pliku albo poleceniem chmod +x). { -app-name } znajdzie go tam, a także w dowolnym innym miejscu, gdy Zynk jest uruchomiony.
zynk-get = Pobierz Zynk
zynk-send-old-windows = „Wyślij przez Zynk” wymaga systemu Windows 10 w wersji 2004 lub nowszej. „Udostępnij w Zynk” działa w tym systemie Windows.
settings-file-export-done = Zapisano ustawienia w { $file }
settings-file-export-failed = Nie można zapisać ustawień: { $reason }
settings-file-import-unreadable = Nie można odczytać { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] Zaimportowano { $count } zmianę
        [few] Zaimportowano { $count } zmiany
        [many] Zaimportowano { $count } zmian
       *[other] Zaimportowano { $count } zmiany
    }
