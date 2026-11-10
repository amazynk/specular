# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Typ
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Värd
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Port
dicomweb-hint-port = 104 eller 11112
# The application entity title the server answers to.
dicomweb-field-ae = Serverns AE-titel
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Vår AE-titel
# machine-drafted, needs native review
dicomweb-field-tls = Säker anslutning (TLS)
dicomweb-tls-on = Säker anslutning på
dicomweb-tls-off = Säker anslutning av
dicomweb-problem-host = Ange serverns värdnamn eller adress.
dicomweb-problem-port = Porten är ett tal från 1 till 65535.
dicomweb-problem-ae = En AE-titel har 1 till 16 tecken, utan omvänt snedstreck.
# machine-drafted, needs native review
dicomweb-dimse-note-open = DICOM-servrar (DIMSE) går att söka i, öppna och skicka till.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Hämta med
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Ta emot på adress
# machine-drafted, needs native review
pacs-hint-receive-address = Den här datorns adress, t.ex. 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Ta emot på port
# machine-drafted, needs native review
pacs-hint-receive-port = T.ex. 11113
# machine-drafted, needs native review
pacs-move-note = För C-MOVE måste servern känna till vår AE-titel { $ae } med den här adressen och porten.
    Specular lyssnar där bara medan en serie kommer in.
# machine-drafted, needs native review
pacs-problem-receive-address = Ange den här datorns adress (t.ex. 192.168.1.20) att ta emot på.
# machine-drafted, needs native review
pacs-problem-receive-port = Ange en port att ta emot på. C-MOVE behöver en.
dicomweb-verify = Verifiera
dicomweb-verifying = Kontrollerar servern…
dicomweb-verify-ok = { $server } svarade.

## The Studies browser and the palette.

studies-send = Skicka till PACS…
studies-send-description = Skickar den här serien, dess undersökning eller allt på skivan till en server.
palette-send-to-pacs = Skicka till PACS…
palette-send-to-pacs-help = Skicka aktuell serie eller undersökning till en PACS-server.

## The Send page.

pacs-title = Skicka till PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } undersökning
   *[other] { $studies } undersökningar
} · { $files ->
    [one] { $files } fil
   *[other] { $files } filer
}
pacs-what = Skicka
pacs-scope-series = Den här serien ({ $files ->
    [one] { $files } fil
   *[other] { $files } filer
})
pacs-scope-study = Den här undersökningen ({ $files ->
    [one] { $files } fil
   *[other] { $files } filer
})
pacs-scope-all = Allt inläst ({ $studies ->
    [one] { $studies } undersökning
   *[other] { $studies } undersökningar
}, { $files ->
    [one] { $files } fil
   *[other] { $files } filer
})
pacs-anonymise = Anonymisera
pacs-anonymise-note = Patientuppgifter tas bort från de skickade kopiorna. Dina filer ändras inte.
pacs-identified-note = Filerna skickas som de är, med patientuppgifter.
# The list of servers to choose from.
pacs-to = Till
pacs-no-servers = Inga servrar ännu. Lägg till en under DICOMweb-servrar.
pacs-send = Skicka…
pacs-go-back = Tillbaka
pacs-confirm-send = Skicka
pacs-confirm = Skicka { $studies ->
    [one] { $studies } undersökning
   *[other] { $studies } undersökningar
} ({ $files ->
    [one] { $files } fil
   *[other] { $files } filer
}) till { $server }?
pacs-busy = En sändning pågår fortfarande.
pacs-started = Skickar.
pacs-stopping = Stoppar…
pacs-sending = Skickar { $done } av { $total } till { $server }…
pacs-sent-all = { $total ->
    [one] { $total } fil skickades till { $server }.
   *[other] Alla { $total } filer skickades till { $server }.
}
pacs-sent-some = { $sent } av { $total } filer skickades till { $server }.
pacs-stopped = Stoppades efter { $sent } av { $total ->
        [one] { $total } fil
       *[other] { $total } filer
    }.
pacs-stopped-because = Stoppades efter { $sent } av { $total ->
        [one] { $total } fil
       *[other] { $total } filer
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } fil lagrades med en varning.
   *[other] { $count } filer lagrades med en varning.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = Och { $count } till.

## Why one file was not sent.

pacs-reason-unreadable = Kunde inte läsas: { $reason }
pacs-reason-not-dicom = Inte en DICOM-fil
pacs-reason-burned-in = Har patientuppgifter inbrända i bilden och skickas därför inte anonymiserad
pacs-reason-document = Ett dokument som inte kan anonymiseras
pacs-reason-anonymise = Kunde inte anonymiseras: { $reason }
pacs-reason-not-accepted = Servern tar inte emot { $class }
pacs-reason-convert = Kunde inte konverteras för servern: { $reason }
pacs-reason-resources = Servern har slut på utrymme eller resurser
pacs-reason-mismatch = Servern säger att data inte stämmer med sin typ
pacs-reason-understand = Servern kunde inte läsa den
pacs-reason-class = Servern stöder inte den här typen av objekt
pacs-reason-not-authorised = Servern tillåter det inte
pacs-reason-status = Servern avvisade den (status { $code })
pacs-reason-refused = Servern avvisade den
pacs-reason-too-large = För stor för servern
pacs-reason-http = Servern avvisade den (fel { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Nätverket är avstängt.
pacs-fatal-unknown-host = { $host } är inte ett känt värdnamn.
pacs-fatal-connect = Inget svarar på { $host }, port { $port }.
pacs-fatal-unreachable = { $host } kan inte nås.
pacs-fatal-timeout = { $host } svarade inte i tid.
pacs-fatal-cancelled = Stoppad.
pacs-fatal-called-ae = { $host } svarar inte på AE-titeln { $ae }. Kontrollera serverns AE-titel.
pacs-fatal-calling-ae = { $host } känner inte till vår AE-titel { $ae }. Lägg till den på servern.
pacs-fatal-busy = { $host } är upptagen. Försök igen senare.
pacs-fatal-rejected = { $host } avvisade anslutningen.
pacs-fatal-aborted = { $host } avslutade anslutningen.
pacs-fatal-nothing-accepted = { $host } tar inte emot någon av de här objekttyperna.
pacs-fatal-protocol = { $host } svarade på ett sätt som Specular inte förstår.
pacs-fatal-auth = { $host } godtog inte inloggningen.
pacs-fatal-http = { $host } svarade med fel { $status }.

## Teaching quiz.

palette-teaching-quiz = Undervisningsquiz
palette-teaching-quiz-help = Stega genom nyckelbilderna eller ett undervisningsfall, med fynden dolda tills du visar dem.
# "Case 2 of 5".
quiz-case = Fall { $n } av { $total }
quiz-hint-hidden = Blanksteg visar fynden · → nästa fall · Esc avslutar
quiz-hint-revealed = Blanksteg döljer dem igen · → nästa fall · Esc avslutar
quiz-hidden = Fynden är dolda.
quiz-no-findings = Inga fynd registrerade.
quiz-notes = Fallanteckningar: { $notes }
quiz-ended = Quizet är avslutat.
quiz-empty = Det finns inga fall att visa. Markera nyckelbilder med K eller välj en mapp med ett undervisningsfall.
quiz-not-a-case = Den här mappen är inte ett undervisningsfall som exporterats av Specular.
quiz-pick-title = Välj en mapp med ett undervisningsfall

## Teaching case folder (Export key images).

figure-keys-case = Mapp med undervisningsfall
figure-case-notes = Fallanteckningar
figure-case-findings = Fynd
figure-case-none = Inga registrerade
figure-case-readme = Undervisningsfall exporterat av Specular. Patientuppgifter är borttagna.
    images: nyckelbilderna, med inbränd text svärtad.
    DICOM: anonymiserade kopior och ett Key Object Selection-manifest "For Teaching".
    case.txt: fallanteckningarna och fynden för varje bild. case.json: samma sak för skript.
    För Radiopaedia: ladda upp DICOM-mappen eller bilderna och kopiera anteckningarna från case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } svarar inte på sökningar (C-FIND).
pacs-find-status = { $host } avvisade sökningen (status { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } skickar inte serier med C-GET.
    Välj C-MOVE i serverns inställningar.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } tar inte emot C-MOVE-begäranden.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } känner inte till vår AE-titel { $ae }.
    Lägg till den på servern med den här datorns adress och port { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } har slut på resurser. Försök igen senare.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } vägrade skicka serien (status { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } har inga bilder för den här serien.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } vägrade skicka bilder över den här anslutningen.
    Välj C-MOVE i serverns inställningar.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } kunde inte skicka bilderna till den här datorn.
    Kontrollera adressen, porten och brandväggen, och att servern känner till vår AE-titel { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } kunde inte skicka { $count ->
    [one] bilden
   *[other] någon av de { $count } bilderna
} i en form som Specular kan läsa.
# machine-drafted, needs native review
pacs-retrieve-listen = Kan inte ta emot på { $address }, port { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Certifikatet för { $host } är inte betrott.
    Utfärdaren måste finnas bland systemets betrodda certifikat.
pacs-fatal-tls = Ingen säker anslutning till { $host }.
    Kontrollera att port { $port } talar DICOM över TLS.
