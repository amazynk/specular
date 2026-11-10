# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Zobrazit tagy DICOM
palette-next-series = Další série
palette-prev-series = Předchozí série
disabled-not-dicom = Obrázek není soubor DICOM
disabled-no-other-series = V této složce není jiná série
dicom-tags-title = Tagy DICOM
dicom-tags-title-file = Tagy DICOM: { $file }
dicom-tags-filter = Filtrovat tagy
dicom-tags-show-patient = Zobrazit údaje pacienta
dicom-tags-col-tag = Tag
dicom-tags-col-keyword = Klíčové slovo
dicom-tags-col-vr = VR
dicom-tags-col-length = Délka
dicom-tags-col-value = Hodnota
dicom-tags-hidden = Skryto
dicom-tags-hidden-count = Skryté hodnoty pacienta: { $count }
dicom-tags-no-match = Žádný tag neodpovídá
dicom-tags-reading = Načítání tagů…
dicom-tags-error = Tagy nelze načíst: { $reason }
dicom-tags-items = Položky: { $count }
dicom-tags-fragments = Fragmenty: { $count }
dicom-tags-item = Položka { $index }
dicom-tags-bytes = Bajty: { $count }
dicom-tags-empty = (prázdné)
dicom-tags-omitted = Další neuvedené prvky: { $count }
dicom-series-group = Série
dicom-series-slice = Řez
dicom-series-slice-value = { $index } z { $count }
dicom-series-frames = Snímky: { $frames }
dicom-series-location = Poloha řezu
dicom-series-mm = { $mm } mm
dicom-series-spacing = Rozestup řezů
dicom-series-which = Série
dicom-series-which-value = { $index } z { $count }
dicom-series-order = Pořadí
dicom-series-order-position = Podle polohy
dicom-series-order-instance = Podle čísla instance
dicom-series-order-time = Podle času akvizice
dicom-series-order-name = Podle názvu souboru
dicom-series-gaps = Chybějící řezy
dicom-series-gaps-value = Chybějící řezy: { $count }
dicom-series-gaps-where = Za řezem { $slices }
dicom-series-irregular = Nepravidelný rozestup
dicom-series-tilt = Náklon gantry
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Duplikáty přeskočeny
dicom-series-toast = Série { $index } z { $series } – řezy: { $count }
dicom-series-capped = Načteno jen prvních souborů složky: { $count }

studies-title = Studie
studies-title-medium = Studie: { $medium }
studies-show-patient = Zobrazit údaje pacienta
studies-patient-hidden = Skryto
studies-patient-hidden-note = Jména a data narození pacientů jsou skryta
studies-none = Nenalezeny žádné studie
studies-reading = Čtení disku…
studies-reading-count = Čtení disku: { $done } z { $total ->
        [one] { $total } souboru
        [few] { $total } souborů
        [many] { $total } souboru
       *[other] { $total } souborů
    }
studies-opening = Otevírání série…
studies-error = Disk nelze přečíst: { $reason }
studies-missing = Chybějící soubory uvedené na disku: { $count }
studies-capped = Načteno jen prvních souborů: { $count }
# machine-drafted, needs native review
studies-stopped = Čtení zastaveno po { $done } z { $total ->
        [one] { $total } souboru
        [few] { $total } souborů
        [many] { $total } souboru
       *[other] { $total } souborů
    }: seznam je neúplný
studies-stopped-early = Čtení zastaveno: seznam je neúplný
studies-cancel-description = Zastaví čtení disku. Co už bylo přečteno, zůstane v seznamu.
studies-patient-n = Pacient { $index }
studies-patient-unknown = Neznámý pacient
studies-born = Narozen(a) { $date }
studies-no-description = Bez popisu
studies-series = Série { $number }: { $description }
studies-series-number = Série { $number }
studies-images = Snímky: { $count }
studies-images-missing = Chybí: { $count }
studies-others = Další objekty: { $count }
studies-no-images = Tato série nemá na disku žádné snímky
studies-medium-inserted = Byl vložen disk s lékařskými studiemi
studies-medium-show = Zobrazit studie
palette-studies = Zobrazit studie na tomto disku
