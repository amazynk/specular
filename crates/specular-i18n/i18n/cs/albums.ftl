# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Alba
albums-new = Nové album…
albums-new-smart = Nové chytré album…
albums-save-search = Uložit hledání jako chytré album…
albums-smart = Chytré album
# The album the photos go to when added without choosing.
albums-target = Sem se přidává
albums-group-count = { $count ->
    [one] { $count } album
    [few] { $count } alba
    [many] { $count } alba
   *[other] { $count } alb
}
albums-need-library = Alba jsou v knihovně. Nejdřív přidejte tuto složku do knihovny.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Přejmenovat { $album }
albums-a11y-edit = Upravit { $album }
albums-a11y-delete = Smazat album { $album }
albums-a11y-rename-group = Přejmenovat skupinu { $group }
albums-a11y-delete-group = Smazat skupinu { $group }

## Sheets
albums-name = Název
albums-unnamed = Zatím bez názvu
albums-new-title = Nové album
albums-new-smart-title = Nové chytré album
albums-edit-smart-title = Upravit chytré album
albums-rename-title = Přejmenovat album
albums-rename-group-title = Přejmenovat skupinu
albums-create = Vytvořit
albums-rename = Přejmenovat
albums-save = Uložit
albums-groups-note = Lomítkem / ho dáte do skupiny: Klient X / výběr.
albums-name-empty = Zadejte název.
albums-name-taken = { $album } už existuje.
albums-name-clash = { $album } je album, takže nemůže být zároveň skupinou.

## Smart album rules
albums-rules = Pravidla
albums-rules-note = Pravidla platí shora dolů. Prázdná pravidla se vynechají.
albums-rules-none = Přidejte aspoň jedno pravidlo.
albums-rule-label = Pravidlo { $number }
albums-rule-empty = Prázdné pravidlo
albums-rule-add = Přidat pravidlo
albums-rule-combine = Spojit s pravidly výše
albums-rule-first = Fotky, které odpovídají
albums-rule-and = A
albums-rule-or = Nebo
albums-rule-except = Kromě

## Adding and removing photos
albums-add-menu = Přidat do alba…
albums-remove-menu = Odebrat z alba
albums-add-title = { $count ->
    [one] Přidat { $count } fotku do alba
    [few] Přidat { $count } fotky do alba
    [many] Přidat { $count } fotky do alba
   *[other] Přidat { $count } fotek do alba
}
albums-add = Přidat
albums-add-new = Nové album
albums-add-list = Alba
albums-add-none = Zatím žádná alba. Zadejte název a nějaké vytvořte.
albums-add-note = Nebo zadejte název nového alba.
albums-added = { $count ->
    [one] Do { $album } přidána { $count } fotka.
    [few] Do { $album } přidány { $count } fotky.
    [many] Do { $album } přidáno { $count } fotky.
   *[other] Do { $album } přidáno { $count } fotek.
}
albums-added-missing = { $count ->
    [one] { $count } není v knihovně, a proto nebyla přidána.
    [few] { $count } nejsou v knihovně, a proto nebyly přidány.
    [many] { $count } není v knihovně, a proto nebylo přidáno.
   *[other] { $count } není v knihovně, a proto nebylo přidáno.
}
albums-created = { $album } vytvořeno.
albums-removed = { $count ->
    [one] Z { $album } odebrána { $count } fotka.
    [few] Z { $album } odebrány { $count } fotky.
    [many] Z { $album } odebráno { $count } fotky.
   *[other] Z { $album } odebráno { $count } fotek.
}
albums-remove = Odebrat
albums-remove-confirm = { $count ->
    [one] Odebrat { $count } fotku z { $album }?
    [few] Odebrat { $count } fotky z { $album }?
    [many] Odebrat { $count } fotky z { $album }?
   *[other] Odebrat { $count } fotek z { $album }?
}
albums-remove-confirm-detail = Soubory zůstanou na disku i v knihovně.
albums-delete = Smazat
albums-delete-confirm = Smazat album { $album }?
albums-delete-group-confirm = { $count ->
    [one] Smazat skupinu { $group } a její { $count } album?
    [few] Smazat skupinu { $group } a její { $count } alba?
    [many] Smazat skupinu { $group } a jejích { $count } alba?
   *[other] Smazat skupinu { $group } a jejích { $count } alb?
}
albums-delete-confirm-detail = Fotky zůstanou na disku i v knihovně.
albums-deleted = { $count ->
    [one] Album smazáno.
    [few] Smazána { $count } alba.
    [many] Smazáno { $count } alba.
   *[other] Smazáno { $count } alb.
}
albums-failed = Alba nelze změnit: { $reason }

## Date timeline
albums-timeline = Časová osa
albums-timeline-description = Vyberte období a uvidíte jen jeho fotky. Shift+klik vybere několik za sebou.
albums-timeline-picked = Zobrazeno: { $period }
albums-timeline-cleared = Zobrazena všechna data
