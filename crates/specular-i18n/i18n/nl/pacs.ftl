# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Soort
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Host
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Poort
dicomweb-hint-port = 104 of 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE-title van de server
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Onze AE-title
# machine-drafted, needs native review
dicomweb-field-tls = Beveiligde verbinding (TLS)
dicomweb-tls-on = Beveiligde verbinding aan
dicomweb-tls-off = Beveiligde verbinding uit
dicomweb-problem-host = Voer de hostnaam of het adres van de server in.
dicomweb-problem-port = De poort is een getal van 1 tot 65535.
dicomweb-problem-ae = Een AE-title heeft 1 tot 16 tekens, zonder backslash.
# machine-drafted, needs native review
dicomweb-dimse-note-open = DICOM-servers (DIMSE) kunnen worden doorzocht en geopend, en er kan naartoe worden gestuurd.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Ophalen met
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Ontvangen op adres
# machine-drafted, needs native review
pacs-hint-receive-address = Adres van deze computer, zoals 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Ontvangen op poort
# machine-drafted, needs native review
pacs-hint-receive-port = Zoals 11113
# machine-drafted, needs native review
pacs-move-note = Voor C-MOVE moet de server onze AE-title { $ae } kennen met dit adres en deze poort.
    Specular luistert daar alleen zolang er een serie binnenkomt.
# machine-drafted, needs native review
pacs-problem-receive-address = Voer het adres van deze computer in (zoals 192.168.1.20) om op te ontvangen.
# machine-drafted, needs native review
pacs-problem-receive-port = Voer een poort in om op te ontvangen. C-MOVE heeft er een nodig.
dicomweb-verify = Controleren
dicomweb-verifying = Server controleren…
dicomweb-verify-ok = { $server } heeft geantwoord.

## The Studies browser and the palette.

studies-send = Naar PACS sturen…
studies-send-description = Stuurt deze serie, het bijbehorende onderzoek of alles op de schijf naar een server.
palette-send-to-pacs = Naar PACS sturen…
palette-send-to-pacs-help = De huidige serie of het huidige onderzoek naar een PACS-server sturen.

## The Send page.

pacs-title = Naar PACS sturen
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } onderzoek
   *[other] { $studies } onderzoeken
} · { $files ->
    [one] { $files } bestand
   *[other] { $files } bestanden
}
pacs-what = Sturen
pacs-scope-series = Deze serie ({ $files ->
    [one] { $files } bestand
   *[other] { $files } bestanden
})
pacs-scope-study = Dit onderzoek ({ $files ->
    [one] { $files } bestand
   *[other] { $files } bestanden
})
pacs-scope-all = Alles wat is ingelezen ({ $studies ->
    [one] { $studies } onderzoek
   *[other] { $studies } onderzoeken
}, { $files ->
    [one] { $files } bestand
   *[other] { $files } bestanden
})
pacs-anonymise = Anonimiseren
pacs-anonymise-note = Patiëntgegevens worden uit de verstuurde kopieën verwijderd. Je bestanden blijven ongewijzigd.
pacs-identified-note = De bestanden worden ongewijzigd verstuurd, met patiëntgegevens.
# The list of servers to choose from.
pacs-to = Naar
pacs-no-servers = Nog geen servers. Voeg er een toe in DICOMweb-servers.
pacs-send = Sturen…
pacs-go-back = Terug
pacs-confirm-send = Sturen
pacs-confirm = { $studies ->
    [one] { $studies } onderzoek
   *[other] { $studies } onderzoeken
} ({ $files ->
    [one] { $files } bestand
   *[other] { $files } bestanden
}) naar { $server } sturen?
pacs-busy = Er loopt nog een verzending.
pacs-started = Bezig met versturen.
pacs-stopping = Stoppen…
pacs-sending = { $done } van { $total } naar { $server } versturen…
pacs-sent-all = { $total ->
    [one] { $total } bestand is naar { $server } verstuurd.
   *[other] Alle { $total } bestanden zijn naar { $server } verstuurd.
}
pacs-sent-some = { $sent } van { $total } bestanden zijn naar { $server } verstuurd.
pacs-stopped = Gestopt na { $sent } van { $total ->
        [one] { $total } bestand
       *[other] { $total } bestanden
    }.
pacs-stopped-because = Gestopt na { $sent } van { $total ->
        [one] { $total } bestand
       *[other] { $total } bestanden
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } bestand is met een waarschuwing opgeslagen.
   *[other] { $count } bestanden zijn met een waarschuwing opgeslagen.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = En nog { $count }.

## Why one file was not sent.

pacs-reason-unreadable = Kon niet worden gelezen: { $reason }
pacs-reason-not-dicom = Geen DICOM-bestand
pacs-reason-burned-in = Bevat in het beeld ingebrande patiëntgegevens en wordt daarom niet geanonimiseerd verstuurd
pacs-reason-document = Een document dat niet kan worden geanonimiseerd
pacs-reason-anonymise = Kon niet worden geanonimiseerd: { $reason }
pacs-reason-not-accepted = De server neemt geen { $class } aan
pacs-reason-convert = Kon niet voor de server worden omgezet: { $reason }
pacs-reason-resources = De server heeft geen ruimte of middelen meer
pacs-reason-mismatch = Volgens de server passen de gegevens niet bij hun type
pacs-reason-understand = De server kon het niet lezen
pacs-reason-class = De server ondersteunt dit soort object niet
pacs-reason-not-authorised = De server staat het niet toe
pacs-reason-status = De server weigerde het (status { $code })
pacs-reason-refused = De server weigerde het
pacs-reason-too-large = Te groot voor de server
pacs-reason-http = De server weigerde het (fout { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Het netwerk staat uit.
pacs-fatal-unknown-host = { $host } is geen bekende hostnaam.
pacs-fatal-connect = Niets antwoordt op { $host }, poort { $port }.
pacs-fatal-unreachable = { $host } is niet bereikbaar.
pacs-fatal-timeout = { $host } antwoordde niet op tijd.
pacs-fatal-cancelled = Gestopt.
pacs-fatal-called-ae = { $host } antwoordt niet op de AE-title { $ae }. Controleer de AE-title van de server.
pacs-fatal-calling-ae = { $host } kent onze AE-title { $ae } niet. Voeg deze toe op de server.
pacs-fatal-busy = { $host } is bezet. Probeer het later opnieuw.
pacs-fatal-rejected = { $host } weigerde de verbinding.
pacs-fatal-aborted = { $host } heeft de verbinding verbroken.
pacs-fatal-nothing-accepted = { $host } neemt geen van deze soorten objecten aan.
pacs-fatal-protocol = { $host } antwoordde op een manier die Specular niet begrijpt.
pacs-fatal-auth = { $host } accepteerde de aanmelding niet.
pacs-fatal-http = { $host } antwoordde met fout { $status }.

## Teaching quiz.

palette-teaching-quiz = Lesquiz
palette-teaching-quiz-help = Door de sleutelbeelden of een lescasus gaan, met de bevindingen verborgen tot je ze toont.
# "Case 2 of 5".
quiz-case = Casus { $n } van { $total }
quiz-hint-hidden = Spatie toont de bevindingen · → volgende casus · Esc stopt
quiz-hint-revealed = Spatie verbergt ze weer · → volgende casus · Esc stopt
quiz-hidden = Bevindingen verborgen.
quiz-no-findings = Geen bevindingen vastgelegd.
quiz-notes = Casusnotities: { $notes }
quiz-ended = Quiz beëindigd.
quiz-empty = Er zijn geen casussen om te tonen. Markeer sleutelbeelden met K of kies een lescasusmap.
quiz-not-a-case = Deze map is geen lescasus die door Specular is geëxporteerd.
quiz-pick-title = Lescasusmap kiezen

## Teaching case folder (Export key images).

figure-keys-case = Lescasusmap
figure-case-notes = Casusnotities
figure-case-findings = Bevindingen
figure-case-none = Geen vastgelegd
figure-case-readme = Lescasus geëxporteerd door Specular. Patiëntgegevens zijn verwijderd.
    images: de sleutelbeelden, ingebrande tekst zwartgemaakt.
    DICOM: geanonimiseerde kopieën en een Key Object Selection-manifest "For Teaching".
    case.txt: de casusnotities en de bevindingen per beeld. case.json: hetzelfde voor scripts.
    Voor Radiopaedia: upload de DICOM-map of de beelden en kopieer de notities uit case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } beantwoordt geen zoekopdrachten (C-FIND).
pacs-find-status = { $host } weigerde de zoekopdracht (status { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } stuurt geen series met C-GET.
    Kies C-MOVE in de instellingen van de server.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } neemt geen C-MOVE-verzoeken aan.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } kent onze AE-title { $ae } niet.
    Voeg deze toe op de server met het adres van deze computer en poort { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } heeft geen middelen meer. Probeer het later opnieuw.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } weigerde de serie te sturen (status { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } heeft geen beelden voor deze serie.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } weigerde beelden via deze verbinding te sturen.
    Kies C-MOVE in de instellingen van de server.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } kon de beelden niet naar deze computer sturen.
    Controleer het adres, de poort en de firewall, en of de server onze AE-title { $ae } kent.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } kon { $count ->
    [one] het beeld niet
   *[other] geen van de { $count } beelden
} sturen in een vorm die Specular kan lezen.
# machine-drafted, needs native review
pacs-retrieve-listen = Kan niet ontvangen op { $address }, poort { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Het certificaat van { $host } wordt niet vertrouwd.
    De uitgever ervan moet bij de vertrouwde certificaten van het systeem staan.
pacs-fatal-tls = Geen beveiligde verbinding met { $host }.
    Controleer of poort { $port } DICOM via TLS spreekt.
