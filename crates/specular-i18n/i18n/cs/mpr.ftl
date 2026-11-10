# Multiplanar reformatting (MPR).

palette-mpr = Multiplanární rekonstrukce (MPR)
palette-mpr-slab = Další MPR vrstva (tenká, MIP, MinIP, průměr)
disabled-no-volume = Toto není objem: otevřete sérii, vícesnímkový soubor, 3D objem nebo z-stack
disabled-mpr-closed = Nejprve otevřete multiplanární zobrazení
mpr-axial = Axiální
mpr-coronal = Koronální
mpr-sagittal = Sagitální
mpr-oblique = Šikmá
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D zobrazení
mpr-thin = Tenká
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Průměr
mpr-slab = Vrstva: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Vrstva: { $mode }, { $thickness } Å
mpr-slab-unit = Vrstva: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Vrstva: { $mode }, voxely: { $thickness }
mpr-thicker = Silnější vrstva
mpr-thinner = Tenčí vrstva
mpr-reset = Obnovit zobrazení
mpr-close = Zavřít MPR
mpr-maximise = Maximalizovat zobrazení
mpr-export-planes = Exportovat roviny
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Proudnice z { $files }: vrstva { $thickness } mm
mpr-tracts-off = Proudnice z { $files }: skryté
mpr-tracts-none = Vedle tohoto objemu nejsou proudnice TrackVis ani MRtrix
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = voxely: { $value }
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } z { $count }
mpr-axis-channel = Kanál
mpr-axis-unnamed = Osa { $axis }
mpr-one-channel = MPR zobrazuje vždy jen jeden kanál
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = Kanály v barvě: { $count }, každý ve svém rozsahu zobrazení
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Zobrazen jeden kanál: { $reason }
mpr-isotropic = Soubor neuvádí velikost voxelu: voxely zobrazeny jako krychle
mpr-isotropic-z = Soubor neuvádí rozestup řezů: řezy zobrazeny pixel od sebe
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Načítání řezů: { $loaded } z { $count }
mpr-loaded = Řezy: { $count }, { $width } × { $height }
mpr-downsampled = Zobrazeno v 1/{ $factor } rozlišení (limity paměti nebo grafiky)
mpr-converted = Hodnoty zobrazeny jako 32bitová čísla s plovoucí čárkou
mpr-opening = Otevírání objemu…
mpr-gpu-memory = { $size } na grafické kartě
mpr-refused = Nelze sestavit objem: { $reason }
mpr-failed = Objem se nenačetl: { $reason }
mpr-no-gpu = Grafická karta tento objem nepojme: { $reason }
mpr-planes-saved = { $count ->
    [one] Uložena { $count } rovina do { $folder }
    [few] Uloženy { $count } roviny do { $folder }
    [many] Uloženo { $count } roviny do { $folder }
   *[other] Uloženo { $count } rovin do { $folder }
}
mpr-planes-failed = Roviny se nepodařilo uložit: { $reason }
mpr-view-a11y = Zobrazení { $view }, { $position } mm
mpr-crosshair-a11y = Zaměřovač na { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Zaměřovač na { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Zaměřovač na { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Zaměřovač na voxelu { $x }, { $y }, { $z }
mpr-controls = Ovládání MPR

# 3D view palette entries.
palette-mpr-preset = Další 3D předvolba (CT kost, měkké tkáně, plíce, angio; MR)
palette-mpr-render = Další 3D vykreslení (stínované, Cinematic, MIP, MinIP)
palette-mpr-clip = Další 3D ořez (žádný, box 2D zobrazení, odříznutý roh)
palette-mpr-export-stl = Exportovat 3D povrch jako STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Další vrstva proudnic v MPR
palette-mpr-maximise = Maximalizovat zobrazení MPR, nebo ukázat všechna zobrazení
palette-mpr-export-planes = Exportovat roviny MPR jako obrázky…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] CT kost
    [soft] CT měkké tkáně
    [lung] CT plíce
    [angio] CT angio
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Stínované
}
vr-clip = { $clip ->
    [box] Ořez: box zobrazení
    [cut] Ořez: odříznutý roh
   *[off] Bez ořezu
}
vr-smooth = Vyhladit povrch
vr-export = Exportovat STL
vr-threshold = Povrch na { $value }
vr-stl-exporting = Vytváření povrchu…
vr-stl-saved = { $count ->
    [0] Na tomto prahu nic není: povrch nebyl uložen
   *[other] Uloženo do { $name }, trojúhelníky: { $count }
}
vr-stl-failed = Povrch nelze uložit: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, povrch na { $threshold }. Shift+šipky otáčí, plus a minus přibližují.

## Surový objem: jak je uspořádán soubor .raw / .vol bez hlavičky

# machine-drafted, needs native review
raw-volume-title = Otevřít surový objem
raw-volume-file = { $file } nemá hlavičku. Určete, jak jsou vzorky uspořádány.
raw-volume-guess-label = Navržené uspořádání
raw-volume-guess = { $index } z { $count }
raw-volume-custom = Vlastní
raw-volume-width = Šířka (voxely)
raw-volume-height = Výška (voxely)
raw-volume-slices = Řezy
raw-volume-type = Typ vzorku
raw-volume-type-unsigned = { $bits }bitové bez znaménka
raw-volume-type-signed = { $bits }bitové se znaménkem
raw-volume-type-float = { $bits }bitové s plovoucí čárkou
raw-volume-order = Pořadí bajtů
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Posun hlavičky
raw-volume-bytes = { $count ->
    [one] { $count } bajt
    [few] { $count } bajty
    [many] { $count } bajtu
   *[other] { $count } bajtů
}
raw-volume-pixel = Velikost pixelu
raw-volume-spacing = Rozestup řezů
raw-volume-mm = { $value } mm
raw-volume-exact = Přesně vyplní soubor ({ $size })
raw-volume-fits = Využije { $size } z { $file }; zbývá { $left }
raw-volume-too-big = Potřebuje { $size }, ale soubor má { $file }
raw-volume-cancel = Zrušit
raw-volume-open = Otevřít
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Konektivita: { $seed }; přesuňte zaměřovací kříž na jiný seed
# machine-drafted, needs native review
mpr-measure-hint-edit = Přetažením konce jej posunete; na druhý konec jej odstraníte
# machine-drafted, needs native review
mpr-measure-removed = Měření odstraněno
mpr-measure-lengths-only = MPR měří jen délky
mpr-more = Další…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Zakřivená MPR podél klikané křivky
palette-mpr-curve-layout = Zakřivená MPR: narovnaná nebo natažená
palette-mpr-curve-save = Uložit křivku…
palette-mpr-curve-load = Načíst křivku…
palette-mpr-export-segments = Exportovat každý segment jako STL…
palette-mpr-dental = Dentální rozvržení (panoramatický snímek a příčné řezy)
palette-mpr-nerve = Vyznačit mandibulární kanál
cpr-kind = { $kind ->
    [stretched] Natažená
   *[straightened] Narovnaná
}
cpr-button = Zakřivená
cpr-controls = Ovládání zakřivené MPR
cpr-at = V { $position }
cpr-section = Příčný řez
cpr-hint-start = Klikejte na body podél struktury v zobrazení { $view }
cpr-hint-more = Klikněte ještě na jeden bod
cpr-add-points = Přidat body
cpr-undo-point = Vrátit bod
# machine-drafted, needs native review
cpr-mark = Označit zde
cpr-mark-set = Značka { $n } nastavena.
cpr-mark-removed = Značka odebrána.
cpr-mark-here = Značka { $n }
cpr-diameter = Průměr { $value }
cpr-diameters = Průměr { $min } až { $max }
cpr-mark-span = Značky { $from } až { $to }: { $length } podél středové linie
cpr-mark-total = Všechny značky: { $length }
cpr-clear = Vymazat křivku
cpr-save = Uložit křivku
cpr-load = Načíst křivku
cpr-done = Hotovo
cpr-a11y = { $name } v délce { $length }, kurzor v { $position }. Kolečko posouvá kurzor.
cpr-none = Zatím žádná křivka: nejprve klikněte na body
cpr-cleared = Křivka vymazána
cpr-saved = Křivka uložena: { $name }
cpr-save-failed = Křivku nelze uložit: { $reason }
cpr-loaded = Křivka načtena
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 středová linie načtena
    [few] { $count } středové linie načteny
    [many] { $count } středové linie načteno
   *[other] { $count } středových linií načteno
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] Nalezena 1 uložená středová linie
    [few] Nalezeny { $count } uložené středové linie
    [many] Nalezeno { $count } uložené středové linie
   *[other] Nalezeno { $count } uložených středových linií
}
# A centreline without a name, by its place in the list.
cpr-centreline = Středová linie { $number }
cpr-new = Nová středová linie
cpr-rename = Přejmenovat
cpr-name = Název středové linie
cpr-delete = Smazat
# $name: the centreline's name.
cpr-delete-title = Smazat středovou linii „{ $name }“?
cpr-delete-detail = Její body zmizí. Soubory uložené na disku zůstanou.
cpr-deleted = Smazáno: „{ $name }“
cpr-load-failed = Křivku nelze načíst: { $reason }
cpr-load-not-json = soubor není JSON
cpr-load-not-curves = soubor není křivka Specular
cpr-load-empty = soubor neobsahuje žádnou křivku ze dvou nebo více bodů
cpr-panoramic = Panoramatický snímek
cpr-panoramic-slab = Panoramatický snímek, vrstva { $thickness } mm
cpr-panoramic-thin = Panoramatický snímek, tenký

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Dentální
dental-not-for-diagnosis = Není určeno k diagnostice ani plánování léčby
dental-nerve = Mandibulární kanál
dental-nerve-hint = Klikejte na body podél mandibulárního kanálu na panoramatickém snímku nebo v příčném řezu
dental-nerve-done = Vyznačování mandibulárního kanálu vypnuto
dental-spacing = Každých { $spacing } mm
dental-section = Řez { $number }
dental-off = Nejprve otevřete dentální rozvržení
dental-suggesting = Hledání zubního oblouku…
dental-suggested = Navržen zubní oblouk: tažením jeho bodů jej upravte
dental-no-arch = Zubní oblouk nenalezen: klikejte na body podél něj v axiálním zobrazení
dental-vendor = Dentální CBCT { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Exportovat segmenty
seg-stl-detail = Segmenty: { $count ->
        [one] { $count } trojúhelník
        [few] { $count } trojúhelníky
        [many] { $count } trojúhelníku
       *[other] { $count } trojúhelníků
    }
seg-stl-detail-full = Segmenty: plné rozlišení
seg-stl-none = Žádné segmenty k exportu
seg-stl-not-labels = tento objem není mapa štítků
seg-stl-exporting = Vytváření povrchů segmentů…
seg-stl-saved = { $count ->
    [one] Uložen { $count } segment do { $folder }
    [few] Uloženy { $count } segmenty do { $folder }
    [many] Uloženo { $count } segmentu do { $folder }
   *[other] Uloženo { $count } segmentů do { $folder }
}
seg-stl-open = Neuzavřené: { $count }.
seg-stl-empty = Segmenty jsou prázdné: nic nebylo uloženo
seg-stl-failed = Segmenty nelze uložit: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Tři roviny (MPR)
toolbar-mpr-close = Zavřít tři roviny (MPR)
settings-viewing-toolbar-items-mpr = Tři roviny (MPR)
