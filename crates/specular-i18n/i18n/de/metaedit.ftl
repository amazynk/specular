# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Metadaten bearbeiten
palette-edit-metadata-help = Titel, Beschreibung, Urheber, Rechte und Orte der ausgewählten Fotos bearbeiten.
metaedit-menu = Metadaten bearbeiten…
metaedit-title = Metadaten
metaedit-close = Metadaten schließen
metaedit-photos = { $count ->
    [one] 1 Foto
   *[other] { $count } Fotos
}
metaedit-none-selected = Wählen Sie Fotos aus, um ihre Metadaten zu bearbeiten.
metaedit-reading = Metadaten werden gelesen…
metaedit-mixed = Gemischt: Eingabe gilt für alle
metaedit-clearing = Auf allen Fotos geleert. Esc behält sie.
metaedit-keywords-others = einige haben mehr
metaedit-saved = { $field } gespeichert.
metaedit-cleared = { $field } geleert.
metaedit-write-failed = { $count ->
    [one] Metadaten für 1 Foto nicht gespeichert: { $error }
   *[other] Metadaten für { $count } Fotos nicht gespeichert: { $error }
}
metaedit-writes-sidecars = In XMP-Filialdateien gespeichert.
metaedit-writes-files = In JPEG-, TIFF- und DNG-Dateien gespeichert; andere Dateien erhalten XMP-Filialdateien.

metaedit-group-templates = Vorlagen
metaedit-group-description = Beschreibung
metaedit-group-creator = Urheber
metaedit-group-rights = Rechte
metaedit-group-shown = Gezeigter Ort
metaedit-group-created = Aufnahmeort
metaedit-group-workflow = Arbeitsablauf
metaedit-templates-none = Noch keine Vorlagen. Eine Vorlage füllt oft benutzte Felder aus.

metaedit-field-title = Titel
metaedit-field-headline = Überschrift
metaedit-field-description = Beschreibung
metaedit-field-keywords = Stichwörter (getrennt durch ;)
metaedit-field-persons-shown = Gezeigte Personen (getrennt durch ;)
metaedit-field-event = Ereignis
metaedit-field-creator = Urheber (getrennt durch ;)
metaedit-field-creator-job-title = Position des Urhebers
metaedit-field-contact-address = Adresse
metaedit-field-contact-city = Ort
metaedit-field-contact-region = Bundesland oder Region
metaedit-field-contact-postal-code = Postleitzahl
metaedit-field-contact-country = Land
metaedit-field-contact-phone = Telefon
metaedit-field-contact-email = E-Mail
metaedit-field-contact-url = Webseite
metaedit-field-copyright = Copyright-Vermerk
metaedit-field-credit = Bildnachweis
metaedit-field-source = Quelle
metaedit-field-usage-terms = Nutzungsbedingungen
metaedit-field-shown-sublocation = Ortsdetail
metaedit-field-shown-city = Ort
metaedit-field-shown-state = Bundesland oder Region
metaedit-field-shown-country = Land
metaedit-field-shown-country-code = Ländercode
metaedit-field-created-sublocation = Ortsdetail
metaedit-field-created-city = Ort
metaedit-field-created-state = Bundesland oder Region
metaedit-field-created-country = Land
metaedit-field-created-country-code = Ländercode
metaedit-field-instructions = Anweisungen
metaedit-field-job-id = Auftragskennung

metaedit-template-new = Neue Vorlage
metaedit-template-edit = Bearbeiten
metaedit-template-remove = Entfernen
metaedit-template-done = Fertig
metaedit-template-name = Name der Vorlage
metaedit-template-default-name = Vorlage { $n }
metaedit-template-title = Vorlage
metaedit-template-hint = Leere Felder bleiben unverändert.
metaedit-template-replace-keywords = Stichwörter der Fotos ersetzen
metaedit-template-applied = { $count ->
    [one] „{ $name }“ auf 1 Foto angewendet.
   *[other] „{ $name }“ auf { $count } Fotos angewendet.
}
metaedit-template-remove-ask = Die Vorlage „{ $name }“ entfernen?
metaedit-template-remove-keep = Die Fotos behalten ihre Metadaten.
metaedit-template-remove-yes = Entfernen
metaedit-template-removed = Vorlage „{ $name }“ entfernt.

app-info-edit-metadata = Metadaten bearbeiten…

settings-gallery-metadata-into-files = Metadaten in Bilddateien schreiben
settings-gallery-metadata-into-files-help = Der Metadaten-Editor schreibt auch in JPEG-, TIFF- und DNG-Dateien und ändert sonst nichts daran. Aus: nur XMP-Filialdateien.
