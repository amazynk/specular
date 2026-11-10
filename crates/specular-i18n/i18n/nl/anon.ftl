# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

palette-export-anonymised = Geanonimiseerde kopie exporteren…
palette-export-patient-disc = Map voor patiëntschijf exporteren…
palette-mask-burned-in-text = Ingebrande tekst maskeren
anon-title = Geanonimiseerde kopie exporteren
anon-disc-title = Map voor patiëntschijf exporteren
anon-images = Afbeeldingen
anon-images-current = Deze afbeelding
anon-images-series = { $count ->
    [one] Deze serie ({ $count } bestand)
   *[other] Deze serie ({ $count } bestanden)
}
anon-images-study = { $count ->
    [one] Dit onderzoek ({ $count } bestand)
   *[other] Dit onderzoek ({ $count } bestanden)
}
anon-images-reading = Map lezen…
anon-anonymise = Anonimiseren
anon-dates = Datums en tijden
anon-dates-remove = Verwijderen
anon-dates-keep = Behouden
anon-dates-shift = Verschuiven
anon-patient = Patiëntkenmerken
anon-device = Apparaatidentiteit
anon-institution = Identiteit van de instelling
anon-descriptions = Beschrijvingen
anon-descriptions-remove = Verwijderen
anon-descriptions-clean = Opschonen
anon-structured = Inhoud van verslagen
anon-uids = UID’s
anon-uids-replace = Vervangen
anon-uids-keep = Behouden
anon-overlays = Overlays en curven
anon-keep = Behouden
anon-remove = Verwijderen
anon-on = Aan
anon-off = Uit
anon-burned-in = Ingebrande tekst
anon-masks = { $count ->
    [one] { $count } gebied gemaskeerd
   *[other] { $count } gebieden gemaskeerd
}
anon-masks-none = Geen maskers
anon-folder = Map
anon-folder-choose = Kiezen…
anon-export = Exporteren
anon-risk-present = Deze afbeeldingen geven aan dat ze patiënttekst tonen: maskeer die vóór het exporteren.
anon-risk-possible = Afbeeldingen van dit soort tonen vaak patiënttekst: controleer vóór het exporteren.
anon-progress = { $done } van { $total } exporteren…
anon-stopping = Stoppen…
anon-mask-title = Ingebrande tekst maskeren
anon-mask-hint = Sleep over tekst om die te maskeren. Gemaskeerde gebieden worden zwart in elk frame.
anon-mask-suggest = Voorstellen
anon-mask-remove = Verwijderen
anon-mask-clear = Wissen
anon-mask-done = Klaar
anon-mask-area = Gebied { $n }, { $width } × { $height } pixels
anon-mask-suggested = { $count ->
    [0] Geen tekst gevonden bij de randen
    [one] { $count } tekstgebied voorgesteld
   *[other] { $count } tekstgebieden voorgesteld
}
anon-done = { $count ->
    [one] { $count } geanonimiseerd bestand naar { $folder } geschreven
   *[other] { $count } geanonimiseerde bestanden naar { $folder } geschreven
}
anon-disc-done = { $count ->
    [one] Map voor patiëntschijf met { $count } afbeelding naar { $folder } geschreven
   *[other] Map voor patiëntschijf met { $count } afbeeldingen naar { $folder } geschreven
}
anon-skipped-burned-in = { $count ->
    [one] { $count } bestand met ingebrande tekst en zonder maskers is overgeslagen
   *[other] { $count } bestanden met ingebrande tekst en zonder maskers zijn overgeslagen
}
anon-skipped-failed = { $count ->
    [one] { $count } bestand kon niet worden geëxporteerd
   *[other] { $count } bestanden konden niet worden geëxporteerd
}
anon-stopped = Export gestopt
anon-failed = Kan niet exporteren: { $reason }
anon-not-empty = De map bevat al een patiëntschijf
