# machine-drafted, needs native review
# Specular — Praca w tle: wskaźnik widoczny, gdy coś działa w tle, i panel, który otwiera. Polski.

## Panel

work-title = Praca w tle
work-show = Pokaż pracę w tle
work-empty = Nic nie działa w tle.

## Zadania

work-kind-library = Biblioteka zdjęć
work-kind-ai-search = Wyszukiwanie AI
work-kind-faces = Osoby
work-kind-text = Tekst na zdjęciach
work-kind-culling = Oceny do selekcji
work-kind-download = Pobieranie modelu AI
work-kind-similar = Podobne ujęcia

## Kroki

work-step-finding = Wyszukiwanie zdjęć
work-step-reading = Odczytywanie szczegółów zdjęć
work-step-signatures = Porównywanie zdjęć
work-step-grouping = Grupowanie podobnych ujęć
work-step-text-index = Tworzenie indeksu wyszukiwania
work-step-embedding = Indeksowanie zdjęć do wyszukiwania AI
work-step-switching = Ponowne indeksowanie dla nowego rozmiaru modelu
work-step-faces = Wyszukiwanie twarzy
work-step-ocr = Odczytywanie tekstu na zdjęciach
work-step-scoring = Ocena ostrości i ekspozycji
work-step-downloading = Pobieranie
work-step-focus = Sprawdzanie ostrości
work-step-item = { $step } · { $item }

## Postęp

work-count-photos = { $done } z { $total } { $count ->
    [one] zdjęcia
    [few] zdjęć
    [many] zdjęć
   *[other] zdjęcia
}
work-count = { $done } z { $total }
work-count-left = { $count } · pozostało { $left }
work-so-far-photos = Na razie { $done } { $count ->
    [one] zdjęcie
    [few] zdjęcia
    [many] zdjęć
   *[other] zdjęcia
}
work-so-far = Na razie { $done }

## Pozostały czas

work-eta-estimating = Szacowanie…
work-eta-seconds = Pozostało około { $n ->
    [one] { $n } sekundy
    [few] { $n } sekund
    [many] { $n } sekund
   *[other] { $n } sekundy
}
work-eta-minutes = Pozostało około { $n ->
    [one] { $n } minuty
    [few] { $n } minut
    [many] { $n } minut
   *[other] { $n } minuty
}
work-eta-hours = Pozostało około { $h ->
    [one] { $h } godziny
    [few] { $h } godzin
    [many] { $h } godzin
   *[other] { $h } godziny
}
work-eta-hours-minutes = Pozostało około { $h ->
    [one] { $h } godziny
    [few] { $h } godzin
    [many] { $h } godzin
   *[other] { $h } godziny
} i { $m ->
    [one] { $m } minuty
    [few] { $m } minut
    [many] { $m } minut
   *[other] { $m } minuty
}

## Dlaczego zadanie czeka

work-paused-browsing = Wstrzymano podczas przeglądania
work-paused-battery = Wstrzymano przy zasilaniu z baterii
work-paused-low-power = Wstrzymano w trybie oszczędzania energii
work-paused-heat = Wstrzymano, dopóki komputer jest gorący
work-paused-memory = Wstrzymano, dopóki brakuje pamięci
work-paused-user = Wstrzymano

## Co dalej

work-then = Następnie: { $steps }

## Przyciski

work-pause = Wstrzymaj
work-resume = Wznów
work-settings = Ustawienia…

## Wskaźnik

work-chip = { $job } · { $done } z { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Praca w tle: { $status }

## Komunikaty

work-started = { $job }: rozpoczęto w tle
work-finished = { $job }: zakończono
