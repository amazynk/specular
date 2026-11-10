# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Svenska.

palette-dicom-servers = DICOMweb-servrar…

dicomweb-title = DICOMweb-servrar
dicomweb-servers-intro = Undersökningar på en server söks och öppnas bara när du ber om det. Inget skickas innan dess.
dicomweb-servers = Servrar
dicomweb-servers-empty = Inga servrar ännu. Lägg till en med dess DICOMweb-adress.
dicomweb-session-only = Lösenord och token sparas bara tills Specular stängs.
dicomweb-add = Lägg till…
dicomweb-edit = Redigera…
dicomweb-remove = Ta bort
dicomweb-close = Stäng
dicomweb-browse = Bläddra
dicomweb-back = Tillbaka
dicomweb-save = Spara
dicomweb-search = Sök
dicomweb-stop = Stoppa
dicomweb-removed = { $server } togs bort
# machine-drafted, needs native review
dicomweb-remove-ask = Ta bort ”{ $server }” från de sparade servrarna?
dicomweb-remove-detail = Inget på disken går förlorat. Dess lösenord eller token för den här sessionen glöms bort.
# Servrar som nås utan kryptering (http://).
dicomweb-not-encrypted = Okrypterad

# Sidan för att lägga till eller redigera en server.
dicomweb-add-title = Lägg till en server
dicomweb-edit-title = Serverinställningar
dicomweb-field-name = Namn
dicomweb-hint-name = Valfritt
dicomweb-field-url = Adress
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Inloggning
dicomweb-auth-none = Ingen inloggning
dicomweb-auth-basic = Användarnamn och lösenord
dicomweb-auth-bearer = Åtkomsttoken
dicomweb-field-user = Användarnamn
dicomweb-field-password = Lösenord
dicomweb-field-token = Åtkomsttoken
dicomweb-hint-secret-kept = Lämna tomt för att behålla det angivna
dicomweb-secret-session = Lösenordet eller token sparas bara tills Specular stängs, aldrig på disk.
dicomweb-secret-needed = Ange lösenordet eller token igen: det sparas bara tills Specular stängs.
dicomweb-http-warning = Adressen är inte krypterad: lösenordet eller token skulle skickas i klartext.
dicomweb-url-empty = Ange serverns DICOMweb-adress.
dicomweb-url-scheme = Adressen måste börja med https:// eller http://.
dicomweb-url-host = Adressen saknar servernamn.
dicomweb-url-userinfo = Ange användarnamn och lösenord i inloggningsfälten, inte i adressen.
dicomweb-save-failed = Serverlistan kunde inte sparas: { $reason }

# Söksidan.
dicomweb-search-title = Hitta undersökningar
dicomweb-field-patient-name = Patientnamn
dicomweb-field-patient-id = Patient-ID
dicomweb-field-date-from = Från
dicomweb-field-date-to = Till
dicomweb-hint-date = ÅÅÅÅMMDD
dicomweb-field-modality = Modalitet
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Remissnummer
dicomweb-show-patient = Visa patientdata
dicomweb-switch-on = På
dicomweb-switch-off = Av
dicomweb-patient-shown = Patientdata visas
dicomweb-patient-hidden = Patientdata dolda
dicomweb-studies = Undersökningar
dicomweb-search-hint = Fyll i valfria fält och sök.
dicomweb-no-studies = Inga undersökningar matchar.
dicomweb-searching = Söker…
dicomweb-stopped = Stoppad
dicomweb-load-more = Läs in fler
dicomweb-study-untitled = Undersökning utan beskrivning
dicomweb-series-untitled = Serie utan beskrivning
dicomweb-study-counts = { $series ->
    [one] En serie
   *[other] { $series } serier
}, { $images ->
    [one] en bild
   *[other] { $images } bilder
}
dicomweb-series-images = { $count ->
    [one] En bild
   *[other] { $count } bilder
}
dicomweb-patient-id = ID { $id }
dicomweb-accession = Remiss { $number }
dicomweb-found = { $count ->
    [one] En undersökning hittades
   *[other] { $count } undersökningar hittades
}
dicomweb-series-found = { $count ->
    [one] En serie
   *[other] { $count } serier
}

# Fel vid kommunikation med en server.
dicomweb-error-offline = { $server } kan inte nås. Kontrollera nätverket och adressen.
dicomweb-error-auth = Servern avvisade användarnamnet, lösenordet eller token.
dicomweb-error-not-found = Det finns ingen DICOMweb-tjänst på den här adressen.
dicomweb-error-other = Servern kunde inte svara: { $reason }

# En serie som strömmas till visaren.
dicomweb-loading = Läser in { $series }…
dicomweb-loading-progress = Läser in { $series }: { $done } av { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] en bild inläst
   *[other] alla { $total } bilder inlästa
}
dicomweb-loaded-failed = { $failed } av { $total } bilder kunde inte läsas in.
dicomweb-offline-cached = { $server } kan inte nås: visar bilderna som sparades tidigare.
dicomweb-cache-failed = Nedladdningsmappen kunde inte öppnas: { $reason }
