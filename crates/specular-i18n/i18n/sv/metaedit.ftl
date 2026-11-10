# Metadataredigeraren: IPTC-fält för de markerade bilderna (galleriet, Biblioteket, gallringsläget och
# visaren).

palette-edit-metadata = Redigera metadata
palette-edit-metadata-help = Redigera titel, bildtext, skapare, rättigheter och platser för de markerade bilderna.
metaedit-menu = Redigera metadata…
metaedit-title = Metadata
metaedit-close = Stäng metadata
metaedit-photos = { $count ->
    [one] 1 bild
   *[other] { $count } bilder
}
metaedit-none-selected = Markera bilder för att redigera deras metadata.
metaedit-reading = Läser metadata…
metaedit-mixed = Blandat: skriv för att ange på alla
metaedit-clearing = Rensas på alla bilder. Esc behåller dem.
metaedit-keywords-others = vissa har fler
metaedit-saved = Sparade { $field }.
metaedit-cleared = Rensade { $field }.
metaedit-write-failed = { $count ->
    [one] Metadata sparades inte för 1 bild: { $error }
   *[other] Metadata sparades inte för { $count } bilder: { $error }
}
metaedit-writes-sidecars = Sparas i XMP-sidofiler.
metaedit-writes-files = Sparas i JPEG-, TIFF- och DNG-filer; andra filer får XMP-sidofiler.

metaedit-group-templates = Mallar
metaedit-group-description = Beskrivning
metaedit-group-creator = Skapare
metaedit-group-rights = Rättigheter
metaedit-group-shown = Plats som visas
metaedit-group-created = Plats där bilden togs
metaedit-group-workflow = Arbetsflöde
metaedit-templates-none = Inga mallar än. En mall fyller i fält du ofta använder.

metaedit-field-title = Titel
metaedit-field-headline = Rubrik
metaedit-field-description = Bildtext
metaedit-field-keywords = Nyckelord (avgränsade med ;)
metaedit-field-persons-shown = Personer som visas (avgränsade med ;)
metaedit-field-event = Händelse
metaedit-field-creator = Skapare (avgränsade med ;)
metaedit-field-creator-job-title = Skaparens befattning
metaedit-field-contact-address = Adress
metaedit-field-contact-city = Ort
metaedit-field-contact-region = Län eller region
metaedit-field-contact-postal-code = Postnummer
metaedit-field-contact-country = Land
metaedit-field-contact-phone = Telefon
metaedit-field-contact-email = E-post
metaedit-field-contact-url = Webbplats
metaedit-field-copyright = Upphovsrättsmeddelande
metaedit-field-credit = Krediteringsrad
metaedit-field-source = Källa
metaedit-field-usage-terms = Användningsvillkor
metaedit-field-shown-sublocation = Delplats
metaedit-field-shown-city = Ort
metaedit-field-shown-state = Län eller region
metaedit-field-shown-country = Land
metaedit-field-shown-country-code = Landskod
metaedit-field-created-sublocation = Delplats
metaedit-field-created-city = Ort
metaedit-field-created-state = Län eller region
metaedit-field-created-country = Land
metaedit-field-created-country-code = Landskod
metaedit-field-instructions = Instruktioner
metaedit-field-job-id = Jobb-ID

metaedit-template-new = Ny mall
metaedit-template-edit = Redigera
metaedit-template-remove = Ta bort
metaedit-template-done = Klar
metaedit-template-name = Mallens namn
metaedit-template-default-name = Mall { $n }
metaedit-template-title = Mall
metaedit-template-hint = Tomma fält lämnas orörda.
metaedit-template-replace-keywords = Ersätt bildernas nyckelord
metaedit-template-applied = { $count ->
    [one] Använde ”{ $name }” på 1 bild.
   *[other] Använde ”{ $name }” på { $count } bilder.
}
metaedit-template-remove-ask = Ta bort mallen ”{ $name }”?
metaedit-template-remove-keep = Bilderna behåller sina metadata.
metaedit-template-remove-yes = Ta bort
metaedit-template-removed = Tog bort mallen ”{ $name }”.

app-info-edit-metadata = Redigera metadata…

settings-gallery-metadata-into-files = Skriv metadata i bildfilerna
settings-gallery-metadata-into-files-help = Metadataredigeraren skriver även i JPEG-, TIFF- och DNG-filer utan att ändra något annat i dem. Av: endast XMP-sidofiler.
