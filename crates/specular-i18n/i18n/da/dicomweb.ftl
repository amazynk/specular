# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Dansk.

palette-dicom-servers = DICOMweb-servere…

dicomweb-title = DICOMweb-servere
dicomweb-servers-intro = Undersøgelser på en server søges og åbnes kun, når du beder om det. Intet sendes før.
dicomweb-servers = Servere
dicomweb-servers-empty = Ingen servere endnu. Tilføj en med dens DICOMweb-adresse.
dicomweb-session-only = Adgangskoder og tokens gemmes kun, indtil Specular lukkes.
dicomweb-add = Tilføj…
dicomweb-edit = Rediger…
dicomweb-remove = Fjern
dicomweb-close = Luk
dicomweb-browse = Gennemse
dicomweb-back = Tilbage
dicomweb-save = Gem
dicomweb-search = Søg
dicomweb-stop = Stop
dicomweb-removed = { $server } fjernet
# machine-drafted, needs native review
dicomweb-remove-ask = Fjern “{ $server }” fra de gemte servere?
dicomweb-remove-detail = Intet på disken går tabt. Dens adgangskode eller token for denne session glemmes.
# Servere, der nås uden kryptering (http://).
dicomweb-not-encrypted = Ikke krypteret

# Siden til at tilføje eller redigere en server.
dicomweb-add-title = Tilføj en server
dicomweb-edit-title = Serverindstillinger
dicomweb-field-name = Navn
dicomweb-hint-name = Valgfrit
dicomweb-field-url = Adresse
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Login
dicomweb-auth-none = Intet login
dicomweb-auth-basic = Brugernavn og adgangskode
dicomweb-auth-bearer = Adgangstoken
dicomweb-field-user = Brugernavn
dicomweb-field-password = Adgangskode
dicomweb-field-token = Adgangstoken
dicomweb-hint-secret-kept = Lad feltet være tomt for at beholde det angivne
dicomweb-secret-session = Adgangskoden eller tokenet gemmes kun, indtil Specular lukkes, aldrig på disken.
dicomweb-secret-needed = Indtast adgangskoden eller tokenet igen: det gemmes kun, indtil Specular lukkes.
dicomweb-http-warning = Denne adresse er ikke krypteret: adgangskoden eller tokenet ville blive sendt i klartekst.
dicomweb-url-empty = Indtast serverens DICOMweb-adresse.
dicomweb-url-scheme = Adressen skal begynde med https:// eller http://.
dicomweb-url-host = Adressen har intet servernavn.
dicomweb-url-userinfo = Angiv brugernavn og adgangskode i login-felterne, ikke i adressen.
dicomweb-save-failed = Serverlisten kunne ikke gemmes: { $reason }

# Søgesiden.
dicomweb-search-title = Find undersøgelser
dicomweb-field-patient-name = Patientnavn
dicomweb-field-patient-id = Patient-ID
dicomweb-field-date-from = Fra
dicomweb-field-date-to = Til
dicomweb-hint-date = ÅÅÅÅMMDD
dicomweb-field-modality = Modalitet
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Rekvisitionsnummer
dicomweb-show-patient = Vis patientdata
dicomweb-switch-on = Til
dicomweb-switch-off = Fra
dicomweb-patient-shown = Patientdata vises
dicomweb-patient-hidden = Patientdata skjult
dicomweb-studies = Undersøgelser
dicomweb-search-hint = Udfyld et vilkårligt felt, og søg.
dicomweb-no-studies = Ingen undersøgelser matcher.
dicomweb-searching = Søger…
dicomweb-stopped = Stoppet
dicomweb-load-more = Indlæs flere
dicomweb-study-untitled = Undersøgelse uden beskrivelse
dicomweb-series-untitled = Serie uden beskrivelse
dicomweb-study-counts = { $series ->
    [one] Én serie
   *[other] { $series } serier
}, { $images ->
    [one] ét billede
   *[other] { $images } billeder
}
dicomweb-series-images = { $count ->
    [one] Ét billede
   *[other] { $count } billeder
}
dicomweb-patient-id = ID { $id }
dicomweb-accession = Rekvisition { $number }
dicomweb-found = { $count ->
    [one] Én undersøgelse fundet
   *[other] { $count } undersøgelser fundet
}
dicomweb-series-found = { $count ->
    [one] Én serie
   *[other] { $count } serier
}

# Fejl ved kommunikation med en server.
dicomweb-error-offline = { $server } kan ikke nås. Kontrollér netværket og adressen.
dicomweb-error-auth = Serveren afviste brugernavnet, adgangskoden eller tokenet.
dicomweb-error-not-found = Der er ingen DICOMweb-tjeneste på denne adresse.
dicomweb-error-other = Serveren kunne ikke svare: { $reason }

# En serie, der streames ind i fremviseren.
dicomweb-loading = Indlæser { $series }…
dicomweb-loading-progress = Indlæser { $series }: { $done } af { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] ét billede indlæst
   *[other] alle { $total } billeder indlæst
}
dicomweb-loaded-failed = { $failed } af { $total } billeder kunne ikke indlæses.
dicomweb-offline-cached = { $server } kan ikke nås: viser de billeder, der blev gemt tidligere.
dicomweb-cache-failed = Download-mappen kunne ikke åbnes: { $reason }
