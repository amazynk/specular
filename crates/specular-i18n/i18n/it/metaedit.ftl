# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Modifica metadati
palette-edit-metadata-help = Modifica titolo, didascalia, autore, diritti e luoghi delle foto selezionate.
metaedit-menu = Modifica metadati…
metaedit-title = Metadati
metaedit-close = Chiudi metadati
metaedit-photos = { $count ->
    [one] 1 foto
   *[other] { $count } foto
}
metaedit-none-selected = Seleziona delle foto per modificarne i metadati.
metaedit-reading = Lettura dei metadati…
metaedit-mixed = Misto: scrivi per applicare a tutte
metaedit-clearing = Cancellato su tutte le foto. Esc li mantiene.
metaedit-keywords-others = alcune ne hanno altre
metaedit-saved = { $field } salvato.
metaedit-cleared = { $field } cancellato.
metaedit-write-failed = { $count ->
    [one] Metadati non salvati per 1 foto: { $error }
   *[other] Metadati non salvati per { $count } foto: { $error }
}
metaedit-writes-sidecars = Salvato in file XMP collaterali.
metaedit-writes-files = Salvato nei file JPEG, TIFF e DNG; gli altri file ricevono file XMP collaterali.

metaedit-group-templates = Modelli
metaedit-group-description = Descrizione
metaedit-group-creator = Autore
metaedit-group-rights = Diritti
metaedit-group-shown = Luogo raffigurato
metaedit-group-created = Luogo di scatto
metaedit-group-workflow = Flusso di lavoro
metaedit-templates-none = Ancora nessun modello. Un modello compila i campi che usi spesso.

metaedit-field-title = Titolo
metaedit-field-headline = Titolo breve
metaedit-field-description = Didascalia
metaedit-field-keywords = Parole chiave (separate da ;)
metaedit-field-persons-shown = Persone raffigurate (separate da ;)
metaedit-field-event = Evento
metaedit-field-creator = Autore (separati da ;)
metaedit-field-creator-job-title = Qualifica dell’autore
metaedit-field-contact-address = Indirizzo
metaedit-field-contact-city = Città
metaedit-field-contact-region = Stato o provincia
metaedit-field-contact-postal-code = CAP
metaedit-field-contact-country = Paese
metaedit-field-contact-phone = Telefono
metaedit-field-contact-email = E-mail
metaedit-field-contact-url = Sito web
metaedit-field-copyright = Avviso di copyright
metaedit-field-credit = Riga di credito
metaedit-field-source = Fonte
metaedit-field-usage-terms = Condizioni d’uso
metaedit-field-shown-sublocation = Località
metaedit-field-shown-city = Città
metaedit-field-shown-state = Stato o provincia
metaedit-field-shown-country = Paese
metaedit-field-shown-country-code = Codice paese
metaedit-field-created-sublocation = Località
metaedit-field-created-city = Città
metaedit-field-created-state = Stato o provincia
metaedit-field-created-country = Paese
metaedit-field-created-country-code = Codice paese
metaedit-field-instructions = Istruzioni
metaedit-field-job-id = ID lavoro

metaedit-template-new = Nuovo modello
metaedit-template-edit = Modifica
metaedit-template-remove = Rimuovi
metaedit-template-done = Fine
metaedit-template-name = Nome del modello
metaedit-template-default-name = Modello { $n }
metaedit-template-title = Modello
metaedit-template-hint = I campi vuoti non vengono modificati.
metaedit-template-replace-keywords = Sostituisci le parole chiave delle foto
metaedit-template-applied = { $count ->
    [one] «{ $name }» applicato a 1 foto.
   *[other] «{ $name }» applicato a { $count } foto.
}
metaedit-template-remove-ask = Rimuovere il modello «{ $name }»?
metaedit-template-remove-keep = Le foto mantengono i loro metadati.
metaedit-template-remove-yes = Rimuovi
metaedit-template-removed = Modello «{ $name }» rimosso.

app-info-edit-metadata = Modifica metadati…

settings-gallery-metadata-into-files = Scrivi i metadati nei file immagine
settings-gallery-metadata-into-files-help = L’editor dei metadati scrive anche nei file JPEG, TIFF e DNG, senza cambiare nient’altro. Disattivato: solo file XMP collaterali.
