# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Polski. machine-drafted, needs native review

palette-tether = Fotografowanie z kablem…
palette-tether-capture = Zrób zdjęcie (z kablem)

tether-title = Fotografowanie z kablem
tether-close = Zamknij fotografowanie z kablem
tether-default-folder = Z kablem

## Cameras
tether-none = Nie znaleziono aparatu.
tether-none-hint = Podłącz aparat kablem USB i włącz go. Jeśli ma tryb USB, wybierz PTP lub połączenie z komputerem.
tether-look-again = Szukaj ponownie
tether-connect = Połącz
tether-next-camera = Następny aparat
tether-connecting = Łączenie…
tether-connected = Połączono z { $camera }
tether-disconnect = Rozłącz
tether-try-again = Spróbuj ponownie
tether-gone = Aparat został odłączony.
tether-not-connected = Brak połączonego aparatu. Najpierw otwórz fotografowanie z kablem.

## Why a camera can't be opened
tether-permission = Ten użytkownik nie może otworzyć urządzenia USB aparatu { $node }.
tether-permission-rule = Pozwala na to reguła udev. Jako administrator dodaj ten wiersz do { $file }:
tether-permission-reload = Następnie uruchom „sudo udevadm control --reload” i podłącz aparat ponownie.
tether-copy-rule = Kopiuj regułę
tether-busy-other = Inny program używa aparatu.
tether-busy-named = Aparatu używa { $programs }.
tether-busy-hint = Zamknij ten program lub odmontuj aparat w menedżerze plików, a potem spróbuj ponownie.
tether-driver = Sterownik systemowy tego aparatu nie pozwala innym programom go używać.

## The camera
tether-battery = Akumulator { PERCENT($percent) }
tether-card = { $card }: wolne { $free }
tether-card-shots = { $card }: wolne { $free }, zdjęć: { $shots }
tether-no-card = Brak karty pamięci.
tether-no-settings = Aparat nie podaje ustawień ekspozycji.
tether-no-remote = Tego aparatu nie można wyzwolić stąd. Zdjęcia zrobione jego spustem są pobierane.
tether-capture = Zrób zdjęcie
tether-step-down = { $setting }: mniej ({ $value })
tether-step-up = { $setting }: więcej ({ $value })

## Exposure settings
tether-prop-mode = Tryb
tether-prop-shutter = Czas naświetlania
tether-prop-aperture = Przysłona
tether-prop-iso = ISO
tether-prop-bias = Korekta ekspozycji
tether-prop-white-balance = Balans bieli
tether-auto = Automatycznie
tether-bulb = Tryb B
tether-camera-value = Ustawienie aparatu { $value }
tether-wb-manual = Ręczny
tether-wb-one-push = Pomiar jednorazowy
tether-wb-daylight = Światło dzienne
tether-wb-fluorescent = Świetlówka
tether-wb-tungsten = Żarówka
tether-wb-flash = Lampa błyskowa
tether-mode-manual = Ręczny
tether-mode-program = Program
tether-mode-aperture = Preselekcja przysłony
tether-mode-shutter = Preselekcja czasu
tether-mode-creative = Kreatywny
tether-mode-action = Sport
tether-mode-portrait = Portret

## Where shots go
tether-folder = Folder
tether-names = Nazwy
tether-names-camera = Nazwy z aparatu
tether-arrival = Styl Develop
tether-choose-folder = Folder…
tether-choose-style = Styl…
tether-downloading = Pobieranie… { PERCENT($percent) }
tether-saved = Zapisano { $name }
tether-saved-count = { $count ->
    [one] Zapisano { $count } zdjęcie
    [few] Zapisano { $count } zdjęcia
    [many] Zapisano { $count } zdjęć
   *[other] Zapisano { $count } zdjęcia
}

## Failures
tether-busy = Aparat jest zajęty. Spróbuj ponownie.
tether-card-full = Karta pamięci jest pełna.
tether-set-refused = Aparat nie przyjął tego ustawienia. Sprawdź pokrętło trybów.
tether-disk = Nie udało się zapisać zdjęcia: { $error }
tether-error = Błąd aparatu: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Folder fotografowania z kablem
settings-cull-tether-folder-help = Gdzie zapisywane są zdjęcia z aparatu podłączonego kablem.
settings-cull-tether-folder-unset = Obrazy › Z kablem
settings-cull-tether-rename = Nazwy plików przy fotografowaniu z kablem
settings-cull-tether-rename-help = Nowe nazwy, z tymi samymi znacznikami co Zmień nazwy w Imporcie. Puste zachowuje nazwy z aparatu.
