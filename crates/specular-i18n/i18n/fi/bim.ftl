# machine-drafted, needs native review
## 3D-mallien tarkastelu: kävely ja lento, leikkaustasot ja leikkauslaatikko, tallennetut
## näkymät, BCF-aiheet (3D-tila). Mainitut näppäimet ovat oletusnäppäimet.

cheatsheet-review = Tarkastelu

palette-mesh-walk = Kävele
palette-mesh-fly = Lennä
palette-mesh-views = Tallennetut näkymät
palette-mesh-save-view = Tallenna näkymä
palette-mesh-section-plane = Lisää leikkaustaso
palette-mesh-section-box = Leikkauslaatikko
palette-mesh-section-flip = Käännä leikkaustaso
palette-mesh-section-clear = Poista kaikki leikkaukset
palette-mesh-measure-cut = Mittaa leikkauksesta
palette-mesh-bcf-topics = BCF-aiheet
# machine-drafted, needs native review
palette-mesh-model-checks = Mallin tarkistukset
# machine-drafted, needs native review
palette-mesh-models = Kohtauksen mallit
# machine-drafted, needs native review
palette-mesh-add-model = Lisää malli…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Lisää mallit kansiosta…

# Kävely ja lento: ilmoitukset ja lukemarivit, $what-arvon mukaan. $eye: silmän korkeus
# lattiasta yksikön kanssa (vain walk-line).
mesh-nav = { $what ->
    [walk-toast] Kävely: W A S D liikkuu, vedä katsoaksesi, Esc lopettaa
    [fly-toast] Lento: W A S D liikkuu, Q E alas ja ylös, Esc lopettaa
    [orbit] Kierto
    [walk-line] Kävely · silmä { $eye } lattiasta
    [walk-keys] W A S D liikkuu · veto katsoo · Shift nopeammin · Esc lopettaa
    [preparing] Seinät eivät ole vielä valmiita: kuljet niiden läpi
    [fly-keys] W A S D liikkuu · Q E alas, ylös · veto katsoo · Shift nopeammin · Esc lopettaa
   *[fly-line] Lento
}

# Leikkausten, mittauksen ja tallennettujen näkymien lyhyet tekstit, $what-arvon mukaan.
bim-note = { $what ->
    [plane-added] Leikkaustaso lisätty: siirrä sitä vetämällä kahvasta
    [box-on] Leikkauslaatikko päällä: siirrä tahkoa vetämällä sen kahvasta
    [box-off] Leikkauslaatikko pois
    [cleared] Leikkaukset poistettu
    [measure-on] Napsauta kahta pistettä leikkauksessa
    [measure-needs-cut] Lisää ensin leikkaustaso tai laatikko
    [measure-off-cut] Napsauta leikkausta
    [measure-hint] Mittaus: napsauta kahta pistettä leikkauksessa
    [ask-view-name] Näkymän nimi
    [ask-rename] Uusi nimi
    [views-empty] Ei tallennettuja näkymiä. Tallenna tämä painamalla N.
    [views-hints] Enter siirry · N tallenna · F2 nimeä · Delete poista · P tallenna PNG:nä
    [delete-detail] Vain tallennettu näkymä poistetaan. Malli ei muutu.
    [delete-ok] Poista
    [not-over-model] Valitse toinen tiedosto: tämä korvaisi mallin.
   *[view-name] Näkymä
}

# BCF:n (BIM Collaboration Format) lyhyet tekstit, $what-arvon mukaan.
bcf-note = { $what ->
    [title-none] BCF-aiheet
    [new-file] uusi
    [empty] Ei aiheita. Avaa BCF-tiedosto painamalla O tai lisää aihe painamalla N.
    [hints] Enter siirry · ← → katselupisteet · N aihe · C kommentti · S tila · E otsikko · O avaa · Ctrl+S vie
    [ask-title] Aiheen otsikko
    [ask-comment] Kommentti
    [untitled] Nimetön
    [topic-added] Aihe lisätty tällä näkymällä
   *[comment-added] Kommentti lisätty tällä näkymällä
}

# $planes: leikkaustasojen määrä.
bim-sections-line = { $planes ->
    [0] Ei leikkaustasoja
    [one] 1 leikkaustaso
   *[other] { $planes } leikkaustasoa
}
# $size: leikkauslaatikon koko yksikön kanssa.
bim-box-line = laatikko { $size }
# $count: tasojen enimmäismäärä.
bim-planes-full = Enintään { $count } leikkaustasoa
# Vetokahva ruudunlukijoille. $n: sen numero.
bim-handle = { $kind ->
    [box] Leikkauslaatikon tahko { $n }
   *[plane] Leikkaustaso { $n }
}
# $length: pituus yksikön kanssa.
bim-measure-line = Pituus leikkauksessa: { $length }
bim-views-title = { $count ->
    [one] 1 tallennettu näkymä
   *[other] { $count } tallennettua näkymää
}
# $name: näkymän nimi.
bim-view-saved = ”{ $name }” tallennettu
bim-view-delete-title = Poistetaanko näkymä ”{ $name }”?
# $file: kirjoitettu tiedosto.
bim-saved-file = { $file } tallennettu
# $reason: miksi se epäonnistui.
bim-failed = { $what ->
    [open] BCF-tiedoston avaaminen epäonnistui: { $reason }
    [views] Näkymät säilyvät vain tämän istunnon ajan: { $reason }
   *[save] Tallennus epäonnistui: { $reason }
}
# $count: aiheet; $file: arkiston tiedostonimi.
bcf-title = { $count ->
    [one] 1 aihe
   *[other] { $count } aihetta
} · { $file }
bcf-comments = { $count ->
    [one] 1 kommentti
   *[other] { $count } kommenttia
}
# $at: näytettävä katselupiste; $count: aiheen katselupisteet.
bcf-viewpoint-of = Katselupiste { $at }/{ $count }
bcf-opened = { $count ->
    [one] 1 aihe avattu
   *[other] { $count } aihetta avattu
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Malleja: 1
   *[other] Malleja: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Tämä tiedosto ei ole IFC-malli.
    [not-ifc-open] Avaa ensin IFC-malli. Vain IFC-malleja voi yhdistää.
    [this-model] Tämä malli
    [hints] A lisää malli · Shift+A lisää kansio · Space näytä tai piilota · C väritä malleittain · Enter siirry · Delete poista
    [origin-ok] Sijoita origoon
    [first-stays] Avattu malli pysyy. Vaihda se avaamalla toinen tiedosto.
    [remove-detail] Se poistuu kohtauksesta. Levyllä oleva tiedosto säilyy.
   *[remove-ok] Poista
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 rakennusosa
   *[other] { $elements } rakennusosaa
} · { $shown ->
    [hidden] piilotettu
   *[shown] näkyvissä
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] avattu malli
    [map-crs] karttakoordinaattien mukaan ({ $crs })
    [map] karttakoordinaattien mukaan
    [site] tontin leveys- ja pituusasteen mukaan (likimääräinen)
    [project] jaettujen projektikoordinaattien mukaan
   *[origin] origossa (ei kohdistettu)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Mallit käyttävät eri koordinaattijärjestelmiä: { $first } ja { $this }.
    [only-first] Avattu malli on georeferoitu, tämä ei.
    [only-this] Tämä malli on georeferoitu, avattu ei.
   *[broken] Tämän mallin karttamuunnosta ei voi käyttää (sen mittakaava on nolla).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Siirretty { $distance }, käännetty { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Luetaan 1 malli…
   *[other] Luetaan { $count } mallia…
}
# machine-drafted, needs native review
fed-ask-origin = Tiedostoa { $file } ei voi kohdistaa. Sijoitetaanko se origoon?
# machine-drafted, needs native review
fed-no-files = Kansiossa { $folder } ei ole IFC-tiedostoja.
# machine-drafted, needs native review
fed-not-added = Tiedostoa { $file } ei lisätty: { $reason }
# machine-drafted, needs native review
fed-added = { $file } lisätty. Malleja kohtauksessa: { $count }.
# machine-drafted, needs native review
fed-remove-title = Poistetaanko { $file } kohtauksesta?
# machine-drafted, needs native review
fed-removed = { $file } poistettu kohtauksesta.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] piilotettu
   *[shown] näkyvissä
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Väritys malleittain päällä
   *[off] Väritys malleittain pois
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI ja tallennetut näkymät
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 annotaatio
   *[other] { $count } annotaatiota
}
# machine-drafted, needs native review
pmi-view = Tallennettu näkymä
# machine-drafted, needs native review
pmi-view-all = näyttää kaikki annotaatiot
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] näyttää 1 annotaation
   *[other] näyttää { $count } annotaatiota
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Näytetään näkymä ”{ $name }”. A näyttää kaikki.
# machine-drafted, needs native review
pmi-has-faces = pinnat korostettu
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Mallissa ei ole PMI-tietoja.
    [hints] ↑ ↓ valitse · Enter siirry · A kaikki · D kuten piirretty · Shift+A piilota
    [hidden] Annotaatiot piilotettu
    [shown] Annotaatiot näkyvissä
    [as-drawn] Annotaatiot niin kuin tiedosto ne piirtää
    [as-labels] Annotaatiot nimiöinä
   *[all-shown] Kaikki annotaatiot näkyvissä
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Mitta
    [tolerance] Geometrinen toleranssi
    [datum] Peruste
    [target] Osaperuste
   *[graphic] Piirretty annotaatio
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Etäisyys
    [angular] Kulma
    [diameter] Halkaisija
    [radius] Säde
    [length] Pituus
    [thickness] Paksuus
   *[other] Mitta
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Suoruus
    [flatness] Tasomaisuus
    [circularity] Ympyrämäisyys
    [cylindricity] Lieriömäisyys
    [line-profile] Viivan muoto
    [surface-profile] Pinnan muoto
    [angularity] Kaltevuus
    [perpendicularity] Kohtisuoruus
    [parallelism] Yhdensuuntaisuus
    [position] Paikka
    [concentricity] Samankeskisyys
    [coaxiality] Samanakselisuus
    [symmetry] Symmetrisyys
    [circular-runout] Heitto
   *[total-runout] Kokonaisheitto
}
