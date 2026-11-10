# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = Kartat
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Tasot
cheatsheet-map-coordinates = Koordinaatit
cheatsheet-map-measure = Mittaus

## Palette entries and their one-line descriptions.
palette-map-layers = Näytä tai piilota tasopaneeli
palette-map-layers-help = Kartan tasot: järjestys, peittävyys, näkyvyys ja tyyli.
palette-map-add-layer = Lisää taso tiedostosta…
palette-map-add-layer-help = Piirrä toinen karttatiedosto tämän päälle, kuten GPX-reitti shapefilen päälle.
palette-map-zoom-to-layer = Zoomaa tasoon
palette-map-zoom-to-layer-help = Näytä koko tasopaneelissa valittu taso.
palette-map-attribute-table = Näytä tai piilota ominaisuustaulu
palette-map-attribute-table-help = Valitun tason kohteet taulukkona: lajittele, suodata, hae ja valitse.
palette-map-coordinate-format = Seuraava koordinaattimuoto
palette-map-coordinate-format-help = Desimaaliasteet, asteet minuutit sekunnit, UTM tai MGRS.
palette-map-units = Metriset tai brittiläiset yksiköt
palette-map-units-help = Pituudet ja pinta-alat metreinä tai jalkoina ja maileina.
palette-map-copy-coordinate = Kopioi koordinaatti
palette-map-copy-coordinate-help = Kopioi osoittimen kohdan tai kartan keskikohdan sijainti.
palette-map-go-to-coordinate = Siirry koordinaattiin…
palette-map-go-to-coordinate-help = Kirjoita sijainti missä tahansa muodossa keskittääksesi kartan siihen.
palette-map-measure-distance = Mittaa etäisyys
palette-map-measure-distance-help = Napsauta pisteitä kartalla: pituus maan pinnalla ja suuntakulma.
palette-map-measure-area = Mittaa pinta-ala
palette-map-measure-area-help = Napsauta alueen kulmia: sen koko ja ympärysmitta maan pinnalla.
palette-map-elevation-profile = Korkeusprofiili
palette-map-elevation-profile-help = Piirrä viiva korkeusmallin yli nähdäksesi sen korkeudet kaaviona.
palette-map-overview = Näytä tai piilota yleiskartta
palette-map-overview-help = Pieni kartta koko alueesta, näytöllä oleva osa merkittynä.

## Why a map command is unavailable.
disabled-not-map = Vain kartoille, kun karttatyökalut ovat käytössä
disabled-no-lonlat = Kartalla ei ole tunnettua koordinaattijärjestelmää
disabled-no-features = Kartalla ei ole kohteita
disabled-no-heights = Kartalla ei ole korkeuksia

## Settings › Maps.
settings-section-maps = Kartat
settings-maps-coordinate-format = Koordinaattimuoto
settings-maps-coordinate-format-help = Miten sijainnit näytetään ja kopioidaan. Siirry koordinaattiin lukee kaikki muodot.
settings-maps-coordinate-format-dd = Desimaaliasteet
settings-maps-coordinate-format-dms = Asteet, minuutit, sekunnit
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Yksiköt
settings-maps-units-help = Pituudet ja pinta-alat kartoilla: mittakaavapalkki ja mittaukset. Automaattinen noudattaa asetusta Metatiedot › Yksiköt.
settings-maps-units-auto = Automaattinen
settings-maps-units-metric = Metrinen
settings-maps-units-imperial = Brittiläinen

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Leveys, pituus { $position }
}
map-height = Korkeus { $height }
map-height-label = Korkeus
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Mittakaavapalkki: { $length }
map-north-label = Pohjoisnuoli: pohjoinen on { $degrees }° myötäpäivään ylhäältä
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] asteet, minuutit, sekunnit
    [utm] UTM
    [mgrs] MGRS
   *[dd] desimaaliasteet
}
map-format-toast = Koordinaatit: { $format }
map-units-metric = Yksiköt: metrit ja kilometrit
map-units-imperial = Yksiköt: jalat ja mailit
toast-copied-coordinate = Koordinaatti kopioitu

## Go to coordinate.
map-goto-hint = Desimaaliasteet, DMS, UTM tai MGRS. Enter siirtyy sinne.
map-goto-empty = Kirjoita sijainti.
map-goto-unrecognised = Ei sijainti tunnetussa muodossa.
map-goto-range = Leveys- tai pituusaste on alueen ulkopuolella.
map-goto-no-way = Tämä kartta ei voi sijoittaa sijaintia.
map-goto-outside = Sijainti on kartan ulkopuolella.
map-goto-toast = Keskitetty kohtaan { $position }

## The layer panel.
map-layers-title = Tasot: { $shown }/{ $total } näkyvissä
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } kohde
   *[other] { $features } kohdetta
}, peittävyys { PERCENT($opacity) }
map-kind-area = Alueet
map-kind-line = Viivat
map-kind-point = Pisteet
map-kind-empty = Ei kohteita
map-layer-visible = Näytä tai piilota taso
map-layer-opacity-down = Vähemmän peittävä
map-layer-opacity-up = Peittävämpi
map-layer-colour = Seuraava väri
map-layer-width = Seuraava viivan leveys
map-layer-point = Seuraava pisteen koko
map-layer-up = Siirrä ylös
map-layer-down = Siirrä alas
map-layer-remove = Poista
map-layer-keep = Säilytä
map-layer-remove-ask = Poistetaanko { $name } kartalta? Tiedosto jää levylle.
map-layer-adding = Lisätään tasoa…
map-removed = { $name } poistettu kartalta
map-added = { $layers ->
    [one] { $name } lisätty
   *[other] { $name } lisätty: { $layers } tasoa
}
map-add-failed = Kohdetta { $name } ei voi lisätä: { $reason }
map-add-filter = Karttatiedostot
map-add-not-map = se ei ole kartta.
map-add-not-vector = siinä ei ole kohteita.
map-add-no-crs = siinä ei ole projektiotiedostoa (.prj).

## Identify.
map-identify-more = ja { $count } muuta ominaisuustaulussa
map-cell = Solu { $x }, { $y }
map-band = Kanava { $n }
map-no-data = Ei dataa

## The attribute table.
map-table-title = { $layer }: { $shown }/{ $total } kohdetta, { $selected } valittu
map-table-search = Hae, tai kenttä = arvo
map-table-selected-only = Vain valitut
map-table-empty = Mikään kohde ei täsmää.
map-table-many = monta
map-table-stats-number = { $field }: { $count ->
        [one] { $count } arvo
       *[other] { $count } arvoa
    }, { $empty } tyhjää · min { $min } · max { $max } · keskiarvo { $mean } · summa { $sum } · keskihajonta { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } arvo
       *[other] { $count } arvoa
    }, { $empty } tyhjää · { $distinct } erilaista
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } arvo
       *[other] { $count } arvoa
    }, { $empty } tyhjää · { $distinct } erilaista · yleisin { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Napsauta pisteitä reitin varrelta.
map-measure-hint-area = Napsauta alueen kulmia.
map-measure-hint-profile = Napsauta pisteitä viivan varrelta.
map-measure-keys = Enter lopettaa. Askelpalautin poistaa viimeisen pisteen. Esc sulkee.
map-measure-distance = Etäisyys { $length }
map-measure-bearing = Suuntakulma { $bearing }
map-measure-segment = Viimeinen osa { $length }, suuntakulma { $bearing }
map-measure-area = Pinta-ala { $area }
map-measure-perimeter = Ympärysmitta { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Pituus { $length } · alin { $min } · ylin { $max } · nousu { $climb } · lasku { $descent }
map-profile-no-heights = Pituus { $length }. Viivan varrella ei ole korkeuksia.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: ei korkeutta
