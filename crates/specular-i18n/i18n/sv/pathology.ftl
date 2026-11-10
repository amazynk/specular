# machine-drafted, needs native review
# Specular — Patologi: färgseparation, export och import av annoteringar, etikettvolymer över volymer. Svenska.

## Palett
palette-stain-separation = Separera färgningar
palette-stain-1 = Visa eller dölj färgning 1
palette-stain-2 = Visa eller dölj färgning 2
palette-stain-3 = Visa eller dölj färgning 3
palette-stain-mode = Färgningar som ljus eller optisk densitet
palette-stain-estimate = Uppskatta färgningar från område
palette-stain-normalise = Normalisera färgningarnas färger
palette-stain-reference = Använd område som färgningsreferens
palette-stain-reset = Publicerade färgningsvektorer
palette-export-annotations = Exportera annoteringar…
palette-import-annotations = Importera annoteringar…
palette-label-volume = Lägg etikettvolym ovanpå…

## Skäl till inaktiverade kommandon
disabled-not-colour-image = Kräver en färgbild
disabled-stains-off = Slå först på färgseparation
disabled-no-region = Rita först ett område
disabled-no-slide-annotations = Inga annoteringar, områden eller punkter att exportera
disabled-no-label-target = Öppna en volym i de multiplanara vyerna, eller en medicinsk bild

## Namn på färgningar
stain-hematoxylin = Hematoxylin
stain-eosin = Eosin
stain-dab = DAB
stain-residual = Rest
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Meddelanden och teckenförklaring för färgningar
stain-off = Färgseparation av
# { $set }: H&E, H-DAB eller H&E-DAB.
stain-on = Färgningar: { $set }
stain-shown = { $stain } visas
stain-hidden = { $stain } dold
stain-mode-light = Färgningar som ljus
stain-mode-od = Färgningar som optisk densitet
# Teckenförklaringens rubrik: ”H&E · ljus”, ”H-DAB · optisk densitet”.
stain-legend-light = { $set } · ljus
stain-legend-od = { $set } · optisk densitet
stain-legend-estimated = Uppskattat från område
stain-legend-normalised = Normaliserat
stain-legend-published = Publicerade vektorer
# { $pixels }: använda färgade pixlar.
stain-estimated = Färgningar uppskattade från { $pixels } pixlar
stain-estimate-failed = Kan inte uppskatta färgningar: { $reason }
stain-estimate-background = området är mest bakgrund
stain-estimate-one-colour = området har bara en färg
stain-estimate-three = uppskattningen kräver H&E eller H-DAB
stain-estimate-no-pixels = områdets pixlar är inte inlästa än
stain-reference-set = Området är satt som färgningsreferens
stain-normalise-on = Färgningarnas färger normaliserade
stain-normalise-off = Färgningarnas färger som skannade
stain-reset-done = Publicerade färgningsvektorer

## Dialogrutan Exportera annoteringar
annot-export-title = Exportera annoteringar
annot-export-format = Format
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Etikettmasker
annot-export-output = Masker som
annot-export-output-png = PNG-rutor
annot-export-output-tiff = TIFF-rutor
annot-export-output-ome = OME-TIFF-pyramid
annot-export-region = Område
annot-export-region-slide = Hela snittet
annot-export-region-view = Synligt område
annot-export-region-drawn = Senast ritade område
annot-export-level = Nivå
# { $level }: 0 för full upplösning; { $width } × { $height } dess storlek i pixlar.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Rutstorlek
annot-export-empty = Tomma rutor
annot-export-empty-skip = Hoppa över
annot-export-empty-keep = Skriv
annot-export-what = { $count ->
    [one] 1 form
   *[other] { $count } former
}, { $classes ->
    [one] 1 klass
   *[other] { $classes } klasser
}
annot-export-lines = Linjer utelämnas i COCO och masker.
annot-export-export = Exportera…
annot-export-writing = Skriver…
# { $files }: skrivna filer.
annot-export-done = { $files ->
    [one] 1 fil exporterad
   *[other] { $files } filer exporterade
}
annot-export-failed = Exporten misslyckades: { $reason }
annot-import-done = { $count ->
    [one] 1 annotering importerad
   *[other] { $count } annoteringar importerade
}
annot-import-failed = Kan inte importera: { $reason }
annot-import-skipped = { $count ->
    [one] 1 form saknade användbar kontur
   *[other] { $count } former saknade användbar kontur
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Etikettvolymer
label-volume-file = Etikettvolym
label-volume-table = Färgtabell
# { $file }: etikettvolymens filnamn; { $count }: dess etiketter.
label-volume-loaded = { $count ->
    [one] { $file }: 1 etikett
   *[other] { $file }: { $count } etiketter
}
label-volume-failed = Kan inte lägga { $file } ovanpå: { $reason }
label-volume-outside = { $file } överlappar inte den här volymen
label-volume-not-whole = dess värden är inte heltal
label-volume-colour = den innehåller färg, inte etiketter
label-volume-empty = den har inga etiketter
label-volume-too-many = den har för många etiketter
label-volume-too-large = den är för stor för omsampling
label-volume-degenerate = dess orientering är ogiltig
# En etikett utan namn i färgtabellen.
label-volume-unnamed = Etikett { $value }
