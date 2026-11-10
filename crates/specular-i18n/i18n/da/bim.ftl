# machine-drafted, needs native review
## Gennemgang af 3D-modeller: gå og flyv, snitplaner og snitboks, gemte visninger, BCF-emner
## (3D-tilstand). De nævnte taster er standardtasterne.

cheatsheet-review = Gennemgang

palette-mesh-walk = Gå
palette-mesh-fly = Flyv
palette-mesh-views = Gemte visninger
palette-mesh-save-view = Gem visningen
palette-mesh-section-plane = Tilføj et snitplan
palette-mesh-section-box = Snitboks
palette-mesh-section-flip = Vend snitplanet
palette-mesh-section-clear = Fjern alle snit
palette-mesh-measure-cut = Mål på snittet
palette-mesh-bcf-topics = BCF-emner
# machine-drafted, needs native review
palette-mesh-model-checks = Modelkontrol
# machine-drafted, needs native review
palette-mesh-models = Modeller i scenen
# machine-drafted, needs native review
palette-mesh-add-model = Tilføj model…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Tilføj modeller fra en mappe…

# Gå og flyv: beskeder og aflæsningslinjer, efter $what. $eye: øjenhøjden over gulvet, med
# enhed (kun walk-line).
mesh-nav = { $what ->
    [walk-toast] Går: W A S D flytter, træk for at se dig om, Esc stopper
    [fly-toast] Flyver: W A S D flytter, Q E ned og op, Esc stopper
    [orbit] Orbit
    [walk-line] Går · øjet { $eye } over gulvet
    [walk-keys] W A S D flytter · træk ser · Shift hurtigere · Esc stopper
    [preparing] Væggene er ikke klar endnu: du går igennem dem
    [fly-keys] W A S D flytter · Q E ned, op · træk ser · Shift hurtigere · Esc stopper
   *[fly-line] Flyver
}

# Korte tekster om snit, måling og gemte visninger, efter $what.
bim-note = { $what ->
    [plane-added] Snitplan tilføjet: træk i håndtaget for at flytte det
    [box-on] Snitboks til: træk i en sides håndtag for at flytte den
    [box-off] Snitboks fra
    [cleared] Snit fjernet
    [measure-on] Klik på to punkter på snittet
    [measure-needs-cut] Tilføj først et snitplan eller boksen
    [measure-off-cut] Klik på snittet
    [measure-hint] Måler: klik på to punkter på snittet
    [ask-view-name] Visningens navn
    [ask-rename] Nyt navn
    [views-empty] Ingen gemte visninger. Tryk N for at gemme denne.
    [views-hints] Enter gå til · N gem · F2 omdøb · Delete fjern · P gem som PNG
    [delete-detail] Kun den gemte visning fjernes. Modellen ændres ikke.
    [delete-ok] Fjern
    [not-over-model] Vælg en anden fil: denne ville erstatte modellen.
   *[view-name] Visning
}

# Korte BCF-tekster (BIM Collaboration Format), efter $what.
bcf-note = { $what ->
    [title-none] BCF-emner
    [new-file] ny
    [empty] Ingen emner. Tryk O for at åbne en BCF-fil eller N for at tilføje et emne.
    [hints] Enter gå til · ← → synspunkter · N emne · C kommentar · S status · E titel · O åbn · Ctrl+S eksportér
    [ask-title] Emnets titel
    [ask-comment] Kommentar
    [untitled] Uden titel
    [topic-added] Emne tilføjet med denne visning
   *[comment-added] Kommentar tilføjet med denne visning
}

# $planes: hvor mange snitplaner.
bim-sections-line = { $planes ->
    [0] Ingen snitplaner
    [one] 1 snitplan
   *[other] { $planes } snitplaner
}
# $size: snitboksens størrelse med enhed.
bim-box-line = boks { $size }
# $count: det højeste antal planer.
bim-planes-full = Højst { $count } snitplaner
# Et trækhåndtag, til skærmlæsere. $n: dets nummer.
bim-handle = { $kind ->
    [box] Snitboksens side { $n }
   *[plane] Snitplan { $n }
}
# $length: længden med enhed.
bim-measure-line = Længde på snittet: { $length }
bim-views-title = { $count ->
    [one] 1 gemt visning
   *[other] { $count } gemte visninger
}
# $name: en visnings navn.
bim-view-saved = “{ $name }” gemt
bim-view-delete-title = Fjern visningen “{ $name }”?
# $file: en skrevet fil.
bim-saved-file = { $file } gemt
# $reason: hvorfor det mislykkedes.
bim-failed = { $what ->
    [open] Kunne ikke åbne BCF-filen: { $reason }
    [views] Visningerne gemmes kun i denne session: { $reason }
   *[save] Kunne ikke gemme: { $reason }
}
# $count: emner; $file: arkivets filnavn.
bcf-title = { $count ->
    [one] 1 emne
   *[other] { $count } emner
} · { $file }
bcf-comments = { $count ->
    [one] 1 kommentar
   *[other] { $count } kommentarer
}
# $at: det viste synspunkt; $count: emnets synspunkter.
bcf-viewpoint-of = Synspunkt { $at } af { $count }
bcf-opened = { $count ->
    [one] 1 emne åbnet
   *[other] { $count } emner åbnet
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modeller: 1
   *[other] Modeller: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Denne fil er ikke en IFC-model.
    [not-ifc-open] Åbn først en IFC-model. Kun IFC-modeller kan kombineres.
    [this-model] Denne model
    [hints] A tilføj model · Shift+A tilføj mappe · Space vis eller skjul · C farv efter model · Enter gå til · Delete fjern
    [origin-ok] Placér i origo
    [first-stays] Den åbnede model bliver. Åbn en anden fil for at skifte den.
    [remove-detail] Den forlader scenen. Filen på disken bliver.
   *[remove-ok] Fjern
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 element
   *[other] { $elements } elementer
} · { $shown ->
    [hidden] skjult
   *[shown] vist
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] åbnet model
    [map-crs] efter kortkoordinater ({ $crs })
    [map] efter kortkoordinater
    [site] efter grundens bredde- og længdegrad (omtrentlig)
    [project] efter fælles projektkoordinater
   *[origin] i origo (ikke justeret)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Modellerne bruger forskellige koordinatsystemer: { $first } og { $this }.
    [only-first] Den åbnede model er georefereret, denne er ikke.
    [only-this] Denne model er georefereret, den åbnede er ikke.
   *[broken] Denne models kortkonvertering kan ikke bruges (dens skala er nul).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Flyttet { $distance }, drejet { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Læser 1 model…
   *[other] Læser { $count } modeller…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } kan ikke justeres. Placér den i origo?
# machine-drafted, needs native review
fed-no-files = Ingen IFC-filer i { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } blev ikke tilføjet: { $reason }
# machine-drafted, needs native review
fed-added = { $file } tilføjet. Modeller i scenen: { $count }.
# machine-drafted, needs native review
fed-remove-title = Fjern { $file } fra scenen?
# machine-drafted, needs native review
fed-removed = { $file } fjernet fra scenen.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] skjult
   *[shown] vist
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Farv efter model til
   *[off] Farv efter model fra
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI og gemte visninger
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 annotering
   *[other] { $count } annoteringer
}
# machine-drafted, needs native review
pmi-view = Gemt visning
# machine-drafted, needs native review
pmi-view-all = viser alle annoteringer
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] viser 1 annotering
   *[other] viser { $count } annoteringer
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Viser visningen “{ $name }”. A viser alle.
# machine-drafted, needs native review
pmi-has-faces = flader fremhævet
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Ingen PMI i denne model.
    [hints] ↑ ↓ vælg · Enter gå til · A alle · D som tegnet · Shift+A skjul
    [hidden] Annoteringer skjult
    [shown] Annoteringer vist
    [as-drawn] Annoteringer, som filen tegner dem
    [as-labels] Annoteringer som etiketter
   *[all-shown] Alle annoteringer vist
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Mål
    [tolerance] Geometrisk tolerance
    [datum] Reference
    [target] Delreference
   *[graphic] Tegnet annotering
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Afstand
    [angular] Vinkel
    [diameter] Diameter
    [radius] Radius
    [length] Længde
    [thickness] Tykkelse
   *[other] Mål
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Retlinethed
    [flatness] Planhed
    [circularity] Rundhed
    [cylindricity] Cylindricitet
    [line-profile] Linjeprofil
    [surface-profile] Fladeprofil
    [angularity] Hældning
    [perpendicularity] Vinkelrethed
    [parallelism] Parallelitet
    [position] Position
    [concentricity] Koncentricitet
    [coaxiality] Koaksialitet
    [symmetry] Symmetri
    [circular-runout] Cirkulært kast
   *[total-runout] Totalkast
}
