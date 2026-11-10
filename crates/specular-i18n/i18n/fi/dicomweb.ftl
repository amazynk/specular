# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Suomi.

palette-dicom-servers = DICOMweb-palvelimet…

dicomweb-title = DICOMweb-palvelimet
dicomweb-servers-intro = Palvelimen tutkimuksia haetaan ja avataan vain pyynnöstäsi. Mitään ei lähetetä etukäteen.
dicomweb-servers = Palvelimet
dicomweb-servers-empty = Ei vielä palvelimia. Lisää palvelin sen DICOMweb-osoitteella.
dicomweb-session-only = Salasanat ja tunnukset säilytetään vain, kunnes Specular suljetaan.
dicomweb-add = Lisää…
dicomweb-edit = Muokkaa…
dicomweb-remove = Poista
dicomweb-close = Sulje
dicomweb-browse = Selaa
dicomweb-back = Takaisin
dicomweb-save = Tallenna
dicomweb-search = Hae
dicomweb-stop = Pysäytä
dicomweb-removed = Poistettu: { $server }
# machine-drafted, needs native review
dicomweb-remove-ask = Poistetaanko ”{ $server }” tallennetuista palvelimista?
dicomweb-remove-detail = Levyltä ei katoa mitään. Palvelimen salasana tai käyttöoikeustunnus unohdetaan tämän istunnon osalta.
# Palvelimet, joihin yhteys on salaamaton (http://).
dicomweb-not-encrypted = Salaamaton

# Palvelimen lisäys- tai muokkaussivu.
dicomweb-add-title = Lisää palvelin
dicomweb-edit-title = Palvelimen asetukset
dicomweb-field-name = Nimi
dicomweb-hint-name = Valinnainen
dicomweb-field-url = Osoite
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Kirjautuminen
dicomweb-auth-none = Ei kirjautumista
dicomweb-auth-basic = Käyttäjätunnus ja salasana
dicomweb-auth-bearer = Käyttöoikeustunnus
dicomweb-field-user = Käyttäjätunnus
dicomweb-field-password = Salasana
dicomweb-field-token = Käyttöoikeustunnus
dicomweb-hint-secret-kept = Jätä tyhjäksi, niin annettu arvo säilyy
dicomweb-secret-session = Salasana tai käyttöoikeustunnus säilytetään vain, kunnes Specular suljetaan, ei koskaan levyllä.
dicomweb-secret-needed = Anna salasana tai käyttöoikeustunnus uudelleen: se säilytetään vain, kunnes Specular suljetaan.
dicomweb-http-warning = Tämä osoite ei ole salattu: salasana tai käyttöoikeustunnus lähetettäisiin selväkielisenä.
dicomweb-url-empty = Anna palvelimen DICOMweb-osoite.
dicomweb-url-scheme = Osoitteen on alettava https:// tai http://.
dicomweb-url-host = Osoitteessa ei ole palvelimen nimeä.
dicomweb-url-userinfo = Kirjoita käyttäjätunnus ja salasana kirjautumiskenttiin, ei osoitteeseen.
dicomweb-save-failed = Palvelinluetteloa ei voitu tallentaa: { $reason }

# Hakusivu.
dicomweb-search-title = Hae tutkimuksia
dicomweb-field-patient-name = Potilaan nimi
dicomweb-field-patient-id = Potilastunnus
dicomweb-field-date-from = Alkaen
dicomweb-field-date-to = Asti
dicomweb-hint-date = VVVVKKPP
dicomweb-field-modality = Modaliteetti
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Tutkimuspyynnön numero
dicomweb-show-patient = Näytä potilastiedot
dicomweb-switch-on = Päällä
dicomweb-switch-off = Pois
dicomweb-patient-shown = Potilastiedot näkyvissä
dicomweb-patient-hidden = Potilastiedot piilotettu
dicomweb-studies = Tutkimukset
dicomweb-search-hint = Täytä mitä tahansa kenttiä ja hae.
dicomweb-no-studies = Ei vastaavia tutkimuksia.
dicomweb-searching = Haetaan…
dicomweb-stopped = Pysäytetty
dicomweb-load-more = Lataa lisää
dicomweb-study-untitled = Tutkimus ilman kuvausta
dicomweb-series-untitled = Sarja ilman kuvausta
dicomweb-study-counts = Sarjoja: { $series }, kuvia: { $images }
dicomweb-series-images = Kuvia: { $count }
dicomweb-patient-id = ID { $id }
dicomweb-accession = Pyyntö { $number }
dicomweb-found = Tutkimuksia löytyi: { $count }
dicomweb-series-found = Sarjoja: { $count }

# Virheet palvelinyhteydessä.
dicomweb-error-offline = Palvelimeen { $server } ei saada yhteyttä. Tarkista verkko ja osoite.
dicomweb-error-auth = Palvelin hylkäsi käyttäjätunnuksen, salasanan tai tunnuksen.
dicomweb-error-not-found = Tässä osoitteessa ei ole DICOMweb-palvelua.
dicomweb-error-other = Palvelin ei voinut vastata: { $reason }

# Katseluohjelmaan suoratoistettava sarja.
dicomweb-loading = Ladataan: { $series }…
dicomweb-loading-progress = Ladataan: { $series } – { $done }/{ $total }
dicomweb-loaded = { $series }: kuvia ladattu: { $total }
dicomweb-loaded-failed = Kuvia ei voitu ladata: { $failed }/{ $total }
dicomweb-offline-cached = Palvelimeen { $server } ei saada yhteyttä: näytetään aiemmin tallennetut kuvat.
dicomweb-cache-failed = Latauskansiota ei voitu avata: { $reason }
