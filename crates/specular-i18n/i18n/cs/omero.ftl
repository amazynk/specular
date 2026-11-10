# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Čeština. machine-drafted, needs native review

palette-omero-servers = Servery OMERO…

omero-title = Servery OMERO
omero-servers-intro = Projekty, screeny a obrázky na serveru se zobrazí až při procházení. Předtím se nic neodesílá.
omero-servers = Servery
omero-servers-empty = Zatím žádné servery. Přidejte server pomocí jeho adresy OMERO.web.
omero-session-only = Hesla se uchovávají jen do zavření aplikace Specular.
omero-add = Přidat…
omero-edit = Upravit…
omero-remove = Odebrat
omero-close = Zavřít
omero-browse = Procházet
omero-back = Zpět
omero-save = Uložit
omero-stop = Zastavit
omero-open = Otevřít
# $server a server's name.
omero-removed = Odebráno: { $server }
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Nešifrováno
# A server browsed without signing in.
omero-public = Veřejný
# $user the user name a server is signed in with.
omero-signed-in-as = Jako { $user }

# The page that adds or edits a server.
omero-add-title = Přidat server
omero-edit-title = Nastavení serveru
omero-field-name = Název
omero-hint-name = Nepovinné
omero-field-url = Adresa
omero-hint-url = https://omero.example.org
omero-field-user = Uživatelské jméno
omero-hint-user = Prázdné pro veřejná data
omero-field-password = Heslo
omero-hint-password-kept = Ponechte prázdné pro zachování zadaného hesla
omero-password-session = Heslo se uchovává jen do zavření aplikace Specular, nikdy na disku.
omero-password-needed = Zadejte heslo: uchovává se jen do zavření aplikace Specular.
omero-http-warning = Tato adresa není šifrovaná: heslo by bylo odesláno jako prostý text.
omero-url-invalid = Zadejte adresu OMERO.web začínající https:// nebo http://.
# $reason why the list could not be saved.
omero-save-failed = Seznam serverů se nepodařilo uložit: { $reason }

# The browse page.
omero-browse-title = Procházení
omero-top = Projekty a screeny
omero-datasets = Datové sady
omero-plates = Destičky
omero-wells = Jamky
omero-fields = Pole
omero-images = Obrázky
omero-kind-project = Projekt
omero-kind-dataset = Datová sada
omero-kind-image = Obrázek
omero-kind-screen = Screen
omero-kind-plate = Destička
omero-kind-well = Jamka
# $well a well's row and column, as B7.
omero-well = Jamka { $well }
omero-well-fields = { $count ->
    [one] Jedno pole
    [few] { $count } pole
    [many] { $count } pole
   *[other] { $count } polí
}
omero-children = { $count ->
    [one] Jedna položka
    [few] { $count } položky
    [many] { $count } položky
   *[other] { $count } položek
}
omero-load-more = Načíst další
omero-loading = Načítání…
omero-empty = Nic tu není
omero-stopped = Zastaveno
omero-found = { $count ->
    [one] Jedna položka
    [few] { $count } položky
    [many] { $count } položky
   *[other] { $count } položek
}
# $reason why the image could not be opened.
omero-open-failed = Obrázek se nepodařilo otevřít: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Alespoň jeden kanál zůstává zobrazen
# $server a server's name.
omero-session-ended = Relace na serveru { $server } skončila. Pokračujte zadáním hesla.

# Errors.
# $server a server's name.
omero-error-offline = Server { $server } není dostupný.
omero-error-sign-in = Tento server vyžaduje přihlášení.
omero-error-expired = Relace skončila.
omero-error-login = Uživatelské jméno nebo heslo bylo odmítnuto.
omero-error-forbidden = Váš účet k tomu nemusí mít přístup.
omero-error-not-found = Na serveru nenalezeno (mohlo být odstraněno).
# $code an HTTP status code.
omero-error-status = Server odpověděl chybou { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Odpověď serveru nelze přečíst: { $reason }
