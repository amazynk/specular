# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Type
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Vært
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Port
dicomweb-hint-port = 104 eller 11112
# The application entity title the server answers to.
dicomweb-field-ae = Serverens AE-titel
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Vores AE-titel
# machine-drafted, needs native review
dicomweb-field-tls = Sikker forbindelse (TLS)
dicomweb-tls-on = Sikker forbindelse til
dicomweb-tls-off = Sikker forbindelse fra
dicomweb-problem-host = Angiv serverens værtsnavn eller adresse.
dicomweb-problem-port = Porten er et tal fra 1 til 65535.
dicomweb-problem-ae = En AE-titel har 1 til 16 tegn, uden omvendt skråstreg.
# machine-drafted, needs native review
dicomweb-dimse-note-open = DICOM-servere (DIMSE) kan gennemsøges, åbnes og sendes til.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Hent med
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Modtag på adresse
# machine-drafted, needs native review
pacs-hint-receive-address = Denne computers adresse, fx 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Modtag på port
# machine-drafted, needs native review
pacs-hint-receive-port = Fx 11113
# machine-drafted, needs native review
pacs-move-note = Til C-MOVE skal serveren kende vores AE-titel { $ae } med denne adresse og port.
    Specular lytter der kun, mens en serie kommer ind.
# machine-drafted, needs native review
pacs-problem-receive-address = Angiv denne computers adresse (fx 192.168.1.20), som der skal modtages på.
# machine-drafted, needs native review
pacs-problem-receive-port = Angiv en port at modtage på. C-MOVE kræver en.
dicomweb-verify = Bekræft
dicomweb-verifying = Tjekker serveren…
dicomweb-verify-ok = { $server } svarede.

## The Studies browser and the palette.

studies-send = Send til PACS…
studies-send-description = Sender denne serie, dens undersøgelse eller alt på disken til en server.
palette-send-to-pacs = Send til PACS…
palette-send-to-pacs-help = Send den aktuelle serie eller undersøgelse til en PACS-server.

## The Send page.

pacs-title = Send til PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } undersøgelse
   *[other] { $studies } undersøgelser
} · { $files ->
    [one] { $files } fil
   *[other] { $files } filer
}
pacs-what = Send
pacs-scope-series = Denne serie ({ $files ->
    [one] { $files } fil
   *[other] { $files } filer
})
pacs-scope-study = Denne undersøgelse ({ $files ->
    [one] { $files } fil
   *[other] { $files } filer
})
pacs-scope-all = Alt indlæst ({ $studies ->
    [one] { $studies } undersøgelse
   *[other] { $studies } undersøgelser
}, { $files ->
    [one] { $files } fil
   *[other] { $files } filer
})
pacs-anonymise = Anonymisér
pacs-anonymise-note = Patientoplysninger fjernes fra de sendte kopier. Dine filer ændres ikke.
pacs-identified-note = Filerne sendes, som de er, med patientoplysninger.
# The list of servers to choose from.
pacs-to = Til
pacs-no-servers = Ingen servere endnu. Tilføj en under DICOMweb-servere.
pacs-send = Send…
pacs-go-back = Tilbage
pacs-confirm-send = Send
pacs-confirm = Send { $studies ->
    [one] { $studies } undersøgelse
   *[other] { $studies } undersøgelser
} ({ $files ->
    [one] { $files } fil
   *[other] { $files } filer
}) til { $server }?
pacs-busy = En afsendelse kører stadig.
pacs-started = Sender.
pacs-stopping = Stopper…
pacs-sending = Sender { $done } af { $total } til { $server }…
pacs-sent-all = { $total ->
    [one] { $total } fil blev sendt til { $server }.
   *[other] Alle { $total } filer blev sendt til { $server }.
}
pacs-sent-some = { $sent } af { $total } filer blev sendt til { $server }.
pacs-stopped = Stoppet efter { $sent } af { $total ->
        [one] { $total } fil
       *[other] { $total } filer
    }.
pacs-stopped-because = Stoppet efter { $sent } af { $total ->
        [one] { $total } fil
       *[other] { $total } filer
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } fil blev gemt med en advarsel.
   *[other] { $count } filer blev gemt med en advarsel.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = Og { $count } mere.

## Why one file was not sent.

pacs-reason-unreadable = Kunne ikke læses: { $reason }
pacs-reason-not-dicom = Ikke en DICOM-fil
pacs-reason-burned-in = Har patientoplysninger indbrændt i billedet og sendes derfor ikke anonymiseret
pacs-reason-document = Et dokument, der ikke kan anonymiseres
pacs-reason-anonymise = Kunne ikke anonymiseres: { $reason }
pacs-reason-not-accepted = Serveren modtager ikke { $class }
pacs-reason-convert = Kunne ikke konverteres til serveren: { $reason }
pacs-reason-resources = Serveren mangler plads eller ressourcer
pacs-reason-mismatch = Serveren siger, at dataene ikke passer til deres type
pacs-reason-understand = Serveren kunne ikke læse den
pacs-reason-class = Serveren understøtter ikke denne slags objekt
pacs-reason-not-authorised = Serveren tillader det ikke
pacs-reason-status = Serveren afviste den (status { $code })
pacs-reason-refused = Serveren afviste den
pacs-reason-too-large = For stor til serveren
pacs-reason-http = Serveren afviste den (fejl { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Netværket er slået fra.
pacs-fatal-unknown-host = { $host } er ikke et kendt værtsnavn.
pacs-fatal-connect = Intet svarer på { $host }, port { $port }.
pacs-fatal-unreachable = { $host } kan ikke nås.
pacs-fatal-timeout = { $host } svarede ikke i tide.
pacs-fatal-cancelled = Stoppet.
pacs-fatal-called-ae = { $host } svarer ikke på AE-titlen { $ae }. Tjek serverens AE-titel.
pacs-fatal-calling-ae = { $host } kender ikke vores AE-titel { $ae }. Tilføj den på serveren.
pacs-fatal-busy = { $host } er optaget. Prøv igen senere.
pacs-fatal-rejected = { $host } afviste forbindelsen.
pacs-fatal-aborted = { $host } afbrød forbindelsen.
pacs-fatal-nothing-accepted = { $host } modtager ingen af disse slags objekter.
pacs-fatal-protocol = { $host } svarede på en måde, som Specular ikke forstår.
pacs-fatal-auth = { $host } accepterede ikke login.
pacs-fatal-http = { $host } svarede med fejl { $status }.

## Teaching quiz.

palette-teaching-quiz = Undervisningsquiz
palette-teaching-quiz-help = Gå gennem nøglebillederne eller en undervisningssag, med fundene skjult, indtil du viser dem.
# "Case 2 of 5".
quiz-case = Sag { $n } af { $total }
quiz-hint-hidden = Mellemrum viser fundene · → næste sag · Esc afslutter
quiz-hint-revealed = Mellemrum skjuler dem igen · → næste sag · Esc afslutter
quiz-hidden = Fund skjult.
quiz-no-findings = Ingen fund registreret.
quiz-notes = Sagsnoter: { $notes }
quiz-ended = Quizzen er slut.
quiz-empty = Der er ingen sager at vise. Markér nøglebilleder med K, eller vælg en mappe med en undervisningssag.
quiz-not-a-case = Denne mappe er ikke en undervisningssag eksporteret af Specular.
quiz-pick-title = Vælg en mappe med en undervisningssag

## Teaching case folder (Export key images).

figure-keys-case = Mappe med undervisningssag
figure-case-notes = Sagsnoter
figure-case-findings = Fund
figure-case-none = Intet registreret
figure-case-readme = Undervisningssag eksporteret af Specular. Patientoplysninger er fjernet.
    images: nøglebillederne, med indbrændt tekst overstreget med sort.
    DICOM: anonymiserede kopier og et Key Object Selection-manifest "For Teaching".
    case.txt: sagsnoterne og fundene for hvert billede. case.json: det samme til scripts.
    Til Radiopaedia: upload DICOM-mappen eller billederne, og kopiér noterne fra case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } besvarer ikke søgninger (C-FIND).
pacs-find-status = { $host } afviste søgningen (status { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } sender ikke serier med C-GET.
    Vælg C-MOVE i serverens indstillinger.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } tager ikke imod C-MOVE-forespørgsler.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } kender ikke vores AE-titel { $ae }.
    Tilføj den på serveren med denne computers adresse og port { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } mangler ressourcer. Prøv igen senere.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } afviste at sende serien (status { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } har ingen billeder til denne serie.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } afviste at sende billeder over denne forbindelse.
    Vælg C-MOVE i serverens indstillinger.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } kunne ikke sende billederne til denne computer.
    Tjek adressen, porten og firewallen, og at serveren kender vores AE-titel { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } kunne ikke sende { $count ->
    [one] billedet
   *[other] nogen af de { $count } billeder
} i en form, som Specular kan læse.
# machine-drafted, needs native review
pacs-retrieve-listen = Kan ikke modtage på { $address }, port { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Certifikatet for { $host } er ikke betroet.
    Dets udsteder skal være blandt systemets betroede certifikater.
pacs-fatal-tls = Ingen sikker forbindelse til { $host }.
    Kontrollér, at port { $port } taler DICOM over TLS.
