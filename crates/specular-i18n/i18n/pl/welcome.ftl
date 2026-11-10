# machine-drafted, needs native review
# Specular — Ekran startowy (gdy Specular otwiera się bez pliku i gdy zamknięto ostatni obraz),
# jego karty „Warto wypróbować” i jednorazowe wskazówki. Polski.

## Ekran startowy

welcome-title = { -app-name }
welcome-tagline = Szybka i wierna przeglądarka zdjęć oraz obrazów technicznych, naukowych i medycznych.
welcome-open = Otwórz plik…
welcome-open-about = Obraz, dokument lub plik danych.
welcome-browse = Przeglądaj folder…
welcome-browse-about = Wybierz folder i zobacz go jako galerię.
welcome-pictures = Twoje obrazy
welcome-pictures-about = Wszystkie zdjęcia z biblioteki według daty i miejsca.
welcome-pictures-setup-title = Utwórz bibliotekę…
welcome-pictures-setup = Zaczynając od folderu { $folder }.
welcome-pictures-ask = Dodać folder { $folder } do biblioteki?
welcome-pictures-add = Dodaj folder
welcome-drop = Możesz też upuścić pliki lub foldery w dowolnym miejscu tego okna.
welcome-recent = Ostatnie foldery
welcome-recent-name = { $name }, w { $parent }
welcome-tips = Warto wypróbować
welcome-tips-hide = Ukryj te wskazówki
welcome-tips-hidden = Wskazówki ukryte. „Pokaż powitanie” w palecie poleceń przywraca je.

## Warto wypróbować: nazwa i jeden wiersz. Klawisz obok nazwy to skrót użytkownika.

welcome-tip-gallery = Galeria
welcome-tip-gallery-about = Cały folder na jednym widoku.
welcome-tip-compare = Porównanie
welcome-tip-compare-about = Zaznacz w galerii od dwóch do czterech obrazów, aby zobaczyć je obok siebie.
welcome-tip-cull = Tryb selekcji
welcome-tip-cull-about = Wybieraj, odrzucaj i oceniaj zdjęcia z sesji jednym klawiszem.
welcome-tip-slideshow = Pokaz slajdów
welcome-tip-slideshow-about = Folder na pełnym ekranie, obraz po obrazie.
welcome-tip-palette = Paleta poleceń
welcome-tip-palette-about = Znajdź dowolne polecenie lub ustawienie po nazwie.
welcome-tip-studies = Badania
welcome-tip-studies-about = Pacjenci, badania i serie z płyty lub folderu DICOM.
welcome-tip-dicom-servers = Serwery DICOMweb
welcome-tip-dicom-servers-about = Przeglądaj i otwieraj badania z systemu PACS.
welcome-tip-dicom-tags = Tagi DICOM
welcome-tip-dicom-tags-about = Wszystkie tagi obrazu, z wyszukiwaniem.
welcome-tip-window-level = Okno i poziom
welcome-tip-window-level-about = Przeciągnij, aby ustawić kontrast obrazu DICOM.
welcome-tip-omero-servers = Serwery OMERO
welcome-tip-omero-servers-about = Przeglądaj projekty mikroskopowe w OMERO i IDR.
welcome-tip-stretch = Rozciągnięcie
welcome-tip-stretch-about = Wydobądź słabe szczegóły z danych astronomicznych i naukowych.
welcome-tip-drawing-layers = Warstwy rysunku
welcome-tip-drawing-layers-about = Pokazuj i ukrywaj warstwy rysunku technicznego.
welcome-tip-needs-image = Najpierw otwórz obraz, a potem naciśnij { $key }.
welcome-tip-needs-gallery = W galerii zaznacz od dwóch do czterech obrazów, a potem naciśnij { $key }.
welcome-tip-key = Skrót: { $key }

## Jednorazowe wskazówki: każda raz, dyskretnie, najwyżej jedna na uruchomienie. Ustawienia ›
## Ogólne › Pokazuj wskazówki wyłącza je wszystkie.

hint-gallery = Naciśnij { $key }, aby zobaczyć ten folder jako galerię.
hint-gallery-first = Zaznacz od dwóch do czterech obrazów i naciśnij { $compare }, aby je porównać, lub { $cull }, aby przejrzeć folder w trybie selekcji.
hint-raw = Folder ze zdjęciami RAW: naciśnij { $key }, aby przejrzeć go w trybie selekcji (wybierz, odrzuć, oceń).
hint-pages =
    { $count ->
        [one] Ten plik ma { $count } stronę. { $next } i { $prev } przewracają strony.
        [few] Ten plik ma { $count } strony. { $next } i { $prev } przewracają strony.
        [many] Ten plik ma { $count } stron. { $next } i { $prev } przewracają strony.
       *[other] Ten plik ma { $count } strony. { $next } i { $prev } przewracają strony.
    }
hint-turn-off = Wyłącz wskazówki
hint-turned-off = Wskazówki wyłączone. Ustawienia › Ogólne › Pokazuj wskazówki włącza je ponownie.

## Paleta poleceń

palette-show-welcome = Pokaż powitanie
palette-show-welcome-help = Ekran startowy z opcjami Otwórz, Przeglądaj, Twoje obrazy i rzeczami do wypróbowania.
palette-close = Zamknij
palette-close-help = Zamknij obraz lub folder i wróć do ekranu startowego. Na ekranie startowym: zakończ.

## Pierwsze uruchomienie (pod pytaniem o profil)

profile-on-start = Gdy { -app-name } otwiera się bez pliku:
profile-skip = Pomiń
