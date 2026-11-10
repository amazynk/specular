# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Album
albums-new = Nyt album…
albums-new-smart = Nyt smart album…
albums-save-search = Gem søgningen som smart album…
albums-smart = Smart album
# The album the photos go to when added without choosing.
albums-target = Tilføjelser havner her
albums-group-count = { $count ->
    [one] { $count } album
   *[other] { $count } album
}
albums-need-library = Album findes i biblioteket. Føj først denne mappe til biblioteket.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Omdøb { $album }
albums-a11y-edit = Rediger { $album }
albums-a11y-delete = Slet albummet { $album }
albums-a11y-rename-group = Omdøb gruppen { $group }
albums-a11y-delete-group = Slet gruppen { $group }

## Sheets
albums-name = Navn
albums-unnamed = Intet navn endnu
albums-new-title = Nyt album
albums-new-smart-title = Nyt smart album
albums-edit-smart-title = Rediger smart album
albums-rename-title = Omdøb album
albums-rename-group-title = Omdøb gruppe
albums-create = Opret
albums-rename = Omdøb
albums-save = Gem
albums-groups-note = Brug / for at lægge det i en gruppe: Kunde X / udvalgte.
albums-name-empty = Skriv et navn.
albums-name-taken = { $album } findes allerede.
albums-name-clash = { $album } er et album og kan derfor ikke også være en gruppe.

## Smart album rules
albums-rules = Regler
albums-rules-note = Reglerne gælder oppefra og ned. Tomme regler springes over.
albums-rules-none = Tilføj mindst én regel.
albums-rule-label = Regel { $number }
albums-rule-empty = Tom regel
albums-rule-add = Tilføj en regel
albums-rule-combine = Kombiner med reglerne ovenfor
albums-rule-first = Fotos, der matcher
albums-rule-and = Og
albums-rule-or = Eller
albums-rule-except = Undtagen

## Adding and removing photos
albums-add-menu = Føj til album…
albums-remove-menu = Fjern fra album
albums-add-title = { $count ->
    [one] Føj { $count } foto til et album
   *[other] Føj { $count } fotos til et album
}
albums-add = Tilføj
albums-add-new = Nyt album
albums-add-list = Album
albums-add-none = Ingen album endnu. Skriv et navn for at oprette et.
albums-add-note = Eller skriv et navn til et nyt album.
albums-added = { $count ->
    [one] Føjede { $count } foto til { $album }.
   *[other] Føjede { $count } fotos til { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } er ikke i biblioteket og blev derfor ikke tilføjet.
   *[other] { $count } er ikke i biblioteket og blev derfor ikke tilføjet.
}
albums-created = Oprettede { $album }.
albums-removed = { $count ->
    [one] Fjernede { $count } foto fra { $album }.
   *[other] Fjernede { $count } fotos fra { $album }.
}
albums-remove = Fjern
albums-remove-confirm = { $count ->
    [one] Fjern { $count } foto fra { $album }?
   *[other] Fjern { $count } fotos fra { $album }?
}
albums-remove-confirm-detail = Filerne bliver på disken og i biblioteket.
albums-delete = Slet
albums-delete-confirm = Slet albummet { $album }?
albums-delete-group-confirm = { $count ->
    [one] Slet gruppen { $group } og dens { $count } album?
   *[other] Slet gruppen { $group } og dens { $count } album?
}
albums-delete-confirm-detail = Fotoene bliver på disken og i biblioteket.
albums-deleted = { $count ->
    [one] Albummet blev slettet.
   *[other] { $count } album blev slettet.
}
albums-failed = Kunne ikke ændre albummene: { $reason }

## Date timeline
albums-timeline = Tidslinje
albums-timeline-description = Vælg en periode for kun at vise dens fotos. Shift+klik vælger flere i træk.
albums-timeline-picked = Viser { $period }
albums-timeline-cleared = Viser alle datoer
