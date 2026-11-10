# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

palette-export-anonymised = Eksportér anonymiseret kopi…
palette-export-patient-disc = Eksportér mappe til patientdisk…
palette-mask-burned-in-text = Maskér indbrændt tekst
anon-title = Eksportér anonymiseret kopi
anon-disc-title = Eksportér mappe til patientdisk
anon-images = Billeder
anon-images-current = Dette billede
anon-images-series = { $count ->
    [one] Denne serie ({ $count } fil)
   *[other] Denne serie ({ $count } filer)
}
anon-images-study = { $count ->
    [one] Denne undersøgelse ({ $count } fil)
   *[other] Denne undersøgelse ({ $count } filer)
}
anon-images-reading = Læser mappen…
anon-anonymise = Anonymisér
anon-dates = Datoer og klokkeslæt
anon-dates-remove = Fjern
anon-dates-keep = Behold
anon-dates-shift = Forskyd
anon-patient = Patientkarakteristika
anon-device = Enhedsidentitet
anon-institution = Institutionens identitet
anon-descriptions = Beskrivelser
anon-descriptions-remove = Fjern
anon-descriptions-clean = Rens
anon-structured = Rapportindhold
anon-uids = UID’er
anon-uids-replace = Erstat
anon-uids-keep = Behold
anon-overlays = Overlejringer og kurver
anon-keep = Behold
anon-remove = Fjern
anon-on = Til
anon-off = Fra
anon-burned-in = Indbrændt tekst
anon-masks = { $count ->
    [one] { $count } område maskeret
   *[other] { $count } områder maskeret
}
anon-masks-none = Ingen masker
anon-folder = Mappe
anon-folder-choose = Vælg…
anon-export = Eksportér
anon-risk-present = Billederne angiver, at de viser patienttekst: maskér den før eksport.
anon-risk-possible = Billeder af denne type viser ofte patienttekst: kontrollér før eksport.
anon-progress = Eksporterer { $done } af { $total }…
anon-stopping = Stopper…
anon-mask-title = Maskér indbrændt tekst
anon-mask-hint = Træk hen over tekst for at maskere den. Maskerede områder bliver sorte i hvert billede.
anon-mask-suggest = Foreslå
anon-mask-remove = Fjern
anon-mask-clear = Ryd
anon-mask-done = Færdig
anon-mask-area = Område { $n }, { $width } × { $height } pixel
anon-mask-suggested = { $count ->
    [0] Ingen tekst fundet nær kanterne
    [one] { $count } tekstområde foreslået
   *[other] { $count } tekstområder foreslået
}
anon-done = { $count ->
    [one] { $count } anonymiseret fil skrevet til { $folder }
   *[other] { $count } anonymiserede filer skrevet til { $folder }
}
anon-disc-done = { $count ->
    [one] Mappe til patientdisk med { $count } billede skrevet til { $folder }
   *[other] Mappe til patientdisk med { $count } billeder skrevet til { $folder }
}
anon-skipped-burned-in = { $count ->
    [one] { $count } fil med indbrændt tekst og uden masker blev sprunget over
   *[other] { $count } filer med indbrændt tekst og uden masker blev sprunget over
}
anon-skipped-failed = { $count ->
    [one] { $count } fil kunne ikke eksporteres
   *[other] { $count } filer kunne ikke eksporteres
}
anon-stopped = Eksporten blev stoppet
anon-failed = Kunne ikke eksportere: { $reason }
anon-not-empty = Mappen indeholder allerede en patientdisk
