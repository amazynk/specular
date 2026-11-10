# machine-drafted, needs native review
## Revize 3D modelů: chůze a let, řezové roviny a řezový kvádr, uložené pohledy, témata BCF
## (režim 3D). Uvedené klávesy jsou výchozí.

cheatsheet-review = Revize

palette-mesh-walk = Chůze
palette-mesh-fly = Let
palette-mesh-views = Uložené pohledy
palette-mesh-save-view = Uložit pohled
palette-mesh-section-plane = Přidat řezovou rovinu
palette-mesh-section-box = Řezový kvádr
palette-mesh-section-flip = Obrátit řezovou rovinu
palette-mesh-section-clear = Odstranit všechny řezy
palette-mesh-measure-cut = Měřit na řezu
palette-mesh-bcf-topics = Témata BCF
# machine-drafted, needs native review
palette-mesh-model-checks = Kontroly modelu
# machine-drafted, needs native review
palette-mesh-models = Modely ve scéně
# machine-drafted, needs native review
palette-mesh-add-model = Přidat model…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Přidat modely ze složky…

# Chůze a let: oznámení a řádky údajů podle $what. $eye: výška oka nad podlahou s jednotkou
# (jen walk-line).
mesh-nav = { $what ->
    [walk-toast] Chůze: W A S D pohyb, tažení rozhlížení, Esc ukončí
    [fly-toast] Let: W A S D pohyb, Q E dolů a nahoru, Esc ukončí
    [orbit] Orbit
    [walk-line] Chůze · oko { $eye } nad podlahou
    [walk-keys] W A S D pohyb · tažení rozhlížení · Shift rychleji · Esc ukončí
    [preparing] Stěny ještě nejsou připravené: projdete jimi
    [fly-keys] W A S D pohyb · Q E dolů, nahoru · tažení rozhlížení · Shift rychleji · Esc ukončí
   *[fly-line] Let
}

# Krátké texty k řezům, měření a uloženým pohledům podle $what.
bim-note = { $what ->
    [plane-added] Řezová rovina přidána: posuňte ji tažením za úchyt
    [box-on] Řezový kvádr zapnut: stěnu posuňte tažením za její úchyt
    [box-off] Řezový kvádr vypnut
    [cleared] Řezy odstraněny
    [measure-on] Klikněte na dva body na řezu
    [measure-needs-cut] Nejdřív přidejte řezovou rovinu nebo kvádr
    [measure-off-cut] Klikněte na řez
    [measure-hint] Měření: klikněte na dva body na řezu
    [ask-view-name] Název pohledu
    [ask-rename] Nový název
    [views-empty] Žádné uložené pohledy. Klávesou N uložíte tento.
    [views-hints] Enter přejít · N uložit · F2 přejmenovat · Delete odstranit · P uložit jako PNG
    [delete-detail] Odstraní se jen uložený pohled. Model se nezmění.
    [delete-ok] Odstranit
    [not-over-model] Vyberte jiný soubor: tento by nahradil model.
   *[view-name] Pohled
}

# Krátké texty BCF (BIM Collaboration Format) podle $what.
bcf-note = { $what ->
    [title-none] Témata BCF
    [new-file] nový
    [empty] Žádná témata. Klávesou O otevřete soubor BCF, klávesou N přidáte téma.
    [hints] Enter přejít · ← → pohledy · N téma · C komentář · S stav · E název · O otevřít · Ctrl+S exportovat
    [ask-title] Název tématu
    [ask-comment] Komentář
    [untitled] Bez názvu
    [topic-added] Téma přidáno s tímto pohledem
   *[comment-added] Komentář přidán s tímto pohledem
}

# $planes: počet řezových rovin.
bim-sections-line = { $planes ->
    [0] Žádné řezové roviny
    [one] 1 řezová rovina
    [few] { $planes } řezové roviny
    [many] { $planes } řezové roviny
   *[other] { $planes } řezových rovin
}
# $size: velikost řezového kvádru s jednotkou.
bim-box-line = kvádr { $size }
# $count: největší možný počet rovin.
bim-planes-full = Nejvýše { $count } řezových rovin
# Úchyt pro tažení, pro čtečky obrazovky. $n: jeho číslo.
bim-handle = { $kind ->
    [box] Stěna řezového kvádru { $n }
   *[plane] Řezová rovina { $n }
}
# $length: délka s jednotkou.
bim-measure-line = Délka na řezu: { $length }
bim-views-title = { $count ->
    [one] 1 uložený pohled
    [few] { $count } uložené pohledy
    [many] { $count } uloženého pohledu
   *[other] { $count } uložených pohledů
}
# $name: název pohledu.
bim-view-saved = Uloženo: „{ $name }“
bim-view-delete-title = Odstranit pohled „{ $name }“?
# $file: zapsaný soubor.
bim-saved-file = Uloženo: { $file }
# $reason: proč se to nepovedlo.
bim-failed = { $what ->
    [open] Soubor BCF nelze otevřít: { $reason }
    [views] Pohledy zůstanou jen pro tuto relaci: { $reason }
   *[save] Nelze uložit: { $reason }
}
# $count: témata; $file: název souboru archivu.
bcf-title = { $count ->
    [one] 1 téma
    [few] { $count } témata
    [many] { $count } tématu
   *[other] { $count } témat
} · { $file }
bcf-comments = { $count ->
    [one] 1 komentář
    [few] { $count } komentáře
    [many] { $count } komentáře
   *[other] { $count } komentářů
}
# $at: zobrazený pohled; $count: pohledy tématu.
bcf-viewpoint-of = Pohled { $at } z { $count }
bcf-opened = { $count ->
    [one] Otevřeno 1 téma
    [few] Otevřena { $count } témata
    [many] Otevřeno { $count } tématu
   *[other] Otevřeno { $count } témat
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modely: 1
    [few] Modely: { $count }
    [many] Modely: { $count }
   *[other] Modely: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Tento soubor není model IFC.
    [not-ifc-open] Nejprve otevřete model IFC. Kombinovat lze jen modely IFC.
    [this-model] Tento model
    [hints] A přidat model · Shift+A přidat složku · Space zobrazit nebo skrýt · C obarvit podle modelu · Enter přejít · Delete odstranit
    [origin-ok] Umístit do počátku
    [first-stays] Otevřený model zůstává. Chcete-li ho změnit, otevřete jiný soubor.
    [remove-detail] Zmizí ze scény. Soubor na disku zůstane.
   *[remove-ok] Odstranit
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 prvek
    [few] { $elements } prvky
    [many] { $elements } prvku
   *[other] { $elements } prvků
} · { $shown ->
    [hidden] skrytý
   *[shown] zobrazený
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] otevřený model
    [map-crs] podle mapových souřadnic ({ $crs })
    [map] podle mapových souřadnic
    [site] podle zeměpisné šířky a délky pozemku (přibližně)
    [project] podle sdílených souřadnic projektu
   *[origin] v počátku (nezarovnáno)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Modely používají různé souřadnicové systémy: { $first } a { $this }.
    [only-first] Otevřený model je georeferencovaný, tento ne.
    [only-this] Tento model je georeferencovaný, otevřený ne.
   *[broken] Mapový převod tohoto modelu nelze použít (jeho měřítko je nula).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Posunuto o { $distance }, otočeno o { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Načítá se 1 model…
    [few] Načítají se { $count } modely…
    [many] Načítá se { $count } modelu…
   *[other] Načítá se { $count } modelů…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } nelze zarovnat. Umístit ho do počátku?
# machine-drafted, needs native review
fed-no-files = Ve složce { $folder } nejsou žádné soubory IFC.
# machine-drafted, needs native review
fed-not-added = { $file } nebyl přidán: { $reason }
# machine-drafted, needs native review
fed-added = { $file } přidán. Modelů ve scéně: { $count }.
# machine-drafted, needs native review
fed-remove-title = Odstranit { $file } ze scény?
# machine-drafted, needs native review
fed-removed = { $file } odstraněn ze scény.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] skryt
   *[shown] zobrazen
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Obarvení podle modelu zapnuto
   *[off] Obarvení podle modelu vypnuto
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI a uložené pohledy
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 anotace
    [few] { $count } anotace
    [many] { $count } anotace
   *[other] { $count } anotací
}
# machine-drafted, needs native review
pmi-view = Uložený pohled
# machine-drafted, needs native review
pmi-view-all = zobrazuje všechny anotace
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] zobrazuje 1 anotaci
    [few] zobrazuje { $count } anotace
    [many] zobrazuje { $count } anotace
   *[other] zobrazuje { $count } anotací
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Zobrazen pohled „{ $name }“. A zobrazí vše.
# machine-drafted, needs native review
pmi-has-faces = plochy zvýrazněny
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Model nemá žádné PMI.
    [hints] ↑ ↓ vybrat · Enter přejít · A vše · D jak nakresleno · Shift+A skrýt
    [hidden] Anotace skryty
    [shown] Anotace zobrazeny
    [as-drawn] Anotace tak, jak je kreslí soubor
    [as-labels] Anotace jako popisky
   *[all-shown] Všechny anotace zobrazeny
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Kóta
    [tolerance] Geometrická tolerance
    [datum] Základna
    [target] Částečná základna
   *[graphic] Kreslená anotace
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Vzdálenost
    [angular] Úhel
    [diameter] Průměr
    [radius] Poloměr
    [length] Délka
    [thickness] Tloušťka
   *[other] Rozměr
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Přímost
    [flatness] Rovinnost
    [circularity] Kruhovitost
    [cylindricity] Válcovitost
    [line-profile] Tvar čáry
    [surface-profile] Tvar plochy
    [angularity] Sklon
    [perpendicularity] Kolmost
    [parallelism] Rovnoběžnost
    [position] Poloha
    [concentricity] Soustřednost
    [coaxiality] Souosost
    [symmetry] Souměrnost
    [circular-runout] Kruhové házení
   *[total-runout] Celkové házení
}
