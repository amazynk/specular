# Multiplanar reformatting (MPR).

palette-mpr = Multiplanar rekonstruktion (MPR)
palette-mpr-slab = Næste MPR-skive (tynd, MIP, MinIP, gennemsnit)
disabled-no-volume = Dette er ikke et volumen: åbn en serie, en fil med flere billeder, et 3D-volumen eller en z-stak
disabled-mpr-closed = Åbn først de multiplanare visninger
mpr-axial = Aksial
mpr-coronal = Koronal
mpr-sagittal = Sagittal
mpr-oblique = Skrå
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D-visning
mpr-thin = Tynd
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Gennemsnit
mpr-slab = Skive: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Skive: { $mode }, { $thickness } Å
mpr-slab-unit = Skive: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Skive: { $mode }, { $thickness } voxels
mpr-thicker = Tykkere skive
mpr-thinner = Tyndere skive
mpr-reset = Nulstil visninger
mpr-close = Luk MPR
mpr-maximise = Maksimér visning
mpr-export-planes = Eksportér planer
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Streamlines fra { $files }: { $thickness } mm skive
mpr-tracts-off = Streamlines fra { $files }: skjult
mpr-tracts-none = Ingen TrackVis- eller MRtrix-streamlines ved siden af denne volumen
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } voxels
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } af { $count }
mpr-axis-channel = Kanal
mpr-axis-unnamed = Akse { $axis }
mpr-one-channel = MPR viser én kanal ad gangen
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } kanaler i farve, hver i sit visningsinterval
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Én kanal vises: { $reason }
mpr-isotropic = Ingen voxelstørrelse i filen: voxels vist som terninger
mpr-isotropic-z = Ingen snitafstand i filen: snit vist én pixel fra hinanden
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Indlæser { $loaded } af { $count ->
        [one] { $count } snit
       *[other] { $count } snit
    }
mpr-loaded = { $count ->
        [one] { $count } snit
       *[other] { $count } snit
    }, { $width } × { $height }
mpr-downsampled = Vist i 1/{ $factor } opløsning (hukommelses- eller grafikgrænser)
mpr-converted = Værdier vist som 32-bit kommatal
mpr-opening = Åbner volumenet…
mpr-gpu-memory = { $size } på grafikkortet
mpr-refused = Kan ikke danne et volumen: { $reason }
mpr-failed = Volumenet blev ikke indlæst: { $reason }
mpr-no-gpu = Grafikkortet kan ikke rumme dette volumen: { $reason }
mpr-planes-saved = Gemte { $count ->
        [one] { $count } plan
       *[other] { $count } planer
    } i { $folder }
mpr-planes-failed = Planerne kunne ikke gemmes: { $reason }
mpr-view-a11y = { $view }-visning, { $position } mm
mpr-crosshair-a11y = Trådkors ved { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Trådkors ved { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Trådkors ved { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Trådkors ved voxel { $x }, { $y }, { $z }
mpr-controls = MPR-styring

# 3D view palette entries.
palette-mpr-preset = Næste 3D-forudindstilling (CT knogle, bløddele, lunge, angio; MR)
palette-mpr-render = Næste 3D-gengivelse (skygget, Cinematic, MIP, MinIP)
palette-mpr-clip = Næste 3D-beskæring (ingen, 2D-visningernes boks, et hjørne skåret væk)
palette-mpr-export-stl = Eksportér 3D-overfladen som STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Næste streamline-skive i MPR
palette-mpr-maximise = Maksimér MPR-visningen, eller vis alle visninger
palette-mpr-export-planes = Eksportér MPR-planerne som billeder…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] CT knogle
    [soft] CT bløddele
    [lung] CT lunge
    [angio] CT-angio
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Skygget
}
vr-clip = { $clip ->
    [box] Beskæring: visningernes boks
    [cut] Beskæring: hjørne skåret væk
   *[off] Ingen beskæring
}
vr-smooth = Udglat overfladen
vr-export = Eksportér STL
vr-threshold = Overflade ved { $value }
vr-stl-exporting = Bygger overfladen…
vr-stl-saved = { $count ->
    [0] Intet ved denne tærskel: ingen overflade gemt
   *[other] Gemte { $count } trekanter i { $name }
}
vr-stl-failed = Overfladen kunne ikke gemmes: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, overflade ved { $threshold }. Skift+pile roterer, plus og minus zoomer.

## Rå volumen: hvordan en .raw- / .vol-fil uden header er opbygget

# machine-drafted, needs native review
raw-volume-title = Åbn rå volumen
raw-volume-file = { $file } har ingen header. Angiv, hvordan værdierne er placeret.
raw-volume-guess-label = Foreslået opbygning
raw-volume-guess = { $index } af { $count }
raw-volume-custom = Tilpasset
raw-volume-width = Bredde (voxels)
raw-volume-height = Højde (voxels)
raw-volume-slices = Snit
raw-volume-type = Værditype
raw-volume-type-unsigned = { $bits }-bit uden fortegn
raw-volume-type-signed = { $bits }-bit med fortegn
raw-volume-type-float = { $bits }-bit kommatal
raw-volume-order = Byterækkefølge
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Header-forskydning
raw-volume-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } byte
}
raw-volume-pixel = Pixelstørrelse
raw-volume-spacing = Snitafstand
raw-volume-mm = { $value } mm
raw-volume-exact = Fylder filen præcist ({ $size })
raw-volume-fits = Bruger { $size } af { $file }; { $left } i overskud
raw-volume-too-big = Kræver { $size }, men filen er { $file }
raw-volume-cancel = Annuller
raw-volume-open = Åbn
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Konnektivitet for { $seed }; flyt trådkorset til et andet seed
# machine-drafted, needs native review
mpr-measure-hint-edit = Træk et endepunkt for at flytte det; oven på det andet for at fjerne det
# machine-drafted, needs native review
mpr-measure-removed = Måling fjernet
mpr-measure-lengths-only = MPR måler kun længder
mpr-more = Mere…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Kurvet MPR langs en klikket kurve
palette-mpr-curve-layout = Kurvet MPR: udrettet eller udstrakt
palette-mpr-curve-save = Gem kurven…
palette-mpr-curve-load = Indlæs en kurve…
palette-mpr-export-segments = Eksportér hvert segment som STL…
palette-mpr-dental = Dentallayout (panorama og tværsnit)
palette-mpr-nerve = Tegn mandibularkanalen
cpr-kind = { $kind ->
    [stretched] Udstrakt
   *[straightened] Udrettet
}
cpr-button = Kurvet
cpr-controls = Kontroller til kurvet MPR
cpr-at = Ved { $position }
cpr-section = Tværsnit
cpr-hint-start = Klik punkter langs strukturen i { $view }-visningen
cpr-hint-more = Klik ét punkt mere
cpr-add-points = Tilføj punkter
cpr-undo-point = Fortryd punkt
# machine-drafted, needs native review
cpr-mark = Markér her
cpr-mark-set = Markering { $n } sat.
cpr-mark-removed = Markering fjernet.
cpr-mark-here = Markering { $n }
cpr-diameter = Diameter { $value }
cpr-diameters = Diameter { $min } til { $max }
cpr-mark-span = Markering { $from } til { $to }: { $length } langs centerlinjen
cpr-mark-total = Alle markeringer: { $length }
cpr-clear = Ryd kurven
cpr-save = Gem kurven
cpr-load = Indlæs kurve
cpr-done = Færdig
cpr-a11y = { $name } langs { $length }, markør ved { $position }. Hjulet flytter markøren.
cpr-none = Ingen kurve endnu: klik først punkter
cpr-cleared = Kurven er ryddet
cpr-saved = Kurven er gemt: { $name }
cpr-save-failed = Kurven kunne ikke gemmes: { $reason }
cpr-loaded = Kurven er indlæst
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 centerlinje indlæst
   *[other] { $count } centerlinjer indlæst
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] 1 gemt centerlinje fundet
   *[other] { $count } gemte centerlinjer fundet
}
# A centreline without a name, by its place in the list.
cpr-centreline = Centerlinje { $number }
cpr-new = Ny centerlinje
cpr-rename = Omdøb
cpr-name = Navn på centerlinje
cpr-delete = Slet
# $name: the centreline's name.
cpr-delete-title = Slet centerlinjen “{ $name }”?
cpr-delete-detail = Dens punkter forsvinder. Filer, der er gemt på disken, bliver.
cpr-deleted = Slettede “{ $name }”
cpr-load-failed = Kurven kunne ikke indlæses: { $reason }
cpr-load-not-json = filen er ikke JSON
cpr-load-not-curves = filen er ikke en Specular-kurve
cpr-load-empty = filen indeholder ingen kurve med to punkter eller flere
cpr-panoramic = Panorama
cpr-panoramic-slab = Panorama, skive { $thickness } mm
cpr-panoramic-thin = Panorama, tynd

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Dental
dental-not-for-diagnosis = Ikke til diagnose eller behandlingsplanlægning
dental-nerve = Mandibularkanalen
dental-nerve-hint = Klik punkter langs mandibularkanalen på panoramaet eller et tværsnit
dental-nerve-done = Optegning af mandibularkanalen slået fra
dental-spacing = Hver { $spacing } mm
dental-section = Snit { $number }
dental-off = Åbn først dentallayoutet
dental-suggesting = Finder tandbuen…
dental-suggested = Tandbue foreslået: træk dens punkter for at justere
dental-no-arch = Ingen tandbue fundet: klik punkter langs den i den aksiale visning
dental-vendor = Dental-CBCT fra { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Eksportér segmenter
seg-stl-detail = Segmenter: { $count ->
        [one] { $count } trekant
       *[other] { $count } trekanter
    }
seg-stl-detail-full = Segmenter: fuld detalje
seg-stl-none = Ingen segmenter at eksportere
seg-stl-not-labels = denne volumen er ikke et etiketkort
seg-stl-exporting = Bygger segmenternes overflader…
seg-stl-saved = { $count ->
    [one] Gemte { $count } segment i { $folder }
   *[other] Gemte { $count } segmenter i { $folder }
}
seg-stl-open = Ikke lukkede: { $count }.
seg-stl-empty = Segmenterne er tomme: intet gemt
seg-stl-failed = Segmenterne kunne ikke gemmes: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Tre planer (MPR)
toolbar-mpr-close = Luk tre planer (MPR)
settings-viewing-toolbar-items-mpr = Tre planer (MPR)
