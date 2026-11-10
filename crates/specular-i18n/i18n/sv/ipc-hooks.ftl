# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Fjärrstyrning och händelser
settings-integration-ipc-help = Andra program på den här datorn, som körs som du, kan styra { -app-name } och få veta vad som händer i det via en privat socket (specular ipc). Inget lyssnar på nätverket.

## The hooks panel under "Remote control and events".
hooks-panel-title = Krokar
hooks-panel-intro = Program som settings.toml kör när något händer. Vart och ett frågar en gång före första körningen.
hooks-panel-none = Inga krokar. Lägg till dem som [[hooks]] i settings.toml.
hooks-panel-keywords = krokar hooks händelser automatisering skript program
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Vid { $events }
hooks-panel-untrusted = Frågar före första körningen
hooks-panel-allowed = Tillåten
hooks-panel-remove = Ta bort…
hooks-panel-unused = Används inte

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Köra ”{ $name }” ({ $program }) när { $events }?
hook-consent-detail = Den startar programmet med fotonas namn: { $args }
hook-consent-trust = Tillåt bara program du litar på. Den frågar igen om kroken ändras.
hook-consent-allow = Tillåt
hook-consent-not-now = Inte nu
hook-or = { " " }eller{ " " }
hook-event-opened = ett foto öppnas
hook-event-marked = foton markeras
hook-event-keywords = nyckelord ändras
hook-event-exported = foton exporteras
hook-event-imported = foton importeras
hook-event-captured = en kamera skickar ett foto
hook-event-develop-saved = en redigering i Framkalla sparas
hook-event-any = något händer

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Ta bort kroken ”{ $name }”?
hook-remove-detail = Den tas bort ur settings.toml. Programmet och dina filer finns kvar på disken.
hook-remove-ok = Ta bort
hook-removed = Kroken ”{ $name }” togs bort.
hook-settings-unwritable = Kunde inte ändra settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = Kroken ”{ $name }” fungerade inte: { $reason }
hook-failed-exit = den avslutades med kod { $code }
hook-failed-timeout = den stoppades efter { $seconds } s
hook-failed-start = den kunde inte starta ({ $reason })
hook-dropped = Kroken ”{ $name }” är upptagen: { $count ->
    [one] en händelse hoppades över.
   *[other] { $count } händelser hoppades över.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] en fil som skulle ersättas finns redan: inget exporterades
   *[other] { $count } filer som skulle ersättas finns redan: inget exporterades
}
export-recipes-remote-skipped = { $file } finns redan
