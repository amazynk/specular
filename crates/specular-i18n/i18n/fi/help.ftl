# machine-drafted, needs native review
## Käyttöoppaan paneeli, kaatumisraportit, diagnostiikkapaketti, päivitysten tarkistus ja Tietoja. Suomi.
## Tyyli: lyhyt, selkeä, ei täytettä; yksi ajatus virkettä kohden.

## Paletti

palette-help-manual = Ohje
palette-help-manual-help = Avaa käyttöopas näytöllä olevaa asiaa käsittelevältä sivulta.
palette-save-diagnostics = Tallenna diagnostiikkapaketti…
palette-save-diagnostics-help = Tallenna kaatumisraportit, loki ja järjestelmätiedot zip-tiedostoon. Mitään ei lähetetä.
# Paletin kohta, joka avaa käyttöoppaan osion; { $title } on sivu tai ”Sivu › Osio”.
palette-help-topic = Ohje: { $title }
palette-help-topic-help = Avaa tämä käyttöoppaan osio.
# Paletissa ei-käytettävissä olevan komennon alla.
palette-disabled-help = Ei käytettävissä nyt. Paina Enter nähdäksesi syyn.

## Ohjepaneeli

help-title = Ohje
help-search-placeholder = Hae käyttöoppaasta
help-contents = Sisältö
help-back = Takaisin
help-forward = Eteenpäin
help-no-results = Haulla ”{ $query }” ei löytynyt mitään.
help-results = { $count ->
    [one] 1 osio löytyi
   *[other] { $count } osiota löytyi
}
help-section-start = Aloitus
help-section-modes = Katselu ja tilat
help-section-tasks = Miten…
help-section-reference = Viite
help-keys-unbound = Ei asetettu
# Käyttöoppaassa mainittu komento ilman näppäintä; se ajetaan paletista.
help-key-unbound = { $name } (paletti)
help-keys-command = Komento
help-keys-keys = Näppäimet
help-commands = Kaikki komennot ovat komentopaletissa ({ $key }). Kirjoita osa nimestä.
help-commands-no-key = Kaikki komennot ovat komentopaletissa. Kirjoita osa nimestä.
help-link = Siirry: { $title }
help-open = Ohje
# Paneelin nimi ruudunlukijalle; { $page } on näytetty sivu.
help-label = Ohje: { $page }
help-disabled-why = Miksi?

## Seuraavalla käynnistyskerralla tarjottava kaatumisraportti

crash-title = { -app-name } sulkeutui odottamatta
crash-body = Raportti tallennettiin tälle tietokoneelle. Mitään ei lähetetty.
crash-view = Näytä raportti
crash-hide = Piilota raportti
crash-copy = Kopioi
crash-copied = Raportti kopioitu
crash-save-bundle = Tallenna diagnostiikkapaketti…
crash-dismiss = Ohita

## Diagnostiikkapaketti

diag-title = Tallenna diagnostiikkapaketti
diag-intro = Zip-tiedosto liitettäväksi virheraporttiin. Se tallennetaan tälle tietokoneelle. Mitään ei lähetetä.
diag-contents = Sisältö
diag-include-names = Sisällytä tiedostonimet
diag-include-names-help = Pois: nimet korvataan muodoilla file-1.jpg, dir-1 ja niin edelleen.
diag-privacy = Kotikansiosi näkyy muodossa ~ ja käyttäjänimesi muodossa <user>. Salasanoja, palvelinosoitteita ja kuvia ei koskaan sisällytetä.
diag-where = Ilmoita ongelmasta: { $issues } · { $email }
diag-save = Tallenna…
diag-cancel = Peruuta
diag-saved = Diagnostiikkapaketti tallennettu: { $path }
diag-failed = Diagnostiikkapakettia ei voitu tallentaa: { $reason }
diag-entry-summary = Versio, järjestelmä, GPU ja ajuri
diag-entry-crash = Viimeisin kaatumisraportti
diag-entry-crashes = Aiemmat kaatumisraportit ({ $count })
diag-entry-log = Lokin viimeiset { $count } riviä
diag-entry-settings = Asetukset ilman salasanoja ja osoitteita
diag-entry-gpu = GPU- ja näyttötiedot
diag-entry-readme = Mikä tämä paketti on
diag-size = { $size }
# Paketin sisältämä README.txt.
diag-readme =
    { -app-name } – diagnostiikkapaketti.

    Se sisältää sen, mikä auttaa löytämään ongelman: version ja järjestelmätiedot, kaatumisraportit, lokin lopun ja asetukset.

    Kotikansiosi näkyy muodossa ~ ja käyttäjänimesi muodossa <user>. Ellet valinnut niiden sisällyttämistä, tiedosto- ja kansionimet korvataan muodoilla file-1, dir-1 ja niin edelleen. Salasanat ja palvelinosoitteet poistetaan.

    Mitään ei lähetetty. Sinä päätät, jaatko tämän tiedoston.

## Päivitykset (Asetukset)

settings-section-updates = Päivitykset
settings-updates-check = Tarkista päivitykset
settings-updates-check-help = Kysy kerran päivässä, onko uusi versio saatavilla. Pyyntö sisältää vain versionumeron: ei tunnistetta, ei evästettä. Pois: { -app-name } ei koskaan muodosta yhteyttä päivityksiä varten.
settings-updates-channel = Kanava
settings-updates-channel-help = Vakaa: vain julkaisut. Beta: myös testiversiot.
settings-updates-channel-stable = Vakaa
settings-updates-channel-beta = Beta
settings-updates-check-now = Tarkista nyt
settings-updates-check-now-help = Tarkista kerran nyt.
update-available = Versio { $version } on saatavilla
update-notes = Julkaisutiedot
update-download = Lataa
update-none = { -app-name } on ajan tasalla.
update-checking = Tarkistetaan päivityksiä…
update-failed = Päivityksiä ei voitu tarkistaa: { $reason }
update-last-checked = Tarkistettu viimeksi { $when }

## Tietoja

about-build = Koontiversio { $commit } · { $profile } · { $target }
about-build-no-commit = Koontiversio { $profile } · { $target }
about-help = Ohje
about-diagnostics = Tallenna diagnostiikkapaketti…
# machine-drafted, needs native review
about-renderer = Renderöijän tiedot…
about-licences-count = { $count ->
    [one] 1 kolmannen osapuolen komponentti
   *[other] { $count } kolmannen osapuolen komponenttia
}
about-licence-group = { $licence } ({ $count })
about-contact = Ongelmat: { $issues } · Tuki: { $email }
