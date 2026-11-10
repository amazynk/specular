# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Dansk. machine-drafted, needs native review

palette-omero-servers = OMERO-servere…

omero-title = OMERO-servere
omero-servers-intro = Projekter, screens og billeder på en server vises først, når du gennemser den. Intet sendes før.
omero-servers = Servere
omero-servers-empty = Ingen servere endnu. Tilføj en med dens OMERO.web-adresse.
omero-session-only = Adgangskoder gemmes kun, indtil Specular lukkes.
omero-add = Tilføj…
omero-edit = Rediger…
omero-remove = Fjern
omero-close = Luk
omero-browse = Gennemse
omero-back = Tilbage
omero-save = Gem
omero-stop = Stop
omero-open = Åbn
# $server a server's name.
omero-removed = { $server } fjernet
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Ikke krypteret
# A server browsed without signing in.
omero-public = Offentlig
# $user the user name a server is signed in with.
omero-signed-in-as = Som { $user }

# The page that adds or edits a server.
omero-add-title = Tilføj en server
omero-edit-title = Serverindstillinger
omero-field-name = Navn
omero-hint-name = Valgfrit
omero-field-url = Adresse
omero-hint-url = https://omero.example.org
omero-field-user = Brugernavn
omero-hint-user = Tom for offentlige data
omero-field-password = Adgangskode
omero-hint-password-kept = Lad stå tom for at beholde den angivne
omero-password-session = Adgangskoden gemmes kun, indtil Specular lukkes, aldrig på disken.
omero-password-needed = Indtast adgangskoden: den gemmes kun, indtil Specular lukkes.
omero-http-warning = Denne adresse er ikke krypteret: adgangskoden ville blive sendt i klartekst.
omero-url-invalid = Indtast OMERO.web-adressen, der begynder med https:// eller http://.
# $reason why the list could not be saved.
omero-save-failed = Serverlisten kunne ikke gemmes: { $reason }

# The browse page.
omero-browse-title = Gennemse
omero-top = Projekter og screens
omero-datasets = Datasæt
omero-plates = Plader
omero-wells = Brønde
omero-fields = Felter
omero-images = Billeder
omero-kind-project = Projekt
omero-kind-dataset = Datasæt
omero-kind-image = Billede
omero-kind-screen = Screen
omero-kind-plate = Plade
omero-kind-well = Brønd
# $well a well's row and column, as B7.
omero-well = Brønd { $well }
omero-well-fields = { $count ->
    [one] Ét felt
   *[other] { $count } felter
}
omero-children = { $count ->
    [one] Ét element
   *[other] { $count } elementer
}
omero-load-more = Indlæs flere
omero-loading = Indlæser…
omero-empty = Intet her
omero-stopped = Stoppet
omero-found = { $count ->
    [one] Ét element
   *[other] { $count } elementer
}
# $reason why the image could not be opened.
omero-open-failed = Billedet kunne ikke åbnes: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Mindst én kanal forbliver vist
# $server a server's name.
omero-session-ended = Sessionen på { $server } er afsluttet. Indtast adgangskoden for at fortsætte.

# Errors.
# $server a server's name.
omero-error-offline = { $server } kunne ikke nås.
omero-error-sign-in = Denne server kræver login.
omero-error-expired = Sessionen er afsluttet.
omero-error-login = Brugernavnet eller adgangskoden blev afvist.
omero-error-forbidden = Din konto må ikke se dette.
omero-error-not-found = Ikke fundet på serveren (det kan være fjernet).
# $code an HTTP status code.
omero-error-status = Serveren svarede med fejl { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Serverens svar kunne ikke læses: { $reason }
