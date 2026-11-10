# Specular — Export slideshow as video (DESIGN.md §6.5). Polish. Machine-drafted, needs native review.

palette-export-slideshow = Eksportuj pokaz slajdów jako wideo…

video-export-title = Eksportuj pokaz slajdów jako wideo
video-export-resolution = Rozdzielczość
# $width and $height in pixels.
video-export-resolution-window = Okno ({ $width } × { $height })
video-export-fps = Liczba klatek na sekundę
# $fps frames per second, a number.
video-export-fps-value = { $fps } kl./s
video-export-quality = Jakość
video-export-quality-high = Wysoka
video-export-quality-standard = Standardowa
video-export-quality-compact = Kompaktowa
video-export-images = { $count ->
    [one] Jeden obraz, z kolejnością, przejściem i podpisami pokazu slajdów
    [few] { $count } obrazy, z kolejnością, przejściem i podpisami pokazu slajdów
    [many] { $count } obrazów, z kolejnością, przejściem i podpisami pokazu slajdów
   *[other] { $count } obrazu, z kolejnością, przejściem i podpisami pokazu slajdów
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Około { $length }, około { $size }
video-export-export = Eksportuj…
video-export-replace = Zastąp
# $file a file name.
video-export-replace-question = { $file } już istnieje. Zastąpić?
# $percent a formatted percentage ("42%").
video-export-progress = Eksportowanie… { $percent }
video-export-cancelling = Anulowanie…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Zapisano { $file } ({ $size })
video-export-skipped = { $count ->
    [one] Nie udało się odczytać jednego obrazu, więc go pominięto.
    [few] Nie udało się odczytać { $count } obrazów, więc je pominięto.
    [many] Nie udało się odczytać { $count } obrazów, więc je pominięto.
   *[other] Nie udało się odczytać { $count } obrazu, więc je pominięto.
}
video-export-failed = Nie można wyeksportować wideo: { $reason }
video-export-cancelled = Eksport anulowany
video-export-no-images = Nie udało się odczytać żadnego z obrazów.
video-export-show-in-folder = Pokaż w folderze

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Eksportuj pokaz slajdów jako wideo
video-export-compatibility = Zgodność
video-export-compatibility-current = Obecne odtwarzacze
video-export-compatibility-all = Wszystkie odtwarzacze
