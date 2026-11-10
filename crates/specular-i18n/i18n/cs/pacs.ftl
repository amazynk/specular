# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Druh
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Hostitel
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Port
dicomweb-hint-port = 104 nebo 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE title serveru
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Náš AE title
# machine-drafted, needs native review
dicomweb-field-tls = Zabezpečené připojení (TLS)
dicomweb-tls-on = Zabezpečené připojení zapnuto
dicomweb-tls-off = Zabezpečené připojení vypnuto
dicomweb-problem-host = Zadejte název hostitele nebo adresu serveru.
dicomweb-problem-port = Port je číslo od 1 do 65535.
dicomweb-problem-ae = AE title má 1 až 16 znaků, bez zpětného lomítka.
# machine-drafted, needs native review
dicomweb-dimse-note-open = Servery DICOM (DIMSE) lze prohledávat, otevírat z nich série a odesílat na ně.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Načítat pomocí
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Přijímat na adrese
# machine-drafted, needs native review
pacs-hint-receive-address = Adresa tohoto počítače, např. 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Přijímat na portu
# machine-drafted, needs native review
pacs-hint-receive-port = Např. 11113
# machine-drafted, needs native review
pacs-move-note = Pro C-MOVE musí server znát náš AE title { $ae } s touto adresou a portem.
    Specular zde naslouchá, jen dokud přichází série.
# machine-drafted, needs native review
pacs-problem-receive-address = Zadejte adresu tohoto počítače (např. 192.168.1.20), na které se má přijímat.
# machine-drafted, needs native review
pacs-problem-receive-port = Zadejte port pro příjem. C-MOVE ho potřebuje.
dicomweb-verify = Ověřit
dicomweb-verifying = Kontrola serveru…
dicomweb-verify-ok = Server { $server } odpověděl.

## The Studies browser and the palette.

studies-send = Odeslat do PACS…
studies-send-description = Odešle tuto sérii, její studii nebo vše na disku na server.
palette-send-to-pacs = Odeslat do PACS…
palette-send-to-pacs-help = Odeslat aktuální sérii nebo studii na server PACS.

## The Send page.

pacs-title = Odeslat do PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } studie
    [few] { $studies } studie
    [many] { $studies } studie
   *[other] { $studies } studií
} · { $files ->
    [one] { $files } soubor
    [few] { $files } soubory
    [many] { $files } souboru
   *[other] { $files } souborů
}
pacs-what = Odeslat
pacs-scope-series = Tato série ({ $files ->
    [one] { $files } soubor
    [few] { $files } soubory
    [many] { $files } souboru
   *[other] { $files } souborů
})
pacs-scope-study = Tato studie ({ $files ->
    [one] { $files } soubor
    [few] { $files } soubory
    [many] { $files } souboru
   *[other] { $files } souborů
})
pacs-scope-all = Vše načtené ({ $studies ->
    [one] { $studies } studie
    [few] { $studies } studie
    [many] { $studies } studie
   *[other] { $studies } studií
}, { $files ->
    [one] { $files } soubor
    [few] { $files } soubory
    [many] { $files } souboru
   *[other] { $files } souborů
})
pacs-anonymise = Anonymizovat
pacs-anonymise-note = Z odeslaných kopií se odstraní údaje o pacientovi. Vaše soubory se nezmění.
pacs-identified-note = Soubory se odešlou tak, jak jsou, s údaji o pacientovi.
# The list of servers to choose from.
pacs-to = Kam
pacs-no-servers = Zatím žádné servery. Přidejte server v části Servery DICOMweb.
pacs-send = Odeslat…
pacs-go-back = Zpět
pacs-confirm-send = Odeslat
pacs-confirm = Odeslat { $studies ->
    [one] { $studies } studii
    [few] { $studies } studie
    [many] { $studies } studie
   *[other] { $studies } studií
} ({ $files ->
    [one] { $files } soubor
    [few] { $files } soubory
    [many] { $files } souboru
   *[other] { $files } souborů
}) na server { $server }?
pacs-busy = Odesílání stále probíhá.
pacs-started = Odesílá se.
pacs-stopping = Zastavování…
pacs-sending = Odesílání { $done } z { $total } na server { $server }…
pacs-sent-all = { $total ->
    [one] { $total } soubor byl odeslán na server { $server }.
    [few] Všechny { $total } soubory byly odeslány na server { $server }.
    [many] Všech { $total } souboru bylo odesláno na server { $server }.
   *[other] Všech { $total } souborů bylo odesláno na server { $server }.
}
pacs-sent-some = Na server { $server } bylo odesláno { $sent } z { $total } souborů.
pacs-stopped = Zastaveno po { $sent } z { $total ->
        [one] { $total } souboru
        [few] { $total } souborů
        [many] { $total } souboru
       *[other] { $total } souborů
    }.
pacs-stopped-because = Zastaveno po { $sent } z { $total ->
        [one] { $total } souboru
        [few] { $total } souborů
        [many] { $total } souboru
       *[other] { $total } souborů
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } soubor byl uložen s varováním.
    [few] { $count } soubory byly uloženy s varováním.
    [many] { $count } souboru bylo uloženo s varováním.
   *[other] { $count } souborů bylo uloženo s varováním.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = A { $count } dalších.

## Why one file was not sent.

pacs-reason-unreadable = Nelze přečíst: { $reason }
pacs-reason-not-dicom = Není soubor DICOM
pacs-reason-burned-in = Má údaje o pacientovi vtištěné do obrazu, proto se anonymizovaný neodesílá
pacs-reason-document = Dokument, který nelze anonymizovat
pacs-reason-anonymise = Nelze anonymizovat: { $reason }
pacs-reason-not-accepted = Server nepřijímá { $class }
pacs-reason-convert = Nelze převést pro server: { $reason }
pacs-reason-resources = Server nemá dost místa nebo prostředků
pacs-reason-mismatch = Podle serveru data neodpovídají svému typu
pacs-reason-understand = Server jej nedokázal přečíst
pacs-reason-class = Server tento druh objektu nepodporuje
pacs-reason-not-authorised = Server to nepovoluje
pacs-reason-status = Server jej odmítl (stav { $code })
pacs-reason-refused = Server jej odmítl
pacs-reason-too-large = Pro server příliš velký
pacs-reason-http = Server jej odmítl (chyba { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Síť je vypnutá.
pacs-fatal-unknown-host = { $host } není známý název hostitele.
pacs-fatal-connect = Na { $host }, port { $port }, nic neodpovídá.
pacs-fatal-unreachable = { $host } není dostupný.
pacs-fatal-timeout = { $host } neodpověděl včas.
pacs-fatal-cancelled = Zastaveno.
pacs-fatal-called-ae = { $host } neodpovídá na AE title { $ae }. Zkontrolujte AE title serveru.
pacs-fatal-calling-ae = { $host } nezná náš AE title { $ae }. Přidejte jej na serveru.
pacs-fatal-busy = { $host } je zaneprázdněn. Zkuste to později.
pacs-fatal-rejected = { $host } odmítl spojení.
pacs-fatal-aborted = { $host } ukončil spojení.
pacs-fatal-nothing-accepted = { $host } nepřijímá žádný z těchto druhů objektů.
pacs-fatal-protocol = { $host } odpověděl způsobem, kterému Specular nerozumí.
pacs-fatal-auth = { $host } nepřijal přihlášení.
pacs-fatal-http = { $host } odpověděl chybou { $status }.

## Teaching quiz.

palette-teaching-quiz = Výukový kvíz
palette-teaching-quiz-help = Procházet klíčové snímky nebo výukový případ, nálezy skryté, dokud je neodkryjete.
# "Case 2 of 5".
quiz-case = Případ { $n } z { $total }
quiz-hint-hidden = Mezerník odkryje nálezy · → další případ · Esc ukončí
quiz-hint-revealed = Mezerník je znovu skryje · → další případ · Esc ukončí
quiz-hidden = Nálezy skryty.
quiz-no-findings = Žádné nálezy nezaznamenány.
quiz-notes = Poznámky k případu: { $notes }
quiz-ended = Kvíz ukončen.
quiz-empty = Nejsou žádné případy k zobrazení. Označte klíčové snímky klávesou K nebo vyberte složku výukového případu.
quiz-not-a-case = Tato složka není výukový případ exportovaný aplikací Specular.
quiz-pick-title = Vyberte složku výukového případu

## Teaching case folder (Export key images).

figure-keys-case = Složka výukového případu
figure-case-notes = Poznámky k případu
figure-case-findings = Nálezy
figure-case-none = Nic nezaznamenáno
figure-case-readme = Výukový případ exportovaný aplikací Specular. Údaje o pacientovi jsou odstraněny.
    images: klíčové snímky, vtištěný text začerněn.
    DICOM: anonymizované kopie a manifest Key Object Selection „For Teaching“.
    case.txt: poznámky k případu a nálezy ke každému snímku. case.json: totéž pro skripty.
    Pro Radiopaedii: nahrajte složku DICOM nebo snímky a zkopírujte poznámky z case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } neodpovídá na vyhledávání (C-FIND).
pacs-find-status = { $host } odmítl vyhledávání (stav { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } neposílá série pomocí C-GET.
    V nastavení serveru zvolte C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } nepřijímá požadavky C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } nezná náš AE title { $ae }.
    Přidejte jej na serveru s adresou tohoto počítače a portem { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } nemá volné prostředky. Zkuste to později.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } odmítl odeslat sérii (stav { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } nemá pro tuto sérii žádné snímky.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } odmítl posílat snímky přes toto připojení.
    V nastavení serveru zvolte C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } nemohl odeslat snímky do tohoto počítače.
    Zkontrolujte adresu, port a firewall a také, zda server zná náš AE title { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } nemohl odeslat { $count ->
    [one] snímek
    [few] ani jeden z { $count } snímků
    [many] ani jeden z { $count } snímku
   *[other] ani jeden z { $count } snímků
} ve formě, kterou Specular přečte.
# machine-drafted, needs native review
pacs-retrieve-listen = Nelze přijímat na { $address }, port { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Certifikát { $host } není důvěryhodný.
    Jeho vydavatel musí být mezi důvěryhodnými certifikáty systému.
pacs-fatal-tls = Žádné zabezpečené připojení k { $host }.
    Zkontrolujte, zda port { $port } používá DICOM přes TLS.
