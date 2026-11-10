# machine-drafted, needs native review
## Panel příručky, zprávy o pádu, diagnostický balíček, kontrola aktualizací a O aplikaci. Čeština.
## Styl: krátce, jasně, bez vaty; jedna myšlenka na větu.

## Paleta

palette-help-manual = Nápověda
palette-help-manual-help = Otevřít příručku na stránce k tomu, co je na obrazovce.
palette-save-diagnostics = Uložit diagnostický balíček…
palette-save-diagnostics-help = Uložit zprávy o pádu, protokol a údaje o systému do souboru zip. Nic se neodesílá.
# Položka palety, která otevře oddíl příručky; { $title } je stránka nebo „Stránka › Oddíl“.
palette-help-topic = Nápověda: { $title }
palette-help-topic-help = Otevřít tento oddíl příručky.
# Pod nedostupným příkazem v paletě.
palette-disabled-help = Teď nedostupné. Stisknutím Enter zjistíte proč.

## Panel nápovědy

help-title = Nápověda
help-search-placeholder = Hledat v příručce
help-contents = Obsah
help-back = Zpět
help-forward = Vpřed
help-no-results = Pro „{ $query }“ nebylo nic nalezeno.
help-results = { $count ->
    [one] Nalezen 1 oddíl
    [few] Nalezeny { $count } oddíly
    [many] Nalezeno { $count } oddílu
   *[other] Nalezeno { $count } oddílů
}
help-section-start = Začínáme
help-section-modes = Prohlížení a režimy
help-section-tasks = Jak…
help-section-reference = Reference
help-keys-unbound = Nenastaveno
# Příkaz zmíněný v příručce bez klávesy; spouští se z palety.
help-key-unbound = { $name } (paleta)
help-keys-command = Příkaz
help-keys-keys = Klávesy
help-commands = Každý příkaz je v paletě příkazů ({ $key }). Napište část jeho názvu.
help-commands-no-key = Každý příkaz je v paletě příkazů. Napište část jeho názvu.
help-link = Přejít na { $title }
help-open = Nápověda
# Název panelu pro čtečky obrazovky; { $page } je zobrazená stránka.
help-label = Nápověda: { $page }
help-disabled-why = Proč?

## Zpráva o pádu nabízená při dalším spuštění

crash-title = { -app-name } se neočekávaně ukončil
crash-body = Zpráva byla uložena v tomto počítači. Nic nebylo odesláno.
crash-view = Zobrazit zprávu
crash-hide = Skrýt zprávu
crash-copy = Kopírovat
crash-copied = Zpráva zkopírována
crash-save-bundle = Uložit diagnostický balíček…
crash-dismiss = Zavřít

## Diagnostický balíček

diag-title = Uložit diagnostický balíček
diag-intro = Soubor zip k přiložení k hlášení chyby. Ukládá se v tomto počítači. Nic se neodesílá.
diag-contents = Obsah
diag-include-names = Zahrnout názvy souborů
diag-include-names-help = Vypnuto: názvy se nahradí za file-1.jpg, dir-1 a tak dále.
diag-privacy = Vaše domovská složka se zobrazí jako ~ a vaše uživatelské jméno jako <user>. Hesla, adresy serverů a obrázky nejsou nikdy zahrnuty.
diag-where = Nahlásit problém: { $issues } · { $email }
diag-save = Uložit…
diag-cancel = Zrušit
diag-saved = Diagnostický balíček uložen: { $path }
diag-failed = Diagnostický balíček nelze uložit: { $reason }
diag-entry-summary = Verze, systém, GPU a ovladač
diag-entry-crash = Poslední zpráva o pádu
diag-entry-crashes = Dřívější zprávy o pádu ({ $count })
diag-entry-log = Posledních { $count } řádků protokolu
diag-entry-settings = Nastavení bez hesel a adres
diag-entry-gpu = Údaje o GPU a displeji
diag-entry-readme = Co je tento balíček
diag-size = { $size }
# Soubor README.txt v balíčku.
diag-readme =
    Diagnostický balíček aplikace { -app-name }.

    Obsahuje to, co pomáhá najít problém: verzi a údaje o systému, zprávy o pádu, konec protokolu a nastavení.

    Vaše domovská složka se zobrazí jako ~ a vaše uživatelské jméno jako <user>. Pokud jste nezvolili jejich zahrnutí, názvy souborů a složek se nahradí za file-1, dir-1 a tak dále. Hesla a adresy serverů jsou odstraněny.

    Nic nebylo odesláno. Zda tento soubor sdílíte, rozhodujete vy.

## Aktualizace (Nastavení)

settings-section-updates = Aktualizace
settings-updates-check = Kontrolovat aktualizace
settings-updates-check-help = Jednou denně zjistit, zda existuje nová verze. Požadavek nese jen číslo verze: žádný identifikátor, žádnou cookie. Vypnuto: { -app-name } se kvůli aktualizacím nikdy nepřipojí.
settings-updates-channel = Kanál
settings-updates-channel-help = Stabilní: jen vydané verze. Beta: i testovací verze.
settings-updates-channel-stable = Stabilní
settings-updates-channel-beta = Beta
settings-updates-check-now = Zkontrolovat nyní
settings-updates-check-now-help = Zkontrolovat jednou nyní.
update-available = Je dostupná verze { $version }
update-notes = Poznámky k vydání
update-download = Stáhnout
update-none = { -app-name } je aktuální.
update-checking = Hledání aktualizací…
update-failed = Aktualizace nelze zkontrolovat: { $reason }
update-last-checked = Naposledy zkontrolováno { $when }

## O aplikaci

about-build = Sestavení { $commit } · { $profile } · { $target }
about-build-no-commit = Sestavení { $profile } · { $target }
about-help = Nápověda
about-diagnostics = Uložit diagnostický balíček…
# machine-drafted, needs native review
about-renderer = Informace o vykreslování…
about-licences-count = { $count ->
    [one] 1 komponenta třetí strany
    [few] { $count } komponenty třetích stran
    [many] { $count } komponenty třetích stran
   *[other] { $count } komponent třetích stran
}
about-licence-group = { $licence } ({ $count })
about-contact = Problémy: { $issues } · Podpora: { $email }
