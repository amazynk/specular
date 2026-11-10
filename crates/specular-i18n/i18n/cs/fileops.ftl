# machine-drafted, needs native review
# Operace se soubory: přejmenování, hromadné přejmenování, kopírování a přesun do složky, otočení
# uložené do souboru, zpět a znovu, tapeta, sdílení, odeslání e-mailem, export a import nastavení.

## Paleta, přehled zkratek a nabídky

palette-undo = Zpět
palette-undo-help = Vrátit poslední změnu souborů: přesun do koše, přejmenování, kopírování nebo přesun, otočení.
palette-redo = Znovu
palette-redo-help = Znovu provést to, co vrátil poslední krok zpět.
palette-rename = Přejmenovat…
palette-rename-help = Dát souboru nový název; doprovodné soubory a partner RAW+JPEG se přejmenují také.
palette-batch-rename = Přejmenovat soubory…
palette-batch-rename-help = Přejmenovat vybrané soubory nebo celou složku podle vzoru, s náhledem.
palette-copy-to-folder = Kopírovat do složky…
palette-copy-to-folder-help = Zkopírovat soubor nebo výběr do jiné složky.
palette-move-to-folder = Přesunout do složky…
palette-move-to-folder-help = Přesunout soubor nebo výběr do jiné složky.
palette-rotate-file-left = Otočit soubor doleva
palette-rotate-file-left-help = Otočit samotný soubor o čtvrt otáčky doleva, bezeztrátově, pokud to formát dovolí.
palette-rotate-file-right = Otočit soubor doprava
palette-rotate-file-right-help = Otočit samotný soubor o čtvrt otáčky doprava, bezeztrátově, pokud to formát dovolí.
palette-set-wallpaper-help = Použít obrázek jako pozadí plochy.
palette-share = Sdílet…
palette-share-help = Poslat soubor do jiné aplikace systémovou nabídkou sdílení.
palette-send-by-email = Odeslat e-mailem…
palette-send-by-email-help = Založit nový e-mail v poštovní aplikaci se souborem v příloze.
# machine-drafted, needs native review
palette-share-with-zynk = Sdílet přes Zynk…
palette-share-with-zynk-help = Vytvoří webový odkaz na soubor v aplikaci Zynk.
palette-send-with-zynk = Odeslat přes Zynk…
palette-send-with-zynk-help = Odeslat soubor kontaktu nebo do některého z vašich zařízení pomocí Zynk.
palette-export-settings = Exportovat nastavení…
palette-export-settings-help = Uložit všechna nastavení a klávesové zkratky do souboru.
palette-import-settings = Importovat nastavení…
palette-import-settings-help = Načíst nastavení ze souboru po zobrazení toho, co se změní.

disabled-in-archive-file = Tato stránka je uvnitř archivu a nemá vlastní soubor
disabled-not-rotatable = Tento typ souboru neumí uložit otočení
disabled-not-on-this-desktop = Vaše pracovní prostředí to nenabízí

## Nastavení

settings-browsing-rotate-trim-edges = Oříznout okraje JPEG pro bezeztrátové otočení
settings-browsing-rotate-trim-edges-help = JPEG, jehož rozměr není násobkem 8 nebo 16 pixelů, lze otočit bez ztráty jen odříznutím posledních pixelů. Bez této volby se místo toho změní značka orientace a oříznutí se nabídne pokaždé.
settings-browsing-rotate-keep-modified = Při otáčení zachovat datum změny
settings-browsing-rotate-keep-modified-help = Otočený soubor si ponechá datum poslední změny, takže při řazení podle data zůstane na místě.
settings-browsing-recent-folders = Nedávné složky
settings-browsing-recent-folders-help = Naposledy použité složky pro kopírování a přesun.

## Dialogy

fileop-cancel = Zrušit
rename-title = Přejmenovat
rename-field-label = Název
rename-button = Přejmenovat
rename-sidecars-follow = Přejmenováno také: { $files }
rename-extension-changed = Změna přípony soubor nepřevede; jiné aplikace ho nemusí otevřít.
rename-problem-empty = Zadejte název.
rename-problem-invalid-char = Názvy nesmí obsahovat „{ $char }“.
rename-problem-reserved = „{ $name }“ je název, který Windows vyhrazuje zařízením.
rename-problem-too-long = Název je příliš dlouhý; nejvýše 255 bajtů.
rename-problem-dot = „.“ a „..“ nelze použít jako název.
rename-problem-trailing = Názvy nesmí končit mezerou ani tečkou.
rename-problem-exists = Tento název už má jiný soubor.
rename-problem-read-only = Tuto složku nelze měnit.
rename-problem-duplicate = Tento název by dostaly dva soubory.

batch-rename-title =
    { $count ->
        [one] Přejmenovat { $count } soubor
        [few] Přejmenovat { $count } soubory
        [many] Přejmenovat { $count } souboru
       *[other] Přejmenovat { $count } souborů
    }
batch-rename-field-label = Vzor
batch-rename-tokens = Použijte { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } a { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] Přejmenuje se { $count } soubor
        [few] Přejmenují se { $count } soubory
        [many] Přejmenuje se { $count } souboru
       *[other] Přejmenuje se { $count } souborů
    }{ $problems ->
        [0] .
        [one] ; { $problems } nelze a zůstane beze změny.
        [few] ; { $problems } nelze a zůstanou beze změny.
        [many] ; { $problems } nelze a zůstane beze změny.
       *[other] ; { $problems } nelze a zůstane beze změny.
    }
batch-rename-reading = Načítání dat a fotoaparátů…
batch-rename-preview = Náhled
batch-rename-unchanged = ponechá si název
batch-rename-more =
    { $count ->
        [one] { $count } další níže
        [few] { $count } další níže
        [many] { $count } dalšího níže
       *[other] { $count } dalších níže
    }

transfer-copy-title-one = Kopírovat „{ $file }“ do složky
transfer-copy-title =
    { $count ->
        [one] Kopírovat { $count } soubor do složky
        [few] Kopírovat { $count } soubory do složky
        [many] Kopírovat { $count } souboru do složky
       *[other] Kopírovat { $count } souborů do složky
    }
transfer-move-title-one = Přesunout „{ $file }“ do složky
transfer-move-title =
    { $count ->
        [one] Přesunout { $count } soubor do složky
        [few] Přesunout { $count } soubory do složky
        [many] Přesunout { $count } souboru do složky
       *[other] Přesunout { $count } souborů do složky
    }
transfer-recent = Nedávné složky
transfer-folder = Složka
transfer-choose-folder = Vybrat složku…
transfer-copy-dialog = Kopírovat do složky
transfer-move-dialog = Přesunout do složky
transfer-sidecars =
    { $count ->
        [one] Přidá se { $count } doprovodný soubor.
        [few] Přidají se { $count } doprovodné soubory.
        [many] Přidá se { $count } doprovodného souboru.
       *[other] Přidá se { $count } doprovodných souborů.
    }
transfer-taken =
    { $count ->
        [one] { $count } název už ve složce { $folder } existuje.
        [few] { $count } názvy už ve složce { $folder } existují.
        [many] { $count } názvu už ve složce { $folder } existuje.
       *[other] { $count } názvů už ve složce { $folder } existuje.
    }
transfer-when-taken = Když název už existuje
transfer-skip = Přeskočit
transfer-keep-both = Ponechat oba
transfer-replace-option = Nahradit
transfer-copy = Kopírovat
transfer-move = Přesunout
transfer-replace-title =
    { $count ->
        [one] Nahradit { $count } soubor ve složce { $folder }?
        [few] Nahradit { $count } soubory ve složce { $folder }?
        [many] Nahradit { $count } souboru ve složce { $folder }?
       *[other] Nahradit { $count } souborů ve složce { $folder }?
    }
transfer-replace-note = Stávající soubory půjdou do koše. Krok zpět je vrátí.
transfer-replace = Nahradit
transfer-back = Zpět
transfer-stop = Zastavit

settings-file-import-title = Importovat nastavení
settings-file-import-dialog = Importovat nastavení
settings-file-export-dialog = Exportovat nastavení
settings-file-import-changes = Změny
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = bez klávesy
settings-file-import-unknown = Není nastavením této verze; vynecháno.
settings-file-import-keymap = Klávesové zkratky
settings-file-import-nothing = Nic by se nezměnilo: tato nastavení už platí.
settings-file-import-summary =
    { $count ->
        [one] { $count } změna. Import lze vrátit v Nastavení.
        [few] { $count } změny. Import lze vrátit v Nastavení.
        [many] { $count } změny. Import lze vrátit v Nastavení.
       *[other] { $count } změn. Import lze vrátit v Nastavení.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } položka vypadá jako heslo nebo klíč a nikdy se neimportuje.
        [few] { $count } položky vypadají jako hesla nebo klíče a nikdy se neimportují.
        [many] { $count } položky vypadá jako hesla nebo klíče a nikdy se neimportuje.
       *[other] { $count } položek vypadá jako hesla nebo klíče a nikdy se neimportuje.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } uživatelský příkaz se neimportuje: příkazy spouštějí programy. Pokud mu důvěřujete, zkopírujte ho do souboru nastavení sami.
        [few] { $count } uživatelské příkazy se neimportují: příkazy spouštějí programy. Pokud jim důvěřujete, zkopírujte je do souboru nastavení sami.
        [many] { $count } uživatelského příkazu se neimportuje: příkazy spouštějí programy. Pokud jim důvěřujete, zkopírujte je do souboru nastavení sami.
       *[other] { $count } uživatelských příkazů se neimportuje: příkazy spouštějí programy. Pokud jim důvěřujete, zkopírujte je do souboru nastavení sami.
    }
settings-file-import-button = Importovat

## Výsledky

fileop-renamed = Přejmenováno na { $file }
fileop-renamed-count =
    { $count ->
        [one] Přejmenován { $count } soubor
        [few] Přejmenovány { $count } soubory
        [many] Přejmenováno { $count } souboru
       *[other] Přejmenováno { $count } souborů
    }
fileop-copied =
    { $count ->
        [one] { $count } soubor zkopírován do { $folder }
        [few] { $count } soubory zkopírovány do { $folder }
        [many] { $count } souboru zkopírováno do { $folder }
       *[other] { $count } souborů zkopírováno do { $folder }
    }
fileop-moved =
    { $count ->
        [one] { $count } soubor přesunut do { $folder }
        [few] { $count } soubory přesunuty do { $folder }
        [many] { $count } souboru přesunuto do { $folder }
       *[other] { $count } souborů přesunuto do { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } přeskočen: jeho název tam už existuje.
        [few] { $count } přeskočeny: jejich názvy tam už existují.
        [many] { $count } přeskočeno: jejich názvy tam už existují.
       *[other] { $count } přeskočeno: jejich názvy tam už existují.
    }
fileop-stopped = Zastaveno; co je hotové, zůstává hotové.
fileop-failed-file =
    { $count ->
        [one] { $file } nelze zkopírovat ani přesunout: { $reason }
        [few] { $count } soubory nelze zkopírovat ani přesunout; první, { $file }: { $reason }
        [many] { $count } souboru nelze zkopírovat ani přesunout; první, { $file }: { $reason }
       *[other] { $count } souborů nelze zkopírovat ani přesunout; první, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Kopírování { $count } souboru… { PERCENT($percent) }
        [few] Kopírování { $count } souborů… { PERCENT($percent) }
        [many] Kopírování { $count } souboru… { PERCENT($percent) }
       *[other] Kopírování { $count } souborů… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Přesouvání { $count } souboru… { PERCENT($percent) }
        [few] Přesouvání { $count } souborů… { PERCENT($percent) }
        [many] Přesouvání { $count } souboru… { PERCENT($percent) }
       *[other] Přesouvání { $count } souborů… { PERCENT($percent) }
    }
fileop-done-status = Hotovo
fileop-busy = Poslední změna souborů ještě probíhá; zkuste to za chvíli.
fileop-nothing-to-redo = Není co opakovat
fileop-undone = Vráceno: { $what }
fileop-redone = Znovu provedeno: { $what }
fileop-kind-rename = přejmenování
fileop-kind-batch-rename = přejmenování více souborů
fileop-kind-move = přesun do složky
fileop-kind-copy = kopírování do složky
fileop-kind-rotate = otočení
rename-failed = Nelze přejmenovat: { $reason }

undo-problem-changed = Nelze vrátit: { $file } se mezitím změnil a vrácení by tu změnu ztratilo.
undo-problem-missing = Nelze vrátit: { $file } už neexistuje.
undo-problem-taken = Nelze vrátit: název { $file } teď má jiný soubor.
undo-problem-io = Změnu souboru { $file } nelze vrátit: { $reason }

rotate-done-lossless = { $file } otočen zcela bez ztráty.
rotate-done-lossless-trimmed = { $file } otočen zcela bez ztráty; okraj byl oříznut na { $width } × { $height }.
rotate-done-tag = { $file } otočen změnou značky orientace.
rotate-done-sidecar = Otočení souboru { $file } uloženo do { $sidecar }; samotný soubor RAW se nikdy nemění.
rotate-tag-unreadable = Obrazová data nešlo přečíst dost daleko, aby se dal otočit.
rotate-tag-multi-picture = Obsahuje více obrázků a otočení prvního by poškodilo ostatní.
rotate-tag-trailer = Za obrázkem následují data, která by otočení posunulo.
rotate-tag-not-aligned = Bezeztrátové otočení pixelů by ho ořízlo na { $width } × { $height }.
rotate-trim-offer = Oříznout a otočit
rotate-problem-unsupported = { $file } je typ souboru, který neumí uložit otočení.
rotate-problem-too-large = { $file } je na otočení příliš velký.
rotate-problem-damaged = { $file } je poškozen v místě, kde je uložena orientace: { $reason }
rotate-problem-io = { $file } nelze otočit: { $reason }

share-failed = Nelze sdílet: { $reason }
email-failed = Nelze spustit poštovní aplikaci: { $reason }
# machine-drafted, needs native review
zynk-opening = Otevírání aplikace Zynk…
zynk-failed = Nelze se spojit se Zynk: { $reason }
zynk-get-title-send = „Odeslat přes Zynk“ potřebuje aplikaci Zynk
zynk-get-title-share = „Sdílet přes Zynk“ potřebuje aplikaci Zynk
zynk-get-about = Zynk je bezplatná aplikace pro Mac, Windows, Linux, iPhone a Android. Posílá fotky, videa i celé složky přímo do vašich dalších zařízení a dalším lidem, v jakékoli velikosti. Přenosy jsou koncově šifrované, a když spojení vypadne, pokračují tam, kde skončily. Pro příjemce, který Zynk nemá, vytvoří webový odkaz, ze kterého si soubory stáhne v prohlížeči.
zynk-get-macos = Získejte ji v Mac App Storu. Jakmile bude nainstalovaná, { -app-name } jí vaše soubory předá rovnou.
zynk-get-windows = Získejte ji v Microsoft Storu. Jakmile bude nainstalovaná, { -app-name } jí vaše soubory předá rovnou.
zynk-get-linux = Zynk pro Linux je AppImage, nic se neinstaluje: stáhněte ho, vložte ho do ~/Applications a nastavte ho jako spustitelný (ve vlastnostech souboru nebo příkazem chmod +x). { -app-name } ho najde tam, a kdekoli jinde, když Zynk běží.
zynk-get = Získat Zynk
zynk-send-old-windows = „Odeslat přes Zynk“ vyžaduje Windows 10 verze 2004 nebo novější. „Sdílet přes Zynk“ v tomto Windows funguje.
settings-file-export-done = Nastavení uloženo do { $file }
settings-file-export-failed = Nastavení nelze uložit: { $reason }
settings-file-import-unreadable = { $file } nelze přečíst: { $reason }
settings-file-import-done =
    { $count ->
        [one] Importována { $count } změna
        [few] Importovány { $count } změny
        [many] Importováno { $count } změny
       *[other] Importováno { $count } změn
    }
