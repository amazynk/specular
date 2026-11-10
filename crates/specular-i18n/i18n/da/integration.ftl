# machine-drafted, needs native review
# Integration: åbn med, brugerkommandoer, indsæt (Stage 2). Dansk.
palette-open-with = Åbn med…
palette-paste = Indsæt
open-with-placeholder = Åbn med…
open-with-loading = Søger efter programmer…
open-with-default = { $name } (standard)
open-with-choose-other = Vælg et andet program…
open-with-choose-title = Vælg et program
open-with-failed = Kunne ikke åbne med { $name }: { $reason }
command-missing = { $program } blev ikke fundet
command-failed = { $name } kunne ikke starte: { $reason }
command-exited = { $name } afsluttede med en fejl: { $status }
paste-nothing = Intet at åbne i udklipsholderen
paste-failed = Kunne ikke indsætte billedet: { $reason }
drag-failed = Kunne ikke trække filen: { $reason }
about-storage = Indstillinger: { $settings } · Cache: { $cache }
about-storage-portable = Bærbar tilstand: indstillinger og cache i { $dir }

## Scripting and recipes

palette-copy-view-command = Kopiér visning som shell-kommando
palette-copy-view-python = Kopiér visning som Python
toast-copied-view-command = Kommando kopieret
script-copy-no-image = Intet billede at beskrive
recipe-confirm-title = Kør opskriften “{ $name }” for første gang? Den kører: { $command }
recipe-confirm-run = Kør
recipe-no-file = Opskriften kræver en billedfil
recipe-running = Kører { $name }…
recipe-done = { $name }: { $shapes ->
    [one] 1 figur
   *[other] { $shapes } figurer
} tilføjet
recipe-failed = { $name } mislykkedes: { $reason }
recipe-timeout = stoppet efter { $seconds } s
recipe-no-output = { $name } skrev ingen { $file }
