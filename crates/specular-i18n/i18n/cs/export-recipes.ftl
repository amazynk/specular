# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Exportovat podle receptů…
palette-export-recipes-help = Exportovat výběr podle jednoho nebo více uložených receptů.
palette-develop-export-recipes = Exportovat podle receptů…
palette-develop-export-recipes-help = Exportovat upravenou fotografii, nebo několik, podle uložených receptů.

export-recipes-title = Export podle receptů
export-recipes-new = Nový
export-recipes-edit = Upravit…
export-recipes-duplicate = Duplikovat
export-recipes-delete = Smazat…
export-recipes-start = Exportovat
export-recipes-save = Uložit
export-recipes-open-folder = Otevřít složku
export-recipes-choose-image = Zvolit obrázek…
export-recipes-no-images = Žádné obrázky k exportu.
export-recipes-none-checked = Zaškrtněte alespoň jeden recept.
export-recipes-checked = { $name }: zapnuto
export-recipes-unchecked = { $name }: vypnuto

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Tisk, plná velikost
export-recipes-starter-email = E-mail, malý
export-recipes-unnamed = Recept bez názvu
export-recipes-new-name = Nový recept
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (kopie)

## The list's one-line summary.
export-recipes-summary-mark = vodoznak
export-recipes-size-full = Plná velikost
export-recipes-size-long = { $px } px delší strana
export-recipes-size-short = { $px } px kratší strana
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Vedle originálu
export-recipes-folder-sub = Složka „{ $name }“
export-recipes-folder-ask = Pokaždé se zeptat

## The editor.
export-recipes-new-title = Nový recept
export-recipes-edit-title = Upravit recept
export-recipes-name = Název receptu
export-recipes-text = Vlastní text
export-recipes-pattern = Název souboru
export-recipes-mark-text-field = Text vodoznaku
export-recipes-subfolder = Název složky
export-recipes-format = Formát
export-recipes-quality = Kvalita
export-recipes-lossless = Bezeztrátově
export-recipes-bits = Bitová hloubka
export-recipes-bits-8 = 8 bitů
export-recipes-bits-16 = 16 bitů
export-recipes-space = Barevný prostor
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profil
export-recipes-profile-embed = Vložit
export-recipes-profile-none = Vynechat
export-recipes-folder = Složka
export-recipes-folder-subfolder = Podsložka
export-recipes-collision = Název obsazen
export-recipes-collision-unique = Přidat číslo
export-recipes-collision-overwrite = Nahradit (zeptá se)
export-recipes-collision-skip = Přeskočit
export-recipes-after = Po dokončení
export-recipes-after-nothing = Nic
export-recipes-after-open = Otevřít složku
export-recipes-resize = Velikost
export-recipes-resize-long = Delší strana
export-recipes-resize-short = Kratší strana
export-recipes-resize-mp = Megapixely
export-recipes-resize-exact = Přesná velikost
export-recipes-resize-percent = Procenta
export-recipes-edge = Strana
export-recipes-width = Šířka
export-recipes-height = Výška
export-recipes-megapixels = Megapixely
export-recipes-percent = Procenta
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Zvětšovat
export-recipes-sharpen = Doostřit pro
export-recipes-sharpen-off = Vypnuto
export-recipes-sharpen-screen = Obrazovka
export-recipes-sharpen-matte = Matný papír
export-recipes-sharpen-glossy = Lesklý papír
export-recipes-amount = Síla
export-recipes-amount-low = Nízká
export-recipes-amount-standard = Standardní
export-recipes-amount-high = Vysoká
export-recipes-metadata = Metadata
export-recipes-meta-all = Vše
export-recipes-meta-copyright = Jen autorská práva
export-recipes-meta-contact = Autorská práva a kontakt
export-recipes-meta-none = Žádná
export-recipes-location = Poloha
export-recipes-people = Zobrazené osoby
export-recipes-keep = Ponechat
export-recipes-remove = Odebrat
export-recipes-mark = Vodoznak
export-recipes-mark-none = Žádný
export-recipes-mark-text = Text
export-recipes-mark-image = Obrázek (zvolte)
export-recipes-position = Umístění
export-recipes-pos-top-left = Vlevo nahoře
export-recipes-pos-top = Nahoře
export-recipes-pos-top-right = Vpravo nahoře
export-recipes-pos-left = Vlevo
export-recipes-pos-centre = Uprostřed
export-recipes-pos-right = Vpravo
export-recipes-pos-bottom-left = Vlevo dole
export-recipes-pos-bottom = Dole
export-recipes-pos-bottom-right = Vpravo dole
export-recipes-opacity = Krytí
export-recipes-scale = Šířka
export-recipes-yes = Ano
export-recipes-no = Ne
export-recipes-error-name = Pojmenujte recept.
export-recipes-error-mark-text = Napište text vodoznaku.
export-recipes-error-mark-image = Zvolte obrázek vodoznaku.
export-recipes-error-subfolder = Napište název složky.
export-recipes-saved = Recept „{ $name }“ uložen.

## Delete.
export-recipes-delete-title = Smazat recept „{ $name }“?
export-recipes-delete-detail = Soubory, které exportoval, zůstanou na disku.
export-recipes-delete-confirm = Smazat
export-recipes-delete-keep = Ponechat
export-recipes-deleted = Recept „{ $name }“ smazán.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Složka pro „{ $name }“
export-recipes-asking = Zvolte složku pro každý recept, který se ptá.
export-recipes-no-folder = Nebyla zvolena žádná složka. Nic nebylo exportováno.
export-recipes-planning = Pojmenovávání souborů…
export-recipes-plan-pattern = Recept „{ $name }“: { $reason }
export-recipes-replace-title = { $count ->
    [one] Nahradit 1 soubor?
    [few] Nahradit { $count } soubory?
    [many] Nahradit { $count } souboru?
   *[other] Nahradit { $count } souborů?
}
export-recipes-replace-detail = Originály se nikdy nenahrazují.
export-recipes-replace-confirm = Nahradit
export-recipes-replace-kept = Nic nebylo exportováno.
export-recipes-running = Exportuje se…
export-recipes-progress = { $done } z { $total } souborů
export-recipes-done = { $count ->
    [one] Exportován 1 soubor.
    [few] Exportovány { $count } soubory.
    [many] Exportováno { $count } souboru.
   *[other] Exportováno { $count } souborů.
}
export-recipes-stopped = { $count ->
    [one] Zastaveno. Exportován 1 soubor.
    [few] Zastaveno. Exportovány { $count } soubory.
    [many] Zastaveno. Exportováno { $count } souboru.
   *[other] Zastaveno. Exportováno { $count } souborů.
}
export-recipes-done-skipped = { $count ->
    [one] Přeskočen 1: název byl obsazen.
    [few] Přeskočeny { $count }: názvy byly obsazeny.
    [many] Přeskočeno { $count }: názvy byly obsazeny.
   *[other] Přeskočeno { $count }: názvy byly obsazeny.
}
export-recipes-done-failed = { $count ->
    [one] Selhal 1:
    [few] Selhaly { $count }:
    [many] Selhalo { $count }:
   *[other] Selhalo { $count }:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
