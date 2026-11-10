# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Nederlands. machine-drafted, needs native review

palette-omero-servers = OMERO-servers…

omero-title = OMERO-servers
omero-servers-intro = Projecten, screens en afbeeldingen op een server worden pas opgehaald wanneer u erdoor bladert. Daarvoor wordt niets verzonden.
omero-servers = Servers
omero-servers-empty = Nog geen servers. Voeg er een toe met het OMERO.web-adres.
omero-session-only = Wachtwoorden worden alleen bewaard tot Specular wordt gesloten.
omero-add = Toevoegen…
omero-edit = Bewerken…
omero-remove = Verwijderen
omero-close = Sluiten
omero-browse = Bladeren
omero-back = Terug
omero-save = Opslaan
omero-stop = Stoppen
omero-open = Openen
# $server a server's name.
omero-removed = { $server } verwijderd
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Niet versleuteld
# A server browsed without signing in.
omero-public = Openbaar
# $user the user name a server is signed in with.
omero-signed-in-as = Als { $user }

# The page that adds or edits a server.
omero-add-title = Server toevoegen
omero-edit-title = Serverinstellingen
omero-field-name = Naam
omero-hint-name = Optioneel
omero-field-url = Adres
omero-hint-url = https://omero.example.org
omero-field-user = Gebruikersnaam
omero-hint-user = Leeg voor openbare gegevens
omero-field-password = Wachtwoord
omero-hint-password-kept = Leeg laten om het bestaande te behouden
omero-password-session = Het wachtwoord wordt alleen bewaard tot Specular wordt gesloten, nooit op schijf.
omero-password-needed = Voer het wachtwoord in: het wordt alleen bewaard tot Specular wordt gesloten.
omero-http-warning = Dit adres is niet versleuteld: het wachtwoord zou onversleuteld worden verzonden.
omero-url-invalid = Voer het OMERO.web-adres in, beginnend met https:// of http://.
# $reason why the list could not be saved.
omero-save-failed = De serverlijst kon niet worden opgeslagen: { $reason }

# The browse page.
omero-browse-title = Bladeren
omero-top = Projecten en screens
omero-datasets = Datasets
omero-plates = Platen
omero-wells = Wells
omero-fields = Velden
omero-images = Afbeeldingen
omero-kind-project = Project
omero-kind-dataset = Dataset
omero-kind-image = Afbeelding
omero-kind-screen = Screen
omero-kind-plate = Plaat
omero-kind-well = Well
# $well a well's row and column, as B7.
omero-well = Well { $well }
omero-well-fields = { $count ->
    [one] Eén veld
   *[other] { $count } velden
}
omero-children = { $count ->
    [one] Eén item
   *[other] { $count } items
}
omero-load-more = Meer laden
omero-loading = Laden…
omero-empty = Hier staat niets
omero-stopped = Gestopt
omero-found = { $count ->
    [one] Eén item
   *[other] { $count } items
}
# $reason why the image could not be opened.
omero-open-failed = De afbeelding kon niet worden geopend: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Er blijft minstens één kanaal zichtbaar
# $server a server's name.
omero-session-ended = De sessie op { $server } is beëindigd. Voer het wachtwoord in om verder te gaan.

# Errors.
# $server a server's name.
omero-error-offline = { $server } is niet bereikbaar.
omero-error-sign-in = Voor deze server moet u zich aanmelden.
omero-error-expired = De sessie is beëindigd.
omero-error-login = De gebruikersnaam of het wachtwoord is geweigerd.
omero-error-forbidden = Uw account mag dit niet zien.
omero-error-not-found = Niet gevonden op de server (mogelijk verwijderd).
# $code an HTTP status code.
omero-error-status = De server antwoordde met fout { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Het antwoord van de server kon niet worden gelezen: { $reason }
