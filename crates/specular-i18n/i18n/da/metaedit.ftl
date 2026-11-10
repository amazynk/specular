# Metadataeditoren: IPTC-felter for de valgte fotos (galleriet, Biblioteket, sorteringstilstand og
# fremviseren).

palette-edit-metadata = Rediger metadata
palette-edit-metadata-help = Rediger titel, billedtekst, ophavsmand, rettigheder og steder for de valgte fotos.
metaedit-menu = Rediger metadata…
metaedit-title = Metadata
metaedit-close = Luk metadata
metaedit-photos = { $count ->
    [one] 1 foto
   *[other] { $count } fotos
}
metaedit-none-selected = Vælg fotos for at redigere deres metadata.
metaedit-reading = Læser metadata…
metaedit-mixed = Blandet: skriv for at angive på alle
metaedit-clearing = Ryddes på alle fotos. Esc beholder dem.
metaedit-keywords-others = nogle har flere
metaedit-saved = Gemte { $field }.
metaedit-cleared = Ryddede { $field }.
metaedit-write-failed = { $count ->
    [one] Metadata ikke gemt for 1 foto: { $error }
   *[other] Metadata ikke gemt for { $count } fotos: { $error }
}
metaedit-writes-sidecars = Gemmes i XMP-sidevognsfiler.
metaedit-writes-files = Gemmes i JPEG-, TIFF- og DNG-filer; andre filer får XMP-sidevognsfiler.

metaedit-group-templates = Skabeloner
metaedit-group-description = Beskrivelse
metaedit-group-creator = Ophavsmand
metaedit-group-rights = Rettigheder
metaedit-group-shown = Vist sted
metaedit-group-created = Optagelsessted
metaedit-group-workflow = Arbejdsgang
metaedit-templates-none = Ingen skabeloner endnu. En skabelon udfylder felter, du ofte bruger.

metaedit-field-title = Titel
metaedit-field-headline = Overskrift
metaedit-field-description = Billedtekst
metaedit-field-keywords = Nøgleord (adskilt med ;)
metaedit-field-persons-shown = Viste personer (adskilt med ;)
metaedit-field-event = Begivenhed
metaedit-field-creator = Ophavsmand (adskilt med ;)
metaedit-field-creator-job-title = Ophavsmandens stilling
metaedit-field-contact-address = Adresse
metaedit-field-contact-city = By
metaedit-field-contact-region = Region eller delstat
metaedit-field-contact-postal-code = Postnummer
metaedit-field-contact-country = Land
metaedit-field-contact-phone = Telefon
metaedit-field-contact-email = E-mail
metaedit-field-contact-url = Websted
metaedit-field-copyright = Copyrightmeddelelse
metaedit-field-credit = Kreditering
metaedit-field-source = Kilde
metaedit-field-usage-terms = Brugsvilkår
metaedit-field-shown-sublocation = Delområde
metaedit-field-shown-city = By
metaedit-field-shown-state = Region eller delstat
metaedit-field-shown-country = Land
metaedit-field-shown-country-code = Landekode
metaedit-field-created-sublocation = Delområde
metaedit-field-created-city = By
metaedit-field-created-state = Region eller delstat
metaedit-field-created-country = Land
metaedit-field-created-country-code = Landekode
metaedit-field-instructions = Instruktioner
metaedit-field-job-id = Job-ID

metaedit-template-new = Ny skabelon
metaedit-template-edit = Rediger
metaedit-template-remove = Fjern
metaedit-template-done = Færdig
metaedit-template-name = Skabelonens navn
metaedit-template-default-name = Skabelon { $n }
metaedit-template-title = Skabelon
metaedit-template-hint = Tomme felter lades uændrede.
metaedit-template-replace-keywords = Erstat fotoenes nøgleord
metaedit-template-applied = { $count ->
    [one] Anvendte »{ $name }« på 1 foto.
   *[other] Anvendte »{ $name }« på { $count } fotos.
}
metaedit-template-remove-ask = Fjern skabelonen »{ $name }«?
metaedit-template-remove-keep = Fotos beholder deres metadata.
metaedit-template-remove-yes = Fjern
metaedit-template-removed = Fjernede skabelonen »{ $name }«.

app-info-edit-metadata = Rediger metadata…

settings-gallery-metadata-into-files = Skriv metadata i billedfilerne
settings-gallery-metadata-into-files-help = Metadataeditoren skriver også i JPEG-, TIFF- og DNG-filer uden at ændre andet i dem. Fra: kun XMP-sidevognsfiler.
