# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Inspekcja
ndt-probe = Sonda: { $name }
ndt-probe-profile = Profil wzdłuż linii
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Dwupręcikowy wskaźnik IQI
ndt-probe-wire-iqi = Pręcikowy wskaźnik IQI
ndt-probe-crack = Szerokość pęknięcia
ndt-probe-density = Punkt gęstości

ndt-hint-profile = Przeciągnij linię, aby wykreślić wartości szarości wzdłuż niej. [ i ] zmieniają sondę.
ndt-hint-snr = Przeciągnij obszar na jednolitym polu. Zostanie powiększony do co najmniej 20 × 55 pikseli.
ndt-hint-duplex-iqi = Przeciągnij linię przez wszystkie pary pręcików, wzdłuż wskaźnika IQI.
ndt-hint-wire-iqi = Przeciągnij krótką linię w poprzek najcieńszego widocznego pręcika.
ndt-hint-crack = Klikaj wzdłuż pęknięcia, a potem naciśnij Enter. Szerokość jest odczytywana w 10 punktach.
ndt-hint-density = Kliknij punkt, aby odczytać jego wartość szarości. Wpisz jego znaną gęstość, aby skalibrować.

ndt-refused-needs-scale = Najpierw ustaw skalę: rozmiary pręcików wymagają milimetrów. Użyj Kalibruj na znanej długości.
ndt-refused-nothing-found = Wzdłuż tej linii nic nie wyróżnia się z szumu.
ndt-refused-too-small = Obraz jest mniejszy niż obszar SNR 20 × 55 pikseli.
ndt-refused-no-noise = W tym obszarze nie ma szumu do zmierzenia (wszystkie piksele są jednakowe).

ndt-reading = Odczyt…
ndt-no-pixels = Nie można odczytać pikseli.

ndt-profile-range = Szarość od { $min } do { $max }
ndt-profile-mean = Średnia szarość { $mean }
ndt-profile-density = Gęstość od { $min } do { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn wymaga SRb: zmierz dwupręcikowy wskaźnik IQI lub wpisz wartość.
ndt-snr-stats = Średnia { $mean }, σ { $sd }, obszar { $w } × { $h } px

ndt-duplex-result = Pierwsza nierozdzielona para { $pair }: SRb { $srb } µm, nieostrość { $unsharpness }
ndt-duplex-all-resolved = Wszystkie znalezione pary są rozdzielone.
ndt-duplex-dips = Spadki: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpolowane: iSRb { $isrb } µm, nieostrość { $unsharpness }

ndt-wire-result = Pręcik { $wire }, { $diameter }
ndt-wire-measured = Zmierzona szerokość { $width } (FWHM), średnica około { $diameter }

ndt-crack-mean = Średnia szerokość { $width }
ndt-crack-range = Najwęższe miejsce { $min }, najszersze { $max }
ndt-crack-cuts = Punkty pomiaru: { $count }

ndt-density-value = Szarość { $grey }, gęstość { $density }
ndt-density-grey = Szarość { $grey }
ndt-density-uncalibrated = Brak kalibracji gęstości: wpisz znane gęstości w dwóch punktach.

ndt-ask-density = Znana gęstość w tym miejscu: { $value } (Enter zatwierdza, pusty Enter pomija)
ndt-ask-srb = Podstawowa rozdzielczość przestrzenna SRb: { $value } µm (Enter zatwierdza, Esc pomija)
ndt-bad-density = Wpisz gęstość od 0 do 10.
ndt-bad-srb = Wpisz SRb w mikrometrach, na przykład 80.
ndt-srb-set = Ustawiono SRb { $srb } µm dla tego obrazu.
ndt-density-one-more = Zapamiętano punkt gęstości. Wpisz gęstość w jeszcze jednym punkcie.
ndt-density-calibrated = Skalibrowano gęstość na podstawie dwóch punktów.
ndt-sidecar-failed = Nie można zapisać ustawień badania obok pliku: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m nad miejscem startu
map-export-kml = Zapisz zdjęcia jako KMZ lub KML…
map-export-kmz-full = Zapisz zdjęcia jako KMZ z pełnymi zdjęciami…
map-kml-heading = Kurs { $degrees }°
map-kml-height = { $metres } m nad miejscem startu
map-kml-nothing = Brak zdjęć z położeniem do zapisania.
map-kml-saved = Zdjęcia zapisane w { $name }: { $count }.
map-kml-failed = Nie można zapisać pliku mapy: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Raport z inspekcji
inspection-report-default-title = Raport z inspekcji
inspection-report-folder = Dodaj pozostałe zdjęcia z tego folderu
inspection-report-docx = Zapisz też dokument Word (DOCX)
inspection-report-kmz = Zapisz też KMZ z miejscami zdjęć
inspection-report-caption = Zdjęcie z pomiarami
inspection-report-col-value = Wartość
inspection-report-col-scale = Skala według
inspection-report-docx-failed = Nie można zapisać dokumentu Word.
inspection-report-unreadable = Nie można odczytać żadnego ze zdjęć.
