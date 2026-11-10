# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Album
albums-new = Nytt album…
albums-new-smart = Nytt smart album…
albums-save-search = Spara sökningen som smart album…
albums-smart = Smart album
# The album the photos go to when added without choosing.
albums-target = Tillägg hamnar här
albums-group-count = { $count ->
    [one] { $count } album
   *[other] { $count } album
}
albums-need-library = Album finns i biblioteket. Lägg först till den här mappen i biblioteket.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Byt namn på { $album }
albums-a11y-edit = Redigera { $album }
albums-a11y-delete = Radera albumet { $album }
albums-a11y-rename-group = Byt namn på gruppen { $group }
albums-a11y-delete-group = Radera gruppen { $group }

## Sheets
albums-name = Namn
albums-unnamed = Inget namn än
albums-new-title = Nytt album
albums-new-smart-title = Nytt smart album
albums-edit-smart-title = Redigera smart album
albums-rename-title = Byt namn på album
albums-rename-group-title = Byt namn på grupp
albums-create = Skapa
albums-rename = Byt namn
albums-save = Spara
albums-groups-note = Använd / för att lägga det i en grupp: Kund X / urval.
albums-name-empty = Skriv ett namn.
albums-name-taken = { $album } finns redan.
albums-name-clash = { $album } är ett album och kan därför inte också vara en grupp.

## Smart album rules
albums-rules = Regler
albums-rules-note = Reglerna gäller uppifrån och ned. Tomma regler hoppas över.
albums-rules-none = Lägg till minst en regel.
albums-rule-label = Regel { $number }
albums-rule-empty = Tom regel
albums-rule-add = Lägg till en regel
albums-rule-combine = Kombinera med reglerna ovanför
albums-rule-first = Bilder som matchar
albums-rule-and = Och
albums-rule-or = Eller
albums-rule-except = Utom

## Adding and removing photos
albums-add-menu = Lägg till i album…
albums-remove-menu = Ta bort från album
albums-add-title = { $count ->
    [one] Lägg till { $count } bild i ett album
   *[other] Lägg till { $count } bilder i ett album
}
albums-add = Lägg till
albums-add-new = Nytt album
albums-add-list = Album
albums-add-none = Inga album än. Skriv ett namn för att skapa ett.
albums-add-note = Eller skriv ett namn på ett nytt album.
albums-added = { $count ->
    [one] Lade till { $count } bild i { $album }.
   *[other] Lade till { $count } bilder i { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } finns inte i biblioteket och lades därför inte till.
   *[other] { $count } finns inte i biblioteket och lades därför inte till.
}
albums-created = Skapade { $album }.
albums-removed = { $count ->
    [one] Tog bort { $count } bild från { $album }.
   *[other] Tog bort { $count } bilder från { $album }.
}
albums-remove = Ta bort
albums-remove-confirm = { $count ->
    [one] Ta bort { $count } bild från { $album }?
   *[other] Ta bort { $count } bilder från { $album }?
}
albums-remove-confirm-detail = Filerna ligger kvar på disken och i biblioteket.
albums-delete = Radera
albums-delete-confirm = Radera albumet { $album }?
albums-delete-group-confirm = { $count ->
    [one] Radera gruppen { $group } och dess { $count } album?
   *[other] Radera gruppen { $group } och dess { $count } album?
}
albums-delete-confirm-detail = Bilderna ligger kvar på disken och i biblioteket.
albums-deleted = { $count ->
    [one] Albumet raderades.
   *[other] { $count } album raderades.
}
albums-failed = Kunde inte ändra albumen: { $reason }

## Date timeline
albums-timeline = Tidslinje
albums-timeline-description = Välj en period för att bara visa dess bilder. Shift+klick väljer flera i följd.
albums-timeline-picked = Visar { $period }
albums-timeline-cleared = Visar alla datum
