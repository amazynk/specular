# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Controllo remoto ed eventi
settings-integration-ipc-help = Altri programmi su questo computer, avviati con il tuo utente, possono comandare { -app-name } e sapere cosa vi succede tramite un socket privato (specular ipc). Niente è in ascolto sulla rete.

## The hooks panel under "Remote control and events".
hooks-panel-title = Hook
hooks-panel-intro = Programmi che settings.toml avvia quando succede qualcosa. Ognuno chiede una volta prima del primo avvio.
hooks-panel-none = Nessun hook. Aggiungili come [[hooks]] in settings.toml.
hooks-panel-keywords = hook eventi automazione script programmi
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Quando { $events }
hooks-panel-untrusted = Chiede prima del primo avvio
hooks-panel-allowed = Consentito
hooks-panel-remove = Rimuovi…
hooks-panel-unused = Non usato

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Avviare “{ $name }” ({ $program }) quando { $events }?
hook-consent-detail = Avvia questo programma con i nomi delle foto: { $args }
hook-consent-trust = Consenti solo programmi di cui ti fidi. Lo chiede di nuovo se l’hook cambia.
hook-consent-allow = Consenti
hook-consent-not-now = Non ora
hook-or = { " " }o{ " " }
hook-event-opened = si apre una foto
hook-event-marked = si contrassegnano foto
hook-event-keywords = cambiano le parole chiave
hook-event-exported = si esportano foto
hook-event-imported = si importano foto
hook-event-captured = una fotocamera invia una foto
hook-event-develop-saved = si salva una modifica in Sviluppo
hook-event-any = succede qualcosa

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Rimuovere l’hook “{ $name }”?
hook-remove-detail = Viene tolto da settings.toml. Il programma e i tuoi file restano sul disco.
hook-remove-ok = Rimuovi
hook-removed = Hook “{ $name }” rimosso.
hook-settings-unwritable = Impossibile modificare settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = L’hook “{ $name }” non ha funzionato: { $reason }
hook-failed-exit = è terminato con codice { $code }
hook-failed-timeout = è stato fermato dopo { $seconds } s
hook-failed-start = non è riuscito ad avviarsi ({ $reason })
hook-dropped = L’hook “{ $name }” è occupato: { $count ->
    [one] { $count } evento è stato saltato.
   *[other] { $count } eventi sono stati saltati.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] esiste { $count } file che sostituirebbe: non è stato esportato nulla
   *[other] esistono { $count } file che sostituirebbe: non è stato esportato nulla
}
export-recipes-remote-skipped = { $file } esiste
