# machine-drafted, needs native review
## 3D-modellen beoordelen: lopen en vliegen, doorsnedevlakken en doorsnedebox, opgeslagen
## weergaven, BCF-topics (3D-modus). De genoemde toetsen zijn de standaardtoetsen.

cheatsheet-review = Beoordelen

palette-mesh-walk = Lopen
palette-mesh-fly = Vliegen
palette-mesh-views = Opgeslagen weergaven
palette-mesh-save-view = Weergave opslaan
palette-mesh-section-plane = Doorsnedevlak toevoegen
palette-mesh-section-box = Doorsnedebox
palette-mesh-section-flip = Doorsnedevlak omdraaien
palette-mesh-section-clear = Alle doorsneden verwijderen
palette-mesh-measure-cut = Meten op de doorsnede
palette-mesh-bcf-topics = BCF-topics
# machine-drafted, needs native review
palette-mesh-model-checks = Modelcontroles
# machine-drafted, needs native review
palette-mesh-models = Modellen in de scène
# machine-drafted, needs native review
palette-mesh-add-model = Model toevoegen…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Modellen uit een map toevoegen…

# Lopen en vliegen: meldingen en uitleesregels, per $what. $eye: ooghoogte boven de vloer,
# met eenheid (alleen walk-line).
mesh-nav = { $what ->
    [walk-toast] Lopen: W A S D om te bewegen, slepen om rond te kijken, Esc om te stoppen
    [fly-toast] Vliegen: W A S D om te bewegen, Q E omlaag en omhoog, Esc om te stoppen
    [orbit] Orbit
    [walk-line] Lopen · oog { $eye } boven de vloer
    [walk-keys] W A S D bewegen · slepen kijkt · Shift sneller · Esc stopt
    [preparing] Muren nog niet klaar: u loopt erdoorheen
    [fly-keys] W A S D bewegen · Q E omlaag, omhoog · slepen kijkt · Shift sneller · Esc stopt
   *[fly-line] Vliegen
}

# Korte teksten bij doorsneden, meten en opgeslagen weergaven, per $what.
bim-note = { $what ->
    [plane-added] Doorsnedevlak toegevoegd: sleep de greep om het te verplaatsen
    [box-on] Doorsnedebox aan: sleep de greep van een zijde om die te verplaatsen
    [box-off] Doorsnedebox uit
    [cleared] Doorsneden verwijderd
    [measure-on] Klik op twee punten van de doorsnede
    [measure-needs-cut] Voeg eerst een doorsnedevlak of de box toe
    [measure-off-cut] Klik op de doorsnede
    [measure-hint] Meten: klik op twee punten van de doorsnede
    [ask-view-name] Naam van de weergave
    [ask-rename] Nieuwe naam
    [views-empty] Geen opgeslagen weergaven. Druk op N om deze op te slaan.
    [views-hints] Enter ga naar · N opslaan · F2 hernoemen · Delete verwijderen · P opslaan als PNG
    [delete-detail] Alleen de opgeslagen weergave verdwijnt. Het model verandert niet.
    [delete-ok] Verwijderen
    [not-over-model] Kies een ander bestand: dit zou het model vervangen.
   *[view-name] Weergave
}

# Korte BCF-teksten (BIM Collaboration Format), per $what.
bcf-note = { $what ->
    [title-none] BCF-topics
    [new-file] nieuw
    [empty] Geen topics. Druk op O om een BCF-bestand te openen of op N om een topic toe te voegen.
    [hints] Enter ga naar · ← → standpunten · N topic · C opmerking · S status · E titel · O openen · Ctrl+S exporteren
    [ask-title] Titel van het topic
    [ask-comment] Opmerking
    [untitled] Zonder titel
    [topic-added] Topic toegevoegd met deze weergave
   *[comment-added] Opmerking toegevoegd met deze weergave
}

# $planes: hoeveel doorsnedevlakken.
bim-sections-line = { $planes ->
    [0] Geen doorsnedevlakken
    [one] 1 doorsnedevlak
   *[other] { $planes } doorsnedevlakken
}
# $size: de grootte van de doorsnedebox, met eenheid.
bim-box-line = doorsnedebox { $size }
# $count: het maximale aantal vlakken.
bim-planes-full = Hoogstens { $count } doorsnedevlakken
# Een sleepgreep, voor schermlezers. $n: het nummer.
bim-handle = { $kind ->
    [box] Zijde { $n } van de doorsnedebox
   *[plane] Doorsnedevlak { $n }
}
# $length: de lengte met eenheid.
bim-measure-line = Lengte op de doorsnede: { $length }
bim-views-title = { $count ->
    [one] 1 opgeslagen weergave
   *[other] { $count } opgeslagen weergaven
}
# $name: de naam van een weergave.
bim-view-saved = ‘{ $name }’ opgeslagen
bim-view-delete-title = Weergave ‘{ $name }’ verwijderen?
# $file: een geschreven bestand.
bim-saved-file = { $file } opgeslagen
# $reason: waarom het mislukte.
bim-failed = { $what ->
    [open] Kan het BCF-bestand niet openen: { $reason }
    [views] Weergaven alleen voor deze sessie bewaard: { $reason }
   *[save] Kan niet opslaan: { $reason }
}
# $count: topics; $file: de bestandsnaam van het archief.
bcf-title = { $count ->
    [one] 1 onderwerp
   *[other] { $count } onderwerpen
} · { $file }
bcf-comments = { $count ->
    [one] 1 opmerking
   *[other] { $count } opmerkingen
}
# $at: het getoonde standpunt; $count: de standpunten van het topic.
bcf-viewpoint-of = Standpunt { $at } van { $count }
bcf-opened = { $count ->
    [one] 1 topic geopend
   *[other] { $count } topics geopend
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modellen: 1
   *[other] Modellen: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Dit bestand is geen IFC-model.
    [not-ifc-open] Open eerst een IFC-model. Alleen IFC-modellen kunnen worden gecombineerd.
    [this-model] Dit model
    [hints] A model toevoegen · Shift+A map toevoegen · Space tonen of verbergen · C kleuren per model · Enter ga naar · Delete verwijderen
    [origin-ok] In de oorsprong plaatsen
    [first-stays] Het geopende model blijft. Open een ander bestand om het te wijzigen.
    [remove-detail] Het verdwijnt uit de scène. Het bestand op schijf blijft bestaan.
   *[remove-ok] Verwijderen
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 element
   *[other] { $elements } elementen
} · { $shown ->
    [hidden] verborgen
   *[shown] getoond
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] geopend model
    [map-crs] op kaartcoördinaten ({ $crs })
    [map] op kaartcoördinaten
    [site] op breedte- en lengtegraad van de locatie (bij benadering)
    [project] op gedeelde projectcoördinaten
   *[origin] in de oorsprong (niet uitgelijnd)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] De modellen gebruiken verschillende coördinatenstelsels: { $first } en { $this }.
    [only-first] Het geopende model is gegeorefereerd, dit niet.
    [only-this] Dit model is gegeorefereerd, het geopende niet.
   *[broken] De kaartconversie van dit model is onbruikbaar (de schaal is nul).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = { $distance } verplaatst, { $angle }° gedraaid
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] 1 model wordt gelezen…
   *[other] { $count } modellen worden gelezen…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } kan niet worden uitgelijnd. In de oorsprong plaatsen?
# machine-drafted, needs native review
fed-no-files = Geen IFC-bestanden in { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } is niet toegevoegd: { $reason }
# machine-drafted, needs native review
fed-added = { $file } toegevoegd. Modellen in de scène: { $count }.
# machine-drafted, needs native review
fed-remove-title = { $file } uit de scène verwijderen?
# machine-drafted, needs native review
fed-removed = { $file } uit de scène verwijderd.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] verborgen
   *[shown] getoond
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Kleuren per model aan
   *[off] Kleuren per model uit
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI en opgeslagen weergaven
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 annotatie
   *[other] { $count } annotaties
}
# machine-drafted, needs native review
pmi-view = Opgeslagen weergave
# machine-drafted, needs native review
pmi-view-all = toont alle annotaties
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] toont 1 annotatie
   *[other] toont { $count } annotaties
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Weergave ‘{ $name }’ getoond. A toont alles.
# machine-drafted, needs native review
pmi-has-faces = vlakken gemarkeerd
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Geen PMI in dit model.
    [hints] ↑ ↓ kiezen · Enter ga naar · A alles · D zoals getekend · Shift+A verbergen
    [hidden] Annotaties verborgen
    [shown] Annotaties getoond
    [as-drawn] Annotaties zoals het bestand ze tekent
    [as-labels] Annotaties als labels
   *[all-shown] Alle annotaties getoond
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Maat
    [tolerance] Geometrische tolerantie
    [datum] Referentie
    [target] Referentiedoel
   *[graphic] Getekende annotatie
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Afstand
    [angular] Hoek
    [diameter] Diameter
    [radius] Straal
    [length] Lengte
    [thickness] Dikte
   *[other] Maat
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Rechtheid
    [flatness] Vlakheid
    [circularity] Rondheid
    [cylindricity] Cilindriciteit
    [line-profile] Vorm van een lijn
    [surface-profile] Vorm van een vlak
    [angularity] Hoekzuiverheid
    [perpendicularity] Haaksheid
    [parallelism] Evenwijdigheid
    [position] Plaats
    [concentricity] Concentriciteit
    [coaxiality] Coaxialiteit
    [symmetry] Symmetrie
    [circular-runout] Slingering
   *[total-runout] Totale slingering
}
