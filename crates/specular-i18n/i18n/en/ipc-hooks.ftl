# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Remote control and events
settings-integration-ipc-help = Other programs on this computer, running as you, can drive { -app-name } and hear what happens in it through a private socket (specular ipc). Nothing listens on the network.

## The hooks panel under "Remote control and events".
hooks-panel-title = Hooks
hooks-panel-intro = Programs settings.toml runs when something happens. Each asks once before its first run.
hooks-panel-none = No hooks. Add them as [[hooks]] in settings.toml.
hooks-panel-keywords = hooks events automation scripts programs
# $events: the event names, joined ("marked, exported").
hooks-panel-on = On { $events }
hooks-panel-untrusted = Asks before its first run
hooks-panel-allowed = Allowed
hooks-panel-remove = Remove…
hooks-panel-unused = Not used

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Run “{ $name }” ({ $program }) when { $events }?
hook-consent-detail = It starts this program with the photos’ names: { $args }
hook-consent-trust = Allow only programs you trust. It asks again if the hook changes.
hook-consent-allow = Allow
hook-consent-not-now = Not now
hook-or = { " " }or{ " " }
hook-event-opened = a photo is opened
hook-event-marked = photos are marked
hook-event-keywords = keywords change
hook-event-exported = photos are exported
hook-event-imported = photos are imported
hook-event-captured = a camera sends a photo
hook-event-develop-saved = a Develop edit is saved
hook-event-any = anything happens

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Remove the hook “{ $name }”?
hook-remove-detail = It is taken out of settings.toml. The program and your files stay on disk.
hook-remove-ok = Remove
hook-removed = Removed the hook “{ $name }”.
hook-settings-unwritable = Could not change settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = The hook “{ $name }” did not work: { $reason }
hook-failed-exit = it ended with code { $code }
hook-failed-timeout = it was stopped after { $seconds } s
hook-failed-start = it could not start ({ $reason })
hook-dropped = The hook “{ $name }” is busy: { $count ->
    [one] an event was skipped.
   *[other] { $count } events were skipped.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] a file it would replace exists: nothing was exported
   *[other] { $count } files it would replace exist: nothing was exported
}
export-recipes-remote-skipped = { $file } exists
