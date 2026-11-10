# machine-drafted, needs native review
# Specular — duplikaty i podobne ujęcia (kategorie galerii). Polski.

similar-duplicates = Duplikaty
similar-similar = Podobne ujęcia

## Menu Widok i paleta
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } grupa
    [few] { $groups } grupy
    [many] { $groups } grup
   *[other] { $groups } grupy
}, do zwolnienia { $size }
similar-menu-scanning = { $name } — skanowanie ({ PERCENT($percent) })
similar-menu-none = { $name } — nic nie znaleziono

## Nagłówek
similar-summary = Do przejrzenia: { $groups ->
    [one] { $groups } grupa
    [few] { $groups } grupy
    [many] { $groups } grup
   *[other] { $groups } grupy
}, do odrzucenia: { $files ->
    [one] { $files } plik
    [few] { $files } pliki
    [many] { $files } plików
   *[other] { $files } pliku
}, do zwolnienia: { $size }
similar-progress = Rozstrzygnięto { $resolved } z { $total }
similar-skipped = Pominięto: { $count }
similar-scanning = Wyszukiwanie duplikatów i podobnych ujęć… { $done } z { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Sprawdzanie ostrości… { $done } z { $total }
# machine-drafted, needs native review
# Lista podczas pierwszego skanowania (postęp jest w nagłówku powyżej).
similar-waiting = Grupy pojawią się tutaj po zakończeniu skanowania.
# machine-drafted, needs native review
similar-stopped = Skanowanie zatrzymane: nic nie zgrupowano
similar-stopped-sharpness = Sprawdzanie ostrości zatrzymane: najlepsze ujęcia wybrane na podstawie miniatur
similar-stopped-empty = Skanowanie zostało zatrzymane. Zmierzone dane są zachowane: Skanuj ponownie kontynuuje od tego miejsca.
similar-scan-again = Skanuj ponownie
similar-empty-duplicates = Brak duplikatów w tym folderze.
similar-empty-similar = Brak podobnych ujęć w tym folderze.
similar-all-done = Przejrzano wszystkie grupy.

## Podobne ujęcia: jak bardzo zdjęcia muszą być podobne
similar-sensitivity = Czułość
similar-sensitivity-strict = Ścisła
similar-sensitivity-loose = Luźna

## Stopka listy grup
similar-accept-all = Zaakceptuj wszystkie sugestie
similar-show-skipped = Pokaż pominięte grupy
similar-discard-with = Odrzucanie: { $action }
similar-action-trash = Przenieś do kosza
similar-action-move = Przenieś do folderu…
similar-action-move-to = Przenieś do „{ $folder }”
similar-action-reject = Tylko oznacz jako odrzucone
similar-choose-folder = Wybierz folder na odrzucone pliki…

## Potwierdzenie zbiorcze
similar-confirm = Zaakceptować sugestie dla { $groups ->
    [one] { $groups } grupy
    [few] { $groups } grup
    [many] { $groups } grup
   *[other] { $groups } grupy
}? { $files ->
    [one] { $files } plik
    [few] { $files } pliki
    [many] { $files } plików
   *[other] { $files } pliku
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Zaakceptuj i odrzuć { $files } plik
    [few] Zaakceptuj i odrzuć { $files } pliki
    [many] Zaakceptuj i odrzuć { $files } plików
   *[other] Zaakceptuj i odrzuć { $files } pliku
}
similar-cancel = Anuluj

## Grupa
similar-group-title = Grupa { $index } z { $total }
similar-group-photos = { $count ->
    [one] { $count } zdjęcie
    [few] { $count } zdjęcia
    [many] { $count } zdjęć
   *[other] { $count } zdjęcia
}
similar-to-free = Do zwolnienia: { $size }

## Czynności i stany przeglądu
similar-keep = Zachowaj
similar-discard = Odrzuć
similar-undecided = Nierozstrzygnięte
similar-accept-next = Zaakceptuj i dalej
similar-not-similar = Niepodobne
similar-skip = Pomiń
similar-compare = Porównaj
similar-zoom = { PERCENT(100) }
similar-back = Wstecz
similar-done = Gotowe
similar-suggested-keep = Sugestia: zachowaj
similar-suggested-discard = Sugestia: odrzuć

## Powiadomienia
similar-toast-accepted = { $count ->
    [one] Zaakceptowano { $count } grupę
    [few] Zaakceptowano { $count } grupy
    [many] Zaakceptowano { $count } grup
   *[other] Zaakceptowano { $count } grupy
}
similar-toast-rejected = { $count ->
    [one] Oznaczono { $count } plik jako odrzucony
    [few] Oznaczono { $count } pliki jako odrzucone
    [many] Oznaczono { $count } plików jako odrzucone
   *[other] Oznaczono { $count } pliku jako odrzucone
}
similar-toast-protected = Zdjęcia z oceną, flagą, etykietą, słowami kluczowymi lub edycjami są zawsze zachowywane.
# machine-drafted, needs native review
similar-zoom-already = To zdjęcie jest już wyświetlane w { PERCENT(100) } lub większe

## Dostępność
similar-a11y-member = { $name }: { $state }, { $reason }

## Podpowiedzi klawiszy
similar-hint-list = ↑↓ wybór · Enter przegląd · A akceptuj wszystko · Tab przełącz · Esc zamknij

## Dlaczego zdjęcie jest zachowywane
similar-reason-rated = Ma ocenę
similar-reason-flagged = Oznaczone jako wybrane
similar-reason-labelled = Ma etykietę kolorową
similar-reason-tagged = Ma słowa kluczowe
similar-reason-edited = Ma edycje
similar-reason-original = Oryginał
similar-reason-raw-original = Oryginał RAW
similar-reason-highest-resolution = Najwyższa rozdzielczość
similar-reason-deeper-colour = Większa głębia kolorów
similar-reason-least-compressed = Najmniej skompresowane
similar-reason-has-metadata = Zachowuje metadane
similar-reason-oldest = Najstarsza kopia
similar-reason-sharpest = Najostrzejsze
similar-reason-steadiest = Najmniej poruszone
similar-reason-best-exposure = Najlepsza ekspozycja
similar-reason-least-noise = Najmniej szumu
similar-reason-also-good = Też dobre

## Dlaczego zdjęcie jest sugerowane do odrzucenia
similar-reason-same-file = Identyczny plik
similar-reason-derivative = Kopia pochodna
similar-reason-rendering-of-raw = Wywołanie pliku RAW
similar-reason-smaller-copy = Mniejsza kopia
similar-reason-shallower-colour = Mniejsza głębia kolorów
similar-reason-more-compressed = Bardziej skompresowane
similar-reason-no-metadata = Usunięte metadane
similar-reason-newer-copy = Nowsza kopia
similar-reason-blurrier = Mniej ostre
similar-reason-motion-blur = Poruszone
similar-reason-overexposed = Prześwietlone
similar-reason-underexposed = Niedoświetlone
similar-reason-noisier = Więcej szumu
similar-reason-lower-resolution = Niższa rozdzielczość
