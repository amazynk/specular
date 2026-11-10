# Multiplanar reformatting (MPR).

palette-mpr = Multiplanar rekonstruktion (MPR)
palette-mpr-slab = Nästa MPR-skiva (tunn, MIP, MinIP, medel)
disabled-no-volume = Det här är ingen volym: öppna en serie, en fil med flera bildrutor, en 3D-volym eller en z-stack
disabled-mpr-closed = Öppna de multiplanara vyerna först
mpr-axial = Axial
mpr-coronal = Koronal
mpr-sagittal = Sagittal
mpr-oblique = Sned
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D-vy
mpr-thin = Tunn
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Medel
mpr-slab = Skiva: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Skiva: { $mode }, { $thickness } Å
mpr-slab-unit = Skiva: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Skiva: { $mode }, { $thickness } voxlar
mpr-thicker = Tjockare skiva
mpr-thinner = Tunnare skiva
mpr-reset = Återställ vyer
mpr-close = Stäng MPR
mpr-maximise = Maximera vy
mpr-export-planes = Exportera plan
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Streamlines från { $files }: { $thickness } mm skiva
mpr-tracts-off = Streamlines från { $files }: dolda
mpr-tracts-none = Inga TrackVis- eller MRtrix-streamlines bredvid denna volym
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } voxlar
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } av { $count }
mpr-axis-channel = Kanal
mpr-axis-unnamed = Axel { $axis }
mpr-one-channel = MPR visar en kanal i taget
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } kanaler i färg, var och en i sitt visningsintervall
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = En kanal visas: { $reason }
mpr-isotropic = Ingen voxelstorlek i filen: voxlar visas som kuber
mpr-isotropic-z = Inget snittavstånd i filen: snitt visas en pixel isär
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Läser in { $loaded } av { $count ->
        [one] { $count } snitt
       *[other] { $count } snitt
    }
mpr-loaded = { $count ->
        [one] { $count } snitt
       *[other] { $count } snitt
    }, { $width } × { $height }
mpr-downsampled = Visas i 1/{ $factor } upplösning (minnes- eller grafikgränser)
mpr-converted = Värden visas som 32-bitars flyttal
mpr-opening = Öppnar volymen…
mpr-gpu-memory = { $size } på grafikkortet
mpr-refused = Kan inte skapa en volym: { $reason }
mpr-failed = Volymen lästes inte in: { $reason }
mpr-no-gpu = Grafikkortet rymmer inte den här volymen: { $reason }
mpr-planes-saved = { $count ->
        [one] Sparade { $count } plan i { $folder }
       *[other] Sparade { $count } plan i { $folder }
    }
mpr-planes-failed = Planen kunde inte sparas: { $reason }
mpr-view-a11y = { $view }-vy, { $position } mm
mpr-crosshair-a11y = Hårkors vid { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Hårkors vid { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Hårkors vid { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Hårkors vid voxel { $x }, { $y }, { $z }
mpr-controls = MPR-kontroller

# 3D view palette entries.
palette-mpr-preset = Nästa 3D-förinställning (DT skelett, mjukdelar, lunga, angio; MR)
palette-mpr-render = Nästa 3D-rendering (skuggad, Cinematic, MIP, MinIP)
palette-mpr-clip = Nästa 3D-klippning (ingen, 2D-vyernas låda, ett hörn bortskuret)
palette-mpr-export-stl = Exportera 3D-ytan som STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Nästa streamline-skiva i MPR
palette-mpr-maximise = Maximera MPR-vyn, eller visa alla vyer
palette-mpr-export-planes = Exportera MPR-planen som bilder…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] DT skelett
    [soft] DT mjukdelar
    [lung] DT lunga
    [angio] DT-angio
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Skuggad
}
vr-clip = { $clip ->
    [box] Klippning: vyernas låda
    [cut] Klippning: hörn bortskuret
   *[off] Ingen klippning
}
vr-smooth = Jämna ut ytan
vr-export = Exportera STL
vr-threshold = Yta vid { $value }
vr-stl-exporting = Bygger ytan…
vr-stl-saved = { $count ->
    [0] Inget vid denna tröskel: ingen yta sparad
   *[other] Sparade { $count } trianglar i { $name }
}
vr-stl-failed = Ytan kunde inte sparas: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, yta vid { $threshold }. Skift+piltangenter roterar, plus och minus zoomar.

## Rå volym: hur en .raw- / .vol-fil utan huvud är uppbyggd

# machine-drafted, needs native review
raw-volume-title = Öppna rå volym
raw-volume-file = { $file } saknar huvud. Ange hur värdena är ordnade.
raw-volume-guess-label = Föreslagen ordning
raw-volume-guess = { $index } av { $count }
raw-volume-custom = Anpassad
raw-volume-width = Bredd (voxlar)
raw-volume-height = Höjd (voxlar)
raw-volume-slices = Snitt
raw-volume-type = Värdetyp
raw-volume-type-unsigned = { $bits } bitar utan tecken
raw-volume-type-signed = { $bits } bitar med tecken
raw-volume-type-float = { $bits } bitar flyttal
raw-volume-order = Byteordning
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Huvudförskjutning
raw-volume-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } byte
}
raw-volume-pixel = Pixelstorlek
raw-volume-spacing = Snittavstånd
raw-volume-mm = { $value } mm
raw-volume-exact = Fyller filen exakt ({ $size })
raw-volume-fits = Använder { $size } av { $file }; { $left } blir över
raw-volume-too-big = Kräver { $size }, men filen är { $file }
raw-volume-cancel = Avbryt
raw-volume-open = Öppna
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Konnektivitet för { $seed }; flytta hårkorset till en annan seed
# machine-drafted, needs native review
mpr-measure-hint-edit = Dra en ände för att flytta den; till den andra änden för att ta bort
# machine-drafted, needs native review
mpr-measure-removed = Mätningen togs bort
mpr-measure-lengths-only = MPR mäter bara längder
mpr-more = Mer…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Kurvad MPR längs en klickad kurva
palette-mpr-curve-layout = Kurvad MPR: uträtad eller sträckt
palette-mpr-curve-save = Spara kurvan…
palette-mpr-curve-load = Läs in en kurva…
palette-mpr-export-segments = Exportera varje segment som STL…
palette-mpr-dental = Dentallayout (panorama och tvärsnitt)
palette-mpr-nerve = Rita in mandibularkanalen
cpr-kind = { $kind ->
    [stretched] Sträckt
   *[straightened] Uträtad
}
cpr-button = Kurvad
cpr-controls = Kontroller för kurvad MPR
cpr-at = Vid { $position }
cpr-section = Tvärsnitt
cpr-hint-start = Klicka punkter längs strukturen i { $view }-vyn
cpr-hint-more = Klicka en punkt till
cpr-add-points = Lägg till punkter
cpr-undo-point = Ångra punkt
# machine-drafted, needs native review
cpr-mark = Markera här
cpr-mark-set = Markering { $n } satt.
cpr-mark-removed = Markering borttagen.
cpr-mark-here = Markering { $n }
cpr-diameter = Diameter { $value }
cpr-diameters = Diameter { $min } till { $max }
cpr-mark-span = Markering { $from } till { $to }: { $length } längs mittlinjen
cpr-mark-total = Alla markeringar: { $length }
cpr-clear = Rensa kurvan
cpr-save = Spara kurvan
cpr-load = Läs in kurva
cpr-done = Klar
cpr-a11y = { $name } längs { $length }, markör vid { $position }. Hjulet flyttar markören.
cpr-none = Ingen kurva än: klicka punkter först
cpr-cleared = Kurvan rensades
cpr-saved = Kurvan sparades: { $name }
cpr-save-failed = Kurvan kunde inte sparas: { $reason }
cpr-loaded = Kurvan lästes in
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 mittlinje lästes in
   *[other] { $count } mittlinjer lästes in
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] 1 sparad mittlinje hittades
   *[other] { $count } sparade mittlinjer hittades
}
# A centreline without a name, by its place in the list.
cpr-centreline = Mittlinje { $number }
cpr-new = Ny mittlinje
cpr-rename = Byt namn
cpr-name = Mittlinjens namn
cpr-delete = Ta bort
# $name: the centreline's name.
cpr-delete-title = Ta bort mittlinjen ”{ $name }”?
cpr-delete-detail = Dess punkter försvinner. Filer som sparats på disken finns kvar.
cpr-deleted = Tog bort ”{ $name }”
cpr-load-failed = Kurvan kunde inte läsas in: { $reason }
cpr-load-not-json = filen är inte JSON
cpr-load-not-curves = filen är ingen Specular-kurva
cpr-load-empty = filen innehåller ingen kurva med två punkter eller fler
cpr-panoramic = Panorama
cpr-panoramic-slab = Panorama, skiva { $thickness } mm
cpr-panoramic-thin = Panorama, tunn

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Dental
dental-not-for-diagnosis = Inte för diagnos eller behandlingsplanering
dental-nerve = Mandibularkanalen
dental-nerve-hint = Klicka punkter längs mandibularkanalen i panoramat eller ett tvärsnitt
dental-nerve-done = Inritning av mandibularkanalen av
dental-spacing = Med { $spacing } mm mellanrum
dental-section = Snitt { $number }
dental-off = Öppna dentallayouten först
dental-suggesting = Söker tandbågen…
dental-suggested = Tandbåge föreslagen: dra dess punkter för att justera
dental-no-arch = Ingen tandbåge hittades: klicka punkter längs den i den axiala vyn
dental-vendor = Dental-CBCT från { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Exportera segment
seg-stl-detail = Segment: { $count ->
        [one] { $count } triangel
       *[other] { $count } trianglar
    }
seg-stl-detail-full = Segment: full detaljnivå
seg-stl-none = Inga segment att exportera
seg-stl-not-labels = den här volymen är ingen etikettkarta
seg-stl-exporting = Bygger segmentens ytor…
seg-stl-saved = { $count ->
    [one] Sparade { $count } segment i { $folder }
   *[other] Sparade { $count } segment i { $folder }
}
seg-stl-open = Inte slutna: { $count }.
seg-stl-empty = Segmenten är tomma: inget sparades
seg-stl-failed = Segmenten kunde inte sparas: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Tre plan (MPR)
toolbar-mpr-close = Stäng tre plan (MPR)
settings-viewing-toolbar-items-mpr = Tre plan (MPR)
