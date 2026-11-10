# Editor metadat: pole IPTC vybraných fotek (galerie, Knihovna, režim třídění a
# prohlížeč).

palette-edit-metadata = Upravit metadata
palette-edit-metadata-help = Upravit název, popisek, autora, práva a místa vybraných fotek.
metaedit-menu = Upravit metadata…
metaedit-title = Metadata
metaedit-close = Zavřít metadata
metaedit-photos = { $count ->
    [one] 1 fotka
    [few] { $count } fotky
    [many] { $count } fotky
   *[other] { $count } fotek
}
metaedit-none-selected = Vyberte fotky, jejichž metadata chcete upravit.
metaedit-reading = Čtou se metadata…
metaedit-mixed = Různé: pište a nastaví se u všech
metaedit-clearing = Vymaže se u všech fotek. Esc je ponechá.
metaedit-keywords-others = některé mají další
metaedit-saved = Uloženo: { $field }.
metaedit-cleared = Vymazáno: { $field }.
metaedit-write-failed = { $count ->
    [one] Metadata se neuložila u 1 fotky: { $error }
    [few] Metadata se neuložila u { $count } fotek: { $error }
    [many] Metadata se neuložila u { $count } fotky: { $error }
   *[other] Metadata se neuložila u { $count } fotek: { $error }
}
metaedit-writes-sidecars = Ukládá se do souborů XMP.
metaedit-writes-files = Ukládá se do souborů JPEG, TIFF a DNG; ostatní soubory dostanou soubory XMP.

metaedit-group-templates = Šablony
metaedit-group-description = Popis
metaedit-group-creator = Autor
metaedit-group-rights = Práva
metaedit-group-shown = Zobrazené místo
metaedit-group-created = Místo pořízení
metaedit-group-workflow = Pracovní postup
metaedit-templates-none = Zatím žádné šablony. Šablona vyplní pole, která často používáte.

metaedit-field-title = Název
metaedit-field-headline = Titulek
metaedit-field-description = Popisek
metaedit-field-keywords = Klíčová slova (oddělená ;)
metaedit-field-persons-shown = Zobrazené osoby (oddělené ;)
metaedit-field-event = Událost
metaedit-field-creator = Autor (oddělení ;)
metaedit-field-creator-job-title = Pracovní pozice autora
metaedit-field-contact-address = Adresa
metaedit-field-contact-city = Město
metaedit-field-contact-region = Kraj nebo stát
metaedit-field-contact-postal-code = PSČ
metaedit-field-contact-country = Země
metaedit-field-contact-phone = Telefon
metaedit-field-contact-email = E-mail
metaedit-field-contact-url = Web
metaedit-field-copyright = Upozornění na autorská práva
metaedit-field-credit = Kredit
metaedit-field-source = Zdroj
metaedit-field-usage-terms = Podmínky použití
metaedit-field-shown-sublocation = Upřesnění místa
metaedit-field-shown-city = Město
metaedit-field-shown-state = Kraj nebo stát
metaedit-field-shown-country = Země
metaedit-field-shown-country-code = Kód země
metaedit-field-created-sublocation = Upřesnění místa
metaedit-field-created-city = Město
metaedit-field-created-state = Kraj nebo stát
metaedit-field-created-country = Země
metaedit-field-created-country-code = Kód země
metaedit-field-instructions = Pokyny
metaedit-field-job-id = ID zakázky

metaedit-template-new = Nová šablona
metaedit-template-edit = Upravit
metaedit-template-remove = Odebrat
metaedit-template-done = Hotovo
metaedit-template-name = Název šablony
metaedit-template-default-name = Šablona { $n }
metaedit-template-title = Šablona
metaedit-template-hint = Prázdná pole se nemění.
metaedit-template-replace-keywords = Nahradit klíčová slova fotek
metaedit-template-applied = { $count ->
    [one] Šablona „{ $name }“ použita na 1 fotku.
    [few] Šablona „{ $name }“ použita na { $count } fotky.
    [many] Šablona „{ $name }“ použita na { $count } fotky.
   *[other] Šablona „{ $name }“ použita na { $count } fotek.
}
metaedit-template-remove-ask = Odebrat šablonu „{ $name }“?
metaedit-template-remove-keep = Fotky si svá metadata ponechají.
metaedit-template-remove-yes = Odebrat
metaedit-template-removed = Šablona „{ $name }“ odebrána.

app-info-edit-metadata = Upravit metadata…

settings-gallery-metadata-into-files = Zapisovat metadata do obrazových souborů
settings-gallery-metadata-into-files-help = Editor metadat zapisuje i do souborů JPEG, TIFF a DNG a nic jiného v nich nemění. Vypnuto: jen soubory XMP.
