# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Vis DICOM-tags
palette-next-series = Næste serie
palette-prev-series = Forrige serie
disabled-not-dicom = Billedet er ikke en DICOM-fil
disabled-no-other-series = Der er ingen anden serie i mappen
dicom-tags-title = DICOM-tags
dicom-tags-title-file = DICOM-tags: { $file }
dicom-tags-filter = Filtrér tags
dicom-tags-show-patient = Vis patientdata
dicom-tags-col-tag = Tag
dicom-tags-col-keyword = Nøgleord
dicom-tags-col-vr = VR
dicom-tags-col-length = Længde
dicom-tags-col-value = Værdi
dicom-tags-hidden = Skjult
dicom-tags-hidden-count = { $count ->
    [one] { $count } patientværdi skjult
   *[other] { $count } patientværdier skjult
}
dicom-tags-no-match = Ingen tags passer
dicom-tags-reading = Læser tags…
dicom-tags-error = Kunne ikke læse tags: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } element
   *[other] { $count } elementer
}
dicom-tags-fragments = { $count ->
    [one] { $count } fragment
   *[other] { $count } fragmenter
}
dicom-tags-item = Element { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } byte
}
dicom-tags-empty = (tom)
dicom-tags-omitted = { $count ->
    [one] { $count } element mere ikke vist
   *[other] { $count } elementer mere ikke vist
}
dicom-series-group = Serie
dicom-series-slice = Snit
dicom-series-slice-value = { $index } af { $count }
dicom-series-frames = { $frames ->
        [one] { $frames } billede
       *[other] { $frames } billeder
    }
dicom-series-location = Snitposition
dicom-series-mm = { $mm } mm
dicom-series-spacing = Snitafstand
dicom-series-which = Serie
dicom-series-which-value = { $index } af { $count }
dicom-series-order = Rækkefølge
dicom-series-order-position = Efter position
dicom-series-order-instance = Efter instansnummer
dicom-series-order-time = Efter optagetidspunkt
dicom-series-order-name = Efter filnavn
dicom-series-gaps = Manglende snit
dicom-series-gaps-value = { $count ->
    [one] { $count } snit mangler
   *[other] { $count } snit mangler
}
dicom-series-gaps-where = Efter snit { $slices }
dicom-series-irregular = Uregelmæssig afstand
dicom-series-tilt = Gantry-hældning
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Dubletter sprunget over
dicom-series-toast = { $series ->
    [one] Serie – snit: { $count }
   *[other] Serie { $index } af { $series } – snit: { $count }
}
dicom-series-capped = Kun de første { $count } filer i mappen blev læst

studies-title = Undersøgelser
studies-title-medium = Undersøgelser: { $medium }
studies-show-patient = Vis patientdata
studies-patient-hidden = Skjult
studies-patient-hidden-note = Patientnavne og fødselsdatoer er skjult
studies-none = Ingen undersøgelser fundet
studies-reading = Læser disken…
studies-reading-count = Læser disken: { $done } af { $total ->
        [one] { $total } fil
       *[other] { $total } filer
    }
studies-opening = Åbner serien…
studies-error = Disken kunne ikke læses: { $reason }
studies-missing = { $count ->
    [one] { $count } fil, der er angivet på disken, mangler
   *[other] { $count } filer, der er angivet på disken, mangler
}
studies-capped = Kun de første { $count } filer blev læst
# machine-drafted, needs native review
studies-stopped = Læsningen stoppede efter { $done } af { $total ->
        [one] { $total } fil
       *[other] { $total } filer
    }: listen er ufuldstændig
studies-stopped-early = Læsningen stoppede: listen er ufuldstændig
studies-cancel-description = Stopper læsningen af disken. Det, der er læst indtil nu, bliver på listen.
studies-patient-n = Patient { $index }
studies-patient-unknown = Ukendt patient
studies-born = Født { $date }
studies-no-description = Ingen beskrivelse
studies-series = Serie { $number }: { $description }
studies-series-number = Serie { $number }
studies-images = { $count ->
    [one] { $count } billede
   *[other] { $count } billeder
}
studies-images-missing = { $count } mangler
studies-others = { $count ->
    [one] { $count } andet objekt
   *[other] { $count } andre objekter
}
studies-no-images = Denne serie har ingen billeder på disken
studies-medium-inserted = Der blev indsat en disk med medicinske undersøgelser
studies-medium-show = Vis undersøgelser
palette-studies = Vis undersøgelser på denne disk
