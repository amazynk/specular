# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Polski. machine-drafted, needs native review

palette-omero-servers = Serwery OMERO…

omero-title = Serwery OMERO
omero-servers-intro = Projekty, ekrany i obrazy na serwerze są wyświetlane dopiero podczas przeglądania. Wcześniej nic nie jest wysyłane.
omero-servers = Serwery
omero-servers-empty = Brak serwerów. Dodaj serwer, podając jego adres OMERO.web.
omero-session-only = Hasła są przechowywane tylko do zamknięcia programu Specular.
omero-add = Dodaj…
omero-edit = Edytuj…
omero-remove = Usuń
omero-close = Zamknij
omero-browse = Przeglądaj
omero-back = Wstecz
omero-save = Zapisz
omero-stop = Zatrzymaj
omero-open = Otwórz
# $server a server's name.
omero-removed = Usunięto { $server }
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Bez szyfrowania
# A server browsed without signing in.
omero-public = Publiczny
# $user the user name a server is signed in with.
omero-signed-in-as = Jako { $user }

# The page that adds or edits a server.
omero-add-title = Dodaj serwer
omero-edit-title = Ustawienia serwera
omero-field-name = Nazwa
omero-hint-name = Opcjonalnie
omero-field-url = Adres
omero-hint-url = https://omero.example.org
omero-field-user = Nazwa użytkownika
omero-hint-user = Puste dla danych publicznych
omero-field-password = Hasło
omero-hint-password-kept = Pozostaw puste, aby zachować podane hasło
omero-password-session = Hasło jest przechowywane tylko do zamknięcia programu Specular, nigdy na dysku.
omero-password-needed = Wpisz hasło: jest przechowywane tylko do zamknięcia programu Specular.
omero-http-warning = Ten adres nie jest szyfrowany: hasło zostałoby wysłane jawnym tekstem.
omero-url-invalid = Wpisz adres OMERO.web zaczynający się od https:// lub http://.
# $reason why the list could not be saved.
omero-save-failed = Nie udało się zapisać listy serwerów: { $reason }

# The browse page.
omero-browse-title = Przeglądanie
omero-top = Projekty i ekrany
omero-datasets = Zbiory danych
omero-plates = Płytki
omero-wells = Dołki
omero-fields = Pola
omero-images = Obrazy
omero-kind-project = Projekt
omero-kind-dataset = Zbiór danych
omero-kind-image = Obraz
omero-kind-screen = Ekran
omero-kind-plate = Płytka
omero-kind-well = Dołek
# $well a well's row and column, as B7.
omero-well = Dołek { $well }
omero-well-fields = { $count ->
    [one] Jedno pole
    [few] { $count } pola
    [many] { $count } pól
   *[other] { $count } pola
}
omero-children = { $count ->
    [one] Jeden element
    [few] { $count } elementy
    [many] { $count } elementów
   *[other] { $count } elementu
}
omero-load-more = Wczytaj więcej
omero-loading = Wczytywanie…
omero-empty = Nic tu nie ma
omero-stopped = Zatrzymano
omero-found = { $count ->
    [one] Jeden element
    [few] { $count } elementy
    [many] { $count } elementów
   *[other] { $count } elementu
}
# $reason why the image could not be opened.
omero-open-failed = Nie udało się otworzyć obrazu: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Co najmniej jeden kanał pozostaje widoczny
# $server a server's name.
omero-session-ended = Sesja na serwerze { $server } wygasła. Wpisz hasło, aby kontynuować.

# Errors.
# $server a server's name.
omero-error-offline = Nie można połączyć się z { $server }.
omero-error-sign-in = Ten serwer wymaga zalogowania.
omero-error-expired = Sesja wygasła.
omero-error-login = Nazwa użytkownika lub hasło zostały odrzucone.
omero-error-forbidden = Twoje konto może nie mieć do tego dostępu.
omero-error-not-found = Nie znaleziono na serwerze (mogło zostać usunięte).
# $code an HTTP status code.
omero-error-status = Serwer odpowiedział błędem { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Nie udało się odczytać odpowiedzi serwera: { $reason }
