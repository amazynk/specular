# machine-drafted, needs native review
# Specular — Pathologie: kleuringsscheiding, export en import van annotaties, labelvolumes over volumes. Nederlands.

## Palet
palette-stain-separation = Kleuringen scheiden
palette-stain-1 = Kleuring 1 tonen of verbergen
palette-stain-2 = Kleuring 2 tonen of verbergen
palette-stain-3 = Kleuring 3 tonen of verbergen
palette-stain-mode = Kleuringen als licht of optische dichtheid
palette-stain-estimate = Kleuringen schatten uit gebied
palette-stain-normalise = Kleuringskleuren normaliseren
palette-stain-reference = Gebied als kleuringsreferentie gebruiken
palette-stain-reset = Gepubliceerde kleuringsvectoren
palette-export-annotations = Annotaties exporteren…
palette-import-annotations = Annotaties importeren…
palette-label-volume = Labelvolume eroverheen leggen…

## Redenen voor uitgeschakelde opdrachten
disabled-not-colour-image = Vereist een kleurenafbeelding
disabled-stains-off = Zet eerst kleuringsscheiding aan
disabled-no-region = Teken eerst een gebied
disabled-no-slide-annotations = Geen annotaties, gebieden of punten om te exporteren
disabled-no-label-target = Open een volume in de multiplanaire weergaven, of een medische afbeelding

## Namen van kleuringen
stain-hematoxylin = Hematoxyline
stain-eosin = Eosine
stain-dab = DAB
stain-residual = Rest
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Meldingen en legenda van kleuringen
stain-off = Kleuringsscheiding uit
# { $set }: H&E, H-DAB of H&E-DAB.
stain-on = Kleuringen: { $set }
stain-shown = { $stain } getoond
stain-hidden = { $stain } verborgen
stain-mode-light = Kleuringen als licht
stain-mode-od = Kleuringen als optische dichtheid
# Kop van de legenda: „H&E · licht”, „H-DAB · optische dichtheid”.
stain-legend-light = { $set } · licht
stain-legend-od = { $set } · optische dichtheid
stain-legend-estimated = Geschat uit gebied
stain-legend-normalised = Genormaliseerd
stain-legend-published = Gepubliceerde vectoren
# { $pixels }: gebruikte gekleurde pixels.
stain-estimated = Kleuringen geschat uit { $pixels } pixels
stain-estimate-failed = Kan kleuringen niet schatten: { $reason }
stain-estimate-background = het gebied is grotendeels achtergrond
stain-estimate-one-colour = het gebied bevat één kleur
stain-estimate-three = schatten vereist H&E of H-DAB
stain-estimate-no-pixels = de pixels van het gebied zijn nog niet geladen
stain-reference-set = Gebied ingesteld als kleuringsreferentie
stain-normalise-on = Kleuringskleuren genormaliseerd
stain-normalise-off = Kleuringskleuren zoals gescand
stain-reset-done = Gepubliceerde kleuringsvectoren

## Dialoogvenster Annotaties exporteren
annot-export-title = Annotaties exporteren
annot-export-format = Formaat
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Labelmaskers
annot-export-output = Maskers als
annot-export-output-png = PNG-tegels
annot-export-output-tiff = TIFF-tegels
annot-export-output-ome = OME-TIFF-piramide
annot-export-region = Gebied
annot-export-region-slide = Hele coupe
annot-export-region-view = Zichtbaar deel
annot-export-region-drawn = Laatst getekend gebied
annot-export-level = Niveau
# { $level }: 0 voor volledige resolutie; { $width } × { $height } de grootte in pixels.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Tegelgrootte
annot-export-empty = Lege tegels
annot-export-empty-skip = Overslaan
annot-export-empty-keep = Schrijven
annot-export-what = { $count ->
    [one] 1 vorm
   *[other] { $count } vormen
}, { $classes ->
    [one] 1 klasse
   *[other] { $classes } klassen
}
annot-export-lines = Lijnen worden weggelaten uit COCO en maskers.
annot-export-export = Exporteren…
annot-export-writing = Bezig met schrijven…
# { $files }: geschreven bestanden.
annot-export-done = { $files ->
    [one] 1 bestand geëxporteerd
   *[other] { $files } bestanden geëxporteerd
}
annot-export-failed = Export mislukt: { $reason }
annot-import-done = { $count ->
    [one] 1 annotatie geïmporteerd
   *[other] { $count } annotaties geïmporteerd
}
annot-import-failed = Kan niet importeren: { $reason }
annot-import-skipped = { $count ->
    [one] 1 vorm had geen bruikbare omtrek
   *[other] { $count } vormen hadden geen bruikbare omtrek
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Labelvolumes
label-volume-file = Labelvolume
label-volume-table = Kleurentabel
# { $file }: bestandsnaam van het labelvolume; { $count }: zijn labels.
label-volume-loaded = { $count ->
    [one] { $file }: 1 label
   *[other] { $file }: { $count } labels
}
label-volume-failed = Kan { $file } er niet overheen leggen: { $reason }
label-volume-outside = { $file } overlapt dit volume niet
label-volume-not-whole = de waarden zijn geen gehele getallen
label-volume-colour = het bevat kleur, geen labels
label-volume-empty = het heeft geen labels
label-volume-too-many = het heeft te veel labels
label-volume-too-large = het is te groot om te herbemonsteren
label-volume-degenerate = de oriëntatie is ongeldig
# Een label zonder naam in de kleurentabel.
label-volume-unnamed = Label { $value }
