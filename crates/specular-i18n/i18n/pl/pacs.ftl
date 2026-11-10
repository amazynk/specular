# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Rodzaj
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Host
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Port
dicomweb-hint-port = 104 lub 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE title serwera
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Nasz AE title
# machine-drafted, needs native review
dicomweb-field-tls = Bezpieczne połączenie (TLS)
dicomweb-tls-on = Bezpieczne połączenie włączone
dicomweb-tls-off = Bezpieczne połączenie wyłączone
dicomweb-problem-host = Podaj nazwę hosta lub adres serwera.
dicomweb-problem-port = Port to liczba od 1 do 65535.
dicomweb-problem-ae = AE title ma od 1 do 16 znaków, bez ukośnika odwrotnego.
# machine-drafted, needs native review
dicomweb-dimse-note-open = Serwery DICOM (DIMSE) można przeszukiwać, otwierać z nich serie i wysyłać do nich.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Pobieranie przez
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Odbieraj na adresie
# machine-drafted, needs native review
pacs-hint-receive-address = Adres tego komputera, np. 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Odbieraj na porcie
# machine-drafted, needs native review
pacs-hint-receive-port = Np. 11113
# machine-drafted, needs native review
pacs-move-note = Dla C-MOVE serwer musi znać nasz AE title { $ae } z tym adresem i portem.
    Specular nasłuchuje tam tylko podczas odbierania serii.
# machine-drafted, needs native review
pacs-problem-receive-address = Podaj adres tego komputera (np. 192.168.1.20), na którym odbierać.
# machine-drafted, needs native review
pacs-problem-receive-port = Podaj port do odbioru. C-MOVE go wymaga.
dicomweb-verify = Sprawdź
dicomweb-verifying = Sprawdzanie serwera…
dicomweb-verify-ok = Serwer { $server } odpowiedział.

## The Studies browser and the palette.

studies-send = Wyślij do PACS…
studies-send-description = Wysyła tę serię, jej badanie lub całą zawartość płyty na serwer.
palette-send-to-pacs = Wyślij do PACS…
palette-send-to-pacs-help = Wyślij bieżącą serię lub badanie na serwer PACS.

## The Send page.

pacs-title = Wyślij do PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } badanie
    [few] { $studies } badania
    [many] { $studies } badań
   *[other] { $studies } badania
} · { $files ->
    [one] { $files } plik
    [few] { $files } pliki
    [many] { $files } plików
   *[other] { $files } pliku
}
pacs-what = Wyślij
pacs-scope-series = Ta seria ({ $files ->
    [one] { $files } plik
    [few] { $files } pliki
    [many] { $files } plików
   *[other] { $files } pliku
})
pacs-scope-study = To badanie ({ $files ->
    [one] { $files } plik
    [few] { $files } pliki
    [many] { $files } plików
   *[other] { $files } pliku
})
pacs-scope-all = Wszystko wczytane ({ $studies ->
    [one] { $studies } badanie
    [few] { $studies } badania
    [many] { $studies } badań
   *[other] { $studies } badania
}, { $files ->
    [one] { $files } plik
    [few] { $files } pliki
    [many] { $files } plików
   *[other] { $files } pliku
})
pacs-anonymise = Anonimizuj
pacs-anonymise-note = Dane pacjenta są usuwane z wysyłanych kopii. Twoje pliki się nie zmieniają.
pacs-identified-note = Pliki są wysyłane bez zmian, z danymi pacjenta.
# The list of servers to choose from.
pacs-to = Do
pacs-no-servers = Brak serwerów. Dodaj serwer w sekcji Serwery DICOMweb.
pacs-send = Wyślij…
pacs-go-back = Wróć
pacs-confirm-send = Wyślij
pacs-confirm = Wysłać { $studies ->
    [one] { $studies } badanie
    [few] { $studies } badania
    [many] { $studies } badań
   *[other] { $studies } badania
} ({ $files ->
    [one] { $files } plik
    [few] { $files } pliki
    [many] { $files } plików
   *[other] { $files } pliku
}) na serwer { $server }?
pacs-busy = Wysyłanie nadal trwa.
pacs-started = Wysyłanie.
pacs-stopping = Zatrzymywanie…
pacs-sending = Wysyłanie { $done } z { $total } na serwer { $server }…
pacs-sent-all = { $total ->
    [one] { $total } plik został wysłany na serwer { $server }.
    [few] Wszystkie { $total } pliki zostały wysłane na serwer { $server }.
    [many] Wszystkich { $total } plików zostało wysłanych na serwer { $server }.
   *[other] Wszystkie { $total } pliku zostały wysłane na serwer { $server }.
}
pacs-sent-some = Na serwer { $server } wysłano { $sent } z { $total } plików.
pacs-stopped = Zatrzymano po { $sent } z { $total ->
        [one] { $total } pliku
        [few] { $total } plików
        [many] { $total } plików
       *[other] { $total } pliku
    }.
pacs-stopped-because = Zatrzymano po { $sent } z { $total ->
        [one] { $total } pliku
        [few] { $total } plików
        [many] { $total } plików
       *[other] { $total } pliku
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } plik zapisano z ostrzeżeniem.
    [few] { $count } pliki zapisano z ostrzeżeniem.
    [many] { $count } plików zapisano z ostrzeżeniem.
   *[other] { $count } pliku zapisano z ostrzeżeniem.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = I jeszcze { $count }.

## Why one file was not sent.

pacs-reason-unreadable = Nie można odczytać: { $reason }
pacs-reason-not-dicom = To nie jest plik DICOM
pacs-reason-burned-in = Ma dane pacjenta wtopione w obraz, więc nie jest wysyłany po anonimizacji
pacs-reason-document = Dokument, którego nie można zanonimizować
pacs-reason-anonymise = Nie można zanonimizować: { $reason }
pacs-reason-not-accepted = Serwer nie przyjmuje { $class }
pacs-reason-convert = Nie można przekonwertować dla serwera: { $reason }
pacs-reason-resources = Serwerowi brakuje miejsca lub zasobów
pacs-reason-mismatch = Według serwera dane nie pasują do swojego typu
pacs-reason-understand = Serwer nie mógł go odczytać
pacs-reason-class = Serwer nie obsługuje tego rodzaju obiektu
pacs-reason-not-authorised = Serwer na to nie pozwala
pacs-reason-status = Serwer go odrzucił (status { $code })
pacs-reason-refused = Serwer go odrzucił
pacs-reason-too-large = Za duży dla serwera
pacs-reason-http = Serwer go odrzucił (błąd { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Sieć jest wyłączona.
pacs-fatal-unknown-host = { $host } nie jest znaną nazwą hosta.
pacs-fatal-connect = Nic nie odpowiada pod adresem { $host }, port { $port }.
pacs-fatal-unreachable = { $host } jest nieosiągalny.
pacs-fatal-timeout = { $host } nie odpowiedział na czas.
pacs-fatal-cancelled = Zatrzymano.
pacs-fatal-called-ae = { $host } nie odpowiada na AE title { $ae }. Sprawdź AE title serwera.
pacs-fatal-calling-ae = { $host } nie zna naszego AE title { $ae }. Dodaj go na serwerze.
pacs-fatal-busy = { $host } jest zajęty. Spróbuj ponownie później.
pacs-fatal-rejected = { $host } odrzucił połączenie.
pacs-fatal-aborted = { $host } zakończył połączenie.
pacs-fatal-nothing-accepted = { $host } nie przyjmuje żadnego z tych rodzajów obiektów.
pacs-fatal-protocol = { $host } odpowiedział w sposób, którego Specular nie rozumie.
pacs-fatal-auth = { $host } nie przyjął logowania.
pacs-fatal-http = { $host } odpowiedział błędem { $status }.

## Teaching quiz.

palette-teaching-quiz = Quiz dydaktyczny
palette-teaching-quiz-help = Przeglądaj obrazy kluczowe lub przypadek dydaktyczny, z ukrytymi wynikami, dopóki ich nie odsłonisz.
# "Case 2 of 5".
quiz-case = Przypadek { $n } z { $total }
quiz-hint-hidden = Spacja odsłania wyniki · → następny przypadek · Esc kończy
quiz-hint-revealed = Spacja znów je ukrywa · → następny przypadek · Esc kończy
quiz-hidden = Wyniki ukryte.
quiz-no-findings = Brak zapisanych wyników.
quiz-notes = Notatki do przypadku: { $notes }
quiz-ended = Quiz zakończony.
quiz-empty = Brak przypadków do pokazania. Oznacz obrazy kluczowe klawiszem K lub wybierz folder przypadku dydaktycznego.
quiz-not-a-case = Ten folder nie jest przypadkiem dydaktycznym wyeksportowanym przez Specular.
quiz-pick-title = Wybierz folder przypadku dydaktycznego

## Teaching case folder (Export key images).

figure-keys-case = Folder przypadku dydaktycznego
figure-case-notes = Notatki do przypadku
figure-case-findings = Wyniki
figure-case-none = Brak zapisanych
figure-case-readme = Przypadek dydaktyczny wyeksportowany przez Specular. Dane pacjenta zostały usunięte.
    images: obrazy kluczowe, z zaczernionym tekstem wtopionym w obraz.
    DICOM: zanonimizowane kopie i manifest Key Object Selection „For Teaching”.
    case.txt: notatki do przypadku i wyniki dla każdego obrazu. case.json: to samo dla skryptów.
    Dla Radiopaedii: prześlij folder DICOM lub obrazy i skopiuj notatki z case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } nie odpowiada na wyszukiwania (C-FIND).
pacs-find-status = { $host } odrzucił wyszukiwanie (status { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } nie wysyła serii przez C-GET.
    Wybierz C-MOVE w ustawieniach serwera.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } nie przyjmuje żądań C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } nie zna naszego AE title { $ae }.
    Dodaj go na serwerze z adresem tego komputera i portem { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } nie ma wolnych zasobów. Spróbuj ponownie później.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } odmówił wysłania serii (status { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } nie ma obrazów dla tej serii.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } odmówił wysyłania obrazów przez to połączenie.
    Wybierz C-MOVE w ustawieniach serwera.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } nie mógł wysłać obrazów do tego komputera.
    Sprawdź adres, port i zaporę oraz czy serwer zna nasz AE title { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } nie mógł wysłać { $count ->
    [one] obrazu
    [few] żadnego z { $count } obrazów
    [many] żadnego z { $count } obrazów
   *[other] żadnego z { $count } obrazu
} w formie czytelnej dla Specular.
# machine-drafted, needs native review
pacs-retrieve-listen = Nie można odbierać na { $address }, port { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Certyfikat { $host } nie jest zaufany.
    Jego wystawca musi znajdować się wśród zaufanych certyfikatów systemu.
pacs-fatal-tls = Brak bezpiecznego połączenia z { $host }.
    Sprawdź, czy port { $port } obsługuje DICOM przez TLS.
