# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Svenska. machine-drafted, needs native review

palette-omero-servers = OMERO-servrar…

omero-title = OMERO-servrar
omero-servers-intro = Projekt, screens och bilder på en server listas först när du bläddrar i den. Inget skickas innan dess.
omero-servers = Servrar
omero-servers-empty = Inga servrar än. Lägg till en med dess OMERO.web-adress.
omero-session-only = Lösenord sparas bara tills Specular stängs.
omero-add = Lägg till…
omero-edit = Redigera…
omero-remove = Ta bort
omero-close = Stäng
omero-browse = Bläddra
omero-back = Tillbaka
omero-save = Spara
omero-stop = Stoppa
omero-open = Öppna
# $server a server's name.
omero-removed = { $server } borttagen
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Inte krypterad
# A server browsed without signing in.
omero-public = Offentlig
# $user the user name a server is signed in with.
omero-signed-in-as = Som { $user }

# The page that adds or edits a server.
omero-add-title = Lägg till en server
omero-edit-title = Serverinställningar
omero-field-name = Namn
omero-hint-name = Valfritt
omero-field-url = Adress
omero-hint-url = https://omero.example.org
omero-field-user = Användarnamn
omero-hint-user = Tomt för offentliga data
omero-field-password = Lösenord
omero-hint-password-kept = Lämna tomt för att behålla det angivna
omero-password-session = Lösenordet sparas bara tills Specular stängs, aldrig på disk.
omero-password-needed = Ange lösenordet: det sparas bara tills Specular stängs.
omero-http-warning = Den här adressen är inte krypterad: lösenordet skulle skickas i klartext.
omero-url-invalid = Ange OMERO.web-adressen, som börjar med https:// eller http://.
# $reason why the list could not be saved.
omero-save-failed = Serverlistan kunde inte sparas: { $reason }

# The browse page.
omero-browse-title = Bläddra
omero-top = Projekt och screens
omero-datasets = Datamängder
omero-plates = Plattor
omero-wells = Brunnar
omero-fields = Fält
omero-images = Bilder
omero-kind-project = Projekt
omero-kind-dataset = Datamängd
omero-kind-image = Bild
omero-kind-screen = Screen
omero-kind-plate = Platta
omero-kind-well = Brunn
# $well a well's row and column, as B7.
omero-well = Brunn { $well }
omero-well-fields = { $count ->
    [one] Ett fält
   *[other] { $count } fält
}
omero-children = { $count ->
    [one] Ett objekt
   *[other] { $count } objekt
}
omero-load-more = Läs in fler
omero-loading = Läser in…
omero-empty = Inget här
omero-stopped = Stoppad
omero-found = { $count ->
    [one] Ett objekt
   *[other] { $count } objekt
}
# $reason why the image could not be opened.
omero-open-failed = Bilden kunde inte öppnas: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Minst en kanal förblir synlig
# $server a server's name.
omero-session-ended = Sessionen på { $server } har avslutats. Ange lösenordet för att fortsätta.

# Errors.
# $server a server's name.
omero-error-offline = { $server } kunde inte nås.
omero-error-sign-in = Den här servern kräver inloggning.
omero-error-expired = Sessionen har avslutats.
omero-error-login = Användarnamnet eller lösenordet nekades.
omero-error-forbidden = Ditt konto får inte se detta.
omero-error-not-found = Hittades inte på servern (det kan ha tagits bort).
# $code an HTTP status code.
omero-error-status = Servern svarade med fel { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Serverns svar kunde inte läsas: { $reason }
