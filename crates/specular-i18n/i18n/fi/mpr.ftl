# Multiplanar reformatting (MPR).

palette-mpr = Monitasorekonstruktio (MPR)
palette-mpr-slab = Seuraava MPR-laatta (ohut, MIP, MinIP, keskiarvo)
disabled-no-volume = Tämä ei ole tilavuus: avaa sarja, monikehyksinen tiedosto, 3D-tilavuus tai z-pino
disabled-mpr-closed = Avaa ensin monitasonäkymät
mpr-axial = Aksiaalinen
mpr-coronal = Koronaalinen
mpr-sagittal = Sagittaalinen
mpr-oblique = Vino
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D-näkymä
mpr-thin = Ohut
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Keskiarvo
mpr-slab = Laatta: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Laatta: { $mode }, { $thickness } Å
mpr-slab-unit = Laatta: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Laatta: { $mode }, { $thickness } vokselia
mpr-thicker = Paksumpi laatta
mpr-thinner = Ohuempi laatta
mpr-reset = Palauta näkymät
mpr-close = Sulje MPR
mpr-maximise = Suurenna näkymä
mpr-export-planes = Vie tasot
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Virtaviivat ({ $files }): laatta { $thickness } mm
mpr-tracts-off = Virtaviivat ({ $files }): piilotettu
mpr-tracts-none = Tämän tilavuuden vieressä ei ole TrackVis- tai MRtrix-virtaviivoja
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } vokselia
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position }/{ $count }
mpr-axis-channel = Kanava
mpr-axis-unnamed = Akseli { $axis }
mpr-one-channel = MPR näyttää yhden kanavan kerrallaan
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } kanavaa värein, kukin omalla näyttöalueellaan
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Yksi kanava näytetään: { $reason }
mpr-isotropic = Tiedostossa ei ole vokselikokoa: vokselit näytetään kuutioina
mpr-isotropic-z = Tiedostossa ei ole leikeväliä: leikkeet näytetään pikselin välein
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Ladataan leikkeitä { $loaded }/{ $count }
mpr-loaded = { $count ->
        [one] { $count } leike
       *[other] { $count } leikettä
    }, { $width } × { $height }
mpr-downsampled = Näytetään 1/{ $factor } tarkkuudella (muisti- tai grafiikkarajat)
mpr-converted = Arvot näytetään 32-bittisinä liukulukuina
mpr-opening = Avataan tilavuutta…
mpr-gpu-memory = { $size } näytönohjaimella
mpr-refused = Tilavuutta ei voi muodostaa: { $reason }
mpr-failed = Tilavuus ei latautunut: { $reason }
mpr-no-gpu = Näytönohjain ei pysty käsittelemään tätä tilavuutta: { $reason }
mpr-planes-saved = { $count ->
        [one] { $count } taso tallennettu
       *[other] { $count } tasoa tallennettu
    } kansioon { $folder }
mpr-planes-failed = Tasoja ei voitu tallentaa: { $reason }
mpr-view-a11y = Näkymä { $view }, { $position } mm
mpr-crosshair-a11y = Hiusristikko kohdassa { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Hiusristikko kohdassa { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Hiusristikko kohdassa { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Hiusristikko vokselissa { $x }, { $y }, { $z }
mpr-controls = MPR-säätimet

# 3D view palette entries.
palette-mpr-preset = Seuraava 3D-esiasetus (TT luu, pehmytkudos, keuhko, angio; MK)
palette-mpr-render = Seuraava 3D-renderöinti (varjostettu, Cinematic, MIP, MinIP)
palette-mpr-clip = Seuraava 3D-leikkaus (ei mitään, 2D-näkymien laatikko, kulma leikattu pois)
palette-mpr-export-stl = Vie 3D-pinta STL-tiedostona…
# machine-drafted, needs native review
palette-mpr-tract-slab = Seuraava virtaviivalaatta MPR:ssä
palette-mpr-maximise = Suurenna MPR-näkymä tai näytä kaikki näkymät
palette-mpr-export-planes = Vie MPR-tasot kuvina…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] TT luu
    [soft] TT pehmytkudos
    [lung] TT keuhko
    [angio] TT-angio
   *[mr] MK
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Varjostettu
}
vr-clip = { $clip ->
    [box] Leikkaus: näkymien laatikko
    [cut] Leikkaus: kulma pois
   *[off] Ei leikkausta
}
vr-smooth = Tasoita pinta
vr-export = Vie STL
vr-threshold = Pinta arvossa { $value }
vr-stl-exporting = Rakennetaan pintaa…
vr-stl-saved = { $count ->
    [0] Tällä kynnyksellä ei ole mitään: pintaa ei tallennettu
   *[other] Tallennettu tiedostoon { $name }, kolmioita: { $count }
}
vr-stl-failed = Pintaa ei voitu tallentaa: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, pinta arvossa { $threshold }. Vaihto+nuolet kiertävät, plus ja miinus zoomaavat.

## Raakatilavuus: miten otsakkeeton .raw- / .vol-tiedosto on järjestetty

# machine-drafted, needs native review
raw-volume-title = Avaa raakatilavuus
raw-volume-file = Tiedostossa { $file } ei ole otsaketta. Kerro, miten sen arvot on järjestetty.
raw-volume-guess-label = Ehdotettu järjestys
raw-volume-guess = { $index }/{ $count }
raw-volume-custom = Mukautettu
raw-volume-width = Leveys (vokseleina)
raw-volume-height = Korkeus (vokseleina)
raw-volume-slices = Leikkeet
raw-volume-type = Arvotyyppi
raw-volume-type-unsigned = Etumerkitön { $bits }-bittinen
raw-volume-type-signed = Etumerkillinen { $bits }-bittinen
raw-volume-type-float = Liukuluku { $bits }-bittinen
raw-volume-order = Tavujärjestys
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Otsakkeen siirtymä
raw-volume-bytes = { $count ->
    [one] { $count } tavu
   *[other] { $count } tavua
}
raw-volume-pixel = Pikselikoko
raw-volume-spacing = Leikeväli
raw-volume-mm = { $value } mm
raw-volume-exact = Täyttää tiedoston tarkalleen ({ $size })
raw-volume-fits = Käyttää { $size } / { $file }; { $left } jää yli
raw-volume-too-big = Tarvitsee { $size }, mutta tiedosto on { $file }
raw-volume-cancel = Peruuta
raw-volume-open = Avaa
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Konnektiivisuus: { $seed }; siirrä hiusristikko toiseen siemenpisteeseen
# machine-drafted, needs native review
mpr-measure-hint-edit = Siirrä päätä vetämällä; vedä toisen pään päälle poistaaksesi
# machine-drafted, needs native review
mpr-measure-removed = Mittaus poistettu
mpr-measure-lengths-only = MPR mittaa vain pituuksia
mpr-more = Lisää…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Kaareva MPR napsautettua käyrää pitkin
palette-mpr-curve-layout = Kaareva MPR: suoristettu tai venytetty
palette-mpr-curve-save = Tallenna käyrä…
palette-mpr-curve-load = Lataa käyrä…
palette-mpr-export-segments = Vie jokainen segmentti STL-tiedostona…
palette-mpr-dental = Hammasasettelu (panoraama ja poikkileikkeet)
palette-mpr-nerve = Piirrä mandibulakanava
cpr-kind = { $kind ->
    [stretched] Venytetty
   *[straightened] Suoristettu
}
cpr-button = Kaareva
cpr-controls = Kaarevan MPR:n säätimet
cpr-at = Kohdassa { $position }
cpr-section = Poikkileike
cpr-hint-start = Napsauta pisteitä rakennetta pitkin näkymässä { $view }
cpr-hint-more = Napsauta vielä yksi piste
cpr-add-points = Lisää pisteitä
cpr-undo-point = Kumoa piste
# machine-drafted, needs native review
cpr-mark = Merkitse tähän
cpr-mark-set = Merkki { $n } asetettu.
cpr-mark-removed = Merkki poistettu.
cpr-mark-here = Merkki { $n }
cpr-diameter = Halkaisija { $value }
cpr-diameters = Halkaisija { $min }–{ $max }
cpr-mark-span = Merkit { $from }–{ $to }: { $length } keskilinjaa pitkin
cpr-mark-total = Kaikki merkit: { $length }
cpr-clear = Tyhjennä käyrä
cpr-save = Tallenna käyrä
cpr-load = Lataa käyrä
cpr-done = Valmis
cpr-a11y = { $name }, pituus { $length }, kohdistin kohdassa { $position }. Rulla siirtää kohdistinta.
cpr-none = Ei vielä käyrää: napsauta ensin pisteitä
cpr-cleared = Käyrä tyhjennetty
cpr-saved = Käyrä tallennettu: { $name }
cpr-save-failed = Käyrää ei voitu tallentaa: { $reason }
cpr-loaded = Käyrä ladattu
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 keskilinja ladattu
   *[other] { $count } keskilinjaa ladattu
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] 1 tallennettu keskilinja löytyi
   *[other] { $count } tallennettua keskilinjaa löytyi
}
# A centreline without a name, by its place in the list.
cpr-centreline = Keskilinja { $number }
cpr-new = Uusi keskilinja
cpr-rename = Nimeä uudelleen
cpr-name = Keskilinjan nimi
cpr-delete = Poista
# $name: the centreline's name.
cpr-delete-title = Poistetaanko keskilinja ”{ $name }”?
cpr-delete-detail = Sen pisteet poistuvat. Levylle tallennetut tiedostot säilyvät.
cpr-deleted = ”{ $name }” poistettu
cpr-load-failed = Käyrää ei voitu ladata: { $reason }
cpr-load-not-json = tiedosto ei ole JSON-muotoinen
cpr-load-not-curves = tiedosto ei ole Specular-käyrä
cpr-load-empty = tiedostossa ei ole käyrää, jossa on vähintään kaksi pistettä
cpr-panoramic = Panoraama
cpr-panoramic-slab = Panoraama, laatta { $thickness } mm
cpr-panoramic-thin = Panoraama, ohut

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Hammas
dental-not-for-diagnosis = Ei diagnoosiin eikä hoidon suunnitteluun
dental-nerve = Mandibulakanava
dental-nerve-hint = Napsauta pisteitä mandibulakanavaa pitkin panoraamassa tai poikkileikkeessä
dental-nerve-done = Mandibulakanavan piirto pois
dental-spacing = { $spacing } mm:n välein
dental-section = Leike { $number }
dental-off = Avaa ensin hammasasettelu
dental-suggesting = Etsitään hammaskaarta…
dental-suggested = Hammaskaari ehdotettu: säädä vetämällä sen pisteitä
dental-no-arch = Hammaskaarta ei löytynyt: napsauta pisteitä sitä pitkin aksiaalisessa näkymässä
dental-vendor = Hammas-KKTT ({ $vendor })

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Vie segmentit
seg-stl-detail = Segmentit: { $count ->
        [one] { $count } kolmio
       *[other] { $count } kolmiota
    }
seg-stl-detail-full = Segmentit: täysi tarkkuus
seg-stl-none = Ei vietäviä segmenttejä
seg-stl-not-labels = tämä tilavuus ei ole nimiökartta
seg-stl-exporting = Rakennetaan segmenttien pintoja…
seg-stl-saved = { $count ->
    [one] { $count } segmentti tallennettu kansioon { $folder }
   *[other] { $count } segmenttiä tallennettu kansioon { $folder }
}
seg-stl-open = Sulkemattomia: { $count }.
seg-stl-empty = Segmentit ovat tyhjiä: mitään ei tallennettu
seg-stl-failed = Segmenttejä ei voitu tallentaa: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Kolme tasoa (MPR)
toolbar-mpr-close = Sulje kolme tasoa (MPR)
settings-viewing-toolbar-items-mpr = Kolme tasoa (MPR)
