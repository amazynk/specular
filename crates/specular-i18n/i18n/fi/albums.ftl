# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Albumit
albums-new = Uusi albumi…
albums-new-smart = Uusi älyalbumi…
albums-save-search = Tallenna haku älyalbumiksi…
albums-smart = Älyalbumi
# The album the photos go to when added without choosing.
albums-target = Lisätyt menevät tänne
albums-group-count = { $count ->
    [one] { $count } albumi
   *[other] { $count } albumia
}
albums-need-library = Albumit ovat kirjastossa. Lisää tämä kansio ensin kirjastoon.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Nimeä { $album } uudelleen
albums-a11y-edit = Muokkaa: { $album }
albums-a11y-delete = Poista albumi { $album }
albums-a11y-rename-group = Nimeä ryhmä { $group } uudelleen
albums-a11y-delete-group = Poista ryhmä { $group }

## Sheets
albums-name = Nimi
albums-unnamed = Ei vielä nimeä
albums-new-title = Uusi albumi
albums-new-smart-title = Uusi älyalbumi
albums-edit-smart-title = Muokkaa älyalbumia
albums-rename-title = Nimeä albumi uudelleen
albums-rename-group-title = Nimeä ryhmä uudelleen
albums-create = Luo
albums-rename = Nimeä uudelleen
albums-save = Tallenna
albums-groups-note = Lisää ryhmään merkillä /: Asiakas X / valitut.
albums-name-empty = Kirjoita nimi.
albums-name-taken = { $album } on jo olemassa.
albums-name-clash = { $album } on albumi, joten se ei voi olla myös ryhmä.

## Smart album rules
albums-rules = Säännöt
albums-rules-note = Säännöt pätevät ylhäältä alas. Tyhjät säännöt ohitetaan.
albums-rules-none = Lisää vähintään yksi sääntö.
albums-rule-label = Sääntö { $number }
albums-rule-empty = Tyhjä sääntö
albums-rule-add = Lisää sääntö
albums-rule-combine = Yhdistä yllä oleviin sääntöihin
albums-rule-first = Kuvat, joihin pätee
albums-rule-and = Ja
albums-rule-or = Tai
albums-rule-except = Paitsi

## Adding and removing photos
albums-add-menu = Lisää albumiin…
albums-remove-menu = Poista albumista
albums-add-title = { $count ->
    [one] Lisää { $count } kuva albumiin
   *[other] Lisää { $count } kuvaa albumiin
}
albums-add = Lisää
albums-add-new = Uusi albumi
albums-add-list = Albumit
albums-add-none = Ei vielä albumeja. Luo albumi kirjoittamalla nimi.
albums-add-note = Tai kirjoita uuden albumin nimi.
albums-added = { $count ->
    [one] { $count } kuva lisättiin albumiin { $album }.
   *[other] { $count } kuvaa lisättiin albumiin { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } ei ole kirjastossa, joten sitä ei lisätty.
   *[other] { $count } ei ole kirjastossa, joten niitä ei lisätty.
}
albums-created = { $album } luotiin.
albums-removed = { $count ->
    [one] { $count } kuva poistettiin albumista { $album }.
   *[other] { $count } kuvaa poistettiin albumista { $album }.
}
albums-remove = Poista
albums-remove-confirm = { $count ->
    [one] Poistetaanko { $count } kuva albumista { $album }?
   *[other] Poistetaanko { $count } kuvaa albumista { $album }?
}
albums-remove-confirm-detail = Tiedostot pysyvät levyllä ja kirjastossa.
albums-delete = Poista
albums-delete-confirm = Poistetaanko albumi { $album }?
albums-delete-group-confirm = { $count ->
    [one] Poistetaanko ryhmä { $group } ja sen { $count } albumi?
   *[other] Poistetaanko ryhmä { $group } ja sen { $count } albumia?
}
albums-delete-confirm-detail = Kuvat pysyvät levyllä ja kirjastossa.
albums-deleted = { $count ->
    [one] Albumi poistettiin.
   *[other] { $count } albumia poistettiin.
}
albums-failed = Albumeja ei voitu muuttaa: { $reason }

## Date timeline
albums-timeline = Aikajana
albums-timeline-description = Valitse jakso, niin näytetään vain sen kuvat. Shift+napsautus valitsee useita peräkkäin.
albums-timeline-picked = Näytetään { $period }
albums-timeline-cleared = Näytetään kaikki päivämäärät
