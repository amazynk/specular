# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Näytä DICOM-tagit
palette-next-series = Seuraava sarja
palette-prev-series = Edellinen sarja
disabled-not-dicom = Kuva ei ole DICOM-tiedosto
disabled-no-other-series = Kansiossa ei ole muuta sarjaa
dicom-tags-title = DICOM-tagit
dicom-tags-title-file = DICOM-tagit: { $file }
dicom-tags-filter = Suodata tageja
dicom-tags-show-patient = Näytä potilastiedot
dicom-tags-col-tag = Tagi
dicom-tags-col-keyword = Avainsana
dicom-tags-col-vr = VR
dicom-tags-col-length = Pituus
dicom-tags-col-value = Arvo
dicom-tags-hidden = Piilotettu
dicom-tags-hidden-count = { $count ->
    [one] { $count } potilasarvo piilotettu
   *[other] { $count } potilasarvoa piilotettu
}
dicom-tags-no-match = Yksikään tagi ei täsmää
dicom-tags-reading = Luetaan tageja…
dicom-tags-error = Tageja ei voitu lukea: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } kohde
   *[other] { $count } kohdetta
}
dicom-tags-fragments = { $count ->
    [one] { $count } fragmentti
   *[other] { $count } fragmenttia
}
dicom-tags-item = Kohde { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } tavu
   *[other] { $count } tavua
}
dicom-tags-empty = (tyhjä)
dicom-tags-omitted = { $count ->
    [one] { $count } elementti lisää ei listattu
   *[other] { $count } elementtiä lisää ei listattu
}
dicom-series-group = Sarja
dicom-series-slice = Leike
dicom-series-slice-value = { $index }/{ $count }
dicom-series-frames = { $frames ->
        [one] { $frames } kehys
       *[other] { $frames } kehystä
    }
dicom-series-location = Leikkeen sijainti
dicom-series-mm = { $mm } mm
dicom-series-spacing = Leikeväli
dicom-series-which = Sarja
dicom-series-which-value = { $index }/{ $count }
dicom-series-order = Järjestys
dicom-series-order-position = Sijainnin mukaan
dicom-series-order-instance = Instanssinumeron mukaan
dicom-series-order-time = Kuvausajan mukaan
dicom-series-order-name = Tiedostonimen mukaan
dicom-series-gaps = Puuttuvat leikkeet
dicom-series-gaps-value = { $count ->
    [one] { $count } leike puuttuu
   *[other] { $count } leikettä puuttuu
}
dicom-series-gaps-where = Leikkeen { $slices } jälkeen
dicom-series-irregular = Epäsäännöllinen leikeväli
dicom-series-tilt = Gantryn kallistus
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Kaksoiskappaleet ohitettu
dicom-series-toast = { $series ->
    [one] Sarja – leikkeitä: { $count }
   *[other] Sarja { $index }/{ $series } – leikkeitä: { $count }
}
dicom-series-capped = Kansiosta luettiin vain { $count } ensimmäistä tiedostoa

studies-title = Tutkimukset
studies-title-medium = Tutkimukset: { $medium }
studies-show-patient = Näytä potilastiedot
studies-patient-hidden = Piilotettu
studies-patient-hidden-note = Potilaiden nimet ja syntymäajat on piilotettu
studies-none = Tutkimuksia ei löytynyt
studies-reading = Luetaan levyä…
studies-reading-count = Luetaan levyä: { $done }/{ $total ->
        [one] { $total } tiedosto
       *[other] { $total } tiedostoa
    }
studies-opening = Avataan sarjaa…
studies-error = Levyä ei voitu lukea: { $reason }
studies-missing = Levyllä lueteltuja tiedostoja puuttuu: { $count }
studies-capped = Vain ensimmäiset { $count } tiedostoa luettiin
# machine-drafted, needs native review
studies-stopped = Lukeminen pysäytettiin { $done }/{ $total ->
        [one] { $total } tiedoston
       *[other] { $total } tiedoston
    } jälkeen: luettelo on puutteellinen
studies-stopped-early = Lukeminen pysäytettiin: luettelo on puutteellinen
studies-cancel-description = Lopettaa levyn lukemisen. Tähän mennessä luettu pysyy luettelossa.
studies-patient-n = Potilas { $index }
studies-patient-unknown = Tuntematon potilas
studies-born = Syntynyt { $date }
studies-no-description = Ei kuvausta
studies-series = Sarja { $number }: { $description }
studies-series-number = Sarja { $number }
studies-images = Kuvia: { $count }
studies-images-missing = Puuttuu: { $count }
studies-others = Muita objekteja: { $count }
studies-no-images = Tässä sarjassa ei ole kuvia levyllä
studies-medium-inserted = Levy, jolla on lääketieteellisiä tutkimuksia, asetettiin
studies-medium-show = Näytä tutkimukset
palette-studies = Näytä tämän levyn tutkimukset
