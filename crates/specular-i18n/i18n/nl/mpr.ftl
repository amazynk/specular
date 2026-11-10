# Multiplanar reformatting (MPR).

palette-mpr = Multiplanaire reconstructie (MPR)
palette-mpr-slab = Volgende MPR-slab (dun, MIP, MinIP, gemiddelde)
disabled-no-volume = Dit is geen volume: open een serie, een bestand met meerdere frames, een 3D-volume of een z-stack
disabled-mpr-closed = Open eerst de multiplanaire weergaven
mpr-axial = Axiaal
mpr-coronal = Coronaal
mpr-sagittal = Sagittaal
mpr-oblique = Schuin
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D-weergave
mpr-thin = Dun
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Gemiddelde
mpr-slab = Slab: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Slab: { $mode }, { $thickness } Å
mpr-slab-unit = Slab: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Slab: { $mode }, { $thickness } voxels
mpr-thicker = Dikkere slab
mpr-thinner = Dunnere slab
mpr-reset = Weergaven herstellen
mpr-close = MPR sluiten
mpr-maximise = Weergave maximaliseren
mpr-export-planes = Vlakken exporteren
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Streamlines uit { $files }: slab { $thickness } mm
mpr-tracts-off = Streamlines uit { $files }: verborgen
mpr-tracts-none = Geen TrackVis- of MRtrix-streamlines naast dit volume
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } voxels
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } van { $count }
mpr-axis-channel = Kanaal
mpr-axis-unnamed = As { $axis }
mpr-one-channel = MPR toont één kanaal tegelijk
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } kanalen in kleur, elk in zijn eigen weergavebereik
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Eén kanaal getoond: { $reason }
mpr-isotropic = Geen voxelgrootte in het bestand: voxels als kubussen getoond
mpr-isotropic-z = Geen plakafstand in het bestand: plakken één pixel uit elkaar getoond
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = { $loaded } van { $count ->
        [one] { $count } plak
       *[other] { $count } plakken
    } laden
mpr-loaded = { $count ->
        [one] { $count } plak
       *[other] { $count } plakken
    }, { $width } × { $height }
mpr-downsampled = Getoond op 1/{ $factor } resolutie (geheugen- of grafische limieten)
mpr-converted = Waarden getoond als 32-bits floats
mpr-opening = Volume openen…
mpr-gpu-memory = { $size } op de grafische kaart
mpr-refused = Kan geen volume maken: { $reason }
mpr-failed = Het volume is niet geladen: { $reason }
mpr-no-gpu = De grafische kaart kan dit volume niet bevatten: { $reason }
mpr-planes-saved = { $count ->
        [one] { $count } vlak opgeslagen
       *[other] { $count } vlakken opgeslagen
    } in { $folder }
mpr-planes-failed = De vlakken konden niet worden opgeslagen: { $reason }
mpr-view-a11y = { $view }-weergave, { $position } mm
mpr-crosshair-a11y = Draadkruis op { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Draadkruis op { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Draadkruis op { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Draadkruis op voxel { $x }, { $y }, { $z }
mpr-controls = MPR-bediening

# 3D view palette entries.
palette-mpr-preset = Volgende 3D-voorinstelling (CT bot, weke delen, long, angio; MR)
palette-mpr-render = Volgende 3D-rendering (gearceerd, Cinematic, MIP, MinIP)
palette-mpr-clip = Volgende 3D-clipping (geen, kader van de 2D-weergaven, hoek weggesneden)
palette-mpr-export-stl = 3D-oppervlak als STL exporteren…
# machine-drafted, needs native review
palette-mpr-tract-slab = Volgende streamline-slab in MPR
palette-mpr-maximise = De MPR-weergave maximaliseren, of alle weergaven tonen
palette-mpr-export-planes = De MPR-vlakken als afbeeldingen exporteren…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] CT bot
    [soft] CT weke delen
    [lung] CT long
    [angio] CT-angio
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Gearceerd
}
vr-clip = { $clip ->
    [box] Clipping: kader van de weergaven
    [cut] Clipping: hoek weggesneden
   *[off] Geen clipping
}
vr-smooth = Oppervlak gladmaken
vr-export = STL exporteren
vr-threshold = Oppervlak bij { $value }
vr-stl-exporting = Oppervlak opbouwen…
vr-stl-saved = { $count ->
    [0] Niets bij deze drempel: geen oppervlak opgeslagen
   *[other] { $count } driehoeken opgeslagen in { $name }
}
vr-stl-failed = Het oppervlak kon niet worden opgeslagen: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, oppervlak bij { $threshold }. Shift+pijltjes draaien, plus en min zoomen.

## Ruw volume: hoe een .raw- / .vol-bestand zonder header is opgebouwd

# machine-drafted, needs native review
raw-volume-title = Ruw volume openen
raw-volume-file = { $file } heeft geen header. Geef aan hoe de waarden zijn ingedeeld.
raw-volume-guess-label = Voorgestelde indeling
raw-volume-guess = { $index } van { $count }
raw-volume-custom = Aangepast
raw-volume-width = Breedte (voxels)
raw-volume-height = Hoogte (voxels)
raw-volume-slices = Plakken
raw-volume-type = Waardetype
raw-volume-type-unsigned = { $bits }-bit zonder teken
raw-volume-type-signed = { $bits }-bit met teken
raw-volume-type-float = { $bits }-bit drijvende komma
raw-volume-order = Bytevolgorde
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Header-offset
raw-volume-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } bytes
}
raw-volume-pixel = Pixelgrootte
raw-volume-spacing = Plakafstand
raw-volume-mm = { $value } mm
raw-volume-exact = Vult het bestand precies ({ $size })
raw-volume-fits = Gebruikt { $size } van { $file }; { $left } over
raw-volume-too-big = Heeft { $size } nodig, maar het bestand is { $file }
raw-volume-cancel = Annuleren
raw-volume-open = Openen
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Connectiviteit van { $seed }; verplaats het dradenkruis naar een andere seed
# machine-drafted, needs native review
mpr-measure-hint-edit = Sleep een uiteinde om het te verplaatsen; op het andere uiteinde om te verwijderen
# machine-drafted, needs native review
mpr-measure-removed = Meting verwijderd
mpr-measure-lengths-only = MPR meet alleen lengtes
mpr-more = Meer…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Gebogen MPR langs een aangeklikte curve
palette-mpr-curve-layout = Gebogen MPR: rechtgetrokken of uitgerekt
palette-mpr-curve-save = Curve opslaan…
palette-mpr-curve-load = Curve laden…
palette-mpr-export-segments = Elk segment als STL exporteren…
palette-mpr-dental = Tandheelkundige indeling (panorama en dwarsdoorsneden)
palette-mpr-nerve = Mandibulair kanaal intekenen
cpr-kind = { $kind ->
    [stretched] Uitgerekt
   *[straightened] Rechtgetrokken
}
cpr-button = Gebogen
cpr-controls = Bediening gebogen MPR
cpr-at = Op { $position }
cpr-section = Dwarsdoorsnede
cpr-hint-start = Klik punten langs de structuur in de { $view }-weergave
cpr-hint-more = Klik nog één punt
cpr-add-points = Punten toevoegen
cpr-undo-point = Punt ongedaan maken
# machine-drafted, needs native review
cpr-mark = Hier markeren
cpr-mark-set = Markering { $n } geplaatst.
cpr-mark-removed = Markering verwijderd.
cpr-mark-here = Markering { $n }
cpr-diameter = Diameter { $value }
cpr-diameters = Diameter { $min } tot { $max }
cpr-mark-span = Markering { $from } tot { $to }: { $length } langs de middellijn
cpr-mark-total = Alle markeringen: { $length }
cpr-clear = Curve wissen
cpr-save = Curve opslaan
cpr-load = Curve laden
cpr-done = Klaar
cpr-a11y = { $name } over { $length }, cursor op { $position }. Het wiel verplaatst de cursor.
cpr-none = Nog geen curve: klik eerst punten
cpr-cleared = Curve gewist
cpr-saved = Curve opgeslagen: { $name }
cpr-save-failed = De curve kon niet worden opgeslagen: { $reason }
cpr-loaded = Curve geladen
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 middellijn geladen
   *[other] { $count } middellijnen geladen
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] 1 opgeslagen middellijn gevonden
   *[other] { $count } opgeslagen middellijnen gevonden
}
# A centreline without a name, by its place in the list.
cpr-centreline = Middellijn { $number }
cpr-new = Nieuwe middellijn
cpr-rename = Naam wijzigen
cpr-name = Naam van de middellijn
cpr-delete = Verwijderen
# $name: the centreline's name.
cpr-delete-title = De middellijn ‘{ $name }’ verwijderen?
cpr-delete-detail = De punten verdwijnen. Opgeslagen bestanden blijven op de schijf.
cpr-deleted = ‘{ $name }’ verwijderd
cpr-load-failed = De curve kon niet worden geladen: { $reason }
cpr-load-not-json = het bestand is geen JSON
cpr-load-not-curves = het bestand is geen Specular-curve
cpr-load-empty = het bestand bevat geen curve van twee of meer punten
cpr-panoramic = Panorama
cpr-panoramic-slab = Panorama, slab van { $thickness } mm
cpr-panoramic-thin = Panorama, dun

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Tandheelkundig
dental-not-for-diagnosis = Niet voor diagnose of behandelplanning
dental-nerve = Mandibulair kanaal
dental-nerve-hint = Klik punten langs het mandibulair kanaal op het panorama of een dwarsdoorsnede
dental-nerve-done = Intekenen van het mandibulair kanaal uit
dental-spacing = Elke { $spacing } mm
dental-section = Doorsnede { $number }
dental-off = Open eerst de tandheelkundige indeling
dental-suggesting = Tandboog zoeken…
dental-suggested = Tandboog voorgesteld: sleep de punten om aan te passen
dental-no-arch = Geen tandboog gevonden: klik punten langs de boog in de axiale weergave
dental-vendor = Tandheelkundige CBCT van { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Segmenten exporteren
seg-stl-detail = Segmenten: { $count ->
        [one] { $count } driehoek
       *[other] { $count } driehoeken
    }
seg-stl-detail-full = Segmenten: volledig detail
seg-stl-none = Geen segmenten om te exporteren
seg-stl-not-labels = dit volume is geen labelkaart
seg-stl-exporting = Oppervlakken van de segmenten opbouwen…
seg-stl-saved = { $count ->
    [one] { $count } segment opgeslagen in { $folder }
   *[other] { $count } segmenten opgeslagen in { $folder }
}
seg-stl-open = Niet gesloten: { $count }.
seg-stl-empty = De segmenten zijn leeg: niets opgeslagen
seg-stl-failed = De segmenten konden niet worden opgeslagen: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Drie vlakken (MPR)
toolbar-mpr-close = Drie vlakken sluiten (MPR)
settings-viewing-toolbar-items-mpr = Drie vlakken (MPR)
