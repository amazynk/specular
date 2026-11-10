# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Bediening op afstand en gebeurtenissen
settings-integration-ipc-help = Andere programma’s op deze computer, die onder uw account draaien, kunnen { -app-name } besturen en horen wat erin gebeurt via een privé-socket (specular ipc). Niets luistert op het netwerk.

## The hooks panel under "Remote control and events".
hooks-panel-title = Hook-programma’s
hooks-panel-intro = Programma’s die settings.toml start als er iets gebeurt. Elk vraagt één keer vóór de eerste keer.
hooks-panel-none = Geen hooks. Voeg ze toe als [[hooks]] in settings.toml.
hooks-panel-keywords = hooks gebeurtenissen automatisering scripts programma’s
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Als { $events }
hooks-panel-untrusted = Vraagt vóór de eerste keer
hooks-panel-allowed = Toegestaan
hooks-panel-remove = Verwijderen…
hooks-panel-unused = Niet gebruikt

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = “{ $name }” ({ $program }) starten als { $events }?
hook-consent-detail = Dit programma start met de namen van de foto’s: { $args }
hook-consent-trust = Sta alleen programma’s toe die u vertrouwt. Het vraagt opnieuw als de hook verandert.
hook-consent-allow = Toestaan
hook-consent-not-now = Niet nu
hook-or = { " " }of{ " " }
hook-event-opened = een foto wordt geopend
hook-event-marked = foto’s worden gemarkeerd
hook-event-keywords = trefwoorden veranderen
hook-event-exported = foto’s worden geëxporteerd
hook-event-imported = foto’s worden geïmporteerd
hook-event-captured = een camera een foto stuurt
hook-event-develop-saved = een bewerking in Ontwikkelen wordt opgeslagen
hook-event-any = er iets gebeurt

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = De hook “{ $name }” verwijderen?
hook-remove-detail = Hij wordt uit settings.toml gehaald. Het programma en uw bestanden blijven op schijf.
hook-remove-ok = Verwijderen
hook-removed = De hook “{ $name }” is verwijderd.
hook-settings-unwritable = Kan settings.toml niet wijzigen: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = De hook “{ $name }” werkte niet: { $reason }
hook-failed-exit = hij stopte met code { $code }
hook-failed-timeout = hij is na { $seconds } s gestopt
hook-failed-start = hij kon niet starten ({ $reason })
hook-dropped = De hook “{ $name }” is bezet: { $count ->
    [one] { $count } gebeurtenis is overgeslagen.
   *[other] { $count } gebeurtenissen zijn overgeslagen.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] { $count } bestand dat het zou vervangen bestaat al: er is niets geëxporteerd
   *[other] { $count } bestanden die het zou vervangen bestaan al: er is niets geëxporteerd
}
export-recipes-remote-skipped = { $file } bestaat al
