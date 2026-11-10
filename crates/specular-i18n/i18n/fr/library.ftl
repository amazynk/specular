# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Bibliothèque

## Top bar
library-title = Bibliothèque
library-locations-button = Emplacements

## Locations sidebar
library-locations = Emplacements
library-all = Tous les emplacements
library-add = Ajouter un emplacement…
library-remove = Retirer { $location } de la bibliothèque
library-offline = Hors ligne
library-location-pending = { $count } à indexer
library-a11y-location = { $location }, { $count ->
    [one] { $count } image
   *[other] { $count } images
}
library-a11y-location-offline = { $location }, Hors ligne, { $count ->
    [one] { $count } image
   *[other] { $count } images
}

## Indexing progress
library-walking = Images trouvées: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Images trouvées: { $count }
library-reading = Indexation… { $done } sur { $total }
library-held = L’indexation se met en pause pendant que vous naviguez
library-up-to-date = À jour
# machine-drafted, needs native review
library-stopped = Indexation arrêtée
library-stopped-pending = Indexation arrêtée · { $count } à indexer
library-resume = Reprendre
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indexation arrêtée : { $reason }
library-reason-stalled = rien n’a bougé depuis { $minutes ->
    [one] une minute
   *[other] { $minutes } minutes
}. Un fichier ou un disque ne répond peut-être pas.
library-retry = Réessayer
library-retry-description = Relance l’indexation. Ce qui a déjà été indexé reste consultable.
library-save-diagnostics = Enregistrer un paquet de diagnostic…
library-stop-description = Arrête l’indexation. Ce qui a déjà été indexé reste consultable ; Reprendre ou Réanalyser (F5) la poursuit.
library-resume-description = Parcourt à nouveau tous les emplacements et indexe ce qui reste.
library-opening = Ouverture de la bibliothèque…
library-searching = Recherche…

## Empty and error states
library-empty = Aucun emplacement. Ajoutez un dossier : ses images et celles de ses sous-dossiers s’affichent ici dans une seule grille.
library-empty-offline = Ces emplacements sont hors ligne. Leurs images réapparaîtront à leur retour.
library-indexing = Recherche d’images…
# machine-drafted, needs native review
library-remove-confirm = Retirer « { $location } » de la bibliothèque ?
library-remove-confirm-detail = Vos photos restent sur le disque, avec leurs notes, étiquettes et mots-clés. La bibliothèque oublie ce qu’elle y a indexé ; si vous ajoutez de nouveau le dossier, il est réindexé.
library-remove-confirm-ok = Retirer
library-indexing-in = Recherche d’images dans { $location }… { $count } trouvées jusqu’ici
# machine-drafted, needs native review
library-empty-stopped = L’indexation a été arrêtée avant que ces images ne soient lues. Reprendre la poursuit à partir de là.
library-unavailable = Impossible d’ouvrir la bibliothèque : { $reason }
library-read-only = Cette bibliothèque a été écrite par une version plus récente de Specular : elle peut être consultée mais pas mise à jour.

## Screen readers
library-a11y-list = Bibliothèque, { $count ->
    [one] { $count } image
   *[other] { $count } images
}
library-a11y-added = { $location } ajouté à la bibliothèque
library-a11y-removed = { $location } retiré de la bibliothèque

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Ajouter tout le contenu de « { $folder } » ?
library-home-confirm-detail = Ce dossier contient des fichiers de programmes, des téléchargements et des caches, pas seulement des photos. L’indexation est plus longue, et la bibliothèque se remplit d’icônes et d’autres images.
library-home-confirm-pictures = Votre dossier Images ne contient que vos images.
library-home-confirm-use-pictures = Ajouter Images
library-home-confirm-keep = Ajouter le dossier personnel

## Folder watches
# machine-drafted, needs native review
library-unwatched = Certains dossiers ne sont pas surveillés
library-unwatched-detail = { $minutes ->
    [one] Les changements apparaissent en moins d’une minute
   *[other] Les changements apparaissent en { $minutes } min environ
}
library-unwatched-description = Specular n’a plus de surveillances de dossier disponibles. Ouvre l’aide pour savoir comment relever la limite.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Bibliothèque
settings-library-skip-folders = Ignorer les dossiers nommés
settings-library-skip-folders-help = La bibliothèque ne cherche pas dans les dossiers portant ces noms, séparés par des virgules. Les dossiers cachés sont toujours ignorés.
settings-library-skip-marked = Ignorer les dossiers contenant
settings-library-skip-marked-help = Ni dans les dossiers contenant l’un de ceux-ci : .git pour les dépôts de code, pyvenv.cfg pour les environnements Python, CACHEDIR.TAG pour les caches. Le dossier d’un emplacement n’est jamais ignoré.
settings-library-show-timeline = Frise chronologique
settings-library-show-timeline-help = Montre combien de photos compte chaque année, mois ou jour, au-dessus de la grille de la bibliothèque.
