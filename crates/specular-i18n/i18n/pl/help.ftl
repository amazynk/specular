# machine-drafted, needs native review
## Panel podręcznika, raporty awarii, pakiet diagnostyczny, sprawdzanie aktualizacji i Informacje. Polski.
## Styl: krótko, jasno, bez wypełniaczy; jedna myśl na zdanie.

## Paleta

palette-help-manual = Pomoc
palette-help-manual-help = Otwórz podręcznik na stronie o tym, co jest na ekranie.
palette-save-diagnostics = Zapisz pakiet diagnostyczny…
palette-save-diagnostics-help = Zapisz raporty awarii, dziennik i dane systemu w pliku zip. Nic nie jest wysyłane.
# Pozycja palety otwierająca sekcję podręcznika; { $title } to strona lub „Strona › Sekcja”.
palette-help-topic = Pomoc: { $title }
palette-help-topic-help = Otwórz tę sekcję podręcznika.
# Pod niedostępnym poleceniem w palecie.
palette-disabled-help = Teraz niedostępne. Naciśnij Enter, aby przeczytać dlaczego.

## Panel pomocy

help-title = Pomoc
help-search-placeholder = Szukaj w podręczniku
help-contents = Spis treści
help-back = Wstecz
help-forward = Dalej
help-no-results = Nic nie znaleziono dla „{ $query }”.
help-results = { $count ->
    [one] Znaleziono 1 sekcję
    [few] Znaleziono { $count } sekcje
    [many] Znaleziono { $count } sekcji
   *[other] Znaleziono { $count } sekcji
}
help-section-start = Pierwsze kroki
help-section-modes = Przeglądanie i tryby
help-section-tasks = Jak…
help-section-reference = Informacje szczegółowe
help-keys-unbound = Nieustawiony
# Polecenie z podręcznika bez klawisza; uruchamia się je z palety.
help-key-unbound = { $name } (paleta)
help-keys-command = Polecenie
help-keys-keys = Klawisze
help-commands = Każde polecenie jest w palecie poleceń ({ $key }). Wpisz część nazwy.
help-commands-no-key = Każde polecenie jest w palecie poleceń. Wpisz część nazwy.
help-link = Przejdź do: { $title }
help-open = Pomoc
# Nazwa panelu dla czytników ekranu; { $page } to wyświetlana strona.
help-label = Pomoc: { $page }
help-disabled-why = Dlaczego?

## Raport awarii proponowany przy następnym uruchomieniu

crash-title = { -app-name } nieoczekiwanie się zamknął
crash-body = Raport zapisano na tym komputerze. Nic nie zostało wysłane.
crash-view = Pokaż raport
crash-hide = Ukryj raport
crash-copy = Kopiuj
crash-copied = Skopiowano raport
crash-save-bundle = Zapisz pakiet diagnostyczny…
crash-dismiss = Zamknij

## Pakiet diagnostyczny

diag-title = Zapisz pakiet diagnostyczny
diag-intro = Plik zip do dołączenia do zgłoszenia błędu. Jest zapisywany na tym komputerze. Nic nie jest wysyłane.
diag-contents = Zawartość
diag-include-names = Dołącz nazwy plików
diag-include-names-help = Wyłączone: nazwy są zastępowane przez file-1.jpg, dir-1 itd.
diag-privacy = Twój folder domowy jest widoczny jako ~, a nazwa użytkownika jako <user>. Hasła, adresy serwerów i obrazy nigdy nie są dołączane.
diag-where = Zgłoś problem: { $issues } · { $email }
diag-save = Zapisz…
diag-cancel = Anuluj
diag-saved = Zapisano pakiet diagnostyczny: { $path }
diag-failed = Nie można zapisać pakietu diagnostycznego: { $reason }
diag-entry-summary = Wersja, system, GPU i sterownik
diag-entry-crash = Ostatni raport awarii
diag-entry-crashes = Wcześniejsze raporty awarii ({ $count })
diag-entry-log = Ostatnie wiersze dziennika: { $count }
diag-entry-settings = Ustawienia, bez haseł i adresów
diag-entry-gpu = Dane GPU i wyświetlacza
diag-entry-readme = Czym jest ten pakiet
diag-size = { $size }
# Plik README.txt w pakiecie.
diag-readme =
    Pakiet diagnostyczny { -app-name }.

    Zawiera to, co pomaga znaleźć problem: wersję i dane systemu, raporty awarii, koniec dziennika i ustawienia.

    Twój folder domowy jest widoczny jako ~, a nazwa użytkownika jako <user>. Jeśli nie wybrano ich dołączenia, nazwy plików i folderów są zastępowane przez file-1, dir-1 itd. Hasła i adresy serwerów są usuwane.

    Nic nie zostało wysłane. To Ty decydujesz, czy udostępnić ten plik.

## Aktualizacje (Ustawienia)

settings-section-updates = Aktualizacje
settings-updates-check = Sprawdzaj aktualizacje
settings-updates-check-help = Raz dziennie sprawdzaj, czy jest nowa wersja. Zapytanie zawiera tylko numer wersji: bez identyfikatora, bez pliku cookie. Wyłączone: { -app-name } nigdy nie łączy się w sprawie aktualizacji.
settings-updates-channel = Kanał
settings-updates-channel-help = Stabilny: tylko wydania. Beta: także wersje testowe.
settings-updates-channel-stable = Stabilny
settings-updates-channel-beta = Beta
settings-updates-check-now = Sprawdź teraz
settings-updates-check-now-help = Sprawdź raz teraz.
update-available = Dostępna jest wersja { $version }
update-notes = Informacje o wydaniu
update-download = Pobierz
update-none = { -app-name } jest aktualny.
update-checking = Sprawdzanie aktualizacji…
update-failed = Nie można sprawdzić aktualizacji: { $reason }
update-last-checked = Ostatnio sprawdzono: { $when }

## Informacje

about-build = Kompilacja { $commit } · { $profile } · { $target }
about-build-no-commit = Kompilacja { $profile } · { $target }
about-help = Pomoc
about-diagnostics = Zapisz pakiet diagnostyczny…
# machine-drafted, needs native review
about-renderer = Informacje o renderowaniu…
about-licences-count = { $count ->
    [one] 1 komponent zewnętrzny
    [few] { $count } komponenty zewnętrzne
    [many] { $count } komponentów zewnętrznych
   *[other] { $count } komponentu zewnętrznego
}
about-licence-group = { $licence } ({ $count })
about-contact = Zgłoszenia: { $issues } · Pomoc techniczna: { $email }
