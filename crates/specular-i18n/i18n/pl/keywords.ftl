# The keyword panel (gallery, Library, cull mode).

palette-keywords = Słowa kluczowe
palette-keywords-help = Dodaj słowa kluczowe do zaznaczonych zdjęć.
keywords-title = Słowa kluczowe
keywords-close = Zamknij słowa kluczowe
keywords-photos = { $count ->
    [one] 1 zdjęcie
    [few] { $count } zdjęcia
    [many] { $count } zdjęć
   *[other] { $count } zdjęcia
}
keywords-field = Dodaj słowo kluczowe
keywords-synonyms-of = Synonimy słowa { $keyword }, oddzielone przecinkami
keywords-none-selected = Zaznacz zdjęcia, aby dodać im słowa kluczowe.
keywords-reading = Odczytywanie słów kluczowych…
keywords-on-photos = Na tych zdjęciach
keywords-on-none = Brak słów kluczowych.
keywords-suggestions = Propozycje
keywords-recent = Ostatnio używane
keywords-list = Lista słów kluczowych
keywords-list-empty = Dodane słowa kluczowe pojawią się tutaj. Możesz też zaimportować listę z Lightrooma.
keywords-create = Dodaj „{ $keyword }”
keywords-count = { $count } z { $total }
keywords-private = Prywatne
keywords-private-mark = prywatne
keywords-synonyms = Synonimy
keywords-forget = Usuń z listy
keywords-import = Importuj listę…
keywords-export = Eksportuj listę…
keywords-forget-ask = Usunąć z listy „{ $keyword }” i słowa kluczowe pod nim?
keywords-forget-keep = Zdjęcia zachowają swoje słowa kluczowe.
keywords-forget-yes = Usuń
keywords-forgotten = { $count ->
    [one] Usunięto z listy { $keyword }.
    [few] Usunięto z listy { $keyword } i słowa kluczowe pod nim (łącznie { $count }).
    [many] Usunięto z listy { $keyword } i słowa kluczowe pod nim (łącznie { $count }).
   *[other] Usunięto z listy { $keyword } i słowa kluczowe pod nim (łącznie { $count }).
}
keywords-added = Dodano { $keyword }.
keywords-removed = Usunięto { $keyword }.
keywords-imported = { $count ->
    [one] 1 nowe słowo kluczowe z { $file }.
    [few] { $count } nowe słowa kluczowe z { $file }.
    [many] { $count } nowych słów kluczowych z { $file }.
   *[other] { $count } nowego słowa kluczowego z { $file }.
}
keywords-import-failed = Nie można odczytać { $file }: { $error }
keywords-exported = Zapisano listę słów kluczowych jako { $file }.
keywords-export-failed = Nie można zapisać listy słów kluczowych: { $error }
keywords-write-failed = { $count ->
    [one] Nie zapisano słów kluczowych dla 1 zdjęcia: { $error }
    [few] Nie zapisano słów kluczowych dla { $count } zdjęć: { $error }
    [many] Nie zapisano słów kluczowych dla { $count } zdjęć: { $error }
   *[other] Nie zapisano słów kluczowych dla { $count } zdjęcia: { $error }
}
keywords-dialog-import = Importuj listę słów kluczowych z Lightrooma
keywords-dialog-export = Eksportuj listę słów kluczowych
keywords-file-kind = Lista słów kluczowych
keywords-file-name = Słowa kluczowe
