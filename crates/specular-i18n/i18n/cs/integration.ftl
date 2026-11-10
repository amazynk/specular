# machine-drafted, needs native review
# Specular — integrace: otevřít v jiné aplikaci, uživatelské příkazy, vložení (fáze 2). Čeština.
palette-open-with = Otevřít v aplikaci…
palette-paste = Vložit
open-with-placeholder = Otevřít v aplikaci…
open-with-loading = Hledání aplikací…
open-with-default = { $name } (výchozí)
open-with-choose-other = Zvolit jinou aplikaci…
open-with-choose-title = Zvolit aplikaci
open-with-failed = Nelze otevřít v aplikaci { $name }: { $reason }
command-missing = Program { $program } nebyl nalezen
command-failed = { $name } nelze spustit: { $reason }
command-exited = { $name } skončil s chybou: { $status }
paste-nothing = Ve schránce není nic k otevření
paste-failed = Obrázek nelze vložit: { $reason }
drag-failed = Soubor nelze přetáhnout: { $reason }
about-storage = Nastavení: { $settings } · Mezipaměť: { $cache }
about-storage-portable = Přenosný režim: nastavení a mezipaměť ve složce { $dir }

## Scripting and recipes

palette-copy-view-command = Kopírovat zobrazení jako příkaz shellu
palette-copy-view-python = Kopírovat zobrazení jako Python
toast-copied-view-command = Příkaz zkopírován
script-copy-no-image = Žádný obrázek k popisu
recipe-confirm-title = Spustit recept „{ $name }“ poprvé? Spouští: { $command }
recipe-confirm-run = Spustit
recipe-no-file = Recept potřebuje soubor obrázku
recipe-running = Spouští se { $name }…
recipe-done = { $name }: { $shapes ->
    [one] přidán 1 tvar
    [few] přidány { $shapes } tvary
    [many] přidáno { $shapes } tvaru
   *[other] přidáno { $shapes } tvarů
}
recipe-failed = { $name } selhal: { $reason }
recipe-timeout = zastaveno po { $seconds } s
recipe-no-output = { $name } nezapsal { $file }
