# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Visa DICOM-taggar
palette-next-series = Nästa serie
palette-prev-series = Föregående serie
disabled-not-dicom = Bilden är inte en DICOM-fil
disabled-no-other-series = Det finns ingen annan serie i mappen
dicom-tags-title = DICOM-taggar
dicom-tags-title-file = DICOM-taggar: { $file }
dicom-tags-filter = Filtrera taggar
dicom-tags-show-patient = Visa patientdata
dicom-tags-col-tag = Tagg
dicom-tags-col-keyword = Nyckelord
dicom-tags-col-vr = VR
dicom-tags-col-length = Längd
dicom-tags-col-value = Värde
dicom-tags-hidden = Dold
dicom-tags-hidden-count = { $count ->
    [one] { $count } patientvärde dolt
   *[other] { $count } patientvärden dolda
}
dicom-tags-no-match = Inga taggar matchar
dicom-tags-reading = Läser taggar…
dicom-tags-error = Det gick inte att läsa taggarna: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } post
   *[other] { $count } poster
}
dicom-tags-fragments = { $count ->
    [one] { $count } fragment
   *[other] { $count } fragment
}
dicom-tags-item = Post { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } byte
}
dicom-tags-empty = (tom)
dicom-tags-omitted = { $count ->
    [one] { $count } element till visas inte
   *[other] { $count } element till visas inte
}
dicom-series-group = Serie
dicom-series-slice = Snitt
dicom-series-slice-value = { $index } av { $count }
dicom-series-frames = { $frames ->
        [one] { $frames } bildruta
       *[other] { $frames } bildrutor
    }
dicom-series-location = Snittposition
dicom-series-mm = { $mm } mm
dicom-series-spacing = Snittavstånd
dicom-series-which = Serie
dicom-series-which-value = { $index } av { $count }
dicom-series-order = Ordning
dicom-series-order-position = Efter position
dicom-series-order-instance = Efter instansnummer
dicom-series-order-time = Efter insamlingstid
dicom-series-order-name = Efter filnamn
dicom-series-gaps = Saknade snitt
dicom-series-gaps-value = { $count ->
    [one] { $count } snitt saknas
   *[other] { $count } snitt saknas
}
dicom-series-gaps-where = Efter snitt { $slices }
dicom-series-irregular = Ojämnt avstånd
dicom-series-tilt = Gantrylutning
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Dubbletter hoppades över
dicom-series-toast = { $series ->
    [one] Serie – snitt: { $count }
   *[other] Serie { $index } av { $series } – snitt: { $count }
}
dicom-series-capped = Bara de första { $count } filerna i mappen lästes

studies-title = Undersökningar
studies-title-medium = Undersökningar: { $medium }
studies-show-patient = Visa patientdata
studies-patient-hidden = Dold
studies-patient-hidden-note = Patientnamn och födelsedatum är dolda
studies-none = Inga undersökningar hittades
studies-reading = Läser skivan…
studies-reading-count = Läser skivan: { $done } av { $total ->
        [one] { $total } fil
       *[other] { $total } filer
    }
studies-opening = Öppnar serien…
studies-error = Det gick inte att läsa skivan: { $reason }
studies-missing = { $count ->
    [one] { $count } fil som listas på skivan saknas
   *[other] { $count } filer som listas på skivan saknas
}
studies-capped = Endast de första { $count } filerna lästes
# machine-drafted, needs native review
studies-stopped = Läsningen stoppades efter { $done } av { $total ->
        [one] { $total } fil
       *[other] { $total } filer
    }: listan är ofullständig
studies-stopped-early = Läsningen stoppades: listan är ofullständig
studies-cancel-description = Slutar läsa skivan. Det som har lästs hittills ligger kvar i listan.
studies-patient-n = Patient { $index }
studies-patient-unknown = Okänd patient
studies-born = Född { $date }
studies-no-description = Ingen beskrivning
studies-series = Serie { $number }: { $description }
studies-series-number = Serie { $number }
studies-images = { $count ->
    [one] { $count } bild
   *[other] { $count } bilder
}
studies-images-missing = { $count } saknas
studies-others = { $count ->
    [one] { $count } annat objekt
   *[other] { $count } andra objekt
}
studies-no-images = Den här serien har inga bilder på skivan
studies-medium-inserted = En skiva med medicinska undersökningar sattes in
studies-medium-show = Visa undersökningar
palette-studies = Visa undersökningar på den här skivan
