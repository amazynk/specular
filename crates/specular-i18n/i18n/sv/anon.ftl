# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

palette-export-anonymised = Exportera anonymiserad kopia…
palette-export-patient-disc = Exportera mapp för patientskiva…
palette-mask-burned-in-text = Maskera inbränd text
anon-title = Exportera anonymiserad kopia
anon-disc-title = Exportera mapp för patientskiva
anon-images = Bilder
anon-images-current = Den här bilden
anon-images-series = { $count ->
    [one] Den här serien ({ $count } fil)
   *[other] Den här serien ({ $count } filer)
}
anon-images-study = { $count ->
    [one] Den här undersökningen ({ $count } fil)
   *[other] Den här undersökningen ({ $count } filer)
}
anon-images-reading = Läser mappen…
anon-anonymise = Anonymisera
anon-dates = Datum och tider
anon-dates-remove = Ta bort
anon-dates-keep = Behåll
anon-dates-shift = Förskjut
anon-patient = Patientegenskaper
anon-device = Enhetsidentitet
anon-institution = Institutionens identitet
anon-descriptions = Beskrivningar
anon-descriptions-remove = Ta bort
anon-descriptions-clean = Rensa
anon-structured = Rapportinnehåll
anon-uids = UID:er
anon-uids-replace = Ersätt
anon-uids-keep = Behåll
anon-overlays = Överlägg och kurvor
anon-keep = Behåll
anon-remove = Ta bort
anon-on = På
anon-off = Av
anon-burned-in = Inbränd text
anon-masks = { $count ->
    [one] { $count } område maskerat
   *[other] { $count } områden maskerade
}
anon-masks-none = Inga masker
anon-folder = Mapp
anon-folder-choose = Välj…
anon-export = Exportera
anon-risk-present = Bilderna anger att de visar patienttext: maskera den före export.
anon-risk-possible = Bilder av det här slaget visar ofta patienttext: kontrollera före export.
anon-progress = Exporterar { $done } av { $total }…
anon-stopping = Stoppar…
anon-mask-title = Maskera inbränd text
anon-mask-hint = Dra över text för att maskera den. Maskerade områden blir svarta i varje bildruta.
anon-mask-suggest = Föreslå
anon-mask-remove = Ta bort
anon-mask-clear = Rensa
anon-mask-done = Klar
anon-mask-area = Område { $n }, { $width } × { $height } pixlar
anon-mask-suggested = { $count ->
    [0] Ingen text hittades nära kanterna
    [one] { $count } textområde föreslogs
   *[other] { $count } textområden föreslogs
}
anon-done = { $count ->
    [one] { $count } anonymiserad fil skrevs till { $folder }
   *[other] { $count } anonymiserade filer skrevs till { $folder }
}
anon-disc-done = { $count ->
    [one] Mapp för patientskiva med { $count } bild skrevs till { $folder }
   *[other] Mapp för patientskiva med { $count } bilder skrevs till { $folder }
}
anon-skipped-burned-in = { $count ->
    [one] { $count } fil med inbränd text och utan masker hoppades över
   *[other] { $count } filer med inbränd text och utan masker hoppades över
}
anon-skipped-failed = { $count ->
    [one] { $count } fil kunde inte exporteras
   *[other] { $count } filer kunde inte exporteras
}
anon-stopped = Exporten stoppades
anon-failed = Det gick inte att exportera: { $reason }
anon-not-empty = Mappen innehåller redan en patientskiva
