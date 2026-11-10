# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Nałóż regiony DS9…
palette-science-load-regions-help = Narysuj na obrazie kształty z pliku regionów DS9 (.reg).
palette-science-export-regions = Zapisz regiony jako DS9…
palette-science-export-regions-help = Zapisz pomiary i nałożone regiony jako plik regionów DS9.
palette-science-edit-regions = Edytuj regiony jako pomiary
palette-science-edit-regions-help = Zamień nałożone regiony w pomiary, które można przesuwać i zmieniać.
palette-science-clear-regions = Usuń nałożone regiony
palette-science-clear-regions-help = Usuń regiony DS9 z obrazu.
palette-science-plate-solve = Rozwiązanie astrometryczne
palette-science-plate-solve-help = Znajdź współrzędne niebieskie obrazu za pomocą ASTAP lub astrometry.net. Uruchom ponownie, aby zatrzymać.
palette-science-save-plate-solution = Zapisz rozwiązanie astrometryczne
palette-science-save-plate-solution-help = Zapisz znalezione współrzędne niebieskie obok obrazu jako plik .wcs.
palette-science-sky-survey = Przegląd nieba jako tło
palette-science-sky-survey-help = Pokaż wokół obrazu następny przegląd nieba albo żaden.
palette-science-samp-send-image = Wyślij obraz do aplikacji SAMP
palette-science-samp-send-image-help = Otwórz ten obraz w DS9, Aladinie i innych aplikacjach w hubie SAMP.
palette-science-samp-send-position = Wyślij pozycję do aplikacji SAMP
palette-science-samp-send-position-help = Skieruj inne aplikacje na pozycję na niebie pod wskaźnikiem.

disabled-samp-off = Najpierw połącz się z hubem SAMP (Ustawienia › Integracja).

settings-integration-samp = Połącz z hubem SAMP
settings-integration-samp-help = Wymieniaj obrazy, tabele i pozycje na niebie z DS9, TOPCAT, Aladinem i innymi aplikacjami astronomicznymi. Używany jest tylko hub na tym komputerze.
settings-integration-astap-path = Program ASTAP
settings-integration-astap-path-help = Program do rozwiązań astrometrycznych ASTAP, jeśli nie zostanie znaleziony automatycznie.
settings-integration-solve-field-path = Program astrometry.net
settings-integration-solve-field-path-help = Program solve-field z astrometry.net, jeśli nie zostanie znaleziony automatycznie.

astro-regions-filter = Regiony DS9
astro-regions-loaded = { $name }: liczba regionów: { $count }.
astro-regions-refused = Nieodczytane wiersze: { $count }. Pierwszy: wiersz { $line }, { $reason }.
astro-regions-approximate = Kształty panda narysowane jako krawędź wewnętrzna i zewnętrzna: { $count }.
astro-regions-none-here = { $name }: żaden region nie wypada na tym obrazie.
astro-regions-failed = Nie można odczytać { $name }: { $reason }
astro-regions-cleared = Usunięto regiony.
astro-regions-nothing = Brak regionów i pomiarów do zapisania.
astro-regions-saved = Zapisano { $name }.
astro-regions-skipped = Pominięte kształty poza współrzędnymi niebieskimi: { $count }.
astro-regions-save-failed = Nie można zapisać regionów: { $reason }
astro-regions-edited = Regiony zamienione w pomiary: { $count }.
astro-regions-no-wcs = Regiony na niebie wymagają obrazu ze współrzędnymi niebieskimi.

astro-solve-no-solver = Nie znaleziono programu do rozwiązań astrometrycznych. Zainstaluj ASTAP lub astrometry.net albo wybierz program w Ustawienia › Integracja.
astro-solve-running = Rozwiązywanie za pomocą { $solver }… { $seconds } s
astro-solve-stop = Zatrzymaj
astro-solve-stopped = Zatrzymano rozwiązywanie astrometryczne.
astro-solve-done = Rozwiązano: { $ra }, { $dec }, { $scale } na piksel.
astro-solve-failed = Rozwiązywanie astrometryczne nie powiodło się: { $reason }
astro-solve-not-local = Rozwiązać można tylko obrazy z folderu na tym komputerze.
astro-solve-none = Ten obraz nie ma jeszcze rozwiązania astrometrycznego.
astro-solve-saved = Zapisano { $name }. Sam obraz nie został zmieniony.
astro-solve-sidecar = Współrzędne niebieskie z { $name }.

astro-samp-connected = Połączono z hubem SAMP.
astro-samp-no-hub = SAMP: { $reason }. Ponowna próba po uruchomieniu huba.
astro-samp-lost = Hub SAMP został zatrzymany. Ponowna próba po uruchomieniu nowego.
astro-samp-sent = Wysłano do aplikacji: { $count }.
astro-samp-nobody = Żadna aplikacja w hubie nie odbiera tego komunikatu.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } nie znajduje się na tym komputerze, więc nie zostanie otwarty.
astro-samp-pointed = Skierowano na { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } nie leży na tym obrazie.
astro-samp-no-table = SAMP: ta tabela nie jest tu nałożona.
astro-samp-rows = Wyróżnione źródła z katalogu: { $count }.

astro-survey-off = Przegląd nieba wyłączony.
astro-survey-on = Przegląd nieba: { $name }. { $credit }
astro-survey-failed = Nie można wyświetlić { $name }: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Kafelki przeglądu nieba z { $host }
