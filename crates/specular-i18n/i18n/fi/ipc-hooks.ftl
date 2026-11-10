# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Etäohjaus ja tapahtumat
settings-integration-ipc-help = Muut tämän tietokoneen ohjelmat, jotka toimivat sinun käyttäjänäsi, voivat ohjata { -app-name }ia ja kuulla, mitä siinä tapahtuu, yksityisen socketin kautta (specular ipc). Mikään ei kuuntele verkossa.

## The hooks panel under "Remote control and events".
hooks-panel-title = Koukut
hooks-panel-intro = Ohjelmat, jotka settings.toml suorittaa, kun jotain tapahtuu. Kukin kysyy kerran ennen ensimmäistä suoritusta.
hooks-panel-none = Ei koukkuja. Lisää ne kohtaan [[hooks]] tiedostossa settings.toml.
hooks-panel-keywords = koukut hooks tapahtumat automaatio skriptit ohjelmat
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Kun { $events }
hooks-panel-untrusted = Kysyy ennen ensimmäistä suoritusta
hooks-panel-allowed = Sallittu
hooks-panel-remove = Poista…
hooks-panel-unused = Ei käytössä

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Suoritetaanko ”{ $name }” ({ $program }), kun { $events }?
hook-consent-detail = Se käynnistää tämän ohjelman kuvien nimillä: { $args }
hook-consent-trust = Salli vain ohjelmat, joihin luotat. Kysytään uudelleen, jos koukku muuttuu.
hook-consent-allow = Salli
hook-consent-not-now = Ei nyt
hook-or = { " " }tai{ " " }
hook-event-opened = kuva avataan
hook-event-marked = kuvia merkitään
hook-event-keywords = avainsanat muuttuvat
hook-event-exported = kuvia viedään
hook-event-imported = kuvia tuodaan
hook-event-captured = kamera lähettää kuvan
hook-event-develop-saved = Kehityksen muokkaus tallennetaan
hook-event-any = mitä tahansa tapahtuu

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Poistetaanko koukku ”{ $name }”?
hook-remove-detail = Se poistetaan tiedostosta settings.toml. Ohjelma ja tiedostosi jäävät levylle.
hook-remove-ok = Poista
hook-removed = Koukku ”{ $name }” poistettu.
hook-settings-unwritable = Tiedostoa settings.toml ei voitu muuttaa: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = Koukku ”{ $name }” ei toiminut: { $reason }
hook-failed-exit = se päättyi koodiin { $code }
hook-failed-timeout = se pysäytettiin { $seconds } s jälkeen
hook-failed-start = sitä ei voitu käynnistää ({ $reason })
hook-dropped = Koukku ”{ $name }” on varattu: { $count ->
    [one] yksi tapahtuma ohitettiin.
   *[other] { $count } tapahtumaa ohitettiin.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] korvattava tiedosto on jo olemassa: mitään ei viety
   *[other] { $count } korvattavaa tiedostoa on jo olemassa: mitään ei viety
}
export-recipes-remote-skipped = { $file } on jo olemassa
