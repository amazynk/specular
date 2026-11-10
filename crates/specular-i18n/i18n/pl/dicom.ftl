# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Pokaż tagi DICOM
palette-next-series = Następna seria
palette-prev-series = Poprzednia seria
disabled-not-dicom = Obraz nie jest plikiem DICOM
disabled-no-other-series = W tym folderze nie ma innej serii
dicom-tags-title = Tagi DICOM
dicom-tags-title-file = Tagi DICOM: { $file }
dicom-tags-filter = Filtruj tagi
dicom-tags-show-patient = Pokaż dane pacjenta
dicom-tags-col-tag = Tag
dicom-tags-col-keyword = Słowo kluczowe
dicom-tags-col-vr = VR
dicom-tags-col-length = Długość
dicom-tags-col-value = Wartość
dicom-tags-hidden = Ukryte
dicom-tags-hidden-count = Ukryte wartości pacjenta: { $count }
dicom-tags-no-match = Żaden tag nie pasuje
dicom-tags-reading = Odczytywanie tagów…
dicom-tags-error = Nie można odczytać tagów: { $reason }
dicom-tags-items = Elementy: { $count }
dicom-tags-fragments = Fragmenty: { $count }
dicom-tags-item = Element { $index }
dicom-tags-bytes = Bajty: { $count }
dicom-tags-empty = (puste)
dicom-tags-omitted = Pozostałe niewymienione elementy: { $count }
dicom-series-group = Seria
dicom-series-slice = Przekrój
dicom-series-slice-value = { $index } z { $count }
dicom-series-frames = Klatki: { $frames }
dicom-series-location = Położenie przekroju
dicom-series-mm = { $mm } mm
dicom-series-spacing = Odstęp przekrojów
dicom-series-which = Seria
dicom-series-which-value = { $index } z { $count }
dicom-series-order = Kolejność
dicom-series-order-position = Według położenia
dicom-series-order-instance = Według numeru instancji
dicom-series-order-time = Według czasu akwizycji
dicom-series-order-name = Według nazwy pliku
dicom-series-gaps = Brakujące przekroje
dicom-series-gaps-value = Brakujące przekroje: { $count }
dicom-series-gaps-where = Po przekroju { $slices }
dicom-series-irregular = Nieregularny odstęp
dicom-series-tilt = Pochylenie gantry
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Pominięto duplikaty
dicom-series-toast = Seria { $index } z { $series } – przekroje: { $count }
dicom-series-capped = Odczytano tylko pierwsze pliki folderu: { $count }

studies-title = Badania
studies-title-medium = Badania: { $medium }
studies-show-patient = Pokaż dane pacjenta
studies-patient-hidden = Ukryte
studies-patient-hidden-note = Nazwiska i daty urodzenia pacjentów są ukryte
studies-none = Nie znaleziono badań
studies-reading = Odczytywanie dysku…
studies-reading-count = Odczytywanie dysku: { $done } z { $total ->
        [one] { $total } pliku
        [few] { $total } plików
        [many] { $total } plików
       *[other] { $total } pliku
    }
studies-opening = Otwieranie serii…
studies-error = Nie można odczytać dysku: { $reason }
studies-missing = Brakujące pliki wymienione na dysku: { $count }
studies-capped = Odczytano tylko pierwsze pliki: { $count }
# machine-drafted, needs native review
studies-stopped = Odczyt przerwany po { $done } z { $total ->
        [one] { $total } pliku
        [few] { $total } plików
        [many] { $total } plików
       *[other] { $total } pliku
    }: lista jest niekompletna
studies-stopped-early = Odczyt przerwany: lista jest niekompletna
studies-cancel-description = Przerywa odczyt dysku. To, co już odczytano, pozostaje na liście.
studies-patient-n = Pacjent { $index }
studies-patient-unknown = Nieznany pacjent
studies-born = Data urodzenia: { $date }
studies-no-description = Brak opisu
studies-series = Seria { $number }: { $description }
studies-series-number = Seria { $number }
studies-images = Obrazy: { $count }
studies-images-missing = Brakuje: { $count }
studies-others = Inne obiekty: { $count }
studies-no-images = Ta seria nie ma obrazów na dysku
studies-medium-inserted = Włożono dysk z badaniami medycznymi
studies-medium-show = Pokaż badania
palette-studies = Pokaż badania na tym dysku
