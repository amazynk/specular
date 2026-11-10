# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Kolejka, powtarzanie i losowo
# machine-drafted, needs native review
palette-next-track = Następna pozycja
# machine-drafted, needs native review
palette-next-track-help = Odtwórz następną pozycję kolejki, a bez niej następny plik tego samego rodzaju w folderze.
# machine-drafted, needs native review
palette-prev-track = Poprzednia pozycja
# machine-drafted, needs native review
palette-prev-track-help = Odtwórz poprzednią pozycję. Po 3 sekundach bieżąca zacznie się od nowa.
# machine-drafted, needs native review
palette-cycle-repeat = Powtarzanie
# machine-drafted, needs native review
palette-cycle-repeat-help = Powtarzanie wyłączone, wszystko lub jedna.
# machine-drafted, needs native review
palette-shuffle = Losowo
# machine-drafted, needs native review
palette-shuffle-help = Odtwarzaj w losowej kolejności, każdą pozycję raz na rundę.
# machine-drafted, needs native review
palette-play-queue = Kolejka odtwarzania
# machine-drafted, needs native review
palette-play-queue-help = Pokaż, co gra dalej: zmień kolejność, usuń, zapisz.
# machine-drafted, needs native review
palette-queue-play-next = Odtwórz jako następne
# machine-drafted, needs native review
palette-queue-play-next-help = Odtwórz plik (w galerii zaznaczenie) po bieżącej pozycji.
# machine-drafted, needs native review
palette-queue-add = Dodaj do kolejki
# machine-drafted, needs native review
palette-queue-add-help = Dodaj plik (w galerii zaznaczenie) na koniec kolejki.
# machine-drafted, needs native review
palette-save-queue = Zapisz kolejkę jako playlistę…
# machine-drafted, needs native review
palette-save-queue-help = Zapisz kolejkę jako playlistę M3U8 lub XSPF.
# machine-drafted, needs native review
transport-prev-track = Poprzednia pozycja
# machine-drafted, needs native review
transport-next-track = Następna pozycja
# machine-drafted, needs native review
queue-title = Kolejka odtwarzania
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 pozycja
    [few] { $count } pozycje
    [many] { $count } pozycji
   *[other] { $count } pozycji
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 pozycja
    [few] { $count } pozycje
    [many] { $count } pozycji
   *[other] { $count } pozycji
} · { $time }
# machine-drafted, needs native review
queue-empty = Kolejka jest pusta.
    Dodaj pliki dźwiękowe i wideo przez Odtwórz jako następne lub Dodaj do kolejki albo otwórz playlistę.
# machine-drafted, needs native review
queue-empty-short = Kolejka jest pusta.
# machine-drafted, needs native review
queue-playing = Odtwarzane
# machine-drafted, needs native review
queue-repeat = Powtarzanie
# machine-drafted, needs native review
queue-repeat-off = Powtarzanie: wył.
# machine-drafted, needs native review
queue-repeat-all = Powtarzanie: wszystko
# machine-drafted, needs native review
queue-repeat-one = Powtarzanie: jedna
# machine-drafted, needs native review
queue-shuffle = Losowo
# machine-drafted, needs native review
queue-shuffle-on = Losowo: wł.
# machine-drafted, needs native review
queue-shuffle-off = Losowo: wył.
# machine-drafted, needs native review
queue-save = Zapisz jako playlistę…
# machine-drafted, needs native review
queue-save-tip = Zapisz kolejkę jako playlistę M3U8 lub XSPF
# machine-drafted, needs native review
queue-clear = Wyczyść…
# machine-drafted, needs native review
queue-clear-tip = Wyczyść kolejkę. Pliki zostają na dysku.
# machine-drafted, needs native review
queue-remove = Usuń z kolejki (plik zostaje na dysku)
# machine-drafted, needs native review
queue-network-item = Pozycja sieciowa, nieodtwarzana: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Brak następnej pozycji.
# machine-drafted, needs native review
queue-at-start = Brak poprzedniej pozycji.
# machine-drafted, needs native review
queue-missing = Pliku { $name } już nie ma. Pominięto.
# machine-drafted, needs native review
queue-network-not-played = Pozycje sieciowe nie są jeszcze odtwarzane.
# machine-drafted, needs native review
queue-nothing-to-add = Nie ma czego dodać. Do kolejki trafiają tylko pliki dźwiękowe, wideo i playlisty.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] Dodano 1 pozycję do kolejki.
    [few] Dodano { $count } pozycje do kolejki.
    [many] Dodano { $count } pozycji do kolejki.
   *[other] Dodano { $count } pozycji do kolejki.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 pozycja zagra jako następna.
    [few] { $count } pozycje zagrają jako następne.
    [many] { $count } pozycji zagra jako następne.
   *[other] { $count } pozycji zagra jako następne.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 pozycja w kolejce.
    [few] { $count } pozycje w kolejce.
    [many] { $count } pozycji w kolejce.
   *[other] { $count } pozycji w kolejce.
}
# machine-drafted, needs native review
queue-playlist-nothing = Nic z { $name } nie da się tu odtworzyć. Kolejka pokazuje jej zawartość.
# machine-drafted, needs native review
queue-playlist-failed = Nie można otworzyć { $name }: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = nie można odczytać pliku.
# machine-drafted, needs native review
queue-playlist-damaged = to nie jest playlista, którą Specular potrafi odczytać.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Wyczyścić kolejkę (1 pozycja)?
    [few] Wyczyścić kolejkę ({ $count } pozycje)?
    [many] Wyczyścić kolejkę ({ $count } pozycji)?
   *[other] Wyczyścić kolejkę ({ $count } pozycji)?
}
# machine-drafted, needs native review
queue-clear-detail = Czyszczona jest tylko lista. Pliki zostają na dysku.
# machine-drafted, needs native review
queue-clear-ok = Wyczyść kolejkę
# machine-drafted, needs native review
queue-cleared = Kolejka wyczyszczona.
# machine-drafted, needs native review
queue-save-title = Zapisz kolejkę jako playlistę
# machine-drafted, needs native review
queue-default-name = Kolejka
# machine-drafted, needs native review
queue-format-m3u8 = Playlista M3U8
# machine-drafted, needs native review
queue-format-xspf = Playlista XSPF
# machine-drafted, needs native review
queue-saved = Zapisano { $name }.
# machine-drafted, needs native review
queue-save-failed = Nie można zapisać { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Odtwórz następny plik dźwiękowy
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Gdy plik dźwiękowy się skończy, gra następny plik dźwiękowy z folderu.
# machine-drafted, needs native review
settings-viewing-play-repeat = Powtarzanie
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Dla plików dźwiękowych, wideo i kolejki.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Wył.
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Wszystko
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Jedna
# machine-drafted, needs native review
settings-viewing-play-shuffle = Losowo
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Odtwarzaj w losowej kolejności, każdą pozycję raz na rundę.
