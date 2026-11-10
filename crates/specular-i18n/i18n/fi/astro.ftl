# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Lisää DS9-alueet kuvan päälle…
palette-science-load-regions-help = Piirrä DS9-aluetiedoston (.reg) muodot kuvaan.
palette-science-export-regions = Tallenna alueet DS9-muodossa…
palette-science-export-regions-help = Tallenna mittaukset ja kuvan päällä olevat alueet DS9-aluetiedostoksi.
palette-science-edit-regions = Muokkaa alueita mittauksina
palette-science-edit-regions-help = Muuta kuvan päällä olevat alueet mittauksiksi, joita voit siirtää ja muuttaa.
palette-science-clear-regions = Poista kuvan päälle lisätyt alueet
palette-science-clear-regions-help = Poista DS9-alueet kuvasta.
palette-science-plate-solve = Astrometrinen ratkaisu
palette-science-plate-solve-help = Etsi kuvan taivaankoordinaatit ASTAPilla tai astrometry.netillä. Suorita uudelleen pysäyttääksesi.
palette-science-save-plate-solution = Tallenna astrometrinen ratkaisu
palette-science-save-plate-solution-help = Tallenna löydetyt taivaankoordinaatit kuvan viereen .wcs-tiedostona.
palette-science-sky-survey = Taivaskartoitus taustaksi
palette-science-sky-survey-help = Näytä seuraava taivaskartoitus kuvan ympärillä, tai ei mitään.
palette-science-samp-send-image = Lähetä kuva SAMP-sovelluksiin
palette-science-samp-send-image-help = Avaa tämä kuva DS9:ssä, Aladinissa ja muissa SAMP-keskittimen sovelluksissa.
palette-science-samp-send-position = Lähetä sijainti SAMP-sovelluksiin
palette-science-samp-send-position-help = Suuntaa muut sovellukset osoittimen alla olevaan taivaan kohtaan.

disabled-samp-off = Yhdistä ensin SAMP-keskittimeen (Asetukset › Integraatio).

settings-integration-samp = Yhdistä SAMP-keskittimeen
settings-integration-samp-help = Vaihda kuvia, taulukoita ja taivaan sijainteja DS9:n, TOPCATin, Aladinin ja muiden tähtitieteen sovellusten kanssa. Vain tämän tietokoneen keskitintä käytetään.
settings-integration-astap-path = ASTAP-ohjelma
settings-integration-astap-path-help = ASTAP-ratkaisija, jos sitä ei löydy automaattisesti.
settings-integration-solve-field-path = astrometry.net-ohjelma
settings-integration-solve-field-path-help = astrometry.netin solve-field-ohjelma, jos sitä ei löydy automaattisesti.

astro-regions-filter = DS9-alueet
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } alue.
       *[other] { $count } aluetta.
    }
astro-regions-refused = { $count ->
        [one] { $count } rivi lukematta.
       *[other] { $count } riviä lukematta.
    } Ensimmäinen: rivi { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda-muoto piirretty sisä- ja ulkoreunoinaan.
       *[other] { $count } panda-muotoa piirretty sisä- ja ulkoreunoinaan.
    }
astro-regions-none-here = { $name }: mikään alue ei osu tähän kuvaan.
astro-regions-failed = Tiedostoa { $name } ei voitu lukea: { $reason }
astro-regions-cleared = Alueet poistettu.
astro-regions-nothing = Ei tallennettavia alueita tai mittauksia.
astro-regions-saved = { $name } tallennettu.
astro-regions-skipped = { $count ->
        [one] { $count } taivaankoordinaattien ulkopuolinen muoto jätettiin pois.
       *[other] { $count } taivaankoordinaattien ulkopuolista muotoa jätettiin pois.
    }
astro-regions-save-failed = Alueita ei voitu tallentaa: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } alue on nyt mittaus.
       *[other] { $count } aluetta on nyt mittauksia.
    }
astro-regions-no-wcs = Taivaan alueet vaativat kuvan, jossa on taivaankoordinaatit.

astro-solve-no-solver = Astrometristä ratkaisijaa ei löytynyt. Asenna ASTAP tai astrometry.net, tai valitse se kohdassa Asetukset › Integraatio.
astro-solve-running = Ratkaistaan: { $solver }… { $seconds } s
astro-solve-stop = Pysäytä
astro-solve-stopped = Astrometrinen ratkaisu pysäytetty.
astro-solve-done = Ratkaistu: { $ra }, { $dec }, { $scale } pikseliä kohden.
astro-solve-failed = Astrometrinen ratkaisu epäonnistui: { $reason }
astro-solve-not-local = Vain tämän tietokoneen kansiossa olevat kuvat voidaan ratkaista.
astro-solve-none = Tälle kuvalle ei ole vielä astrometristä ratkaisua.
astro-solve-saved = { $name } tallennettu. Itse kuva on ennallaan.
astro-solve-sidecar = Taivaankoordinaatit tiedostosta { $name }.

astro-samp-connected = Yhdistetty SAMP-keskittimeen.
astro-samp-no-hub = SAMP: { $reason }. Yritetään uudelleen, kun keskitin käynnistyy.
astro-samp-lost = SAMP-keskitin pysähtyi. Yritetään uudelleen, kun jokin käynnistyy.
astro-samp-sent = Lähetetty sovelluksiin: { $count }.
astro-samp-nobody = Mikään keskittimen sovellus ei kuuntele tätä.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } ei ole tällä tietokoneella, joten sitä ei avata.
astro-samp-pointed = Suunnattu kohtaan { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } ei ole tässä kuvassa.
astro-samp-no-table = SAMP: sitä taulukkoa ei ole lisätty tämän kuvan päälle.
astro-samp-rows = Luettelon lähteitä korostettu: { $count }.

astro-survey-off = Taivaskartoitus pois.
astro-survey-on = Taivaskartoitus: { $name }. { $credit }
astro-survey-failed = Kartoitusta { $name } ei voi näyttää: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Taivaskartoituksen ruutuja palvelimelta { $host }
