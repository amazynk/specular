# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Edit metadata
palette-edit-metadata-help = Edit the title, caption, creator, rights and locations of the selected photos.
metaedit-menu = Edit metadata…
metaedit-title = Metadata
metaedit-close = Close metadata
metaedit-photos = { $count ->
    [one] 1 photo
   *[other] { $count } photos
}
metaedit-none-selected = Select photos to edit their metadata.
metaedit-reading = Reading metadata…
metaedit-mixed = Mixed: type to set on all
metaedit-clearing = Cleared on all photos. Esc keeps them.
metaedit-keywords-others = some have more
metaedit-saved = Saved { $field }.
metaedit-cleared = Cleared { $field }.
metaedit-write-failed = { $count ->
    [one] Metadata not saved for 1 photo: { $error }
   *[other] Metadata not saved for { $count } photos: { $error }
}
metaedit-writes-sidecars = Saved in XMP sidecars.
metaedit-writes-files = Saved in JPEG, TIFF and DNG files; other files get XMP sidecars.

metaedit-group-templates = Templates
metaedit-group-description = Description
metaedit-group-creator = Creator
metaedit-group-rights = Rights
metaedit-group-shown = Location shown
metaedit-group-created = Location created
metaedit-group-workflow = Workflow
metaedit-templates-none = No templates yet. A template fills in fields you use often.

metaedit-field-title = Title
metaedit-field-headline = Headline
metaedit-field-description = Caption
metaedit-field-keywords = Keywords (separated by ;)
metaedit-field-persons-shown = Persons shown (separated by ;)
metaedit-field-event = Event
metaedit-field-creator = Creator (separated by ;)
metaedit-field-creator-job-title = Creator’s job title
metaedit-field-contact-address = Address
metaedit-field-contact-city = City
metaedit-field-contact-region = State or province
metaedit-field-contact-postal-code = Postal code
metaedit-field-contact-country = Country
metaedit-field-contact-phone = Phone
metaedit-field-contact-email = Email
metaedit-field-contact-url = Website
metaedit-field-copyright = Copyright notice
metaedit-field-credit = Credit line
metaedit-field-source = Source
metaedit-field-usage-terms = Usage terms
metaedit-field-shown-sublocation = Sublocation
metaedit-field-shown-city = City
metaedit-field-shown-state = State or province
metaedit-field-shown-country = Country
metaedit-field-shown-country-code = Country code
metaedit-field-created-sublocation = Sublocation
metaedit-field-created-city = City
metaedit-field-created-state = State or province
metaedit-field-created-country = Country
metaedit-field-created-country-code = Country code
metaedit-field-instructions = Instructions
metaedit-field-job-id = Job ID

metaedit-template-new = New template
metaedit-template-edit = Edit
metaedit-template-remove = Remove
metaedit-template-done = Done
metaedit-template-name = Template name
metaedit-template-default-name = Template { $n }
metaedit-template-title = Template
metaedit-template-hint = Empty fields are left alone.
metaedit-template-replace-keywords = Replace the photos’ keywords
metaedit-template-applied = { $count ->
    [one] Applied “{ $name }” to 1 photo.
   *[other] Applied “{ $name }” to { $count } photos.
}
metaedit-template-remove-ask = Remove the template “{ $name }”?
metaedit-template-remove-keep = Photos keep their metadata.
metaedit-template-remove-yes = Remove
metaedit-template-removed = Removed the template “{ $name }”.

app-info-edit-metadata = Edit metadata…

settings-gallery-metadata-into-files = Write metadata into image files
settings-gallery-metadata-into-files-help = The metadata editor also writes into JPEG, TIFF and DNG files, changing nothing else in them. Off: XMP sidecars only.
