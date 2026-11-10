# machine-drafted, needs native review
# Specular — Patologie: separace barviv, export a import anotací, objemy štítků nad objemy. Čeština.

## Paleta
palette-stain-separation = Rozdělit barviva
palette-stain-1 = Zobrazit nebo skrýt barvivo 1
palette-stain-2 = Zobrazit nebo skrýt barvivo 2
palette-stain-3 = Zobrazit nebo skrýt barvivo 3
palette-stain-mode = Barviva jako světlo nebo optická hustota
palette-stain-estimate = Odhadnout barviva z oblasti
palette-stain-normalise = Normalizovat barvy barviv
palette-stain-reference = Použít oblast jako referenci barvení
palette-stain-reset = Publikované vektory barviv
palette-export-annotations = Exportovat anotace…
palette-import-annotations = Importovat anotace…
palette-label-volume = Překrýt objemem štítků…

## Důvody nedostupnosti
disabled-not-colour-image = Vyžaduje barevný obrázek
disabled-stains-off = Nejprve zapněte separaci barviv
disabled-no-region = Nejprve nakreslete oblast
disabled-no-slide-annotations = Žádné anotace, oblasti ani body k exportu
disabled-no-label-target = Otevřete objem v multiplanárních pohledech, nebo medicínský snímek

## Názvy barviv
stain-hematoxylin = Hematoxylin
stain-eosin = Eozin
stain-dab = DAB
stain-residual = Zbytek
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Oznámení a legenda barviv
stain-off = Separace barviv vypnuta
# { $set }: H&E, H-DAB nebo H&E-DAB.
stain-on = Barviva: { $set }
stain-shown = { $stain } zobrazen
stain-hidden = { $stain } skryt
stain-mode-light = Barviva jako světlo
stain-mode-od = Barviva jako optická hustota
# Nadpis legendy: „H&E · světlo“, „H-DAB · optická hustota“.
stain-legend-light = { $set } · světlo
stain-legend-od = { $set } · optická hustota
stain-legend-estimated = Odhadnuto z oblasti
stain-legend-normalised = Normalizováno
stain-legend-published = Publikované vektory
# { $pixels }: použité obarvené pixely.
stain-estimated = Barviva odhadnuta z { $pixels } pixelů
stain-estimate-failed = Barviva nelze odhadnout: { $reason }
stain-estimate-background = oblast je převážně pozadí
stain-estimate-one-colour = oblast obsahuje jedinou barvu
stain-estimate-three = odhad vyžaduje H&E nebo H-DAB
stain-estimate-no-pixels = pixely oblasti ještě nejsou načteny
stain-reference-set = Oblast nastavena jako reference barvení
stain-normalise-on = Barvy barviv normalizovány
stain-normalise-off = Barvy barviv podle skenu
stain-reset-done = Publikované vektory barviv

## Dialog exportu anotací
annot-export-title = Exportovat anotace
annot-export-format = Formát
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Masky štítků
annot-export-output = Masky jako
annot-export-output-png = Dlaždice PNG
annot-export-output-tiff = Dlaždice TIFF
annot-export-output-ome = Pyramida OME-TIFF
annot-export-region = Oblast
annot-export-region-slide = Celý preparát
annot-export-region-view = Viditelná plocha
annot-export-region-drawn = Poslední nakreslená oblast
annot-export-level = Úroveň
# { $level }: 0 pro plné rozlišení; { $width } × { $height } její velikost v pixelech.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Velikost dlaždice
annot-export-empty = Prázdné dlaždice
annot-export-empty-skip = Vynechat
annot-export-empty-keep = Zapsat
annot-export-what = { $count ->
    [one] 1 tvar
    [few] { $count } tvary
    [many] { $count } tvaru
   *[other] { $count } tvarů
}, { $classes ->
    [one] 1 třída
    [few] { $classes } třídy
    [many] { $classes } třídy
   *[other] { $classes } tříd
}
annot-export-lines = Čáry se do COCO ani masek nezahrnují.
annot-export-export = Exportovat…
annot-export-writing = Zapisuje se…
# { $files }: zapsané soubory.
annot-export-done = { $files ->
    [one] Exportován 1 soubor
    [few] Exportovány { $files } soubory
    [many] Exportováno { $files } souboru
   *[other] Exportováno { $files } souborů
}
annot-export-failed = Export se nezdařil: { $reason }
annot-import-done = { $count ->
    [one] Importována 1 anotace
    [few] Importovány { $count } anotace
    [many] Importováno { $count } anotace
   *[other] Importováno { $count } anotací
}
annot-import-failed = Nelze importovat: { $reason }
annot-import-skipped = { $count ->
    [one] 1 tvar neměl použitelný obrys
    [few] { $count } tvary neměly použitelný obrys
    [many] { $count } tvaru nemělo použitelný obrys
   *[other] { $count } tvarů nemělo použitelný obrys
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Objemy štítků
label-volume-file = Objem štítků
label-volume-table = Tabulka barev
# { $file }: název souboru objemu štítků; { $count }: jeho štítky.
label-volume-loaded = { $count ->
    [one] { $file }: 1 štítek
    [few] { $file }: { $count } štítky
    [many] { $file }: { $count } štítku
   *[other] { $file }: { $count } štítků
}
label-volume-failed = { $file } nelze překrýt: { $reason }
label-volume-outside = { $file } se s tímto objemem nepřekrývá
label-volume-not-whole = jeho hodnoty nejsou celá čísla
label-volume-colour = obsahuje barvy, ne štítky
label-volume-empty = nemá žádné štítky
label-volume-too-many = má příliš mnoho štítků
label-volume-too-large = je příliš velký na převzorkování
label-volume-degenerate = jeho orientace je neplatná
# Štítek bez názvu v tabulce barev.
label-volume-unnamed = Štítek { $value }
