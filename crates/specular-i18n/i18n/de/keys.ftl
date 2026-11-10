# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Auf die nächste Taste warten
settings-keys-sequence-timeout-help = Wie lange eine Tastenfolge wie G G auf ihre nächste Taste wartet. Eine Taste, die auch eine längere Folge beginnt, wird nach dieser Pause ausgeführt.
settings-keys-bindings = Tastenbelegungen
settings-keys-bindings-help = Wählen Sie eine Aktion und drücken Sie die Tasten, die Sie zuweisen möchten. Jeder Modus hat eigene Tasten; Cull-Modus und medizinische Bilder greifen auf die des Betrachters zurück. Um Esc zuzuweisen, tippen Sie kurz darauf; Gedrückthalten bricht ab. Die Belegungen werden in keymap.toml gespeichert.
palette-customise-keys = Tasten anpassen…
keys-mode-viewer = Betrachter
keys-mode-cull = Cull-Modus
keys-mode-medical = Medizinische Bilder
keys-modes = Modus
keys-presets = Vorlage
keys-preset-default = Standard
keys-preset-vim = Vim
keys-preset-culling = Sichten wie in Lightroom
keys-preset-photo-mechanic = Wie Photo Mechanic
action-pan-left = Nach links verschieben
action-pan-right = Nach rechts verschieben
action-pan-up = Nach oben verschieben
action-pan-down = Nach unten verschieben
action-pan-page-left = Eine Bildschirmbreite nach links
action-pan-page-right = Eine Bildschirmbreite nach rechts
action-pan-page-up = Eine Bildschirmhöhe nach oben
action-pan-page-down = Eine Bildschirmhöhe nach unten
action-reload = Bild neu laden
keys-action-cull-pick = Behalten
keys-action-cull-reject = Ablehnen
keys-action-cull-unflag = Markierung entfernen
keys-action-cull-rate = { $stars ->
    [0] Bewertung entfernen
    [one] Mit 1 Stern bewerten
   *[other] Mit { $stars } Sternen bewerten
}
keys-action-cull-label = Farbmarkierung: { $label }
keys-action-cull-copy-to = In Ordner { $slot } kopieren
keys-action-cull-move-to = In Ordner { $slot } verschieben
keys-import = Importieren…
keys-export = Exportieren…
keys-export-mode = { $mode } exportieren…
keys-reset-mode = { $mode } zurücksetzen
keys-reset-all = Alle Tasten zurücksetzen
keys-add = Taste für { $action } hinzufügen
keys-change = { $keys } ändern
keys-remove = { $keys } entfernen
keys-reset-action = { $action } zurücksetzen
keys-none = Keine Tasten
keys-changed = Geändert
keys-press = Tasten drücken…
keys-recording-hint = Esc gedrückt halten zum Abbrechen
keys-cancel-recording = Abbrechen
keys-no-match = Keine Aktion oder Taste passt zu „{ $query }“.
keys-list = Aktionen und ihre Tasten
keys-confirm-preset = Die Vorlage { $preset } verwenden? Sie ersetzt alle Ihre Tastenbelegungen.
keys-confirm-reset-mode = Alle Tasten von { $mode } auf die Vorlage zurücksetzen?
keys-confirm-reset-all = Alle Tasten aller Modi auf die Vorlage zurücksetzen?
keys-confirm-ok = Ersetzen
keys-confirm-reset = Zurücksetzen
keys-confirm-cancel = Abbrechen
keys-conflict-same = { $keys } führt bereits „{ $other }“ aus.
keys-conflict-prefix = { $keys } und { $sequence } („{ $other }“) beginnen gleich: Die kürzere wird erst nach einer Pause ausgeführt.
keys-conflict-shadow = { $keys } führt in { $mode } „{ $other }“ aus.
keys-conflict-reserved = { $keys } erreicht Specular möglicherweise nie: { $reason }
keys-choice-replace = Ersetzen
keys-choice-swap = Tauschen
keys-choice-keep = Beide behalten
keys-choice-anyway = Trotzdem zuweisen
keys-choice-cancel = Abbrechen
keys-reserved-switch = sie wechselt Fenster.
keys-reserved-close = sie schließt das Fenster.
keys-reserved-secure = das System verwendet sie.
keys-reserved-desktop = Super-Kürzel gehören dem Desktop.
keys-reserved-workspaces = sie wechselt Terminals oder Arbeitsflächen.
keys-reserved-input = sie wechselt die Eingabemethode oder Tastaturbelegung.
keys-reserved-macos = macOS verwendet sie.
keys-announce-recording = Tasten für { $action } werden aufgezeichnet. Drücken Sie die Tasten oder halten Sie Esc gedrückt, um abzubrechen.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } führt jetzt { $action } aus.
keys-announce-already = { $keys } führt bereits { $action } aus.
keys-announce-cancelled = Aufzeichnung abgebrochen.
keys-announce-removed = { $keys } aus { $action } entfernt.
keys-announce-reset-action = { $action } hat wieder die Tasten der Vorlage.
keys-announce-reset-mode = Die Tasten von { $mode } sind zurückgesetzt.
keys-announce-reset-all = Alle Tasten sind zurückgesetzt.
keys-announce-preset = Vorlage { $preset } wird jetzt verwendet.
keys-toast-imported = Tastenbelegungen importiert
keys-toast-exported = Tastenbelegungen exportiert
keys-toast-reloaded = keymap.toml geändert: Tastenbelegungen neu geladen
keys-toast-error = Die Tastenbelegungen konnten nicht gelesen werden: { $error }
keys-toast-write-error = keymap.toml konnte nicht gespeichert werden: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = keymap.toml konnte nicht gelesen werden: { $error }
keys-problem-key = Unbekannte Taste „{ $key }“ wurde ignoriert.
keys-problem-action = „{ $key }“ nennt eine unbekannte Aktion „{ $action }“; sie wurde ignoriert.
keys-problem-mode = Die Tabelle [{ $mode }] ist kein Modus; sie wurde ignoriert.
keys-problem-duplicate = { $keys } ist in { $mode } doppelt angegeben; die zweite Angabe gilt.
keys-problem-prefix = { $keys } beginnt { $longer } in { $mode } und wird daher erst nach einer Pause ausgeführt.
keys-problem-preset = Unbekannte Vorlage „{ $preset }“; es gelten die Standardtasten.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = keymap.toml bearbeiten
keys-problem-action-suggestion = „{ $key }“ nennt eine unbekannte Aktion „{ $action }“; sie wurde ignoriert. Meinten Sie „{ $suggestion }“?
keys-problem-mode-suggestion = Die Tabelle [{ $mode }] ist kein Modus; sie wurde ignoriert. Meinten Sie [{ $suggestion }]?
keys-problem-preset-suggestion = Unbekannte Vorlage „{ $preset }“; es gelten die Standardtasten. Meinten Sie „{ $suggestion }“?
