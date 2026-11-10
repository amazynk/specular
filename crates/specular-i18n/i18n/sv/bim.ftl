# machine-drafted, needs native review
## Granska 3D-modeller: gå och flyg, snittplan och snittlåda, sparade vyer, BCF-ärenden
## (3D-läge). Tangenterna som nämns är standardtangenterna.

cheatsheet-review = Granska

palette-mesh-walk = Gå
palette-mesh-fly = Flyg
palette-mesh-views = Sparade vyer
palette-mesh-save-view = Spara vyn
palette-mesh-section-plane = Lägg till ett snittplan
palette-mesh-section-box = Snittlåda
palette-mesh-section-flip = Vänd snittplanet
palette-mesh-section-clear = Ta bort alla snitt
palette-mesh-measure-cut = Mät på snittet
palette-mesh-bcf-topics = BCF-ärenden
# machine-drafted, needs native review
palette-mesh-model-checks = Modellkontroller
# machine-drafted, needs native review
palette-mesh-models = Modeller i scenen
# machine-drafted, needs native review
palette-mesh-add-model = Lägg till modell…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Lägg till modeller från en mapp…

# Gå och flyg: aviseringar och avläsningsrader, efter $what. $eye: ögats höjd över golvet,
# med enhet (bara walk-line).
mesh-nav = { $what ->
    [walk-toast] Går: W A S D flyttar, dra för att se dig om, Esc slutar
    [fly-toast] Flyger: W A S D flyttar, Q E ned och upp, Esc slutar
    [orbit] Orbit
    [walk-line] Går · ögat { $eye } över golvet
    [walk-keys] W A S D flyttar · dra tittar · Shift snabbare · Esc slutar
    [preparing] Väggarna är inte klara än: du går igenom dem
    [fly-keys] W A S D flyttar · Q E ned, upp · dra tittar · Shift snabbare · Esc slutar
   *[fly-line] Flyger
}

# Korta texter om snitt, mätning och sparade vyer, efter $what.
bim-note = { $what ->
    [plane-added] Snittplan tillagt: dra i handtaget för att flytta det
    [box-on] Snittlåda på: dra i en sidas handtag för att flytta den
    [box-off] Snittlåda av
    [cleared] Snitten borttagna
    [measure-on] Klicka på två punkter på snittet
    [measure-needs-cut] Lägg först till ett snittplan eller lådan
    [measure-off-cut] Klicka på snittet
    [measure-hint] Mäter: klicka på två punkter på snittet
    [ask-view-name] Vyns namn
    [ask-rename] Nytt namn
    [views-empty] Inga sparade vyer. Tryck N för att spara den här.
    [views-hints] Enter gå till · N spara · F2 byt namn · Delete ta bort · P spara som PNG
    [delete-detail] Bara den sparade vyn tas bort. Modellen ändras inte.
    [delete-ok] Ta bort
    [not-over-model] Välj en annan fil: den här skulle ersätta modellen.
   *[view-name] Vy
}

# Korta BCF-texter (BIM Collaboration Format), efter $what.
bcf-note = { $what ->
    [title-none] BCF-ärenden
    [new-file] ny
    [empty] Inga ärenden. Tryck O för att öppna en BCF-fil eller N för att lägga till ett ärende.
    [hints] Enter gå till · ← → vypunkter · N ärende · C kommentar · S status · E titel · O öppna · Ctrl+S exportera
    [ask-title] Ärendets titel
    [ask-comment] Kommentar
    [untitled] Namnlöst
    [topic-added] Ärende tillagt med den här vyn
   *[comment-added] Kommentar tillagd med den här vyn
}

# $planes: hur många snittplan.
bim-sections-line = { $planes ->
    [0] Inga snittplan
    [one] 1 snittplan
   *[other] { $planes } snittplan
}
# $size: snittlådans storlek med enhet.
bim-box-line = låda { $size }
# $count: högsta möjliga antal plan.
bim-planes-full = Högst { $count } snittplan
# Ett draghandtag, för skärmläsare. $n: dess nummer.
bim-handle = { $kind ->
    [box] Snittlådans sida { $n }
   *[plane] Snittplan { $n }
}
# $length: längden med enhet.
bim-measure-line = Längd på snittet: { $length }
bim-views-title = { $count ->
    [one] 1 sparad vy
   *[other] { $count } sparade vyer
}
# $name: en vys namn.
bim-view-saved = Sparade ”{ $name }”
bim-view-delete-title = Ta bort vyn ”{ $name }”?
# $file: en skriven fil.
bim-saved-file = Sparade { $file }
# $reason: varför det misslyckades.
bim-failed = { $what ->
    [open] Kunde inte öppna BCF-filen: { $reason }
    [views] Vyerna behålls bara under den här sessionen: { $reason }
   *[save] Kunde inte spara: { $reason }
}
# $count: ärenden; $file: arkivets filnamn.
bcf-title = { $count ->
    [one] 1 ärende
   *[other] { $count } ärenden
} · { $file }
bcf-comments = { $count ->
    [one] 1 kommentar
   *[other] { $count } kommentarer
}
# $at: vypunkten som visas; $count: ärendets vypunkter.
bcf-viewpoint-of = Vypunkt { $at } av { $count }
bcf-opened = { $count ->
    [one] 1 ärende öppnat
   *[other] { $count } ärenden öppnade
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
    [not-ifc] Den här filen är ingen IFC-modell.
    [not-ifc-open] Öppna en IFC-modell först. Bara IFC-modeller kan kombineras.
    [this-model] Den här modellen
    [hints] A lägg till modell · Shift+A lägg till mapp · Space visa eller dölj · C färg per modell · Enter gå till · Delete ta bort
    [origin-ok] Placera i origo
    [first-stays] Den öppnade modellen ligger kvar. Öppna en annan fil för att byta den.
    [remove-detail] Den lämnar scenen. Filen på disken finns kvar.
   *[remove-ok] Ta bort
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 objekt
   *[other] { $elements } objekt
} · { $shown ->
    [hidden] dold
   *[shown] visas
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] öppnad modell
    [map-crs] efter kartkoordinater ({ $crs })
    [map] efter kartkoordinater
    [site] efter tomtens latitud och longitud (ungefärlig)
    [project] efter delade projektkoordinater
   *[origin] i origo (inte justerad)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Modellerna använder olika koordinatsystem: { $first } och { $this }.
    [only-first] Den öppnade modellen är georefererad, den här är det inte.
    [only-this] Den här modellen är georefererad, den öppnade är det inte.
   *[broken] Den här modellens kartkonvertering kan inte användas (dess skala är noll).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Flyttad { $distance }, vriden { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Läser 1 modell…
   *[other] Läser { $count } modeller…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } kan inte justeras. Placera den i origo?
# machine-drafted, needs native review
fed-no-files = Inga IFC-filer i { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } lades inte till: { $reason }
# machine-drafted, needs native review
fed-added = { $file } tillagd. Modeller i scenen: { $count }.
# machine-drafted, needs native review
fed-remove-title = Ta bort { $file } från scenen?
# machine-drafted, needs native review
fed-removed = { $file } borttagen från scenen.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] dold
   *[shown] visas
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Färg per modell på
   *[off] Färg per modell av
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI och sparade vyer
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 annotering
   *[other] { $count } annoteringar
}
# machine-drafted, needs native review
pmi-view = Sparad vy
# machine-drafted, needs native review
pmi-view-all = visar alla annoteringar
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] visar 1 annotering
   *[other] visar { $count } annoteringar
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Visar vyn ”{ $name }”. A visar allt.
# machine-drafted, needs native review
pmi-has-faces = ytor markerade
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Ingen PMI i den här modellen.
    [hints] ↑ ↓ välj · Enter gå till · A alla · D som ritat · Shift+A dölj
    [hidden] Annoteringar dolda
    [shown] Annoteringar visas
    [as-drawn] Annoteringar som filen ritar dem
    [as-labels] Annoteringar som etiketter
   *[all-shown] Alla annoteringar visas
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Mått
    [tolerance] Geometrisk tolerans
    [datum] Referens
    [target] Delreferens
   *[graphic] Ritad annotering
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Avstånd
    [angular] Vinkel
    [diameter] Diameter
    [radius] Radie
    [length] Längd
    [thickness] Tjocklek
   *[other] Mått
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Rakhet
    [flatness] Planhet
    [circularity] Rundhet
    [cylindricity] Cylindricitet
    [line-profile] Linjeprofil
    [surface-profile] Ytprofil
    [angularity] Lutning
    [perpendicularity] Vinkelräthet
    [parallelism] Parallellitet
    [position] Läge
    [concentricity] Koncentricitet
    [coaxiality] Koaxialitet
    [symmetry] Symmetri
    [circular-runout] Cirkulärt kast
   *[total-runout] Totalkast
}
