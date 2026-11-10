# Specular — Galerie dla klientów: statyczny folder HTML ze zdjęciami oraz odczyt wyboru klienta.

# machine-drafted, needs native review

palette-client-gallery = Galeria dla klienta…
palette-client-gallery-help = Zapisz zdjęcia jako folder strony WWW dla klienta, z wyborem, który może odesłać.
palette-import-client-picks = Importuj wybór klienta…
palette-import-client-picks-help = Oznacz zdjęcia, które klient wybrał w swojej galerii.

webgallery-heading = Galeria dla klienta
webgallery-subtitle = { $count ->
    [one] Jedno zdjęcie
    [few] { $count } zdjęcia
    [many] { $count } zdjęć
   *[other] { $count } zdjęcia
}
webgallery-title-label = Tytuł
webgallery-title-default = Próbki
webgallery-watermark-label = Tekst znaku wodnego (pusty: brak)
webgallery-source = Zdjęcia
webgallery-source-current = To zdjęcie
webgallery-source-selection = Zaznaczenie
webgallery-source-album = Album „{ $name }”
webgallery-source-search = Wyniki wyszukiwania
webgallery-source-folder = Ten folder
webgallery-source-picks = Wybrane
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Motyw
webgallery-theme-light = Jasny
webgallery-theme-dark = Ciemny
webgallery-theme-minimal = Minimalny
webgallery-size = Dłuższy bok
webgallery-size-value = { $px } px
webgallery-format = Format
webgallery-captions = Pod każdym zdjęciem
webgallery-captions-off = Nic
webgallery-captions-name = Nazwa pliku
webgallery-captions-title = Tytuł
webgallery-captions-all = Nazwa pliku, tytuł i podpis
webgallery-exif = Pokaż aparat i ekspozycję
webgallery-location = Zachowaj lokalizację (GPS) w zdjęciach
webgallery-note-private = Statycznej galerii nie da się zabezpieczyć hasłem.
webgallery-note-share = Każdy, kto ma folder lub jego adres, widzi wszystkie zdjęcia. Udostępnij link tylko klientowi albo użyj kontroli dostępu na swoim hostingu.
webgallery-note-picks = Klienci wybierają serduszkiem. Wybór zostaje w ich przeglądarce, dopóki nie wyślą Ci listy.
webgallery-write = Utwórz galerię…
webgallery-busy = Zapisywanie { $done } z { $total }…
webgallery-stop = Zatrzymaj
webgallery-done = { $count ->
    [one] Galeria utworzona: jedno zdjęcie w { $folder }
    [few] Galeria utworzona: { $count } zdjęcia w { $folder }
    [many] Galeria utworzona: { $count } zdjęć w { $folder }
   *[other] Galeria utworzona: { $count } zdjęcia w { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Nie udało się zapisać jednego zdjęcia.
    [few] Nie udało się zapisać { $failed } zdjęć.
    [many] Nie udało się zapisać { $failed } zdjęć.
   *[other] Nie udało się zapisać { $failed } zdjęcia.
}
webgallery-stopped = Galeria zatrzymana. Nic nie zostało zachowane.
webgallery-failed = Galeria nie została utworzona: { $reason }
webgallery-nothing = Nie udało się zapisać żadnego zdjęcia.

webgallery-page-count = { $count ->
    [one] Jedno zdjęcie
    [few] { $count } zdjęcia
    [many] { $count } zdjęć
   *[other] { $count } zdjęcia
}
webgallery-page-hint = Dotknij serduszka na zdjęciach, które chcesz. Twój wybór zostaje w tej przeglądarce, dopóki go nie wyślesz.
webgallery-page-picked = Wybrane: {"{"}n{"}"}
webgallery-page-pick = Wybierz
webgallery-page-copy = Kopiuj mój wybór
webgallery-page-copied = Skopiowano. Wklej listę do wiadomości.
webgallery-page-copy-by-hand = Skopiuj zaznaczoną listę i wklej ją do wiadomości.
webgallery-page-download-txt = Pobierz picks.txt
webgallery-page-download-json = Pobierz picks.json
webgallery-page-clear = Wyczyść wybór
webgallery-page-clear-ask = Wyczyścić cały wybór w tej przeglądarce?
webgallery-page-no-storage = Ta przeglądarka nie zachowa Twojego wyboru. Wyślij go przed zamknięciem strony.
webgallery-page-noscript = Wybieranie wymaga JavaScriptu. Zdjęcia nadal się otwierają.
webgallery-page-previous = Poprzednie
webgallery-page-next = Następne
webgallery-page-close = Zamknij
webgallery-page-position = {"{"}i{"}"} z {"{"}n{"}"}
webgallery-page-viewer = Przeglądarka zdjęć
webgallery-page-picks = Twój wybór

clientpicks-heading = Importuj wybór klienta
clientpicks-subtitle = { $count ->
    [one] Porównano z jednym wyświetlanym zdjęciem
    [few] Porównano z { $count } wyświetlanymi zdjęciami
    [many] Porównano z { $count } wyświetlanymi zdjęciami
   *[other] Porównano z { $count } wyświetlanego zdjęcia
}
clientpicks-paste-label = Nazwy (wklej listę klienta)
clientpicks-choose = Wybierz plik wyboru…
clientpicks-from-file = Z { $name }
clientpicks-apply-as = Oznacz je
clientpicks-as-pick = Jako wybrane
clientpicks-as-stars = { $stars ->
    [one] Jedna gwiazdka
    [few] { $stars } gwiazdki
    [many] { $stars } gwiazdek
   *[other] { $stars } gwiazdki
}
clientpicks-as-label = Etykieta { $label }
clientpicks-empty = Wklej listę przysłaną przez klienta albo wybierz jego picks.txt lub picks.json.
clientpicks-matched = { $matched } z { $total } nazw pasuje do zdjęć tutaj.
clientpicks-missing = Nie znaleziono: { $names }
clientpicks-ambiguous = W więcej niż jednym folderze, pominięte: { $names }
clientpicks-more = { $names } i { $more } więcej
clientpicks-apply = Zastosuj
clientpicks-applied = { $count ->
    [one] Oznaczono jedno zdjęcie.
    [few] Oznaczono { $count } zdjęcia.
    [many] Oznaczono { $count } zdjęć.
   *[other] Oznaczono { $count } zdjęcia.
}
clientpicks-unchanged = Te zdjęcia były już oznaczone.
clientpicks-read-failed = Nie można odczytać { $name }: { $reason }
