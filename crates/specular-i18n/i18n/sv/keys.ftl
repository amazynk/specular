# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Vänta på nästa tangent
settings-keys-sequence-timeout-help = Hur länge en tangentsekvens som G G väntar på nästa tangent. En tangent som också inleder en längre sekvens körs efter denna paus.
settings-keys-bindings = Kortkommandon
settings-keys-bindings-help = Välj en åtgärd och tryck på tangenterna du vill koppla. Varje läge har egna tangenter; gallringsläget och medicinska bilder faller tillbaka på visarens. Tryck kort på Esc för att koppla Esc; att hålla den avbryter. Kopplingarna sparas i keymap.toml.
palette-customise-keys = Anpassa tangenter…
keys-mode-viewer = Visare
keys-mode-cull = Gallringsläge
keys-mode-medical = Medicinska bilder
keys-modes = Läge
keys-presets = Förinställning
keys-preset-default = Standard
keys-preset-vim = Vim
keys-preset-culling = Gallring som i Lightroom
keys-preset-photo-mechanic = Som Photo Mechanic
action-pan-left = Panorera vänster
action-pan-right = Panorera höger
action-pan-up = Panorera uppåt
action-pan-down = Panorera nedåt
action-pan-page-left = Panorera en skärm åt vänster
action-pan-page-right = Panorera en skärm åt höger
action-pan-page-up = Panorera en skärm uppåt
action-pan-page-down = Panorera en skärm nedåt
action-reload = Läs in bilden igen
keys-action-cull-pick = Välj
keys-action-cull-reject = Avvisa
keys-action-cull-unflag = Ta bort flaggan
keys-action-cull-rate = { $stars ->
    [0] Ta bort betyget
    [one] Ge 1 stjärna
   *[other] Ge { $stars } stjärnor
}
keys-action-cull-label = Färgetikett: { $label }
keys-action-cull-copy-to = Kopiera till mapp { $slot }
keys-action-cull-move-to = Flytta till mapp { $slot }
keys-import = Importera…
keys-export = Exportera…
keys-export-mode = Exportera { $mode }…
keys-reset-mode = Återställ { $mode }
keys-reset-all = Återställ alla tangenter
keys-add = Lägg till en tangent för { $action }
keys-change = Ändra { $keys }
keys-remove = Ta bort { $keys }
keys-reset-action = Återställ { $action }
keys-none = Inga tangenter
keys-changed = Ändrad
keys-press = Tryck på tangenter…
keys-recording-hint = Håll Esc för att avbryta
keys-cancel-recording = Avbryt
keys-no-match = Ingen åtgärd eller tangent matchar ”{ $query }”.
keys-list = Åtgärder och deras tangenter
keys-confirm-preset = Använda förinställningen { $preset }? Den ersätter alla dina kortkommandon.
keys-confirm-reset-mode = Återställa alla tangenter i { $mode } till förinställningen?
keys-confirm-reset-all = Återställa alla tangenter i alla lägen till förinställningen?
keys-confirm-ok = Ersätt
keys-confirm-reset = Återställ
keys-confirm-cancel = Avbryt
keys-conflict-same = { $keys } kör redan ”{ $other }”.
keys-conflict-prefix = { $keys } och { $sequence } (”{ $other }”) börjar likadant: den kortare körs först efter en paus.
keys-conflict-shadow = { $keys } kör ”{ $other }” i { $mode }.
keys-conflict-reserved = { $keys } kanske aldrig når Specular: { $reason }
keys-choice-replace = Ersätt
keys-choice-swap = Byt
keys-choice-keep = Behåll båda
keys-choice-anyway = Koppla ändå
keys-choice-cancel = Avbryt
keys-reserved-switch = den byter fönster.
keys-reserved-close = den stänger fönstret.
keys-reserved-secure = systemet använder den.
keys-reserved-desktop = Super-genvägar tillhör skrivbordet.
keys-reserved-workspaces = den byter terminal eller arbetsyta.
keys-reserved-input = den byter inmatningsmetod eller tangentbordslayout.
keys-reserved-macos = macOS använder den.
keys-announce-recording = Spelar in tangenter för { $action }. Tryck på tangenterna eller håll Esc för att avbryta.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } kör nu { $action }.
keys-announce-already = { $keys } kör redan { $action }.
keys-announce-cancelled = Inspelningen avbröts.
keys-announce-removed = { $keys } togs bort från { $action }.
keys-announce-reset-action = { $action } har förinställningens tangenter igen.
keys-announce-reset-mode = Tangenterna i { $mode } är återställda.
keys-announce-reset-all = Alla tangenter är återställda.
keys-announce-preset = Förinställningen { $preset } används nu.
keys-toast-imported = Kortkommandon importerade
keys-toast-exported = Kortkommandon exporterade
keys-toast-reloaded = keymap.toml ändrades: kortkommandon inlästa igen
keys-toast-error = Det gick inte att läsa kortkommandona: { $error }
keys-toast-write-error = Det gick inte att spara keymap.toml: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = Det gick inte att läsa keymap.toml: { $error }
keys-problem-key = Okänd tangent ”{ $key }” ignorerades.
keys-problem-action = ”{ $key }” anger en okänd åtgärd ”{ $action }”; den ignorerades.
keys-problem-mode = Tabellen [{ $mode }] är inget läge; den ignorerades.
keys-problem-duplicate = { $keys } anges två gånger i { $mode }; den andra används.
keys-problem-prefix = { $keys } inleder { $longer } i { $mode }, så den körs först efter en paus.
keys-problem-preset = Okänd förinställning ”{ $preset }”; standardtangenterna gäller.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = Redigera keymap.toml
keys-problem-action-suggestion = ”{ $key }” anger en okänd åtgärd ”{ $action }”; den ignorerades. Menade du ”{ $suggestion }”?
keys-problem-mode-suggestion = Tabellen [{ $mode }] är inget läge; den ignorerades. Menade du [{ $suggestion }]?
keys-problem-preset-suggestion = Okänd förinställning ”{ $preset }”; standardtangenterna gäller. Menade du ”{ $suggestion }”?
