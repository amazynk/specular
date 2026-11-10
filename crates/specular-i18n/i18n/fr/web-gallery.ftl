# Specular — Galeries client : un dossier HTML statique de photos, et la relecture de la sélection du client.

# machine-drafted, needs native review

palette-client-gallery = Galerie client…
palette-client-gallery-help = Enregistrer les photos dans un dossier de page web pour un client, avec une sélection qu’il peut renvoyer.
palette-import-client-picks = Importer la sélection du client…
palette-import-client-picks-help = Marquer les photos choisies par un client dans sa galerie.

webgallery-heading = Galerie client
webgallery-subtitle = { $count ->
    [one] Une photo
   *[other] { $count } photos
}
webgallery-title-label = Titre
webgallery-title-default = Épreuves
webgallery-watermark-label = Texte du filigrane (vide : aucun)
webgallery-source = Photos incluses
webgallery-source-current = Cette photo
webgallery-source-selection = Sélection
webgallery-source-album = Album « { $name } »
webgallery-source-search = Résultats de recherche
webgallery-source-folder = Ce dossier
webgallery-source-picks = Retenues
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Thème
webgallery-theme-light = Clair
webgallery-theme-dark = Sombre
webgallery-theme-minimal = Épuré
webgallery-size = Grand côté
webgallery-size-value = { $px } px
webgallery-format = Format
webgallery-captions = Sous chaque photo
webgallery-captions-off = Rien
webgallery-captions-name = Nom du fichier
webgallery-captions-title = Titre
webgallery-captions-all = Nom du fichier, titre et légende
webgallery-exif = Afficher l’appareil et l’exposition
webgallery-location = Conserver la position (GPS) dans les photos
webgallery-note-private = Une galerie statique ne peut pas être protégée par un mot de passe.
webgallery-note-share = Toute personne ayant le dossier ou son adresse voit toutes les photos. Ne partagez le lien qu’avec votre client, ou utilisez le contrôle d’accès de votre hébergeur.
webgallery-note-picks = Les clients choisissent avec un cœur. La sélection reste dans leur navigateur jusqu’à ce qu’ils vous envoient la liste.
webgallery-write = Créer la galerie…
webgallery-busy = Écriture de { $done } sur { $total }…
webgallery-stop = Arrêter
webgallery-done = { $count ->
    [one] Galerie créée : une photo dans { $folder }
   *[other] Galerie créée : { $count } photos dans { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Une photo n’a pas pu être écrite.
   *[other] { $failed } photos n’ont pas pu être écrites.
}
webgallery-stopped = Galerie arrêtée. Rien n’a été conservé.
webgallery-failed = Galerie non créée : { $reason }
webgallery-nothing = Aucune photo n’a pu être écrite.

webgallery-page-count = { $count ->
    [one] Une photo
   *[other] { $count } photos
}
webgallery-page-hint = Touchez le cœur sur les photos que vous voulez. Votre sélection reste dans ce navigateur jusqu’à ce que vous l’envoyiez.
webgallery-page-picked = Choisies : {"{"}n{"}"}
webgallery-page-pick = Choisir
webgallery-page-copy = Copier ma sélection
webgallery-page-copied = Copiée. Collez la liste dans un message.
webgallery-page-copy-by-hand = Copiez la liste sélectionnée et collez-la dans un message.
webgallery-page-download-txt = Télécharger picks.txt
webgallery-page-download-json = Télécharger picks.json
webgallery-page-clear = Effacer la sélection
webgallery-page-clear-ask = Effacer toute votre sélection dans ce navigateur ?
webgallery-page-no-storage = Ce navigateur ne gardera pas votre sélection. Envoyez-la avant de fermer la page.
webgallery-page-noscript = Le choix nécessite JavaScript. Les photos s’ouvrent quand même.
webgallery-page-previous = Précédente
webgallery-page-next = Suivante
webgallery-page-close = Fermer
webgallery-page-position = {"{"}i{"}"} sur {"{"}n{"}"}
webgallery-page-viewer = Visionneuse de photos
webgallery-page-picks = Votre sélection

clientpicks-heading = Importer la sélection du client
clientpicks-subtitle = { $count ->
    [one] Comparée à une photo affichée
   *[other] Comparée aux { $count } photos affichées
}
clientpicks-paste-label = Noms (collez la liste du client)
clientpicks-choose = Choisir le fichier de sélection…
clientpicks-from-file = Depuis { $name }
clientpicks-apply-as = Les marquer
clientpicks-as-pick = Comme retenues
clientpicks-as-stars = { $stars ->
    [one] Une étoile
   *[other] { $stars } étoiles
}
clientpicks-as-label = Étiquette { $label }
clientpicks-empty = Collez la liste envoyée par le client, ou choisissez son picks.txt ou picks.json.
clientpicks-matched = { $matched } noms sur { $total } correspondent à des photos ici.
clientpicks-missing = Introuvables : { $names }
clientpicks-ambiguous = Dans plusieurs dossiers, laissés tels quels : { $names }
clientpicks-more = { $names } et { $more } de plus
clientpicks-apply = Appliquer
clientpicks-applied = { $count ->
    [one] Une photo marquée.
   *[other] { $count } photos marquées.
}
clientpicks-unchanged = Ces photos étaient déjà marquées.
clientpicks-read-failed = Impossible de lire { $name } : { $reason }
