# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Français.

## The panel

# The panel's title.
net-title = Téléchargements
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Afficher les téléchargements
# The panel when nothing is downloading any more.
net-empty = Aucun téléchargement en cours.
# Cancels one download.
net-stop = Arrêter

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Données de { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Tuiles d’image de { $host }
net-what-point-cloud = Nuage de points de { $host }
net-what-map-tiles = Tuiles de carte de { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Service de cartes sur { $host }
net-what-dicom = Données DICOM de { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = Série DICOM { $series } de { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Envoi d’études à { $host }
net-what-omero = Données OMERO de { $host }
net-what-zarr-chunks = Blocs Zarr de { $host }
net-what-zarr-metadata = Métadonnées Zarr de { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Impossible d’ouvrir { $address } : { $reason }
# A Local AI model; $name is its name.
net-what-model = Modèle { $name }
net-what-update = Recherche de mises à jour

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } sur { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } sur { $total } { $count ->
    [one] requête
    [many] requêtes
   *[other] requêtes
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } sur { $total } { $count ->
    [one] image
    [many] images
   *[other] images
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } en échec
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] avertissement
    [many] avertissements
   *[other] avertissements
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Démarrage…
# Running, no time left known (many requests, or an unknown size).
net-running = Téléchargement
net-done = Terminé
# Done, but $count of its requests failed.
net-done-failed = Terminé, { $count } en échec
# Cancelled with Stop.
net-stopped = Arrêté
# $reason is one of the reasons below.
net-failed = Échec : { $reason }
net-reason-offline = pas de connexion
net-reason-timeout = le serveur n’a pas répondu à temps
# $code is the HTTP status number.
net-reason-status = le serveur a répondu { $code }
net-reason-auth = connexion refusée
net-reason-other = le téléchargement s’est interrompu

## The indicator

# More downloads than shown in place.
net-more = { $count } de plus
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Téléchargements : { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Téléchargement de { $what }
net-finished = Téléchargé : { $what }
net-failed-announce = Échec du téléchargement : { $what }
net-stopped-announce = Téléchargement arrêté : { $what }

## Pas de réseau (Paramètres › Confidentialité)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Le réseau est désactivé dans Paramètres › Confidentialité.
# The panel under the switch while the network is off.
privacy-off-title = Le réseau est désactivé.
privacy-off-what = Les recherches de mises à jour, les téléchargements de modèles, les serveurs DICOM, OMERO et Zarr, les images dans le cloud et les cartes sont désactivés.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Défini par votre administrateur dans { $file }. Cet interrupteur ne peut pas réactiver le réseau.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Défini pour cette exécution par { $what }. Cet interrupteur ne peut pas réactiver le réseau.

## Open address… (File)

# machine-drafted, needs native review
address-title = Ouvrir une adresse
# machine-drafted, needs native review
address-intro = Un dépôt Zarr, un GeoTIFF dans le cloud (.tif), un nuage de points COPC (.copc.laz), un item ou catalogue STAC (.json) ou un serveur DICOMweb.
address-label = Adresse (http, https ou s3)
address-recent = Récentes
address-open = Ouvrir
address-network-off = Impossible d’ouvrir des adresses : le réseau est désactivé dans Paramètres › Confidentialité.
address-problem-empty = Saisissez ou collez une adresse.
address-problem-scheme = L’adresse doit commencer par http://, https:// ou s3://.
address-problem-host = L’adresse ne contient aucun nom de serveur, ou contient des espaces.
# machine-drafted, needs native review
address-problem-kind = { -app-name } n’ouvre pas cette adresse. Il ouvre un dépôt .zarr, un .tif, un .copc.laz, un .json STAC ou un serveur DICOMweb.
