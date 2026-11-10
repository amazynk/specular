# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Vent på næste tast
settings-keys-sequence-timeout-help = Hvor længe en tastsekvens som G G venter på næste tast. En tast, der også starter en længere sekvens, udføres efter denne pause.
settings-keys-bindings = Tastaturgenveje
settings-keys-bindings-help = Vælg en handling, og tryk på de taster, du vil tildele. Hver tilstand har sine egne taster; sorteringstilstand og medicinske billeder falder tilbage på fremviserens. Tryk kort på Esc for at tildele Esc; hold den nede for at annullere. Genvejene gemmes i keymap.toml.
palette-customise-keys = Tilpas taster…
keys-mode-viewer = Fremviser
keys-mode-cull = Sorteringstilstand
keys-mode-medical = Medicinske billeder
keys-modes = Tilstand
keys-presets = Forudindstilling
keys-preset-default = Standard
keys-preset-vim = Vim
keys-preset-culling = Sortering som i Lightroom
keys-preset-photo-mechanic = Som Photo Mechanic
action-pan-left = Panorer til venstre
action-pan-right = Panorer til højre
action-pan-up = Panorer op
action-pan-down = Panorer ned
action-pan-page-left = Panorer en skærm til venstre
action-pan-page-right = Panorer en skærm til højre
action-pan-page-up = Panorer en skærm op
action-pan-page-down = Panorer en skærm ned
action-reload = Genindlæs billedet
keys-action-cull-pick = Vælg
keys-action-cull-reject = Afvis
keys-action-cull-unflag = Fjern markeringen
keys-action-cull-rate = { $stars ->
    [0] Fjern bedømmelsen
    [one] Giv 1 stjerne
   *[other] Giv { $stars } stjerner
}
keys-action-cull-label = Farveetiket: { $label }
keys-action-cull-copy-to = Kopiér til mappe { $slot }
keys-action-cull-move-to = Flyt til mappe { $slot }
keys-import = Importér…
keys-export = Eksportér…
keys-export-mode = Eksportér { $mode }…
keys-reset-mode = Nulstil { $mode }
keys-reset-all = Nulstil alle taster
keys-add = Tilføj en tast til { $action }
keys-change = Skift { $keys }
keys-remove = Fjern { $keys }
keys-reset-action = Nulstil { $action }
keys-none = Ingen taster
keys-changed = Ændret
keys-press = Tryk på taster…
keys-recording-hint = Hold Esc nede for at annullere
keys-cancel-recording = Annuller
keys-no-match = Ingen handling eller tast matcher “{ $query }”.
keys-list = Handlinger og deres taster
keys-confirm-preset = Brug forudindstillingen { $preset }? Den erstatter alle dine genveje.
keys-confirm-reset-mode = Nulstil alle taster i { $mode } til forudindstillingen?
keys-confirm-reset-all = Nulstil alle taster i alle tilstande til forudindstillingen?
keys-confirm-ok = Erstat
keys-confirm-reset = Nulstil
keys-confirm-cancel = Annuller
keys-conflict-same = { $keys } udfører allerede “{ $other }”.
keys-conflict-prefix = { $keys } og { $sequence } (“{ $other }”) begynder ens: den korteste udføres først efter en pause.
keys-conflict-shadow = { $keys } udfører “{ $other }” i { $mode }.
keys-conflict-reserved = { $keys } når måske aldrig frem til Specular: { $reason }
keys-choice-replace = Erstat
keys-choice-swap = Byt
keys-choice-keep = Behold begge
keys-choice-anyway = Tildel alligevel
keys-choice-cancel = Annuller
keys-reserved-switch = den skifter vindue.
keys-reserved-close = den lukker vinduet.
keys-reserved-secure = systemet bruger den.
keys-reserved-desktop = Super-genveje tilhører skrivebordet.
keys-reserved-workspaces = den skifter terminal eller arbejdsområde.
keys-reserved-input = den skifter inputmetode eller tastaturlayout.
keys-reserved-macos = macOS bruger den.
keys-announce-recording = Optager taster til { $action }. Tryk på tasterne, eller hold Esc nede for at annullere.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } udfører nu { $action }.
keys-announce-already = { $keys } udfører allerede { $action }.
keys-announce-cancelled = Optagelsen er annulleret.
keys-announce-removed = { $keys } er fjernet fra { $action }.
keys-announce-reset-action = { $action } har forudindstillingens taster igen.
keys-announce-reset-mode = Tasterne i { $mode } er nulstillet.
keys-announce-reset-all = Alle taster er nulstillet.
keys-announce-preset = Forudindstillingen { $preset } bruges nu.
keys-toast-imported = Genveje importeret
keys-toast-exported = Genveje eksporteret
keys-toast-reloaded = keymap.toml er ændret: genveje genindlæst
keys-toast-error = Kunne ikke læse genvejene: { $error }
keys-toast-write-error = Kunne ikke gemme keymap.toml: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = Kunne ikke læse keymap.toml: { $error }
keys-problem-key = Ukendt tast “{ $key }” blev ignoreret.
keys-problem-action = “{ $key }” angiver en ukendt handling “{ $action }”; den blev ignoreret.
keys-problem-mode = Tabellen [{ $mode }] er ikke en tilstand; den blev ignoreret.
keys-problem-duplicate = { $keys } er angivet to gange i { $mode }; den anden bruges.
keys-problem-prefix = { $keys } starter { $longer } i { $mode }, så den udføres først efter en pause.
keys-problem-preset = Ukendt forudindstilling “{ $preset }”; standardtasterne gælder.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = Rediger keymap.toml
keys-problem-action-suggestion = “{ $key }” angiver en ukendt handling “{ $action }”; den blev ignoreret. Mente du “{ $suggestion }”?
keys-problem-mode-suggestion = Tabellen [{ $mode }] er ikke en tilstand; den blev ignoreret. Mente du [{ $suggestion }]?
keys-problem-preset-suggestion = Ukendt forudindstilling “{ $preset }”; standardtasterne gælder. Mente du “{ $suggestion }”?
