# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Wachten op de volgende toets
settings-keys-sequence-timeout-help = Hoe lang een toetsreeks zoals G G op de volgende toets wacht. Een toets die ook een langere reeks begint, wordt na deze pauze uitgevoerd.
settings-keys-bindings = Sneltoetsen
settings-keys-bindings-help = Kies een actie en druk op de toetsen die u wilt koppelen. Elke modus heeft eigen toetsen; selectiemodus en medische beelden vallen terug op die van de viewer. Tik kort op Esc om Esc te koppelen; ingedrukt houden annuleert. De koppelingen worden opgeslagen in keymap.toml.
palette-customise-keys = Toetsen aanpassen…
keys-mode-viewer = Viewer
keys-mode-cull = Selectiemodus
keys-mode-medical = Medische beelden
keys-modes = Modus
keys-presets = Voorinstelling
keys-preset-default = Standaard
keys-preset-vim = Vim
keys-preset-culling = Selecteren zoals Lightroom
keys-preset-photo-mechanic = Zoals Photo Mechanic
action-pan-left = Naar links schuiven
action-pan-right = Naar rechts schuiven
action-pan-up = Omhoog schuiven
action-pan-down = Omlaag schuiven
action-pan-page-left = Een scherm naar links
action-pan-page-right = Een scherm naar rechts
action-pan-page-up = Een scherm omhoog
action-pan-page-down = Een scherm omlaag
action-reload = Afbeelding opnieuw laden
keys-action-cull-pick = Kiezen
keys-action-cull-reject = Afwijzen
keys-action-cull-unflag = Markering wissen
keys-action-cull-rate = { $stars ->
    [0] Beoordeling wissen
    [one] 1 ster geven
   *[other] { $stars } sterren geven
}
keys-action-cull-label = Kleurlabel: { $label }
keys-action-cull-copy-to = Kopiëren naar map { $slot }
keys-action-cull-move-to = Verplaatsen naar map { $slot }
keys-import = Importeren…
keys-export = Exporteren…
keys-export-mode = { $mode } exporteren…
keys-reset-mode = { $mode } herstellen
keys-reset-all = Alle toetsen herstellen
keys-add = Toets toevoegen voor { $action }
keys-change = { $keys } wijzigen
keys-remove = { $keys } verwijderen
keys-reset-action = { $action } herstellen
keys-none = Geen toetsen
keys-changed = Gewijzigd
keys-press = Druk op toetsen…
keys-recording-hint = Houd Esc ingedrukt om te annuleren
keys-cancel-recording = Annuleren
keys-no-match = Geen actie of toets komt overeen met ‘{ $query }’.
keys-list = Acties en hun toetsen
keys-confirm-preset = Voorinstelling { $preset } gebruiken? Die vervangt al uw sneltoetsen.
keys-confirm-reset-mode = Alle toetsen van { $mode } terugzetten naar de voorinstelling?
keys-confirm-reset-all = Alle toetsen van elke modus terugzetten naar de voorinstelling?
keys-confirm-ok = Vervangen
keys-confirm-reset = Herstellen
keys-confirm-cancel = Annuleren
keys-conflict-same = { $keys } voert al ‘{ $other }’ uit.
keys-conflict-prefix = { $keys } en { $sequence } (‘{ $other }’) beginnen hetzelfde: de kortste wordt pas na een pauze uitgevoerd.
keys-conflict-shadow = { $keys } voert in { $mode } ‘{ $other }’ uit.
keys-conflict-reserved = { $keys } bereikt Specular mogelijk nooit: { $reason }
keys-choice-replace = Vervangen
keys-choice-swap = Omwisselen
keys-choice-keep = Beide houden
keys-choice-anyway = Toch koppelen
keys-choice-cancel = Annuleren
keys-reserved-switch = die wisselt van venster.
keys-reserved-close = die sluit het venster.
keys-reserved-secure = het systeem gebruikt die.
keys-reserved-desktop = Super-sneltoetsen zijn van het bureaublad.
keys-reserved-workspaces = die wisselt van terminal of werkruimte.
keys-reserved-input = die wisselt van invoermethode of toetsenbordindeling.
keys-reserved-macos = macOS gebruikt die.
keys-announce-recording = Toetsen opnemen voor { $action }. Druk op de toetsen of houd Esc ingedrukt om te annuleren.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } voert nu { $action } uit.
keys-announce-already = { $keys } voert al { $action } uit.
keys-announce-cancelled = Opname geannuleerd.
keys-announce-removed = { $keys } verwijderd van { $action }.
keys-announce-reset-action = { $action } heeft weer de toetsen van de voorinstelling.
keys-announce-reset-mode = De toetsen van { $mode } zijn hersteld.
keys-announce-reset-all = Alle toetsen zijn hersteld.
keys-announce-preset = Voorinstelling { $preset } wordt nu gebruikt.
keys-toast-imported = Sneltoetsen geïmporteerd
keys-toast-exported = Sneltoetsen geëxporteerd
keys-toast-reloaded = keymap.toml gewijzigd: sneltoetsen opnieuw geladen
keys-toast-error = De sneltoetsen konden niet worden gelezen: { $error }
keys-toast-write-error = keymap.toml kon niet worden opgeslagen: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = keymap.toml kon niet worden gelezen: { $error }
keys-problem-key = Onbekende toets ‘{ $key }’ is genegeerd.
keys-problem-action = ‘{ $key }’ noemt een onbekende actie ‘{ $action }’; die is genegeerd.
keys-problem-mode = De tabel [{ $mode }] is geen modus; die is genegeerd.
keys-problem-duplicate = { $keys } staat twee keer in { $mode }; de tweede wordt gebruikt.
keys-problem-prefix = { $keys } begint { $longer } in { $mode } en wordt daardoor pas na een pauze uitgevoerd.
keys-problem-preset = Onbekende voorinstelling ‘{ $preset }’; de standaardtoetsen gelden.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = keymap.toml bewerken
keys-problem-action-suggestion = ‘{ $key }’ noemt een onbekende actie ‘{ $action }’; die is genegeerd. Bedoelde u ‘{ $suggestion }’?
keys-problem-mode-suggestion = De tabel [{ $mode }] is geen modus; die is genegeerd. Bedoelde u [{ $suggestion }]?
keys-problem-preset-suggestion = Onbekende voorinstelling ‘{ $preset }’; de standaardtoetsen gelden. Bedoelde u ‘{ $suggestion }’?
