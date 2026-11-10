# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Nederlands.

palette-dicom-servers = DICOMweb-servers…

dicomweb-title = DICOMweb-servers
dicomweb-servers-intro = Onderzoeken op een server worden alleen gezocht en geopend als u daarom vraagt. Vooraf wordt niets verzonden.
dicomweb-servers = Servers
dicomweb-servers-empty = Nog geen servers. Voeg er een toe met zijn DICOMweb-adres.
dicomweb-session-only = Wachtwoorden en tokens worden alleen bewaard tot Specular wordt gesloten.
dicomweb-add = Toevoegen…
dicomweb-edit = Bewerken…
dicomweb-remove = Verwijderen
dicomweb-close = Sluiten
dicomweb-browse = Bladeren
dicomweb-back = Terug
dicomweb-save = Opslaan
dicomweb-search = Zoeken
dicomweb-stop = Stoppen
dicomweb-removed = { $server } verwijderd
# machine-drafted, needs native review
dicomweb-remove-ask = ‘{ $server }’ uit de opgeslagen servers verwijderen?
dicomweb-remove-detail = Er gaat niets verloren op de schijf. Het wachtwoord of token voor deze sessie wordt vergeten.
# Servers die zonder versleuteling worden bereikt (http://).
dicomweb-not-encrypted = Niet versleuteld

# Pagina om een server toe te voegen of te bewerken.
dicomweb-add-title = Server toevoegen
dicomweb-edit-title = Serverinstellingen
dicomweb-field-name = Naam
dicomweb-hint-name = Optioneel
dicomweb-field-url = Adres
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Aanmelding
dicomweb-auth-none = Geen aanmelding
dicomweb-auth-basic = Gebruikersnaam en wachtwoord
dicomweb-auth-bearer = Toegangstoken
dicomweb-field-user = Gebruikersnaam
dicomweb-field-password = Wachtwoord
dicomweb-field-token = Toegangstoken
dicomweb-hint-secret-kept = Leeg laten om het huidige te behouden
dicomweb-secret-session = Het wachtwoord of token wordt alleen bewaard tot Specular wordt gesloten, nooit op schijf.
dicomweb-secret-needed = Voer het wachtwoord of token opnieuw in: het wordt alleen bewaard tot Specular wordt gesloten.
dicomweb-http-warning = Dit adres is niet versleuteld: het wachtwoord of token zou onversleuteld worden verzonden.
dicomweb-url-empty = Voer het DICOMweb-adres van de server in.
dicomweb-url-scheme = Het adres moet beginnen met https:// of http://.
dicomweb-url-host = Het adres bevat geen servernaam.
dicomweb-url-userinfo = Zet de gebruikersnaam en het wachtwoord in de aanmeldvelden, niet in het adres.
dicomweb-save-failed = De serverlijst kon niet worden opgeslagen: { $reason }

# Zoekpagina.
dicomweb-search-title = Onderzoeken zoeken
dicomweb-field-patient-name = Patiëntnaam
dicomweb-field-patient-id = Patiënt-ID
dicomweb-field-date-from = Van
dicomweb-field-date-to = Tot
dicomweb-hint-date = JJJJMMDD
dicomweb-field-modality = Modaliteit
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Accessienummer
dicomweb-show-patient = Patiëntgegevens tonen
dicomweb-switch-on = Aan
dicomweb-switch-off = Uit
dicomweb-patient-shown = Patiëntgegevens zichtbaar
dicomweb-patient-hidden = Patiëntgegevens verborgen
dicomweb-studies = Onderzoeken
dicomweb-search-hint = Vul een of meer velden in en zoek.
dicomweb-no-studies = Geen overeenkomende onderzoeken.
dicomweb-searching = Zoeken…
dicomweb-stopped = Gestopt
dicomweb-load-more = Meer laden
dicomweb-study-untitled = Onderzoek zonder beschrijving
dicomweb-series-untitled = Serie zonder beschrijving
dicomweb-study-counts = { $series ->
    [one] Eén serie
   *[other] { $series } series
}, { $images ->
    [one] één beeld
   *[other] { $images } beelden
}
dicomweb-series-images = { $count ->
    [one] Eén beeld
   *[other] { $count } beelden
}
dicomweb-patient-id = ID { $id }
dicomweb-accession = Accessie { $number }
dicomweb-found = { $count ->
    [one] Eén onderzoek gevonden
   *[other] { $count } onderzoeken gevonden
}
dicomweb-series-found = { $count ->
    [one] Eén serie
   *[other] { $count } series
}

# Fouten bij communicatie met een server.
dicomweb-error-offline = { $server } is niet bereikbaar. Controleer het netwerk en het adres.
dicomweb-error-auth = De server heeft de gebruikersnaam, het wachtwoord of het token geweigerd.
dicomweb-error-not-found = Op dit adres is geen DICOMweb-dienst.
dicomweb-error-other = De server kon niet antwoorden: { $reason }

# Een serie die naar de viewer wordt gestreamd.
dicomweb-loading = { $series } laden…
dicomweb-loading-progress = { $series } laden: { $done } van { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] één beeld geladen
   *[other] alle { $total } beelden geladen
}
dicomweb-loaded-failed = { $failed } van { $total } beelden konden niet worden geladen.
dicomweb-offline-cached = { $server } is niet bereikbaar: eerder opgeslagen beelden worden getoond.
dicomweb-cache-failed = De downloadmap kon niet worden geopend: { $reason }
