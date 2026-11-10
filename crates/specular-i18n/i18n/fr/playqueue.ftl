# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = File d’attente, répétition et aléatoire
# machine-drafted, needs native review
palette-next-track = Élément suivant
# machine-drafted, needs native review
palette-next-track-help = Lire l’élément suivant de la file d’attente, sinon le fichier suivant du même type dans le dossier.
# machine-drafted, needs native review
palette-prev-track = Élément précédent
# machine-drafted, needs native review
palette-prev-track-help = Lire l’élément précédent. Après 3 secondes, reprendre celui-ci au début.
# machine-drafted, needs native review
palette-cycle-repeat = Répéter
# machine-drafted, needs native review
palette-cycle-repeat-help = Répétition désactivée, tout ou un seul.
# machine-drafted, needs native review
palette-shuffle = Lecture aléatoire
# machine-drafted, needs native review
palette-shuffle-help = Lire dans un ordre aléatoire, chaque élément une fois par tour.
# machine-drafted, needs native review
palette-play-queue = File d’attente
# machine-drafted, needs native review
palette-play-queue-help = Voir ce qui va être lu : réordonner, retirer, enregistrer.
# machine-drafted, needs native review
palette-queue-play-next = Lire ensuite
# machine-drafted, needs native review
palette-queue-play-next-help = Lire le fichier (dans la galerie, la sélection) après l’élément en cours.
# machine-drafted, needs native review
palette-queue-add = Ajouter à la file d’attente
# machine-drafted, needs native review
palette-queue-add-help = Ajouter le fichier (dans la galerie, la sélection) à la fin de la file d’attente.
# machine-drafted, needs native review
palette-save-queue = Enregistrer la file comme playlist…
# machine-drafted, needs native review
palette-save-queue-help = Enregistrer la file d’attente comme playlist M3U8 ou XSPF.
# machine-drafted, needs native review
transport-prev-track = Élément précédent
# machine-drafted, needs native review
transport-next-track = Élément suivant
# machine-drafted, needs native review
queue-title = File d’attente
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 élément
   *[other] { $count } éléments
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 élément
   *[other] { $count } éléments
} · { $time }
# machine-drafted, needs native review
queue-empty = La file d’attente est vide.
    Ajoutez des fichiers son et des vidéos avec Lire ensuite ou Ajouter à la file d’attente, ou ouvrez une playlist.
# machine-drafted, needs native review
queue-empty-short = La file d’attente est vide.
# machine-drafted, needs native review
queue-playing = En cours
# machine-drafted, needs native review
queue-repeat = Répéter
# machine-drafted, needs native review
queue-repeat-off = Répéter : non
# machine-drafted, needs native review
queue-repeat-all = Répéter : tout
# machine-drafted, needs native review
queue-repeat-one = Répéter : un seul
# machine-drafted, needs native review
queue-shuffle = Aléatoire
# machine-drafted, needs native review
queue-shuffle-on = Aléatoire : oui
# machine-drafted, needs native review
queue-shuffle-off = Aléatoire : non
# machine-drafted, needs native review
queue-save = Enregistrer comme playlist…
# machine-drafted, needs native review
queue-save-tip = Enregistrer la file d’attente comme playlist M3U8 ou XSPF
# machine-drafted, needs native review
queue-clear = Vider…
# machine-drafted, needs native review
queue-clear-tip = Vider la file d’attente. Les fichiers restent sur le disque.
# machine-drafted, needs native review
queue-remove = Retirer de la file (le fichier reste sur le disque)
# machine-drafted, needs native review
queue-network-item = Élément réseau, non lu : { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Aucun élément suivant.
# machine-drafted, needs native review
queue-at-start = Aucun élément précédent.
# machine-drafted, needs native review
queue-missing = { $name } n’est plus là. Ignoré.
# machine-drafted, needs native review
queue-network-not-played = Les éléments réseau ne sont pas encore lus.
# machine-drafted, needs native review
queue-nothing-to-add = Rien à ajouter. Seuls les fichiers son, les vidéos et les playlists vont dans la file d’attente.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 élément ajouté à la file d’attente.
   *[other] { $count } éléments ajoutés à la file d’attente.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 élément sera lu ensuite.
   *[other] { $count } éléments seront lus ensuite.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name } : { $count ->
    [one] 1 élément dans la file d’attente.
   *[other] { $count } éléments dans la file d’attente.
}
# machine-drafted, needs native review
queue-playlist-nothing = Rien dans { $name } ne peut être lu ici. La file d’attente montre son contenu.
# machine-drafted, needs native review
queue-playlist-failed = Impossible d’ouvrir { $name } : { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = le fichier est illisible.
# machine-drafted, needs native review
queue-playlist-damaged = ce n’est pas une playlist que Specular sait lire.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Vider la file d’attente (1 élément) ?
   *[other] Vider la file d’attente ({ $count } éléments) ?
}
# machine-drafted, needs native review
queue-clear-detail = Seule la liste est vidée. Les fichiers restent sur le disque.
# machine-drafted, needs native review
queue-clear-ok = Vider la file
# machine-drafted, needs native review
queue-cleared = File d’attente vidée.
# machine-drafted, needs native review
queue-save-title = Enregistrer la file comme playlist
# machine-drafted, needs native review
queue-default-name = File d’attente
# machine-drafted, needs native review
queue-format-m3u8 = Playlist M3U8
# machine-drafted, needs native review
queue-format-xspf = Playlist XSPF
# machine-drafted, needs native review
queue-saved = { $name } enregistré.
# machine-drafted, needs native review
queue-save-failed = Impossible d’enregistrer { $name } : { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Lire le fichier son suivant
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = À la fin d’un fichier son, le fichier son suivant du dossier est lu.
# machine-drafted, needs native review
settings-viewing-play-repeat = Répéter
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Pour les fichiers son, les vidéos et la file d’attente.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Non
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Tout
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Un seul
# machine-drafted, needs native review
settings-viewing-play-shuffle = Lecture aléatoire
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Lire dans un ordre aléatoire, chaque élément une fois par tour.
