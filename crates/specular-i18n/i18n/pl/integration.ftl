# machine-drafted, needs native review
# Integracja: otwieranie za pomocą, polecenia użytkownika, wklejanie (etap 2). Polski.
palette-open-with = Otwórz za pomocą…
palette-paste = Wklej
open-with-placeholder = Otwórz za pomocą…
open-with-loading = Wyszukiwanie aplikacji…
open-with-default = { $name } (domyślna)
open-with-choose-other = Wybierz inną aplikację…
open-with-choose-title = Wybierz aplikację
open-with-failed = Nie można otworzyć za pomocą { $name }: { $reason }
command-missing = Nie znaleziono { $program }
command-failed = Nie można uruchomić { $name }: { $reason }
command-exited = { $name } zakończył działanie z błędem: { $status }
paste-nothing = Schowek nie zawiera nic do otwarcia
paste-failed = Nie można wkleić obrazu: { $reason }
drag-failed = Nie można przeciągnąć pliku: { $reason }
about-storage = Ustawienia: { $settings } · Pamięć podręczna: { $cache }
about-storage-portable = Tryb przenośny: ustawienia i pamięć podręczna w { $dir }

## Scripting and recipes

palette-copy-view-command = Kopiuj widok jako polecenie powłoki
palette-copy-view-python = Kopiuj widok jako Python
toast-copied-view-command = Skopiowano polecenie
script-copy-no-image = Brak obrazu do opisania
recipe-confirm-title = Uruchomić przepis „{ $name }” po raz pierwszy? Uruchamia: { $command }
recipe-confirm-run = Uruchom
recipe-no-file = Przepis wymaga pliku obrazu
recipe-running = Uruchamianie { $name }…
recipe-done = { $name }: { $shapes ->
    [one] dodano 1 kształt
    [few] dodano { $shapes } kształty
    [many] dodano { $shapes } kształtów
   *[other] dodano { $shapes } kształtu
}
recipe-failed = { $name } nie powiódł się: { $reason }
recipe-timeout = zatrzymano po { $seconds } s
recipe-no-output = { $name } nie zapisał { $file }
