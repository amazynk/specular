# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Suomi. machine-drafted, needs native review

palette-omero-servers = OMERO-palvelimet…

omero-title = OMERO-palvelimet
omero-servers-intro = Palvelimen projektit, seulonnat ja kuvat haetaan vasta, kun selaat sitä. Mitään ei lähetetä sitä ennen.
omero-servers = Palvelimet
omero-servers-empty = Ei vielä palvelimia. Lisää palvelin sen OMERO.web-osoitteella.
omero-session-only = Salasanat säilytetään vain, kunnes Specular suljetaan.
omero-add = Lisää…
omero-edit = Muokkaa…
omero-remove = Poista
omero-close = Sulje
omero-browse = Selaa
omero-back = Takaisin
omero-save = Tallenna
omero-stop = Pysäytä
omero-open = Avaa
# $server a server's name.
omero-removed = { $server } poistettu
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Salaamaton
# A server browsed without signing in.
omero-public = Julkinen
# $user the user name a server is signed in with.
omero-signed-in-as = Käyttäjänä { $user }

# The page that adds or edits a server.
omero-add-title = Lisää palvelin
omero-edit-title = Palvelimen asetukset
omero-field-name = Nimi
omero-hint-name = Valinnainen
omero-field-url = Osoite
omero-hint-url = https://omero.example.org
omero-field-user = Käyttäjätunnus
omero-hint-user = Tyhjä julkisille tiedoille
omero-field-password = Salasana
omero-hint-password-kept = Jätä tyhjäksi, niin annettu salasana säilyy
omero-password-session = Salasana säilytetään vain, kunnes Specular suljetaan, ei koskaan levyllä.
omero-password-needed = Anna salasana: se säilytetään vain, kunnes Specular suljetaan.
omero-http-warning = Tämä osoite ei ole salattu: salasana lähetettäisiin selväkielisenä.
omero-url-invalid = Anna OMERO.web-osoite, joka alkaa https:// tai http://.
# $reason why the list could not be saved.
omero-save-failed = Palvelinluetteloa ei voitu tallentaa: { $reason }

# The browse page.
omero-browse-title = Selaa
omero-top = Projektit ja seulonnat
omero-datasets = Aineistot
omero-plates = Levyt
omero-wells = Kuopat
omero-fields = Kentät
omero-images = Kuvat
omero-kind-project = Projekti
omero-kind-dataset = Aineisto
omero-kind-image = Kuva
omero-kind-screen = Seulonta
omero-kind-plate = Levy
omero-kind-well = Kuoppa
# $well a well's row and column, as B7.
omero-well = Kuoppa { $well }
omero-well-fields = { $count ->
    [one] Yksi kenttä
   *[other] { $count } kenttää
}
omero-children = { $count ->
    [one] Yksi kohde
   *[other] { $count } kohdetta
}
omero-load-more = Lataa lisää
omero-loading = Ladataan…
omero-empty = Täällä ei ole mitään
omero-stopped = Pysäytetty
omero-found = { $count ->
    [one] Yksi kohde
   *[other] { $count } kohdetta
}
# $reason why the image could not be opened.
omero-open-failed = Kuvaa ei voitu avata: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Vähintään yksi kanava jää näkyviin
# $server a server's name.
omero-session-ended = Istunto palvelimella { $server } päättyi. Jatka antamalla salasana.

# Errors.
# $server a server's name.
omero-error-offline = Palvelimeen { $server } ei saatu yhteyttä.
omero-error-sign-in = Tämä palvelin vaatii kirjautumisen.
omero-error-expired = Istunto päättyi.
omero-error-login = Käyttäjätunnus tai salasana hylättiin.
omero-error-forbidden = Tililläsi ei ole oikeutta nähdä tätä.
omero-error-not-found = Ei löytynyt palvelimelta (se on ehkä poistettu).
# $code an HTTP status code.
omero-error-status = Palvelin vastasi virheellä { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Palvelimen vastausta ei voitu lukea: { $reason }
