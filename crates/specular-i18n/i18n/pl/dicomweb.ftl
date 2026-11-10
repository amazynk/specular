# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Polski.

palette-dicom-servers = Serwery DICOMweb…

dicomweb-title = Serwery DICOMweb
dicomweb-servers-intro = Badania na serwerze są wyszukiwane i otwierane tylko na Twoje żądanie. Wcześniej nic nie jest wysyłane.
dicomweb-servers = Serwery
dicomweb-servers-empty = Brak serwerów. Dodaj serwer, podając jego adres DICOMweb.
dicomweb-session-only = Hasła i tokeny są przechowywane tylko do zamknięcia programu Specular.
dicomweb-add = Dodaj…
dicomweb-edit = Edytuj…
dicomweb-remove = Usuń
dicomweb-close = Zamknij
dicomweb-browse = Przeglądaj
dicomweb-back = Wstecz
dicomweb-save = Zapisz
dicomweb-search = Szukaj
dicomweb-stop = Zatrzymaj
dicomweb-removed = Usunięto: { $server }
# machine-drafted, needs native review
dicomweb-remove-ask = Usunąć „{ $server }” z zapisanych serwerów?
dicomweb-remove-detail = Nic na dysku nie zostanie utracone. Jego hasło lub token dla tej sesji zostanie zapomniane.
# Serwery osiągane bez szyfrowania (http://).
dicomweb-not-encrypted = Nieszyfrowane

# Strona dodawania lub edycji serwera.
dicomweb-add-title = Dodaj serwer
dicomweb-edit-title = Ustawienia serwera
dicomweb-field-name = Nazwa
dicomweb-hint-name = Opcjonalnie
dicomweb-field-url = Adres
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Logowanie
dicomweb-auth-none = Bez logowania
dicomweb-auth-basic = Nazwa użytkownika i hasło
dicomweb-auth-bearer = Token dostępu
dicomweb-field-user = Nazwa użytkownika
dicomweb-field-password = Hasło
dicomweb-field-token = Token dostępu
dicomweb-hint-secret-kept = Pozostaw puste, aby zachować podaną wartość
dicomweb-secret-session = Hasło lub token jest przechowywany tylko do zamknięcia programu Specular, nigdy na dysku.
dicomweb-secret-needed = Wprowadź ponownie hasło lub token: jest przechowywany tylko do zamknięcia programu Specular.
dicomweb-http-warning = Ten adres nie jest szyfrowany: hasło lub token zostałby wysłany jawnym tekstem.
dicomweb-url-empty = Wprowadź adres DICOMweb serwera.
dicomweb-url-scheme = Adres musi zaczynać się od https:// lub http://.
dicomweb-url-host = Adres nie zawiera nazwy serwera.
dicomweb-url-userinfo = Wpisz nazwę użytkownika i hasło w polach logowania, a nie w adresie.
dicomweb-save-failed = Nie udało się zapisać listy serwerów: { $reason }

# Strona wyszukiwania.
dicomweb-search-title = Znajdź badania
dicomweb-field-patient-name = Nazwisko pacjenta
dicomweb-field-patient-id = ID pacjenta
dicomweb-field-date-from = Od
dicomweb-field-date-to = Do
dicomweb-hint-date = RRRRMMDD
dicomweb-field-modality = Modalność
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Numer zlecenia
dicomweb-show-patient = Pokaż dane pacjenta
dicomweb-switch-on = Wł.
dicomweb-switch-off = Wył.
dicomweb-patient-shown = Dane pacjenta widoczne
dicomweb-patient-hidden = Dane pacjenta ukryte
dicomweb-studies = Badania
dicomweb-search-hint = Wypełnij dowolne pola i wyszukaj.
dicomweb-no-studies = Brak pasujących badań.
dicomweb-searching = Wyszukiwanie…
dicomweb-stopped = Zatrzymano
dicomweb-load-more = Wczytaj więcej
dicomweb-study-untitled = Badanie bez opisu
dicomweb-series-untitled = Seria bez opisu
dicomweb-study-counts = Serie: { $series }, obrazy: { $images }
dicomweb-series-images = Obrazy: { $count }
dicomweb-patient-id = ID { $id }
dicomweb-accession = Zlecenie { $number }
dicomweb-found = Znalezione badania: { $count }
dicomweb-series-found = Serie: { $count }

# Błędy komunikacji z serwerem.
dicomweb-error-offline = Serwer { $server } jest nieosiągalny. Sprawdź sieć i adres.
dicomweb-error-auth = Serwer odrzucił nazwę użytkownika, hasło lub token.
dicomweb-error-not-found = Pod tym adresem nie ma usługi DICOMweb.
dicomweb-error-other = Serwer nie mógł odpowiedzieć: { $reason }

# Seria przesyłana strumieniowo do przeglądarki.
dicomweb-loading = Wczytywanie: { $series }…
dicomweb-loading-progress = Wczytywanie: { $series } – { $done } z { $total }
dicomweb-loaded = { $series }: wczytane obrazy: { $total }
dicomweb-loaded-failed = Obrazy, których nie udało się wczytać: { $failed } z { $total }
dicomweb-offline-cached = Serwer { $server } jest nieosiągalny: wyświetlane są wcześniej zapisane obrazy.
dicomweb-cache-failed = Nie udało się otworzyć folderu pobierania: { $reason }
