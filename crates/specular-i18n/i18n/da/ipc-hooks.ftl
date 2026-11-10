# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Fjernstyring og hændelser
settings-integration-ipc-help = Andre programmer på denne computer, der kører som dig, kan styre { -app-name } og høre, hvad der sker i det, gennem en privat socket (specular ipc). Intet lytter på netværket.

## The hooks panel under "Remote control and events".
hooks-panel-title = Hændelseskroge
hooks-panel-intro = Programmer, som settings.toml kører, når noget sker. Hvert spørger én gang før første kørsel.
hooks-panel-none = Ingen kroge. Tilføj dem som [[hooks]] i settings.toml.
hooks-panel-keywords = kroge hooks hændelser automatisering scripts programmer
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Når { $events }
hooks-panel-untrusted = Spørger før første kørsel
hooks-panel-allowed = Tilladt
hooks-panel-remove = Fjern…
hooks-panel-unused = Ikke brugt

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Kør “{ $name }” ({ $program }), når { $events }?
hook-consent-detail = Programmet startes med billedernes navne: { $args }
hook-consent-trust = Tillad kun programmer, du stoler på. Der spørges igen, hvis krogen ændres.
hook-consent-allow = Tillad
hook-consent-not-now = Ikke nu
hook-or = { " " }eller{ " " }
hook-event-opened = et billede åbnes
hook-event-marked = billeder markeres
hook-event-keywords = nøgleord ændres
hook-event-exported = billeder eksporteres
hook-event-imported = billeder importeres
hook-event-captured = et kamera sender et billede
hook-event-develop-saved = en redigering i Fremkald gemmes
hook-event-any = noget som helst sker

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Fjern krogen “{ $name }”?
hook-remove-detail = Den tages ud af settings.toml. Programmet og dine filer bliver på disken.
hook-remove-ok = Fjern
hook-removed = Krogen “{ $name }” er fjernet.
hook-settings-unwritable = Kunne ikke ændre settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = Krogen “{ $name }” virkede ikke: { $reason }
hook-failed-exit = den sluttede med kode { $code }
hook-failed-timeout = den blev stoppet efter { $seconds } s
hook-failed-start = den kunne ikke starte ({ $reason })
hook-dropped = Krogen “{ $name }” er optaget: { $count ->
    [one] en hændelse blev sprunget over.
   *[other] { $count } hændelser blev sprunget over.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] en fil, der ville blive erstattet, findes: intet blev eksporteret
   *[other] { $count } filer, der ville blive erstattet, findes: intet blev eksporteret
}
export-recipes-remote-skipped = { $file } findes
