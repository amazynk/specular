# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Čeština.

palette-dicom-servers = Servery DICOMweb…

dicomweb-title = Servery DICOMweb
dicomweb-servers-intro = Studie na serveru se vyhledávají a otevírají jen na vaši žádost. Předtím se nic neodesílá.
dicomweb-servers = Servery
dicomweb-servers-empty = Zatím žádné servery. Přidejte server pomocí jeho adresy DICOMweb.
dicomweb-session-only = Hesla a tokeny se uchovávají jen do zavření aplikace Specular.
dicomweb-add = Přidat…
dicomweb-edit = Upravit…
dicomweb-remove = Odebrat
dicomweb-close = Zavřít
dicomweb-browse = Procházet
dicomweb-back = Zpět
dicomweb-save = Uložit
dicomweb-search = Hledat
dicomweb-stop = Zastavit
dicomweb-removed = Odebráno: { $server }
# machine-drafted, needs native review
dicomweb-remove-ask = Odebrat „{ $server }“ z uložených serverů?
dicomweb-remove-detail = Nic na disku se neztratí. Jeho heslo nebo token pro tuto relaci se zapomene.
# Servery dostupné bez šifrování (http://).
dicomweb-not-encrypted = Nešifrováno

# Stránka pro přidání nebo úpravu serveru.
dicomweb-add-title = Přidat server
dicomweb-edit-title = Nastavení serveru
dicomweb-field-name = Název
dicomweb-hint-name = Volitelné
dicomweb-field-url = Adresa
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Přihlášení
dicomweb-auth-none = Bez přihlášení
dicomweb-auth-basic = Uživatelské jméno a heslo
dicomweb-auth-bearer = Přístupový token
dicomweb-field-user = Uživatelské jméno
dicomweb-field-password = Heslo
dicomweb-field-token = Přístupový token
dicomweb-hint-secret-kept = Ponechte prázdné, chcete-li zachovat zadanou hodnotu
dicomweb-secret-session = Heslo nebo token se uchovává jen do zavření aplikace Specular, nikdy na disku.
dicomweb-secret-needed = Zadejte heslo nebo token znovu: uchovává se jen do zavření aplikace Specular.
dicomweb-http-warning = Tato adresa není šifrovaná: heslo nebo token by se odeslal jako prostý text.
dicomweb-url-empty = Zadejte adresu DICOMweb serveru.
dicomweb-url-scheme = Adresa musí začínat https:// nebo http://.
dicomweb-url-host = Adresa neobsahuje název serveru.
dicomweb-url-userinfo = Uživatelské jméno a heslo zadejte do polí přihlášení, ne do adresy.
dicomweb-save-failed = Seznam serverů nelze uložit: { $reason }

# Stránka hledání.
dicomweb-search-title = Najít studie
dicomweb-field-patient-name = Jméno pacienta
dicomweb-field-patient-id = ID pacienta
dicomweb-field-date-from = Od
dicomweb-field-date-to = Do
dicomweb-hint-date = RRRRMMDD
dicomweb-field-modality = Modalita
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Číslo žádanky
dicomweb-show-patient = Zobrazit údaje pacienta
dicomweb-switch-on = Zapnuto
dicomweb-switch-off = Vypnuto
dicomweb-patient-shown = Údaje pacienta zobrazeny
dicomweb-patient-hidden = Údaje pacienta skryty
dicomweb-studies = Studie
dicomweb-search-hint = Vyplňte libovolná pole a spusťte hledání.
dicomweb-no-studies = Žádné odpovídající studie.
dicomweb-searching = Hledání…
dicomweb-stopped = Zastaveno
dicomweb-load-more = Načíst další
dicomweb-study-untitled = Studie bez popisu
dicomweb-series-untitled = Série bez popisu
dicomweb-study-counts = Série: { $series }, snímky: { $images }
dicomweb-series-images = Snímky: { $count }
dicomweb-patient-id = ID { $id }
dicomweb-accession = Žádanka { $number }
dicomweb-found = Nalezené studie: { $count }
dicomweb-series-found = Série: { $count }

# Chyby při komunikaci se serverem.
dicomweb-error-offline = Server { $server } není dostupný. Zkontrolujte síť a adresu.
dicomweb-error-auth = Server odmítl uživatelské jméno, heslo nebo token.
dicomweb-error-not-found = Na této adrese není žádná služba DICOMweb.
dicomweb-error-other = Server nemohl odpovědět: { $reason }

# Série streamovaná do prohlížeče.
dicomweb-loading = Načítání: { $series }…
dicomweb-loading-progress = Načítání: { $series } – { $done } z { $total }
dicomweb-loaded = { $series }: načtené snímky: { $total }
dicomweb-loaded-failed = Snímky, které nelze načíst: { $failed } z { $total }
dicomweb-offline-cached = Server { $server } není dostupný: zobrazují se dříve uložené snímky.
dicomweb-cache-failed = Složku pro stahování nelze otevřít: { $reason }
