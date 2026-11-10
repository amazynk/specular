# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = DICOM-tags tonen
palette-next-series = Volgende serie
palette-prev-series = Vorige serie
disabled-not-dicom = De afbeelding is geen DICOM-bestand
disabled-no-other-series = Er is geen andere serie in deze map
dicom-tags-title = DICOM-tags
dicom-tags-title-file = DICOM-tags: { $file }
dicom-tags-filter = Tags filteren
dicom-tags-show-patient = Patiëntgegevens tonen
dicom-tags-col-tag = Tag
dicom-tags-col-keyword = Trefwoord
dicom-tags-col-vr = VR
dicom-tags-col-length = Lengte
dicom-tags-col-value = Waarde
dicom-tags-hidden = Verborgen
dicom-tags-hidden-count = { $count ->
    [one] { $count } patiëntwaarde verborgen
   *[other] { $count } patiëntwaarden verborgen
}
dicom-tags-no-match = Geen tags komen overeen
dicom-tags-reading = Tags lezen…
dicom-tags-error = De tags konden niet worden gelezen: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
dicom-tags-fragments = { $count ->
    [one] { $count } fragment
   *[other] { $count } fragmenten
}
dicom-tags-item = Item { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } bytes
}
dicom-tags-empty = (leeg)
dicom-tags-omitted = { $count ->
    [one] Nog { $count } element niet getoond
   *[other] Nog { $count } elementen niet getoond
}
dicom-series-group = Serie
dicom-series-slice = Plak
dicom-series-slice-value = { $index } van { $count }
dicom-series-frames = { $frames ->
        [one] { $frames } frame
       *[other] { $frames } frames
    }
dicom-series-location = Plakpositie
dicom-series-mm = { $mm } mm
dicom-series-spacing = Plakafstand
dicom-series-which = Serie
dicom-series-which-value = { $index } van { $count }
dicom-series-order = Volgorde
dicom-series-order-position = Op positie
dicom-series-order-instance = Op instantienummer
dicom-series-order-time = Op acquisitietijd
dicom-series-order-name = Op bestandsnaam
dicom-series-gaps = Ontbrekende plakken
dicom-series-gaps-value = { $count ->
    [one] { $count } plak ontbreekt
   *[other] { $count } plakken ontbreken
}
dicom-series-gaps-where = Na plak { $slices }
dicom-series-irregular = Afstand is onregelmatig
dicom-series-tilt = Gantry-kanteling
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Duplicaten overgeslagen
dicom-series-toast = { $series ->
    [one] Serie – plakken: { $count }
   *[other] Serie { $index } van { $series } – plakken: { $count }
}
dicom-series-capped = Alleen de eerste { $count } bestanden van de map zijn gelezen

studies-title = Onderzoeken
studies-title-medium = Onderzoeken: { $medium }
studies-show-patient = Patiëntgegevens tonen
studies-patient-hidden = Verborgen
studies-patient-hidden-note = Patiëntnamen en geboortedata zijn verborgen
studies-none = Geen onderzoeken gevonden
studies-reading = Schijf lezen…
studies-reading-count = Schijf lezen: { $done } van { $total ->
        [one] { $total } bestand
       *[other] { $total } bestanden
    }
studies-opening = Serie openen…
studies-error = Kan de schijf niet lezen: { $reason }
studies-missing = { $count ->
    [one] { $count } bestand dat op de schijf staat vermeld ontbreekt
   *[other] { $count } bestanden die op de schijf staan vermeld ontbreken
}
studies-capped = Alleen de eerste { $count } bestanden zijn gelezen
# machine-drafted, needs native review
studies-stopped = Lezen gestopt na { $done } van { $total ->
        [one] { $total } bestand
       *[other] { $total } bestanden
    }: de lijst is onvolledig
studies-stopped-early = Lezen gestopt: de lijst is onvolledig
studies-cancel-description = Stopt met het lezen van de schijf. Wat al is gelezen, blijft in de lijst staan.
studies-patient-n = Patiënt { $index }
studies-patient-unknown = Onbekende patiënt
studies-born = Geboren { $date }
studies-no-description = Geen beschrijving
studies-series = Serie { $number }: { $description }
studies-series-number = Serie { $number }
studies-images = { $count ->
    [one] { $count } afbeelding
   *[other] { $count } afbeeldingen
}
studies-images-missing = { $count } ontbreken
studies-others = { $count ->
    [one] { $count } ander object
   *[other] { $count } andere objecten
}
studies-no-images = Deze serie heeft geen afbeeldingen op de schijf
studies-medium-inserted = Er is een schijf met medische onderzoeken geplaatst
studies-medium-show = Onderzoeken tonen
palette-studies = Onderzoeken op deze schijf tonen
