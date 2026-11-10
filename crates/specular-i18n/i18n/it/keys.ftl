# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Attendi il tasto successivo
settings-keys-sequence-timeout-help = Quanto una sequenza come G G attende il tasto successivo. Un tasto che inizia anche una sequenza più lunga viene eseguito dopo questa pausa.
settings-keys-bindings = Scorciatoie da tastiera
settings-keys-bindings-help = Seleziona un’azione e premi i tasti da associare. Ogni modalità ha i propri tasti; la modalità selezione e le immagini mediche ricorrono a quelli del visualizzatore. Per associare Esc, premilo brevemente; tenerlo premuto annulla. Le associazioni sono salvate in keymap.toml.
palette-customise-keys = Personalizza i tasti…
keys-mode-viewer = Visualizzatore
keys-mode-cull = Modalità selezione
keys-mode-medical = Immagini mediche
keys-modes = Modalità
keys-presets = Preimpostazione
keys-preset-default = Predefinita
keys-preset-vim = Vim
keys-preset-culling = Selezione stile Lightroom
keys-preset-photo-mechanic = Stile Photo Mechanic
action-pan-left = Sposta a sinistra
action-pan-right = Sposta a destra
action-pan-up = Sposta in alto
action-pan-down = Sposta in basso
action-pan-page-left = Sposta di una schermata a sinistra
action-pan-page-right = Sposta di una schermata a destra
action-pan-page-up = Sposta di una schermata in alto
action-pan-page-down = Sposta di una schermata in basso
action-reload = Ricarica l’immagine
keys-action-cull-pick = Scegli
keys-action-cull-reject = Scarta
keys-action-cull-unflag = Rimuovi il contrassegno
keys-action-cull-rate = { $stars ->
    [0] Rimuovi la valutazione
    [one] Valuta 1 stella
   *[other] Valuta { $stars } stelle
}
keys-action-cull-label = Etichetta colore: { $label }
keys-action-cull-copy-to = Copia nella cartella { $slot }
keys-action-cull-move-to = Sposta nella cartella { $slot }
keys-import = Importa…
keys-export = Esporta…
keys-export-mode = Esporta { $mode }…
keys-reset-mode = Ripristina { $mode }
keys-reset-all = Ripristina tutti i tasti
keys-add = Aggiungi un tasto per { $action }
keys-change = Modifica { $keys }
keys-remove = Rimuovi { $keys }
keys-reset-action = Ripristina { $action }
keys-none = Nessun tasto
keys-changed = Modificato
keys-press = Premi i tasti…
keys-recording-hint = Tieni premuto Esc per annullare
keys-cancel-recording = Annulla
keys-no-match = Nessuna azione o tasto corrisponde a «{ $query }».
keys-list = Azioni e relativi tasti
keys-confirm-preset = Usare la preimpostazione { $preset }? Sostituisce tutte le tue associazioni.
keys-confirm-reset-mode = Ripristinare tutti i tasti di { $mode } alla preimpostazione?
keys-confirm-reset-all = Ripristinare tutti i tasti di ogni modalità alla preimpostazione?
keys-confirm-ok = Sostituisci
keys-confirm-reset = Ripristina
keys-confirm-cancel = Annulla
keys-conflict-same = { $keys } esegue già «{ $other }».
keys-conflict-prefix = { $keys } e { $sequence } («{ $other }») iniziano allo stesso modo: la più corta viene eseguita solo dopo una pausa.
keys-conflict-shadow = { $keys } esegue «{ $other }» in { $mode }.
keys-conflict-reserved = { $keys } potrebbe non raggiungere mai Specular: { $reason }
keys-choice-replace = Sostituisci
keys-choice-swap = Scambia
keys-choice-keep = Mantieni entrambi
keys-choice-anyway = Associa comunque
keys-choice-cancel = Annulla
keys-reserved-switch = cambia finestra.
keys-reserved-close = chiude la finestra.
keys-reserved-secure = lo usa il sistema.
keys-reserved-desktop = le scorciatoie con Super appartengono al desktop.
keys-reserved-workspaces = cambia terminale o area di lavoro.
keys-reserved-input = cambia metodo di input o layout di tastiera.
keys-reserved-macos = lo usa macOS.
keys-announce-recording = Registrazione dei tasti per { $action }. Premi i tasti o tieni premuto Esc per annullare.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } ora esegue { $action }.
keys-announce-already = { $keys } esegue già { $action }.
keys-announce-cancelled = Registrazione annullata.
keys-announce-removed = { $keys } rimosso da { $action }.
keys-announce-reset-action = { $action } ha di nuovo i tasti della preimpostazione.
keys-announce-reset-mode = I tasti di { $mode } sono stati ripristinati.
keys-announce-reset-all = Tutti i tasti sono stati ripristinati.
keys-announce-preset = Ora è in uso la preimpostazione { $preset }.
keys-toast-imported = Associazioni importate
keys-toast-exported = Associazioni esportate
keys-toast-reloaded = keymap.toml modificato: associazioni ricaricate
keys-toast-error = Impossibile leggere le associazioni: { $error }
keys-toast-write-error = Impossibile salvare keymap.toml: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = Impossibile leggere keymap.toml: { $error }
keys-problem-key = Il tasto sconosciuto «{ $key }» è stato ignorato.
keys-problem-action = «{ $key }» indica un’azione sconosciuta «{ $action }»; è stata ignorata.
keys-problem-mode = La tabella [{ $mode }] non è una modalità; è stata ignorata.
keys-problem-duplicate = { $keys } è indicato due volte in { $mode }; vale il secondo.
keys-problem-prefix = { $keys } inizia { $longer } in { $mode }, quindi viene eseguito solo dopo una pausa.
keys-problem-preset = Preimpostazione sconosciuta «{ $preset }»; valgono i tasti predefiniti.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = Modifica keymap.toml
keys-problem-action-suggestion = «{ $key }» indica un’azione sconosciuta «{ $action }»; è stata ignorata. Forse intendevi «{ $suggestion }»?
keys-problem-mode-suggestion = La tabella [{ $mode }] non è una modalità; è stata ignorata. Forse intendevi [{ $suggestion }]?
keys-problem-preset-suggestion = Preimpostazione sconosciuta «{ $preset }»; valgono i tasti predefiniti. Forse intendevi «{ $suggestion }»?
