# machine-drafted, needs native review
# Specular — Patologi: farveseparation, eksport og import af annoteringer, labelvolumener over volumener. Dansk.

## Palet
palette-stain-separation = Adskil farvninger
palette-stain-1 = Vis eller skjul farvning 1
palette-stain-2 = Vis eller skjul farvning 2
palette-stain-3 = Vis eller skjul farvning 3
palette-stain-mode = Farvninger som lys eller optisk densitet
palette-stain-estimate = Estimér farvninger fra område
palette-stain-normalise = Normalisér farvningernes farver
palette-stain-reference = Brug område som farvningsreference
palette-stain-reset = Publicerede farvningsvektorer
palette-export-annotations = Eksportér annoteringer…
palette-import-annotations = Importér annoteringer…
palette-label-volume = Læg labelvolumen over…

## Årsager til deaktiverede kommandoer
disabled-not-colour-image = Kræver et farvebillede
disabled-stains-off = Slå farveseparation til først
disabled-no-region = Tegn et område først
disabled-no-slide-annotations = Ingen annoteringer, områder eller punkter at eksportere
disabled-no-label-target = Åbn et volumen i de multiplanare visninger eller et medicinsk billede

## Navne på farvninger
stain-hematoxylin = Hæmatoxylin
stain-eosin = Eosin
stain-dab = DAB
stain-residual = Rest
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Meddelelser og forklaring for farvninger
stain-off = Farveseparation fra
# { $set }: H&E, H-DAB eller H&E-DAB.
stain-on = Farvninger: { $set }
stain-shown = { $stain } vist
stain-hidden = { $stain } skjult
stain-mode-light = Farvninger som lys
stain-mode-od = Farvninger som optisk densitet
# Forklaringens overskrift: "H&E · lys", "H-DAB · optisk densitet".
stain-legend-light = { $set } · lys
stain-legend-od = { $set } · optisk densitet
stain-legend-estimated = Estimeret fra område
stain-legend-normalised = Normaliseret
stain-legend-published = Publicerede vektorer
# { $pixels }: anvendte farvede pixels.
stain-estimated = Farvninger estimeret fra { $pixels } pixels
stain-estimate-failed = Kan ikke estimere farvninger: { $reason }
stain-estimate-background = området er mest baggrund
stain-estimate-one-colour = området har kun én farve
stain-estimate-three = estimering kræver H&E eller H-DAB
stain-estimate-no-pixels = områdets pixels er ikke indlæst endnu
stain-reference-set = Område angivet som farvningsreference
stain-normalise-on = Farvningernes farver normaliseret
stain-normalise-off = Farvningernes farver som scannet
stain-reset-done = Publicerede farvningsvektorer

## Dialogen Eksportér annoteringer
annot-export-title = Eksportér annoteringer
annot-export-format = Format
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Labelmasker
annot-export-output = Masker som
annot-export-output-png = PNG-fliser
annot-export-output-tiff = TIFF-fliser
annot-export-output-ome = OME-TIFF-pyramide
annot-export-region = Område
annot-export-region-slide = Hele snittet
annot-export-region-view = Synligt område
annot-export-region-drawn = Senest tegnede område
annot-export-level = Niveau
# { $level }: 0 for fuld opløsning; { $width } × { $height } dets størrelse i pixels.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Flisestørrelse
annot-export-empty = Tomme fliser
annot-export-empty-skip = Spring over
annot-export-empty-keep = Skriv
annot-export-what = { $count ->
    [one] 1 figur
   *[other] { $count } figurer
}, { $classes ->
    [one] 1 klasse
   *[other] { $classes } klasser
}
annot-export-lines = Linjer udelades i COCO og masker.
annot-export-export = Eksportér…
annot-export-writing = Skriver…
# { $files }: skrevne filer.
annot-export-done = { $files ->
    [one] 1 fil eksporteret
   *[other] { $files } filer eksporteret
}
annot-export-failed = Eksport mislykkedes: { $reason }
annot-import-done = { $count ->
    [one] 1 annotering importeret
   *[other] { $count } annoteringer importeret
}
annot-import-failed = Kan ikke importere: { $reason }
annot-import-skipped = { $count ->
    [one] 1 figur havde ingen brugbar kontur
   *[other] { $count } figurer havde ingen brugbar kontur
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Labelvolumener
label-volume-file = Labelvolumen
label-volume-table = Farvetabel
# { $file }: labelvolumenets filnavn; { $count }: dets labels.
label-volume-loaded = { $count ->
    [one] { $file }: 1 label
   *[other] { $file }: { $count } labels
}
label-volume-failed = Kan ikke lægge { $file } over: { $reason }
label-volume-outside = { $file } overlapper ikke dette volumen
label-volume-not-whole = dets værdier er ikke heltal
label-volume-colour = det indeholder farver, ikke labels
label-volume-empty = det har ingen labels
label-volume-too-many = det har for mange labels
label-volume-too-large = det er for stort til at resample
label-volume-degenerate = dets orientering er ugyldig
# En label uden navn i farvetabellen.
label-volume-unnamed = Label { $value }
