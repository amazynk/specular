# machine-drafted, needs native review
# Integrering: öppna med, användarkommandon, klistra in (steg 2). Svenska.
palette-open-with = Öppna med…
palette-paste = Klistra in
open-with-placeholder = Öppna med…
open-with-loading = Letar efter program…
open-with-default = { $name } (standard)
open-with-choose-other = Välj ett annat program…
open-with-choose-title = Välj ett program
open-with-failed = Det gick inte att öppna med { $name }: { $reason }
command-missing = { $program } hittades inte
command-failed = { $name } kunde inte starta: { $reason }
command-exited = { $name } avslutades med ett fel: { $status }
paste-nothing = Det finns inget att öppna i urklipp
paste-failed = Det gick inte att klistra in bilden: { $reason }
drag-failed = Det gick inte att dra filen: { $reason }
about-storage = Inställningar: { $settings } · Cache: { $cache }
about-storage-portable = Portabelt läge: inställningar och cache i { $dir }

## Scripting and recipes

palette-copy-view-command = Kopiera vyn som skalkommando
palette-copy-view-python = Kopiera vyn som Python
toast-copied-view-command = Kommandot kopierat
script-copy-no-image = Ingen bild att beskriva
recipe-confirm-title = Köra receptet ”{ $name }” för första gången? Det kör: { $command }
recipe-confirm-run = Kör
recipe-no-file = Receptet behöver en bildfil
recipe-running = Kör { $name }…
recipe-done = { $name }: { $shapes ->
    [one] 1 form
   *[other] { $shapes } former
} tillagda
recipe-failed = { $name } misslyckades: { $reason }
recipe-timeout = stoppad efter { $seconds } s
recipe-no-output = { $name } skrev ingen { $file }
