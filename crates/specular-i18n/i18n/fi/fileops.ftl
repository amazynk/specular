# machine-drafted, needs native review
# Tiedostotoiminnot: nimeäminen uudelleen, joukkonimeäminen, kopiointi ja siirto kansioon,
# tiedostoon tallennettu kierto, kumoa ja tee uudelleen, taustakuva, jakaminen, lähetys
# sähköpostilla, asetusten vienti ja tuonti.

## Paletti, pikaohje ja valikot

palette-undo = Kumoa
palette-undo-help = Peru viimeisin tiedostomuutos: siirto roskakoriin, uudelleennimeäminen, kopiointi tai siirto, kierto.
palette-redo = Tee uudelleen
palette-redo-help = Tee uudelleen se, minkä viimeisin kumoaminen perui.
palette-rename = Nimeä uudelleen…
palette-rename-help = Anna tiedostolle uusi nimi; sivutiedostot ja RAW+JPEG-pari seuraavat.
palette-batch-rename = Nimeä tiedostot uudelleen…
palette-batch-rename-help = Nimeä valitut tiedostot tai koko kansio uudelleen mallin mukaan, esikatselun kanssa.
palette-copy-to-folder = Kopioi kansioon…
palette-copy-to-folder-help = Kopioi tiedosto tai valinta toiseen kansioon.
palette-move-to-folder = Siirrä kansioon…
palette-move-to-folder-help = Siirrä tiedosto tai valinta toiseen kansioon.
palette-rotate-file-left = Kierrä tiedostoa vasemmalle
palette-rotate-file-left-help = Kierrä itse tiedostoa neljänneskierros vasemmalle, häviöttömästi jos muoto sallii.
palette-rotate-file-right = Kierrä tiedostoa oikealle
palette-rotate-file-right-help = Kierrä itse tiedostoa neljänneskierros oikealle, häviöttömästi jos muoto sallii.
palette-set-wallpaper-help = Käytä kuvaa työpöydän taustakuvana.
palette-share = Jaa…
palette-share-help = Lähetä tiedosto toiseen sovellukseen järjestelmän jakovalikolla.
palette-send-by-email = Lähetä sähköpostilla…
palette-send-by-email-help = Aloita sähköpostisovelluksessa uusi viesti, jonka liitteenä on tiedosto.
# machine-drafted, needs native review
palette-share-with-zynk = Jaa Zynkin kautta…
palette-share-with-zynk-help = Luo tiedostoon verkkolinkki Zynk-sovelluksessa.
palette-send-with-zynk = Lähetä Zynkillä…
palette-send-with-zynk-help = Lähetä tiedosto Zynkillä yhteystiedolle tai jollekin omista laitteistasi.
palette-export-settings = Vie asetukset…
palette-export-settings-help = Tallenna kaikki asetukset ja pikanäppäimet tiedostoon.
palette-import-settings = Tuo asetukset…
palette-import-settings-help = Lue asetukset tiedostosta, kun muutokset on ensin näytetty.

disabled-in-archive-file = Tämä sivu on arkiston sisällä eikä ole oma tiedostonsa
disabled-not-rotatable = Tämä tiedostotyyppi ei voi tallentaa kiertoa
disabled-not-on-this-desktop = Työpöytäsi ei tarjoa tätä

## Asetukset

settings-browsing-rotate-trim-edges = Rajaa JPEG-reunat häviöttömän kierron vuoksi
settings-browsing-rotate-trim-edges-help = JPEG, jonka koko ei ole 8:n tai 16 pikselin monikerta, voidaan kiertää häviöttä vain leikkaamalla sen viimeiset pikselit. Muuten sen suuntatunniste muutetaan, ja rajausta tarjotaan joka kerta.
settings-browsing-rotate-keep-modified = Säilytä muokkauspäivä kierrettäessä
settings-browsing-rotate-keep-modified-help = Kierretty tiedosto säilyttää viimeisimmän muokkauspäivänsä, joten se pysyy paikallaan päivän mukaan lajiteltaessa.
settings-browsing-recent-folders = Viimeaikaiset kansiot
settings-browsing-recent-folders-help = Viimeksi kopiointiin ja siirtoon käytetyt kansiot.

## Ikkunat

fileop-cancel = Peruuta
rename-title = Nimeä uudelleen
rename-field-label = Nimi
rename-button = Nimeä uudelleen
rename-sidecars-follow = Myös nimetty uudelleen: { $files }
rename-extension-changed = Tiedostopäätteen vaihto ei muunna tiedostoa; muut sovellukset eivät ehkä avaa sitä.
rename-problem-empty = Kirjoita nimi.
rename-problem-invalid-char = Nimissä ei voi olla merkkiä ”{ $char }”.
rename-problem-reserved = ”{ $name }” on nimi, jonka Windows varaa laitteille.
rename-problem-too-long = Nimi on liian pitkä; enintään 255 tavua.
rename-problem-dot = ”.” ja ”..” eivät käy nimiksi.
rename-problem-trailing = Nimi ei voi päättyä välilyöntiin tai pisteeseen.
rename-problem-exists = Toisella tiedostolla on jo tämä nimi.
rename-problem-read-only = Tätä kansiota ei voi muuttaa.
rename-problem-duplicate = Kaksi tiedostoa saisi tämän nimen.

batch-rename-title =
    { $count ->
        [one] Nimeä { $count } tiedosto uudelleen
       *[other] Nimeä { $count } tiedostoa uudelleen
    }
batch-rename-field-label = Malli
batch-rename-tokens = Käytä { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } ja { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } tiedosto nimetään uudelleen
       *[other] { $count } tiedostoa nimetään uudelleen
    }{ $problems ->
        [0] .
        [one] ; { $problems } ei onnistu ja pysyy ennallaan.
       *[other] ; { $problems } ei onnistu ja pysyvät ennallaan.
    }
batch-rename-reading = Luetaan päiväyksiä ja kameroita…
batch-rename-preview = Esikatselu
batch-rename-unchanged = säilyttää nimensä
batch-rename-more =
    { $count ->
        [one] { $count } lisää alempana
       *[other] { $count } lisää alempana
    }

transfer-copy-title-one = Kopioi ”{ $file }” kansioon
transfer-copy-title =
    { $count ->
        [one] Kopioi { $count } tiedosto kansioon
       *[other] Kopioi { $count } tiedostoa kansioon
    }
transfer-move-title-one = Siirrä ”{ $file }” kansioon
transfer-move-title =
    { $count ->
        [one] Siirrä { $count } tiedosto kansioon
       *[other] Siirrä { $count } tiedostoa kansioon
    }
transfer-recent = Viimeaikaiset kansiot
transfer-folder = Kansio
transfer-choose-folder = Valitse kansio…
transfer-copy-dialog = Kopioi kansioon
transfer-move-dialog = Siirrä kansioon
transfer-sidecars =
    { $count ->
        [one] { $count } sivutiedosto tulee mukana.
       *[other] { $count } sivutiedostoa tulee mukana.
    }
transfer-taken =
    { $count ->
        [one] { $count } nimi on jo käytössä kansiossa { $folder }.
       *[other] { $count } nimeä on jo käytössä kansiossa { $folder }.
    }
transfer-when-taken = Kun nimi on jo käytössä
transfer-skip = Ohita
transfer-keep-both = Säilytä molemmat
transfer-replace-option = Korvaa
transfer-copy = Kopioi
transfer-move = Siirrä
transfer-replace-title =
    { $count ->
        [one] Korvataanko { $count } tiedosto kansiossa { $folder }?
       *[other] Korvataanko { $count } tiedostoa kansiossa { $folder }?
    }
transfer-replace-note = Siellä nyt olevat tiedostot siirtyvät roskakoriin. Kumoa palauttaa ne.
transfer-replace = Korvaa
transfer-back = Takaisin
transfer-stop = Pysäytä

settings-file-import-title = Tuo asetukset
settings-file-import-dialog = Tuo asetukset
settings-file-export-dialog = Vie asetukset
settings-file-import-changes = Muutokset
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = ei näppäintä
settings-file-import-unknown = Ei tämän version asetus; jätetty pois.
settings-file-import-keymap = Pikanäppäimet
settings-file-import-nothing = Mikään ei muuttuisi: nämä asetukset ovat jo käytössä.
settings-file-import-summary =
    { $count ->
        [one] { $count } muutos. Tuonnin voi kumota Asetuksissa.
       *[other] { $count } muutosta. Tuonnin voi kumota Asetuksissa.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } kohta näyttää salasanalta tai avaimelta, eikä sitä koskaan tuoda.
       *[other] { $count } kohtaa näyttää salasanoilta tai avaimilta, eikä niitä koskaan tuoda.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } käyttäjän komentoa ei tuoda: komennot käynnistävät ohjelmia. Kopioi se itse asetustiedostoosi, jos luotat siihen.
       *[other] { $count } käyttäjän komentoa ei tuoda: komennot käynnistävät ohjelmia. Kopioi ne itse asetustiedostoosi, jos luotat niihin.
    }
settings-file-import-button = Tuo

## Tulokset

fileop-renamed = Nimetty uudelleen: { $file }
fileop-renamed-count =
    { $count ->
        [one] { $count } tiedosto nimetty uudelleen
       *[other] { $count } tiedostoa nimetty uudelleen
    }
fileop-copied =
    { $count ->
        [one] { $count } tiedosto kopioitu kansioon { $folder }
       *[other] { $count } tiedostoa kopioitu kansioon { $folder }
    }
fileop-moved =
    { $count ->
        [one] { $count } tiedosto siirretty kansioon { $folder }
       *[other] { $count } tiedostoa siirretty kansioon { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } ohitettiin: sen nimi on siellä jo käytössä.
       *[other] { $count } ohitettiin: niiden nimet ovat siellä jo käytössä.
    }
fileop-stopped = Pysäytetty; tehty jää voimaan.
fileop-failed-file =
    { $count ->
        [one] Tiedostoa { $file } ei voitu kopioida tai siirtää: { $reason }
       *[other] { $count } tiedostoa ei voitu kopioida tai siirtää; ensimmäinen, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Kopioidaan { $count } tiedosto… { PERCENT($percent) }
       *[other] Kopioidaan { $count } tiedostoa… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Siirretään { $count } tiedosto… { PERCENT($percent) }
       *[other] Siirretään { $count } tiedostoa… { PERCENT($percent) }
    }
fileop-done-status = Valmis
fileop-busy = Edellinen tiedostomuutos on vielä kesken; yritä hetken päästä uudelleen.
fileop-nothing-to-redo = Ei mitään tehtävää uudelleen
fileop-undone = Kumottu: { $what }
fileop-redone = Tehty uudelleen: { $what }
fileop-kind-rename = uudelleennimeäminen
fileop-kind-batch-rename = usean tiedoston uudelleennimeäminen
fileop-kind-move = siirto kansioon
fileop-kind-copy = kopiointi kansioon
fileop-kind-rotate = kierto
rename-failed = Uudelleennimeäminen epäonnistui: { $reason }

undo-problem-changed = Ei voi kumota: { $file } on muuttunut sen jälkeen, ja kumoaminen hävittäisi muutoksen.
undo-problem-missing = Ei voi kumota: { $file } ei ole enää olemassa.
undo-problem-taken = Ei voi kumota: toisella tiedostolla on nyt nimi { $file }.
undo-problem-io = Tiedoston { $file } muutosta ei voi kumota: { $reason }

rotate-done-lossless = { $file } kierretty täysin häviöttä.
rotate-done-lossless-trimmed = { $file } kierretty täysin häviöttä; reuna rajattiin kokoon { $width } × { $height }.
rotate-done-tag = { $file } kierretty muuttamalla suuntatunnistetta.
rotate-done-sidecar = Tiedoston { $file } kierto tallennettu tiedostoon { $sidecar }; itse RAW-tiedostoa ei koskaan muuteta.
rotate-tag-unreadable = Kuvadataa ei voitu lukea tarpeeksi pitkälle sen kiertämiseksi.
rotate-tag-multi-picture = Siinä on useampi kuva, ja ensimmäisen kiertäminen rikkoisi muut.
rotate-tag-trailer = Kuvan perässä on dataa, jonka kiertäminen siirtäisi.
rotate-tag-not-aligned = Pikselien häviötön kierto rajaisi sen kokoon { $width } × { $height }.
rotate-trim-offer = Rajaa ja kierrä
rotate-problem-unsupported = { $file } on tiedostotyyppiä, joka ei voi tallentaa kiertoa.
rotate-problem-too-large = { $file } on liian suuri kierrettäväksi.
rotate-problem-damaged = { $file } on vioittunut kohdasta, johon suunta on tallennettu: { $reason }
rotate-problem-io = Tiedostoa { $file } ei voitu kiertää: { $reason }

share-failed = Jakaminen epäonnistui: { $reason }
email-failed = Sähköpostisovellusta ei voitu käynnistää: { $reason }
# machine-drafted, needs native review
zynk-opening = Avataan Zynk-sovellusta…
zynk-failed = Zynkiin ei saatu yhteyttä: { $reason }
zynk-get-title-send = ”Lähetä Zynkillä” vaatii Zynk-sovelluksen
zynk-get-title-share = ”Jaa Zynkin kautta” vaatii Zynk-sovelluksen
zynk-get-about = Zynk on maksuton sovellus Macille, Windowsille, Linuxille, iPhonelle ja Androidille. Se lähettää kuvia, videoita ja kokonaisia kansioita suoraan muille laitteillesi ja muille ihmisille, minkä kokoisina tahansa. Siirrot ovat päästä päähän salattuja ja jatkuvat siitä, mihin jäivät, jos yhteys katkeaa. Jos vastaanottajalla ei ole Zynkiä, se tekee verkkolinkin, josta tiedostot voi ladata selaimessa.
zynk-get-macos = Hanki se Mac App Storesta. Kun se on asennettu, { -app-name } antaa tiedostosi suoraan sille.
zynk-get-windows = Hanki se Microsoft Storesta. Kun se on asennettu, { -app-name } antaa tiedostosi suoraan sille.
zynk-get-linux = Linuxin Zynk on AppImage, jota ei tarvitse asentaa: lataa se, siirrä se kansioon ~/Applications ja tee siitä suoritettava (sen ominaisuuksissa tai komennolla chmod +x). { -app-name } löytää sen sieltä, ja mistä tahansa muualta, kun Zynk on käynnissä.
zynk-get = Hanki Zynk
zynk-send-old-windows = ”Lähetä Zynkillä” vaatii Windows 10:n version 2004 tai uudemman. ”Jaa Zynkin kautta” toimii tässä Windowsissa.
settings-file-export-done = Asetukset tallennettu tiedostoon { $file }
settings-file-export-failed = Asetuksia ei voitu tallentaa: { $reason }
settings-file-import-unreadable = Tiedostoa { $file } ei voitu lukea: { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } muutos tuotu
       *[other] { $count } muutosta tuotu
    }
