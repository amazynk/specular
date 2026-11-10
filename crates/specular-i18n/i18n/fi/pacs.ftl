# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Tyyppi
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Isäntä
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Portti
dicomweb-hint-port = 104 tai 11112
# The application entity title the server answers to.
dicomweb-field-ae = Palvelimen AE title
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Oma AE title
# machine-drafted, needs native review
dicomweb-field-tls = Suojattu yhteys (TLS)
dicomweb-tls-on = Suojattu yhteys käytössä
dicomweb-tls-off = Suojattu yhteys pois käytöstä
dicomweb-problem-host = Anna palvelimen isäntänimi tai osoite.
dicomweb-problem-port = Portti on luku väliltä 1–65535.
dicomweb-problem-ae = AE titlessä on 1–16 merkkiä, ei kenoviivaa.
# machine-drafted, needs native review
dicomweb-dimse-note-open = DICOM (DIMSE) -palvelimista voi hakea, niiden sarjoja voi avata ja niille voi lähettää.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Noutotapa
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Vastaanotto-osoite
# machine-drafted, needs native review
pacs-hint-receive-address = Tämän tietokoneen osoite, esim. 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Vastaanottoportti
# machine-drafted, needs native review
pacs-hint-receive-port = Esim. 11113
# machine-drafted, needs native review
pacs-move-note = C-MOVEa varten palvelimen on tunnettava oma AE titlemme { $ae } tällä osoitteella ja portilla.
    Specular kuuntelee siellä vain, kun sarja on tulossa.
# machine-drafted, needs native review
pacs-problem-receive-address = Anna tämän tietokoneen osoite (esim. 192.168.1.20), johon vastaanotetaan.
# machine-drafted, needs native review
pacs-problem-receive-port = Anna vastaanottoportti. C-MOVE tarvitsee sen.
dicomweb-verify = Tarkista
dicomweb-verifying = Tarkistetaan palvelinta…
dicomweb-verify-ok = { $server } vastasi.

## The Studies browser and the palette.

studies-send = Lähetä PACSiin…
studies-send-description = Lähettää tämän sarjan, sen tutkimuksen tai kaiken levyllä olevan palvelimelle.
palette-send-to-pacs = Lähetä PACSiin…
palette-send-to-pacs-help = Lähetä nykyinen sarja tai tutkimus PACS-palvelimelle.

## The Send page.

pacs-title = Lähetä PACSiin
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } tutkimus
   *[other] { $studies } tutkimusta
} · { $files ->
    [one] { $files } tiedosto
   *[other] { $files } tiedostoa
}
pacs-what = Lähetä
pacs-scope-series = Tämä sarja ({ $files ->
    [one] { $files } tiedosto
   *[other] { $files } tiedostoa
})
pacs-scope-study = Tämä tutkimus ({ $files ->
    [one] { $files } tiedosto
   *[other] { $files } tiedostoa
})
pacs-scope-all = Kaikki luettu ({ $studies ->
    [one] { $studies } tutkimus
   *[other] { $studies } tutkimusta
}, { $files ->
    [one] { $files } tiedosto
   *[other] { $files } tiedostoa
})
pacs-anonymise = Anonymisoi
pacs-anonymise-note = Potilastiedot poistetaan lähetetyistä kopioista. Omia tiedostojasi ei muuteta.
pacs-identified-note = Tiedostot lähetetään sellaisinaan, potilastietoineen.
# The list of servers to choose from.
pacs-to = Kohde
pacs-no-servers = Ei vielä palvelimia. Lisää palvelin kohdassa DICOMweb-palvelimet.
pacs-send = Lähetä…
pacs-go-back = Takaisin
pacs-confirm-send = Lähetä
pacs-confirm = Lähetetäänkö { $studies ->
    [one] { $studies } tutkimus
   *[other] { $studies } tutkimusta
} ({ $files ->
    [one] { $files } tiedosto
   *[other] { $files } tiedostoa
}) palvelimelle { $server }?
pacs-busy = Lähetys on vielä käynnissä.
pacs-started = Lähetetään.
pacs-stopping = Pysäytetään…
pacs-sending = Lähetetään { $done }/{ $total } palvelimelle { $server }…
pacs-sent-all = { $total ->
    [one] { $total } tiedosto lähetettiin palvelimelle { $server }.
   *[other] Kaikki { $total } tiedostoa lähetettiin palvelimelle { $server }.
}
pacs-sent-some = { $sent }/{ $total } tiedostoa lähetettiin palvelimelle { $server }.
pacs-stopped = Pysäytettiin { $sent }/{ $total ->
        [one] { $total } tiedoston
       *[other] { $total } tiedoston
    } jälkeen.
pacs-stopped-because = Pysäytettiin { $sent }/{ $total ->
        [one] { $total } tiedoston
       *[other] { $total } tiedoston
    } jälkeen: { $reason }
pacs-warnings = { $count ->
    [one] { $count } tiedosto tallennettiin varoituksen kanssa.
   *[other] { $count } tiedostoa tallennettiin varoituksen kanssa.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = Ja { $count } muuta.

## Why one file was not sent.

pacs-reason-unreadable = Ei voitu lukea: { $reason }
pacs-reason-not-dicom = Ei DICOM-tiedosto
pacs-reason-burned-in = Kuvaan on poltettu potilastietoja, joten sitä ei lähetetä anonymisoituna
pacs-reason-document = Asiakirja, jota ei voi anonymisoida
pacs-reason-anonymise = Ei voitu anonymisoida: { $reason }
pacs-reason-not-accepted = Palvelin ei vastaanota tyyppiä { $class }
pacs-reason-convert = Ei voitu muuntaa palvelimelle: { $reason }
pacs-reason-resources = Palvelimelta on loppunut tila tai resurssit
pacs-reason-mismatch = Palvelimen mukaan tiedot eivät vastaa tyyppiään
pacs-reason-understand = Palvelin ei voinut lukea sitä
pacs-reason-class = Palvelin ei tue tämänlaista objektia
pacs-reason-not-authorised = Palvelin ei salli sitä
pacs-reason-status = Palvelin hylkäsi sen (tila { $code })
pacs-reason-refused = Palvelin hylkäsi sen
pacs-reason-too-large = Liian suuri palvelimelle
pacs-reason-http = Palvelin hylkäsi sen (virhe { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Verkko on poistettu käytöstä.
pacs-fatal-unknown-host = { $host } ei ole tunnettu isäntänimi.
pacs-fatal-connect = Osoitteessa { $host }, portissa { $port }, ei vastaa mikään.
pacs-fatal-unreachable = Palvelimeen { $host } ei saada yhteyttä.
pacs-fatal-timeout = { $host } ei vastannut ajoissa.
pacs-fatal-cancelled = Pysäytetty.
pacs-fatal-called-ae = { $host } ei vastaa AE titleen { $ae }. Tarkista palvelimen AE title.
pacs-fatal-calling-ae = { $host } ei tunne omaa AE titleämme { $ae }. Lisää se palvelimelle.
pacs-fatal-busy = { $host } on varattu. Yritä myöhemmin uudelleen.
pacs-fatal-rejected = { $host } hylkäsi yhteyden.
pacs-fatal-aborted = { $host } katkaisi yhteyden.
pacs-fatal-nothing-accepted = { $host } ei vastaanota mitään näistä objektityypeistä.
pacs-fatal-protocol = { $host } vastasi tavalla, jota Specular ei ymmärrä.
pacs-fatal-auth = { $host } ei hyväksynyt kirjautumista.
pacs-fatal-http = { $host } vastasi virheellä { $status }.

## Teaching quiz.

palette-teaching-quiz = Opetusvisa
palette-teaching-quiz-help = Käy läpi avainkuvat tai opetustapaus, löydökset piilossa, kunnes paljastat ne.
# "Case 2 of 5".
quiz-case = Tapaus { $n }/{ $total }
quiz-hint-hidden = Välilyönti näyttää löydökset · → seuraava tapaus · Esc lopettaa
quiz-hint-revealed = Välilyönti piilottaa ne taas · → seuraava tapaus · Esc lopettaa
quiz-hidden = Löydökset piilotettu.
quiz-no-findings = Löydöksiä ei ole kirjattu.
quiz-notes = Tapausmuistiinpanot: { $notes }
quiz-ended = Visa päättyi.
quiz-empty = Näytettäviä tapauksia ei ole. Merkitse avainkuvia K:lla tai valitse opetustapauksen kansio.
quiz-not-a-case = Tämä kansio ei ole Specularin viemä opetustapaus.
quiz-pick-title = Valitse opetustapauksen kansio

## Teaching case folder (Export key images).

figure-keys-case = Opetustapauksen kansio
figure-case-notes = Tapausmuistiinpanot
figure-case-findings = Löydökset
figure-case-none = Ei kirjattu
figure-case-readme = Specularin viemä opetustapaus. Potilastiedot on poistettu.
    images: avainkuvat, kuvaan poltettu teksti peitetty mustalla.
    DICOM: anonymisoidut kopiot ja Key Object Selection -luettelo "For Teaching".
    case.txt: tapausmuistiinpanot ja kunkin kuvan löydökset. case.json: samat skripteille.
    Radiopaediaa varten: lataa DICOM-kansio tai kuvat ja kopioi muistiinpanot tiedostosta case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } ei vastaa hakuihin (C-FIND).
pacs-find-status = { $host } hylkäsi haun (tila { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } ei lähetä sarjoja C-GETillä.
    Valitse C-MOVE palvelimen asetuksista.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } ei ota vastaan C-MOVE-pyyntöjä.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } ei tunne omaa AE titleämme { $ae }.
    Lisää se palvelimelle tämän tietokoneen osoitteella ja portilla { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = Palvelimelta { $host } loppuivat resurssit. Yritä myöhemmin uudelleen.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } kieltäytyi lähettämästä sarjaa (tila { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = Palvelimella { $host } ei ole kuvia tähän sarjaan.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } kieltäytyi lähettämästä kuvia tämän yhteyden kautta.
    Valitse C-MOVE palvelimen asetuksista.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } ei voinut lähettää kuvia tälle tietokoneelle.
    Tarkista osoite, portti ja palomuuri sekä se, että palvelin tuntee oman AE titlemme { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } ei voinut lähettää { $count ->
    [one] kuvaa
   *[other] yhtäkään { $count } kuvasta
} muodossa, jota Specular lukee.
# machine-drafted, needs native review
pacs-retrieve-listen = Vastaanotto osoitteessa { $address }, portissa { $port } ei onnistu: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Palvelimen { $host } varmenne ei ole luotettu.
    Sen myöntäjän on oltava järjestelmän luotettujen varmenteiden joukossa.
pacs-fatal-tls = Ei suojattua yhteyttä palvelimeen { $host }.
    Tarkista, että portissa { $port } on käytössä DICOM TLS:n yli.
