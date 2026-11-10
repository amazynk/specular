# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Czech. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Místní AI
settings-ai-pack = Balíček místní AI
# machine-drafted, needs native review
settings-ai-pack-help = Volitelné funkce, které běží jen na tomto počítači. Nic, co najdou, ho neopustí. Pro každou funkci vyberte model; nic se nestáhne, dokud ho nezvolíte.
# machine-drafted, needs native review
settings-ai-install-from-file = Instalovat ze souboru…
settings-ai-install-from-file-help = Nainstaluje soubory modelů zkopírované do tohoto počítače, pro počítače bez internetu. Každý soubor se ověří podle seznamu dodaného s aplikací { -app-name }.
settings-ai-install-from-folder = Instalovat ze složky…
settings-ai-install-from-folder-help = Totéž pro všechny soubory modelů ve složce.
settings-ai-search = Hledání podle popisu
settings-ai-search-help = Najděte fotky jejich popisem, třeba „pes na pláži“, a lépe seskupte podobné snímky.
settings-ai-text = Text v obrázcích
settings-ai-text-help = Přečte text ve fotkách a skenech, aby je hledání text: našlo.
settings-ai-culling = Pomoc s tříděním
# machine-drafted, needs native review
settings-ai-culling-help = Označí zavřené oči, rozmazání a špatnou expozici, aby pomohl vybrat nejlepší snímek ze série.
settings-ai-people = Lidé
# machine-drafted, needs native review
settings-ai-people-help = Najde tváře a seskupí je podle osob pro hledání people:. Seskupování je vypnuté, dokud ho pro knihovnu nezapnete. Data tváří lze smazat pro jednu osobu nebo pro všechny najednou.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] V tomto sestavení nejsou k dispozici žádné modely.
        [installed] Nainstalováno
        [indexing] Nainstalováno · zaindexováno { $done } z { $total } obrázků
        [downloading] Stahuje se… { $percent }
        [failed] Stahování selhalo
       *[available] Nenainstalováno
    }
ai-removing = Odebírání…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Stáhnout
        [cancel] Zrušit
        [remove] Odebrat
        [forget] Zapomenout
        [faces] Smazat všechna data tváří
       *[pack] Odebrat balíček
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Smazat všechna data tváří? Smažou se všechny nalezené tváře a osoby. Vaše fotky zůstanou.
       *[pack] Odebrat balíček místní AI, jeho modely a vše, co našel? Vaše fotky se nezmění.
    }

## Gallery search
gallery-query-needs-ai = { $field }: vyžaduje balíček místní AI (Nastavení › Místní AI)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: balíček místní AI to zatím neumí vyhledávat

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Tváře zobrazeny
# machine-drafted, needs native review
faces-hidden = Tváře skryty
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Zavřené oči
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Zavřené oči
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Zavřené oči: { $closed } z { $faces } tváří
# machine-drafted, needs native review
cull-flag-blurred = Rozmazané
# machine-drafted, needs native review
cull-flag-dark = Příliš tmavé
# machine-drafted, needs native review
cull-flag-bright = Příliš světlé
# machine-drafted, needs native review
cull-flag-clipped = Oříznutá světla nebo stíny
# machine-drafted, needs native review
# The People view.
people-title = Lidé
# machine-drafted, needs native review
people-open = Lidé
# machine-drafted, needs native review
people-done = Hotovo
# machine-drafted, needs native review
people-back = Zpět
# machine-drafted, needs native review
people-show-photos = Zobrazit fotky
# machine-drafted, needs native review
people-name = Pojmenovat…
# machine-drafted, needs native review
people-name-prompt = Pojmenujte tuto osobu
# machine-drafted, needs native review
people-faces-open = Tváře
# machine-drafted, needs native review
people-faces-of = Tváře: { $name }
# machine-drafted, needs native review
people-merge = Sloučit
# machine-drafted, needs native review
people-merged = Sloučeno osob: { $count }
# machine-drafted, needs native review
people-merge-ask = Sloučit { $from } do { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Sloučit osoby ({ $count }) do { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Jejich tváře připadnou osobě { $into }. Fotky na disku se nezmění.
# machine-drafted, needs native review
people-not-this = Není to tato osoba
# machine-drafted, needs native review
people-face-removed = Tvář byla od této osoby odebrána. Fotka zůstává.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Osoba #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } tvář
        [few] { $count } tváře
        [many] { $count } tváře
       *[other] { $count } tváří
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } osoba
        [few] { $count } osoby
        [many] { $count } osoby
       *[other] { $count } osob
    }
# machine-drafted, needs native review
people-face-a11y = Tvář { $index }
# machine-drafted, needs native review
people-loading = Načítání…
# machine-drafted, needs native review
people-not-installed = Funkce Lidé není nainstalovaná. Stáhněte ji v Nastavení › Místní AI.
# machine-drafted, needs native review
people-grouping-off = Seskupování tváří je pro tuto knihovnu vypnuté. Zapněte „Seskupovat tváře do osob“ v Nastavení › Místní AI.
# machine-drafted, needs native review
people-none = Zatím nebyly nalezeny žádné osoby.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Model
settings-ai-search-size-help = SigLIP 2 B/16 najde fotky nejlépe, v mnoha jazycích. B/32 indexuje asi čtyřikrát rychleji, pro pomalejší grafické karty.
settings-ai-search-size-recommended = Výchozí
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Model
settings-ai-text-size-help = PP-OCRv6 small čte přesněji, tiny je rychlejší.
settings-ai-text-size-recommended = Výchozí
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Velký
# machine-drafted, needs native review
settings-ai-culling-size = Model
settings-ai-culling-size-help = Model, který rozpozná zavřené oči.
settings-ai-culling-size-recommended = Výchozí
settings-ai-culling-size-small = Malý
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Velký
# machine-drafted, needs native review
settings-ai-people-size = Model
settings-ai-people-size-help = SFace je malý. AuraFace je větší, trénovaný na datech, která jeho vydavatel označuje za licencovaná pro komerční použití.
settings-ai-people-size-recommended = Výchozí
settings-ai-people-size-small = Malý
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Malý
        [large] Velký
       *[standard] Standardní
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Výchozí ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Výchozí pro vaši kartu { $gpu }: { $size }
ai-recommended = Výchozí pro tento počítač: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Do paměti tohoto počítače se nevejde žádná velikost.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Běží na: GPU ({ $gpu })
        [gpu-unnamed] Běží na: GPU
        [no-gpu] Běží na: CPU (GPU nenalezena)
        [differed] Běží na: CPU — výsledek GPU se lišil od CPU
        [failed] Běží na: CPU — model na GPU selhal
        [setting] Běží na: CPU (zpracování je nastaveno na „Pouze CPU“)
        [checking] Běží na: CPU — probíhá ověření GPU vůči němu
       *[cpu] Běží na: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } na disku
       *[download] { $bytes } ke stažení
    } · { $memory } paměti{ $gpu ->
        [yes] { " · vyžaduje GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Vyžaduje GPU a tento počítač žádnou použitelnou nemá.
       *[memory] Vyžaduje { $need } paměti, víc než { $have }, které Specular na tomto počítači povoluje pro AI.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = Používá se { $current }, ne { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Zpět na výchozí
        [anyway] Přesto použít
       *[use] Použít tuto velikost
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Vybráno, ale zatím nestaženo: dál se používá velikost { $current }.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] Používá se
        [downloaded] Staženo, nepoužívá se
        [missing] Nestaženo
        [previous] Používá se, dokud nová velikost nezaindexuje knihovnu
        [target] Stahuje se…
        [failed] Stahování selhalo
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Zobrazit licenci
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: přepnout na velikost { $size }? Knihovna se znovu zaindexuje; do té doby se používá současná velikost.
       *[other] { $feature }: přepnout na velikost { $size }? Stáhne se { $download } a knihovna se znovu zaindexuje; do té doby se používá současná velikost.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: přesto použít velikost { $size }? Na tomto počítači nemusí běžet, nebo jen velmi pomalu.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: velikost { $size } teď tomuto počítači vyhovuje lépe. Přepnout? { $download ->
        [none] Knihovna se znovu zaindexuje.
       *[other] Stáhne se { $download } a knihovna se znovu zaindexuje.
    }
ai-suggest-toast = Místní AI: tomuto počítači lépe vyhovuje jiná velikost modelu (Nastavení › Místní AI).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Přepínání na velikost { $size } · zaindexováno { $done } z { $total } obrázků

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Přijde v pozdější verzi
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Mnoho jazyků
        [english] Jen angličtina
        [cjk-latin] Čínština, japonština a latinka
        [zh-latin] Čínština a latinka
        [arabic] Arabština
        [hebrew] Hebrejština
        [korean] Korejština
        [cyrillic] Cyrilice
        [greek] Řečtina
        [thai] Thajština
        [devanagari] Dévanágarí
        [tamil] Tamilština
        [telugu] Telugština
        [eslav] Ruština, ukrajinština a běloruština
        [latin] Latinka (mnoho evropských jazyků)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Rychlejší, pro pomalejší grafické karty
        [closed-eyes] Rozpozná zavřené oči
        [small-faces] Lepší u malých tváří
        [no-translate] Neumí překládat do angličtiny
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Trénovací data: { $source }
        [research] Trénovací data: { $source } (licence jen pro výzkum)
       *[unstated] { $named ->
            [no] Trénovací data: neuvedena, podmínky neuvedeny
           *[yes] Trénovací data: { $source }, podmínky neuvedeny
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Stahuje se z { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Seskupovat tváře do osob
ai-grouping-help = Vypnuto, dokud ho pro tuto knihovnu nezapnete. Vypnutí zastaví seskupování; už seskupené tváře zůstanou, dokud je nesmažete.
ai-grouping-state = { $on ->
        [yes] Zapnuto v této knihovně
       *[no] Vypnuto
    }
ai-grouping-button = { $on ->
        [yes] Vypnout
       *[no] Zapnout
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Nejprve stáhněte { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Odebrat { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Jeho soubory modelu se smažou. Vaše fotky a už nalezený text zůstanou.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Číst text ve fotkách této knihovny
# machine-drafted, needs native review
ai-text-indexing-help = Vypnuto, dokud ho pro tuto knihovnu nezapnete. Pak text: ve vyhledávání Knihovny najde fotky podle slov, která obsahují.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Zapnuto v této knihovně
       *[no] Vypnuto
    }

## Instalace ze souboru

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Zkopírován jeden soubor modelu. Nainstalujte ho tlačítkem Stáhnout.
        [few] Zkopírovány { $count } soubory modelů. Nainstalujte je tlačítkem Stáhnout.
        [many] Zkopírováno { $count } souboru modelu. Nainstalujte je tlačítkem Stáhnout.
       *[other] Zkopírováno { $count } souborů modelů. Nainstalujte je tlačítkem Stáhnout.
    }
ai-sideload-none = Nebyly nalezeny žádné soubory modelů.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } není model, který aplikace { -app-name } zná. Nebyl zkopírován.
ai-sideload-failed = { $file } nelze zkopírovat: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Použít neověřený model?
ai-unverified-detail = { $file } není na seznamu modelů dodaných s aplikací { -app-name }, takže ho nelze ověřit. Použije se jako { $model }.
ai-unverified-use = Použít neověřený model

# machine-drafted, needs native review
ai-failed-internal = Uvnitř aplikace { -app-name } se něco pokazilo. Podrobnosti jsou v protokolu.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Přepnout
       *[keep] Ponechat současnou velikost
    }
