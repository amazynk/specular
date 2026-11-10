# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Relief
cheatsheet-map-time = Temps et météo
cheatsheet-map-online = Services en ligne
cheatsheet-map-photos = Photos et GPS
cheatsheet-map-output = Sortie

## Palette entries and their one-line descriptions.
palette-map-slope = Couleurs de pente
palette-map-slope-help = Colorer un modèle d’élévation selon sa pente.
palette-map-aspect = Couleurs d’exposition
palette-map-aspect-help = Colorer un modèle d’élévation selon la direction vers laquelle ses pentes sont tournées.
palette-map-contours = Afficher ou masquer les courbes de niveau
palette-map-contours-help = Courbes de niveau sur un modèle d’élévation, étiquetées toutes les quelques courbes.
palette-map-terrain-3d = Relief en 3D
palette-map-terrain-3d-help = Afficher le modèle d’élévation comme une surface 3D, avec ses couleurs drapées dessus.
palette-map-time = Afficher ou masquer le curseur temporel
palette-map-time-help = Parcourir les dates d’une grille ou les heures d’une trace.
palette-map-play = Lire ou mettre en pause le temps
palette-map-play-help = Faire avancer le curseur temporel tout seul, pas à pas.
palette-map-wind = Afficher ou masquer les flèches de vent
palette-map-wind-help = Flèches de vitesse et de direction du vent d’après les composantes u et v de la grille.
palette-map-weather-colours = Palette météo suivante
palette-map-weather-colours-help = Palettes de couleurs pour la température, la pluie, le vent et la pression.
palette-map-services = Services de cartes en ligne…
palette-map-services-help = Ajouter des couches depuis des serveurs WMS, WMTS, WFS, OGC API ou STAC.
palette-map-photos-on-track = Afficher ou masquer les photos sur la trace
palette-map-photos-on-track-help = Placer les photos du dossier le long de la trace GPS selon leur heure de prise de vue.
palette-map-geotag = Géolocaliser les photos d’après la trace…
palette-map-geotag-help = Écrire les positions de la trace dans des fichiers annexes XMP des photos du dossier.
palette-map-export = Exporter la carte…
palette-map-export-help = Enregistrer la carte en PNG ou PDF à une échelle, avec légende, barre d’échelle et flèche du nord.
palette-map-print = Imprimer la carte à l’échelle…
palette-map-print-help = Imprimer la carte à une échelle choisie, par exemple 1:{ NUMBER(25000) }.
palette-map-copy-extent = Copier l’emprise
palette-map-copy-extent-help = Copier la zone affichée comme rectangle englobant, ou en WKT.

## Why a map command is unavailable.
disabled-no-time = La carte n’a pas de dates
disabled-no-wind = La carte n’a pas de composantes de vent (u et v)
disabled-no-track = Aucune trace GPS avec des heures
disabled-not-grid = La carte n’est pas une grille de valeurs

## Settings › Maps and Metadata.
settings-maps-services = Services de cartes en ligne
settings-maps-services-help = Ajouter des couches depuis des serveurs WMS, WMTS, WFS, OGC API et STAC. Désactivé : rien n’est téléchargé.
settings-maps-services-allowed = Serveurs de cartes autorisés
settings-maps-services-allowed-help = Serveurs acceptés après l’explication de ce qu’ils voient.
settings-metadata-basemap-service = Une couche d’un service de cartes
settings-metadata-basemap-service-help = La couche choisie dans le panneau des services en ligne de la carte.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NE
    [e] E
    [se] SE
    [s] S
    [sw] SO
    [w] O
    [nw] NO
   *[n] N
}
map-slope-aspect = Pente { $slope }° · orientée { $direction } ({ $aspect }°)
map-slope-flat = Pente { $slope }° · plat
map-slope-on = Couleurs de pente
map-aspect-on = Couleurs d’exposition
map-relief-off = Couleurs d’altitude
map-contours-on = Courbes de niveau affichées
map-contours-off = Courbes de niveau masquées
map-legend-slope = Pente
map-legend-aspect = Exposition : l’orientation des pentes
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° et plus
map-terrain-3d-on = Relief en 3D. Échap revient à la carte.
map-terrain-3d-on-exaggerated = Relief en 3D, altitudes × { $times }. Échap revient à la carte.
map-terrain-exaggeration = Altitudes exagérées { $times } fois.
map-terrain-drape = Couleurs de la carte

## Time, weather and wind.
map-time-grid = { $when } · { $step } sur { $steps }
map-time-track = { $when } · après { $elapsed }
map-time-speed = × { $times }
map-time-play = Lire
map-time-pause = Pause
map-weather-map = Couleurs : { $name }
map-weather-file = Couleurs du fichier
map-wind-on = Flèches de vent affichées
map-wind-off = Flèches de vent masquées
map-wind-at = Vent { $speed } { $unit } du { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Rectangle englobant copié. Copiez à nouveau pour le WKT.
map-extent-copied-wkt = Emprise copiée en WKT

## Online map services.
map-services-title = Services de cartes en ligne
map-service-off = Les services de cartes en ligne sont désactivés.
map-service-off-why = Activez-les pour ajouter des couches depuis des serveurs sur Internet.
map-service-turn-on = Activer
map-service-close = Fermer
map-service-back = Précédent
map-service-cancel = Annuler
map-service-connect = Se connecter
map-service-allow = Autoriser
map-service-remove = Retirer
map-service-download = Télécharger
map-service-list-hint = Entrée ouvre un serveur. Suppr le retire.
map-service-add = Ajouter un serveur…
map-service-add-sub = WMS, WMTS, WFS, OGC API ou STAC
map-service-add-hint = L’adresse du serveur. Entrée se connecte.
map-service-bad-address = Pas une adresse web (http ou https, sans nom d’utilisateur).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Service de cartes en ligne
map-service-consent-server = Specular va contacter { $host }.
map-service-consent-sees = Ce serveur voit votre adresse Internet et la zone de carte demandée.
map-service-consent-private = Vos fichiers et leurs noms ne sont pas envoyés.
map-service-asking = Interrogation de { $host }…
map-service-downloading = Téléchargement depuis { $host }…
map-service-layers-hint = Couches { $kind }. Entrée en utilise une.
map-service-no-layers = Le serveur ne liste aucune couche.
map-service-offer-tiles = Tuiles : devient le fond de la carte des photos
map-service-offer-features = Entités : ajoutées à cette carte pour la zone affichée
map-service-offer-search = Images : rechercher dans la zone affichée
map-service-refused-mercator = Non proposé en Web Mercator (EPSG:3857), nécessaire au fond de carte.
map-service-refused-format = Non proposé en images PNG ou JPEG.
map-service-refused-geojson = Non proposé en GeoJSON, le format lu ici.
map-service-items-hint = { $count ->
        [one] { $count } image trouvée. Entrée la télécharge.
       *[other] { $count } images trouvées. Entrée en télécharge une.
    }
map-service-no-items = Aucune image trouvée ici.
map-service-item-cloud = { $when } · { PERCENT($cloud) } de nuages
map-service-item-no-cog = Pas de Cloud-Optimized GeoTIFF
map-service-download-ask = Télécharger { $size } depuis { $host } ?
map-service-download-ask-unknown = Télécharger cette image depuis { $host } ? Sa taille est inconnue.
map-service-download-cache = Elle est gardée dans le cache des cartes et ouverte.
map-service-size-mb = { $size } Mo
map-service-size-gb = { $size } Go
map-service-too-big = L’image fait { $size }, trop pour être téléchargée.
map-service-opened = Ouverture de l’image téléchargée
map-service-basemap = { $layer } est maintenant le fond de la carte des photos. La carte des photos demande avant son premier téléchargement.
map-service-status = Le serveur a répondu { $status }.
map-service-unknown = Pas un service de cartes que cette version sait lire.
map-service-exception = Le serveur a dit : { $message }
map-service-malformed = La réponse du serveur est invalide : { $reason }
map-service-remove-ask = Retirer « { $server } » des serveurs enregistrés ?
map-service-remove-detail = Rien n’est perdu sur le disque. Les couches déjà sur la carte restent jusqu’à sa fermeture.
map-service-removed = { $server } retiré
map-service-saved = Serveurs de cartes enregistrés
map-service-saved-none = Aucun pour l’instant. Ajoutez-en un dans le panneau des services en ligne de la carte.
map-service-keywords = WMS WMTS WFS OGC API STAC serveurs services cartes

## Photos and geotagging.
map-geotag-title = Géolocaliser les photos d’après la trace
map-photos-on = Photos sur la trace
map-photos-off = Photos sur la trace masquées
map-geotag-offset = Horloge de l’appareil : { $offset } par rapport à l’heure GPS. ← → la changent d’une minute, avec Maj d’une heure, avec Alt d’une seconde.
map-geotag-reading = Lecture des photos…
map-geotag-summary = { $count } photos sur { $total } reçoivent une position.
map-geotag-sidecars = Les positions vont dans des fichiers annexes XMP à côté des photos. Les fichiers photo ne sont pas modifiés.
map-geotag-writing = Écriture…
map-geotag-write = Écrire les fichiers annexes…
map-geotag-guess = Deviner le décalage
map-geotag-will-write = Reçoit { $position }
map-geotag-has-position = A déjà une position : laissée telle quelle
map-geotag-no-time = Pas d’heure de prise de vue : laissée telle quelle
map-geotag-outside = Prise { $offset } hors de la trace : laissée telle quelle
map-geotag-gap = Dans un trou de la trace (à { $seconds } s d’un point) : laissée telle quelle
map-geotag-nothing = Aucune photo ne reçoit de position avec ce décalage.
map-geotag-confirm = Écrire les positions dans { $count ->
    [one] { $count } fichier annexe XMP
   *[other] { $count } fichiers annexes XMP
} ? Les photos elles-mêmes ne sont pas modifiées.
map-geotag-confirm-ok = Écrire les fichiers annexes
map-geotag-done = { $count ->
    [one] { $count } fichier annexe écrit
   *[other] { $count } fichiers annexes écrits
}
map-geotag-done-failed = { $count ->
        [one] { $count } fichier annexe écrit
       *[other] { $count } fichiers annexes écrits
    } ; { $failed } en échec : { $reason }
map-geotag-not-sidecar = Pas un nom de fichier annexe.

## Export and printing.
map-export-title = Exporter la carte
map-print-title = Imprimer la carte à l’échelle
map-export-keys = ↑ ↓ choisir, ← → changer, Entrée lance.
map-export-format = Format
map-export-scale = Échelle
map-export-scale-value = 1:{ $n }
map-export-resolution = Résolution
map-export-dpi = { $dpi } ppp
map-export-paper = Papier
map-export-size = { $width } × { $height } px · { $across } × { $down } km sur le terrain
map-export-save = Enregistrer…
map-export-print = Imprimer…
map-export-too-large = Trop grand à cette échelle : { $width } × { $height } px. Choisissez une échelle ou une résolution plus petite.
map-export-no-scale = Cette carte n’a pas d’échelle sur le terrain.
map-export-no-gpu = L’export nécessite le processeur graphique.
map-export-subtitle = Échelle { $scale } · { $crs }
map-export-data = Données : { $sources }
map-export-saved = { $name } enregistré
map-export-printing = Impression…
map-export-printed = Envoyé à l’imprimante
map-export-print-failed = Non imprimé : { $reason }
