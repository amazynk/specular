# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Exportovat obrázek…
# machine-drafted, needs native review
palette-export-figure-help = Uložit zobrazení jako obrázek s měřítkem, barevnou škálou a popisky.
# machine-drafted, needs native review
palette-toggle-key-image = Označit klíčový snímek
# machine-drafted, needs native review
palette-toggle-key-image-help = Označit tento snímek jako klíčový, nebo označení zrušit.
# machine-drafted, needs native review
palette-copy-key-images = Kopírovat klíčové snímky
# machine-drafted, needs native review
palette-copy-key-images-help = Zkopírovat klíčové snímky jako jeden obrázek.
# machine-drafted, needs native review
palette-export-key-images = Exportovat klíčové snímky…
# machine-drafted, needs native review
palette-export-key-images-help = Uložit klíčové snímky jako prezentaci nebo složku s PNG.
# machine-drafted, needs native review
palette-export-teaching-file = Exportovat výukový soubor…
# machine-drafted, needs native review
palette-export-teaching-file-help = Uložit klíčové snímky bez údajů o pacientovi, pro výuku.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Exportovat obrázek
# machine-drafted, needs native review
figure-format = Formát
# machine-drafted, needs native review
figure-format-png = PNG, 16bitový
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16bitový
# machine-drafted, needs native review
figure-region = Oblast
# machine-drafted, needs native review
figure-region-visible = Viditelná oblast
# machine-drafted, needs native review
figure-region-whole = Celý snímek
# machine-drafted, needs native review
figure-size = Velikost
# machine-drafted, needs native review
figure-size-pixels = Pixely snímku (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = Šířka { $width } px
# machine-drafted, needs native review
figure-scale-bar = Měřítko
# machine-drafted, needs native review
figure-scale-bar-none = Bez kalibrace
# machine-drafted, needs native review
figure-colour-bar = Barevná škála
# machine-drafted, needs native review
figure-colour-bar-none = Bez barevné mapy nebo okna
# machine-drafted, needs native review
figure-inset = Výřez
# machine-drafted, needs native review
figure-inset-rectangle = Poslední obdélník
# machine-drafted, needs native review
figure-inset-none = Nejdřív nakreslete obdélník
# machine-drafted, needs native review
figure-channels = Kanály
# machine-drafted, needs native review
figure-channels-merged = Sloučené
# machine-drafted, needs native review
figure-channels-split = Zvlášť i sloučené
# machine-drafted, needs native review
figure-channels-one = Jeden kanál
# machine-drafted, needs native review
figure-measurements = Měření
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Zapnuto ({ $count })
    [few] Zapnuto ({ $count })
    [many] Zapnuto ({ $count })
   *[other] Zapnuto ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Žádná
# machine-drafted, needs native review
figure-title-row = Název
# machine-drafted, needs native review
figure-provenance = Původ
# machine-drafted, needs native review
figure-provenance-metadata = V souboru
# machine-drafted, needs native review
figure-provenance-drawn = V souboru a nakreslený
# machine-drafted, needs native review
figure-file = Soubor
# machine-drafted, needs native review
figure-choose = Vybrat…
# machine-drafted, needs native review
figure-on = Zapnuto
# machine-drafted, needs native review
figure-off = Vypnuto
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Zapnuto · { $source }
# machine-drafted, needs native review
figure-export = Exportovat
# machine-drafted, needs native review
figure-working = Exportuje se…
# machine-drafted, needs native review
figure-not-full-res = Pixely v plném rozlišení nejsou načteny. Přibližte na { PERCENT(100) } nebo vyberte velikost.
# machine-drafted, needs native review
figure-channel-red = Červená
# machine-drafted, needs native review
figure-channel-green = Zelená
# machine-drafted, needs native review
figure-channel-blue = Modrá
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, { $lo } až { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Okno { $lo } až { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Barvy kanálů { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Kanál { $channel }
# machine-drafted, needs native review
figure-display-hilo = Značky HiLo
# machine-drafted, needs native review
figure-display-as-stored = Tak, jak je uloženo
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Kalibrace: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Zobrazení: { $value }
# machine-drafted, needs native review
figure-not-image = Obrázky se vytvářejí ze snímku v prohlížeči.
# machine-drafted, needs native review
figure-not-ready = Snímek se ještě načítá.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Příliš velké: { $width } × { $height } px. Vyberte viditelnou oblast nebo velikost.

## Key images
# machine-drafted, needs native review
figure-keys-title = Exportovat klíčové snímky
# machine-drafted, needs native review
figure-teaching-title = Exportovat výukový soubor
# machine-drafted, needs native review
figure-keys-images = Snímky
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } klíčový snímek
    [few] { $count } klíčové snímky
    [many] { $count } klíčového snímku
   *[other] { $count } klíčových snímků
}
# machine-drafted, needs native review
figure-keys-pptx = Prezentace (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Složka s PNG
# machine-drafted, needs native review
figure-keys-preset = Předvolba
# machine-drafted, needs native review
figure-keys-standard = Standardní
# machine-drafted, needs native review
figure-keys-teaching = Výukový soubor
# machine-drafted, needs native review
figure-keys-overlays-on = Nakreslená a v poznámkách
# machine-drafted, needs native review
figure-keys-masks = Vtištěný text
# machine-drafted, needs native review
figure-keys-masks-auto = Začerněný
# machine-drafted, needs native review
figure-keys-notes = Poznámky k případu
# machine-drafted, needs native review
figure-keys-destination = Uložit do
# machine-drafted, needs native review
figure-keys-empty = Zatím žádné klíčové snímky. Nejdřív označte snímek jako klíčový.
# machine-drafted, needs native review
figure-keys-teaching-note = Údaje o pacientovi se vynechají a vtištěný text se začerní. Před sdílením zkontrolujte každý snímek.
# machine-drafted, needs native review
figure-keys-none-left = Žádný snímek nešlo exportovat bez údajů o pacientovi.
# machine-drafted, needs native review
figure-keys-copied = Klíčové snímky zkopírovány
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Označeno jako klíčový snímek ({ $count })
    [few] Označeno jako klíčový snímek ({ $count })
    [many] Označeno jako klíčový snímek ({ $count })
   *[other] Označeno jako klíčový snímek ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Už to není klíčový snímek (zbývá { $count })
    [few] Už to není klíčový snímek (zbývají { $count })
    [many] Už to není klíčový snímek (zbývá { $count })
   *[other] Už to není klíčový snímek (zbývá { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Snímek { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Klíčový snímek { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = Uloženo: { $name }
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] Uloženo: { $name }. { $count } snímek byl vynechán: nebyl nalezen jeho vtištěný text.
    [few] Uloženo: { $name }. { $count } snímky byly vynechány: nebyl nalezen jejich vtištěný text.
    [many] Uloženo: { $name }. { $count } snímku bylo vynecháno: nebyl nalezen jejich vtištěný text.
   *[other] Uloženo: { $name }. { $count } snímků bylo vynecháno: nebyl nalezen jejich vtištěný text.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Export se nezdařil: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } už v té složce je. Vyberte prázdnou složku.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Klíčový snímek
