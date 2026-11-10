# machine-drafted, needs native review
# Integratie: openen met, gebruikersopdrachten, plakken (Stage 2). Nederlands.
palette-open-with = Openen met…
palette-paste = Plakken
open-with-placeholder = Openen met…
open-with-loading = Toepassingen zoeken…
open-with-default = { $name } (standaard)
open-with-choose-other = Andere toepassing kiezen…
open-with-choose-title = Kies een toepassing
open-with-failed = Kan niet openen met { $name }: { $reason }
command-missing = { $program } niet gevonden
command-failed = { $name } kan niet starten: { $reason }
command-exited = { $name } is gestopt met een fout: { $status }
paste-nothing = Niets op het klembord om te openen
paste-failed = Kan de afbeelding niet plakken: { $reason }
drag-failed = Kan het bestand niet slepen: { $reason }
about-storage = Instellingen: { $settings } · Cache: { $cache }
about-storage-portable = Draagbare modus: instellingen en cache in { $dir }

## Scripting and recipes

palette-copy-view-command = Weergave kopiëren als shellopdracht
palette-copy-view-python = Weergave kopiëren als Python
toast-copied-view-command = Opdracht gekopieerd
script-copy-no-image = Geen afbeelding om te beschrijven
recipe-confirm-title = Het recept ‘{ $name }’ voor het eerst uitvoeren? Het voert uit: { $command }
recipe-confirm-run = Uitvoeren
recipe-no-file = Het recept heeft een afbeeldingsbestand nodig
recipe-running = { $name } wordt uitgevoerd…
recipe-done = { $name }: { $shapes ->
    [one] 1 vorm
   *[other] { $shapes } vormen
} toegevoegd
recipe-failed = { $name } mislukt: { $reason }
recipe-timeout = gestopt na { $seconds } s
recipe-no-output = { $name } schreef geen { $file }
