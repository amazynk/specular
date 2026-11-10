# Metatietojen muokkain: valittujen kuvien IPTC-kentät (galleria, Kirjasto, karsintatila ja
# katseluohjelma).

palette-edit-metadata = Muokkaa metatietoja
palette-edit-metadata-help = Muokkaa valittujen kuvien otsikkoa, kuvatekstiä, tekijää, oikeuksia ja sijainteja.
metaedit-menu = Muokkaa metatietoja…
metaedit-title = Metatiedot
metaedit-close = Sulje metatiedot
metaedit-photos = { $count ->
    [one] 1 kuva
   *[other] { $count } kuvaa
}
metaedit-none-selected = Valitse kuvat, joiden metatietoja muokkaat.
metaedit-reading = Luetaan metatietoja…
metaedit-mixed = Vaihtelee: kirjoita asettaaksesi kaikkiin
metaedit-clearing = Tyhjennetään kaikista kuvista. Esc säilyttää ne.
metaedit-keywords-others = joissakin on enemmän
metaedit-saved = { $field } tallennettu.
metaedit-cleared = { $field } tyhjennetty.
metaedit-write-failed = { $count ->
    [one] Metatietoja ei tallennettu 1 kuvaan: { $error }
   *[other] Metatietoja ei tallennettu { $count } kuvaan: { $error }
}
metaedit-writes-sidecars = Tallennetaan XMP-sivutiedostoihin.
metaedit-writes-files = Tallennetaan JPEG-, TIFF- ja DNG-tiedostoihin; muut tiedostot saavat XMP-sivutiedostot.

metaedit-group-templates = Mallit
metaedit-group-description = Kuvaus
metaedit-group-creator = Tekijä
metaedit-group-rights = Oikeudet
metaedit-group-shown = Kuvassa näkyvä sijainti
metaedit-group-created = Kuvauspaikka
metaedit-group-workflow = Työnkulku
metaedit-templates-none = Ei vielä malleja. Malli täyttää usein käyttämäsi kentät.

metaedit-field-title = Otsikko
metaedit-field-headline = Pääotsikko
metaedit-field-description = Kuvateksti
metaedit-field-keywords = Avainsanat (erotin ;)
metaedit-field-persons-shown = Kuvassa näkyvät henkilöt (erotin ;)
metaedit-field-event = Tapahtuma
metaedit-field-creator = Tekijä (erotin ;)
metaedit-field-creator-job-title = Tekijän tehtävänimike
metaedit-field-contact-address = Osoite
metaedit-field-contact-city = Kaupunki
metaedit-field-contact-region = Osavaltio tai maakunta
metaedit-field-contact-postal-code = Postinumero
metaedit-field-contact-country = Maa
metaedit-field-contact-phone = Puhelin
metaedit-field-contact-email = Sähköposti
metaedit-field-contact-url = Verkkosivusto
metaedit-field-copyright = Tekijänoikeusilmoitus
metaedit-field-credit = Kuvaaja- ja lähdetiedot
metaedit-field-source = Lähde
metaedit-field-usage-terms = Käyttöehdot
metaedit-field-shown-sublocation = Tarkempi sijainti
metaedit-field-shown-city = Kaupunki
metaedit-field-shown-state = Osavaltio tai maakunta
metaedit-field-shown-country = Maa
metaedit-field-shown-country-code = Maakoodi
metaedit-field-created-sublocation = Tarkempi sijainti
metaedit-field-created-city = Kaupunki
metaedit-field-created-state = Osavaltio tai maakunta
metaedit-field-created-country = Maa
metaedit-field-created-country-code = Maakoodi
metaedit-field-instructions = Ohjeet
metaedit-field-job-id = Työtunnus

metaedit-template-new = Uusi malli
metaedit-template-edit = Muokkaa
metaedit-template-remove = Poista
metaedit-template-done = Valmis
metaedit-template-name = Mallin nimi
metaedit-template-default-name = Malli { $n }
metaedit-template-title = Malli
metaedit-template-hint = Tyhjiä kenttiä ei muuteta.
metaedit-template-replace-keywords = Korvaa kuvien avainsanat
metaedit-template-applied = { $count ->
    [one] ”{ $name }” käytetty 1 kuvaan.
   *[other] ”{ $name }” käytetty { $count } kuvaan.
}
metaedit-template-remove-ask = Poistetaanko malli ”{ $name }”?
metaedit-template-remove-keep = Kuvien metatiedot säilyvät.
metaedit-template-remove-yes = Poista
metaedit-template-removed = Malli ”{ $name }” poistettiin.

app-info-edit-metadata = Muokkaa metatietoja…

settings-gallery-metadata-into-files = Kirjoita metatiedot kuvatiedostoihin
settings-gallery-metadata-into-files-help = Metatietojen muokkain kirjoittaa myös JPEG-, TIFF- ja DNG-tiedostoihin muuttamatta niissä mitään muuta. Pois: vain XMP-sivutiedostot.
