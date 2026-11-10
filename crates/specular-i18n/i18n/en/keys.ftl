# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). English.
# Key names inside messages ($keys) are shown as on the keyboard ("Ctrl+K", "G G") and are
# never translated.

## Settings rows
settings-keys-sequence-timeout = Wait for the next key
settings-keys-sequence-timeout-help = How long a key sequence such as G G waits for its next key. A key that also begins a longer sequence runs after this pause.
settings-keys-bindings = Key bindings
settings-keys-bindings-help = Select an action and press the keys to bind. Each mode has its own keys; cull mode and medical images fall back to the viewer's. To bind Esc, tap it; holding it cancels. The bindings are saved in keymap.toml.

palette-customise-keys = Customise keys…

## Modes (keymap layers)
keys-mode-viewer = Viewer
keys-mode-cull = Cull mode
# Keys for DICOM and other medical images; unbound keys fall back to the viewer's.
keys-mode-medical = Medical images
keys-modes = Mode

## Presets
keys-presets = Preset
keys-preset-default = Default
keys-preset-vim = Vim
keys-preset-culling = Lightroom-style culling
keys-preset-photo-mechanic = Photo Mechanic-style

## Actions with no palette name
action-pan-left = Pan left
action-pan-right = Pan right
action-pan-up = Pan up
action-pan-down = Pan down
action-pan-page-left = Pan a screen left
action-pan-page-right = Pan a screen right
action-pan-page-up = Pan a screen up
action-pan-page-down = Pan a screen down
action-reload = Reload the image
keys-action-cull-pick = Pick
keys-action-cull-reject = Reject
keys-action-cull-unflag = Clear the flag
keys-action-cull-rate = { $stars ->
    [0] Clear the rating
    [one] Rate 1 star
   *[other] Rate { $stars } stars
}
keys-action-cull-label = Colour label: { $label }
# $slot: 1–3, the destination folder slot (Settings → Cull).
keys-action-cull-copy-to = Copy to folder { $slot }
keys-action-cull-move-to = Move to folder { $slot }

## The editor
keys-import = Import…
keys-export = Export…
# Exports only the bindings of the mode shown ($mode: keys-mode-*).
keys-export-mode = Export { $mode }…
# Opens keymap.toml in the built-in editor (also a palette command).
keys-edit-file = Edit keymap.toml
keys-reset-mode = Reset { $mode }
keys-reset-all = Reset all keys
keys-add = Add a key for { $action }
keys-change = Change { $keys }
keys-remove = Remove { $keys }
keys-reset-action = Reset { $action }
keys-none = No keys
keys-changed = Changed
keys-press = Press keys…
keys-recording-hint = Hold Esc to cancel
keys-cancel-recording = Cancel
keys-no-match = No action or key matches “{ $query }”.
# The editor's list as one control, for screen readers.
keys-list = Actions and their keys

## Confirmations
keys-confirm-preset = Use the { $preset } preset? It replaces all your key bindings.
keys-confirm-reset-mode = Reset all keys of { $mode } to the preset?
keys-confirm-reset-all = Reset all keys of every mode to the preset?
keys-confirm-ok = Replace
keys-confirm-reset = Reset
keys-confirm-cancel = Cancel

## Conflicts of a recorded binding
keys-conflict-same = { $keys } already runs “{ $other }”.
keys-conflict-prefix = { $keys } and { $sequence } (“{ $other }”) begin alike: the shorter one runs only after a pause.
keys-conflict-shadow = { $keys } runs “{ $other }” in { $mode }.
keys-conflict-reserved = { $keys } may never reach Specular: { $reason }
keys-choice-replace = Replace
keys-choice-swap = Swap
keys-choice-keep = Keep both
keys-choice-anyway = Bind anyway
keys-choice-cancel = Cancel

## Keys the system or the input method take first
keys-reserved-switch = it switches windows.
keys-reserved-close = it closes the window.
keys-reserved-secure = the system takes it.
keys-reserved-desktop = Super shortcuts belong to the desktop.
keys-reserved-workspaces = it switches terminals or workspaces.
keys-reserved-input = it switches the input method or keyboard layout.
keys-reserved-macos = macOS takes it.

## Announcements (screen readers) and toasts
keys-announce-recording = Recording keys for { $action }. Press the keys, or hold Esc to cancel.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } now runs { $action }.
keys-announce-already = { $keys } already runs { $action }.
keys-announce-cancelled = Recording cancelled.
keys-announce-removed = { $keys } removed from { $action }.
keys-announce-reset-action = { $action } has its preset keys again.
keys-announce-reset-mode = The keys of { $mode } are reset.
keys-announce-reset-all = All keys are reset.
keys-announce-preset = Now using the { $preset } preset.
keys-toast-imported = Key bindings imported
keys-toast-exported = Key bindings exported
keys-toast-reloaded = keymap.toml changed: key bindings reloaded
keys-toast-error = Could not read the key bindings: { $error }
keys-toast-write-error = Could not save keymap.toml: { $error }
# The keys that can follow a key sequence begun ($keys), shown while it waits.
keys-pending = { $keys } …  { $next }

## Problems found in keymap.toml (shown in the editor)
keys-problem-syntax = keymap.toml could not be read: { $error }
keys-problem-key = Unknown key “{ $key }” was ignored.
keys-problem-action = “{ $key }” names an unknown action “{ $action }”; it was ignored.
keys-problem-action-suggestion = “{ $key }” names an unknown action “{ $action }”; it was ignored. Did you mean “{ $suggestion }”?
keys-problem-mode = The table [{ $mode }] is not a mode; it was ignored.
keys-problem-mode-suggestion = The table [{ $mode }] is not a mode; it was ignored. Did you mean [{ $suggestion }]?
keys-problem-duplicate = { $keys } is given twice in { $mode }; the second one is used.
keys-problem-prefix = { $keys } begins { $longer } in { $mode }, so it runs only after a pause.
keys-problem-preset = Unknown preset “{ $preset }”; the default keys apply.
keys-problem-preset-suggestion = Unknown preset “{ $preset }”; the default keys apply. Did you mean “{ $suggestion }”?
