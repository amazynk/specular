# Specular — Nástroj měření (délky, úhly, oblasti a jejich statistiky na libovolném obrázku). Strojový překlad, vyžaduje kontrolu rodilým mluvčím.

palette-measure = Měřit
palette-measure-copy-csv = Kopírovat měření jako CSV
palette-measure-export = Exportovat měření…
toast-copied-measurements = Měření zkopírována jako CSV

measure-tool-title = Měřit
measure-tool-list = Měření
measure-tool-count = { $count ->
    [0] Žádná měření
    [one] Jedno měření
    [few] { $count } měření
    [many] { $count } měření
   *[other] { $count } měření
}
measure-tool-empty = Vyberte nástroj a kreslete do obrázku.

measure-tool-point = Bod
measure-tool-line = Úsečka
measure-tool-polyline = Lomená čára
measure-tool-angle = Úhel
measure-tool-rect = Obdélník
measure-tool-ellipse = Elipsa
measure-tool-polygon = Mnohoúhelník
measure-tool-freehand = Volná ruka
measure-tool-calibrate = Kalibrovat
# machine-drafted, needs native review
measure-tool-spot = Měřicí bod

measure-tool-undo = Zpět
measure-tool-delete = Smazat
measure-tool-copy = Kopírovat CSV
measure-tool-export = Exportovat…
measure-tool-import = Importovat…
measure-tool-done = Hotovo
measure-tool-unit = Jednotka: { $unit }
measure-tool-apply = Použít

measure-tool-hint-line = Táhněte, nebo klikněte na dva body. Šipky posouvají zaměřovač, mezerník umístí bod.
measure-tool-hint-angle = Klikněte na tři body; druhý je vrchol.
measure-tool-hint-rect = Táhněte, nebo klikněte na dva rohy.
measure-tool-hint-ellipse = Táhněte, nebo klikněte na dva rohy jejího rámečku.
measure-tool-hint-polygon = Klikejte na rohy; Enter nebo klik na první roh tvar uzavře.
measure-tool-hint-calibrate = Nakreslete čáru přes něco známé délky.
# machine-drafted, needs native review
measure-tool-hint-spot = Klikněte na bod a přečtěte jeho teplotu.
measure-tool-known-length = Známá délka: { $value } { $unit } (napište číslo, U mění jednotku, Enter použije)
measure-tool-calibrated = Kalibrováno: jeden pixel je { $size } { $unit }
measure-tool-bad-length = Zadejte délku větší než nula.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Plocha { $value }
measure-tool-mean = Průměr { $mean } ± { $sd }
measure-tool-means = Průměr { $values }
measure-tool-range = Rozsah { $min } – { $max } z { $count } px
measure-tool-pixels = Z { $count } px
measure-tool-stats-pending = Čtení pixelů…
measure-tool-stats-none = Zde nejsou hodnoty pixelů
measure-tool-approximate = Ze zmenšené kopie (přibližně)

measure-tool-deleted = Měření smazáno
measure-tool-undone = Vráceno zpět
measure-tool-nothing = Na tomto obrázku nejsou žádná měření
measure-tool-exported = Měření uložena do { $name }
measure-tool-export-failed = Měření nelze uložit: { $reason }
measure-tool-imported = { $count ->
    [one] Importováno jedno měření.
    [few] Importována { $count } měření.
    [many] Importováno { $count } měření.
   *[other] Importováno { $count } měření.
}
measure-tool-skipped = { $count ->
    [one] Jeden tvar nelze přečíst.
    [few] { $count } tvary nelze přečíst.
    [many] { $count } tvaru nelze přečíst.
   *[other] { $count } tvarů nelze přečíst.
}
measure-tool-import-failed = Měření nelze přečíst: { $reason }
measure-tool-too-large = Soubor je příliš velký.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR vyžaduje obrázek DICOM. Uložte jej raději jako CSV nebo GeoJSON.

measure-source-uncalibrated = Pixely (nekalibrováno)
measure-source-dicom-pixel-spacing = Rozteč pixelů DICOM
measure-source-dicom-calibrated = Kalibrovaná rozteč DICOM
measure-source-dicom-imager-pixel-spacing = Rozteč DICOM na detektoru
measure-source-dicom-magnification = Rozteč DICOM s korekcí zvětšení
measure-source-dicom-ultrasound-region = Ultrazvuková oblast DICOM
measure-source-tiff-resolution = Rozlišení TIFF
measure-source-exif-resolution = Rozlišení EXIF
measure-source-ome-physical-size = Fyzická velikost OME
measure-source-fits-wcs = Světové souřadnice FITS
measure-source-vendor-tag = Metadata výrobce
measure-source-drawing-units = Jednotky výkresu
# machine-drafted, needs native review
measure-source-drawing-scale = Podle měřítka výkresu
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Z výkresu (měřítko { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Z výkresu (měřítko { $scale }, výřez { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Měřítko mapy ve středu (přibližné; mapové nástroje měří na Zemi)
measure-source-user = Vaše kalibrace
