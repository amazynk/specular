# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Metagegevens bewerken
palette-edit-metadata-help = De titel, het bijschrift, de maker, de rechten en de locaties van de geselecteerde foto’s bewerken.
metaedit-menu = Metagegevens bewerken…
metaedit-title = Metagegevens
metaedit-close = Metagegevens sluiten
metaedit-photos = { $count ->
    [one] 1 foto
   *[other] { $count } foto’s
}
metaedit-none-selected = Selecteer foto’s om hun metagegevens te bewerken.
metaedit-reading = Metagegevens lezen…
metaedit-mixed = Gemengd: typ om op alle toe te passen
metaedit-clearing = Gewist op alle foto’s. Esc behoudt ze.
metaedit-keywords-others = sommige hebben er meer
metaedit-saved = { $field } opgeslagen.
metaedit-cleared = { $field } gewist.
metaedit-write-failed = { $count ->
    [one] Metagegevens niet opgeslagen voor 1 foto: { $error }
   *[other] Metagegevens niet opgeslagen voor { $count } foto’s: { $error }
}
metaedit-writes-sidecars = Opgeslagen in XMP-sidecars.
metaedit-writes-files = Opgeslagen in JPEG-, TIFF- en DNG-bestanden; andere bestanden krijgen XMP-sidecars.

metaedit-group-templates = Sjablonen
metaedit-group-description = Beschrijving
metaedit-group-creator = Maker
metaedit-group-rights = Rechten
metaedit-group-shown = Getoonde locatie
metaedit-group-created = Opnamelocatie
metaedit-group-workflow = Werkstroom
metaedit-templates-none = Nog geen sjablonen. Een sjabloon vult velden in die je vaak gebruikt.

metaedit-field-title = Titel
metaedit-field-headline = Kop
metaedit-field-description = Bijschrift
metaedit-field-keywords = Trefwoorden (gescheiden door ;)
metaedit-field-persons-shown = Getoonde personen (gescheiden door ;)
metaedit-field-event = Gebeurtenis
metaedit-field-creator = Maker (gescheiden door ;)
metaedit-field-creator-job-title = Functie van de maker
metaedit-field-contact-address = Adres
metaedit-field-contact-city = Plaats
metaedit-field-contact-region = Staat of provincie
metaedit-field-contact-postal-code = Postcode
metaedit-field-contact-country = Land
metaedit-field-contact-phone = Telefoon
metaedit-field-contact-email = E-mail
metaedit-field-contact-url = Webpagina
metaedit-field-copyright = Copyrightvermelding
metaedit-field-credit = Bronvermelding
metaedit-field-source = Bron
metaedit-field-usage-terms = Gebruiksvoorwaarden
metaedit-field-shown-sublocation = Sublocatie
metaedit-field-shown-city = Plaats
metaedit-field-shown-state = Staat of provincie
metaedit-field-shown-country = Land
metaedit-field-shown-country-code = Landcode
metaedit-field-created-sublocation = Sublocatie
metaedit-field-created-city = Plaats
metaedit-field-created-state = Staat of provincie
metaedit-field-created-country = Land
metaedit-field-created-country-code = Landcode
metaedit-field-instructions = Instructies
metaedit-field-job-id = Opdrachtnummer

metaedit-template-new = Nieuw sjabloon
metaedit-template-edit = Bewerken
metaedit-template-remove = Verwijderen
metaedit-template-done = Klaar
metaedit-template-name = Naam van het sjabloon
metaedit-template-default-name = Sjabloon { $n }
metaedit-template-title = Sjabloon
metaedit-template-hint = Lege velden blijven ongewijzigd.
metaedit-template-replace-keywords = De trefwoorden van de foto’s vervangen
metaedit-template-applied = { $count ->
    [one] ‘{ $name }’ toegepast op 1 foto.
   *[other] ‘{ $name }’ toegepast op { $count } foto’s.
}
metaedit-template-remove-ask = Het sjabloon ‘{ $name }’ verwijderen?
metaedit-template-remove-keep = Foto’s houden hun metagegevens.
metaedit-template-remove-yes = Verwijderen
metaedit-template-removed = Sjabloon ‘{ $name }’ verwijderd.

app-info-edit-metadata = Metagegevens bewerken…

settings-gallery-metadata-into-files = Metagegevens in afbeeldingsbestanden schrijven
settings-gallery-metadata-into-files-help = De metagegevenseditor schrijft ook in JPEG-, TIFF- en DNG-bestanden en wijzigt verder niets. Uit: alleen XMP-sidecars.
