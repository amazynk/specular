# Edytor metadanych: pola IPTC zaznaczonych zdjęć (galeria, Biblioteka, tryb selekcji i
# przeglądarka).

palette-edit-metadata = Edytuj metadane
palette-edit-metadata-help = Edytuj tytuł, podpis, autora, prawa i lokalizacje zaznaczonych zdjęć.
metaedit-menu = Edytuj metadane…
metaedit-title = Metadane
metaedit-close = Zamknij metadane
metaedit-photos = { $count ->
    [one] 1 zdjęcie
    [few] { $count } zdjęcia
    [many] { $count } zdjęć
   *[other] { $count } zdjęcia
}
metaedit-none-selected = Zaznacz zdjęcia, aby edytować ich metadane.
metaedit-reading = Odczytywanie metadanych…
metaedit-mixed = Różne: wpisz, aby ustawić dla wszystkich
metaedit-clearing = Wyczyszczone we wszystkich zdjęciach. Esc je zachowa.
metaedit-keywords-others = niektóre mają więcej
metaedit-saved = Zapisano: { $field }.
metaedit-cleared = Wyczyszczono: { $field }.
metaedit-write-failed = { $count ->
    [one] Nie zapisano metadanych dla 1 zdjęcia: { $error }
    [few] Nie zapisano metadanych dla { $count } zdjęć: { $error }
    [many] Nie zapisano metadanych dla { $count } zdjęć: { $error }
   *[other] Nie zapisano metadanych dla { $count } zdjęcia: { $error }
}
metaedit-writes-sidecars = Zapisywane w plikach XMP obok zdjęć.
metaedit-writes-files = Zapisywane w plikach JPEG, TIFF i DNG; pozostałe pliki dostają pliki XMP obok.

metaedit-group-templates = Szablony
metaedit-group-description = Opis
metaedit-group-creator = Autor
metaedit-group-rights = Prawa
metaedit-group-shown = Pokazane miejsce
metaedit-group-created = Miejsce wykonania
metaedit-group-workflow = Obieg pracy
metaedit-templates-none = Brak szablonów. Szablon wypełnia pola, których często używasz.

metaedit-field-title = Tytuł
metaedit-field-headline = Nagłówek
metaedit-field-description = Podpis
metaedit-field-keywords = Słowa kluczowe (oddzielone ;)
metaedit-field-persons-shown = Pokazane osoby (oddzielone ;)
metaedit-field-event = Wydarzenie
metaedit-field-creator = Autor (oddzieleni ;)
metaedit-field-creator-job-title = Stanowisko autora
metaedit-field-contact-address = Adres
metaedit-field-contact-city = Miasto
metaedit-field-contact-region = Województwo lub region
metaedit-field-contact-postal-code = Kod pocztowy
metaedit-field-contact-country = Kraj
metaedit-field-contact-phone = Telefon
metaedit-field-contact-email = E-mail
metaedit-field-contact-url = Witryna
metaedit-field-copyright = Informacja o prawach autorskich
metaedit-field-credit = Podpis autorski
metaedit-field-source = Źródło
metaedit-field-usage-terms = Warunki użytkowania
metaedit-field-shown-sublocation = Dokładne miejsce
metaedit-field-shown-city = Miasto
metaedit-field-shown-state = Województwo lub region
metaedit-field-shown-country = Kraj
metaedit-field-shown-country-code = Kod kraju
metaedit-field-created-sublocation = Dokładne miejsce
metaedit-field-created-city = Miasto
metaedit-field-created-state = Województwo lub region
metaedit-field-created-country = Kraj
metaedit-field-created-country-code = Kod kraju
metaedit-field-instructions = Instrukcje
metaedit-field-job-id = Identyfikator zlecenia

metaedit-template-new = Nowy szablon
metaedit-template-edit = Edytuj
metaedit-template-remove = Usuń
metaedit-template-done = Gotowe
metaedit-template-name = Nazwa szablonu
metaedit-template-default-name = Szablon { $n }
metaedit-template-title = Szablon
metaedit-template-hint = Puste pola pozostają bez zmian.
metaedit-template-replace-keywords = Zastąp słowa kluczowe zdjęć
metaedit-template-applied = { $count ->
    [one] Zastosowano „{ $name }” do 1 zdjęcia.
    [few] Zastosowano „{ $name }” do { $count } zdjęć.
    [many] Zastosowano „{ $name }” do { $count } zdjęć.
   *[other] Zastosowano „{ $name }” do { $count } zdjęcia.
}
metaedit-template-remove-ask = Usunąć szablon „{ $name }”?
metaedit-template-remove-keep = Zdjęcia zachowają swoje metadane.
metaedit-template-remove-yes = Usuń
metaedit-template-removed = Usunięto szablon „{ $name }”.

app-info-edit-metadata = Edytuj metadane…

settings-gallery-metadata-into-files = Zapisuj metadane w plikach obrazów
settings-gallery-metadata-into-files-help = Edytor metadanych zapisuje też w plikach JPEG, TIFF i DNG, nie zmieniając w nich niczego innego. Wyłączone: tylko pliki XMP obok zdjęć.
