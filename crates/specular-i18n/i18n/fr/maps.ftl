# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = Cartes
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Couches
cheatsheet-map-coordinates = Coordonnées
cheatsheet-map-measure = Mesure

## Palette entries and their one-line descriptions.
palette-map-layers = Afficher ou masquer le panneau des couches
palette-map-layers-help = Les couches de la carte : ordre, opacité, visibilité et style.
palette-map-add-layer = Ajouter une couche depuis un fichier…
palette-map-add-layer-help = Superposer un autre fichier de carte, par exemple une trace GPX sur un shapefile.
palette-map-zoom-to-layer = Zoomer sur la couche
palette-map-zoom-to-layer-help = Afficher en entier la couche choisie dans le panneau des couches.
palette-map-attribute-table = Afficher ou masquer la table attributaire
palette-map-attribute-table-help = Les entités de la couche choisie sous forme de table : trier, filtrer, rechercher et sélectionner.
palette-map-coordinate-format = Format de coordonnées suivant
palette-map-coordinate-format-help = Degrés décimaux, degrés minutes secondes, UTM ou MGRS.
palette-map-units = Unités métriques ou impériales
palette-map-units-help = Longueurs et surfaces en mètres, ou en pieds et miles.
palette-map-copy-coordinate = Copier la coordonnée
palette-map-copy-coordinate-help = Copier la position sous le pointeur, ou celle du centre de la carte.
palette-map-go-to-coordinate = Aller à une coordonnée…
palette-map-go-to-coordinate-help = Saisissez une position dans n’importe quel format pour y centrer la carte.
palette-map-measure-distance = Mesurer une distance
palette-map-measure-distance-help = Cliquez des points sur la carte : la longueur à la surface de la Terre et l’azimut.
palette-map-measure-area = Mesurer une surface
palette-map-measure-area-help = Cliquez les coins d’une zone : sa superficie et son périmètre à la surface de la Terre.
palette-map-elevation-profile = Profil altimétrique
palette-map-elevation-profile-help = Tracez une ligne sur un modèle d’élévation pour en représenter les altitudes.
palette-map-overview = Afficher ou masquer la carte d’ensemble
palette-map-overview-help = Une petite carte de toute l’étendue, avec la partie à l’écran marquée.

## Why a map command is unavailable.
disabled-not-map = Uniquement pour les cartes, avec les outils cartographiques activés
disabled-no-lonlat = La carte n’a pas de système de coordonnées connu
disabled-no-features = La carte n’a pas d’entités
disabled-no-heights = La carte n’a pas d’altitudes

## Settings › Maps.
settings-section-maps = Cartes
settings-maps-coordinate-format = Format des coordonnées
settings-maps-coordinate-format-help = Comment les positions sont affichées et copiées. Aller à une coordonnée lit tous les formats.
settings-maps-coordinate-format-dd = Degrés décimaux
settings-maps-coordinate-format-dms = Degrés, minutes, secondes
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Unités
settings-maps-units-help = Longueurs et surfaces sur les cartes : barre d’échelle et mesures. Automatique suit Métadonnées › Unités.
settings-maps-units-auto = Automatique
settings-maps-units-metric = Métriques
settings-maps-units-imperial = Impériales

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Lat, lon { $position }
}
map-height = Altitude { $height }
map-height-label = Altitude
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Barre d’échelle : { $length }
map-north-label = Flèche du nord : le nord est à { $degrees }° dans le sens horaire depuis le haut
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] degrés, minutes, secondes
    [utm] UTM
    [mgrs] MGRS
   *[dd] degrés décimaux
}
map-format-toast = Coordonnées en { $format }
map-units-metric = Unités : mètres et kilomètres
map-units-imperial = Unités : pieds et miles
toast-copied-coordinate = Coordonnée copiée

## Go to coordinate.
map-goto-hint = Degrés décimaux, DMS, UTM ou MGRS. Entrée pour y aller.
map-goto-empty = Saisissez une position.
map-goto-unrecognised = Pas une position dans un format connu.
map-goto-range = Latitude ou longitude hors limites.
map-goto-no-way = Cette carte ne peut pas placer une position.
map-goto-outside = Cette position est en dehors de la carte.
map-goto-toast = Centré sur { $position }

## The layer panel.
map-layers-title = Couches : { $shown } sur { $total } affichées
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name } : { $kind }, { $features ->
    [one] { $features } entité
    [many] { $features } d’entités
   *[other] { $features } entités
}, opacité { PERCENT($opacity) }
map-kind-area = Surfaces
map-kind-line = Lignes
map-kind-point = Points
map-kind-empty = Aucune entité
map-layer-visible = Afficher ou masquer la couche
map-layer-opacity-down = Moins opaque
map-layer-opacity-up = Plus opaque
map-layer-colour = Couleur suivante
map-layer-width = Épaisseur de ligne suivante
map-layer-point = Taille de point suivante
map-layer-up = Monter
map-layer-down = Descendre
map-layer-remove = Retirer
map-layer-keep = Garder
map-layer-remove-ask = Retirer { $name } de la carte ? Le fichier reste sur le disque.
map-layer-adding = Ajout d’une couche…
map-removed = { $name } retiré de la carte
map-added = { $layers ->
    [one] { $name } ajouté
    [many] { $name } ajouté : { $layers } de couches
   *[other] { $name } ajouté : { $layers } couches
}
map-add-failed = Impossible d’ajouter { $name } : { $reason }
map-add-filter = Fichiers de carte
map-add-not-map = ce n’est pas une carte.
map-add-not-vector = il n’a pas d’entités.
map-add-no-crs = il n’a pas de fichier de projection (.prj).

## Identify.
map-identify-more = et { $count } de plus dans la table attributaire
map-cell = Cellule { $x }, { $y }
map-band = Bande { $n }
map-no-data = Pas de données

## The attribute table.
map-table-title = { $layer } : { $shown } entités sur { $total }, { $selected } sélectionnées
map-table-search = Rechercher, ou champ = valeur
map-table-selected-only = Sélection uniquement
map-table-empty = Aucune entité ne correspond.
map-table-many = beaucoup
map-table-stats-number = { $field } : { $count ->
        [one] { $count } valeur
       *[other] { $count } valeurs
    }, { $empty } vides · min { $min } · max { $max } · moyenne { $mean } · somme { $sum } · écart type { $sd }
map-table-stats-text = { $field } : { $count ->
        [one] { $count } valeur
       *[other] { $count } valeurs
    }, { $empty } vides · { $distinct } distinctes
map-table-stats-text-top = { $field } : { $count ->
        [one] { $count } valeur
       *[other] { $count } valeurs
    }, { $empty } vides · { $distinct } distinctes · la plus fréquente { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Cliquez des points le long du trajet.
map-measure-hint-area = Cliquez les coins de la zone.
map-measure-hint-profile = Cliquez des points le long de la ligne.
map-measure-keys = Entrée termine. Retour arrière retire le dernier point. Échap ferme.
map-measure-distance = Distance { $length }
map-measure-bearing = Azimut { $bearing }
map-measure-segment = Dernier tronçon { $length }, azimut { $bearing }
map-measure-area = Surface { $area }
map-measure-perimeter = Périmètre { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Longueur { $length } · plus bas { $min } · plus haut { $max } · montée { $climb } · descente { $descent }
map-profile-no-heights = Longueur { $length }. Aucune altitude le long de la ligne.
map-profile-at = { $distance } : { $height }
map-profile-at-none = { $distance } : pas d’altitude
