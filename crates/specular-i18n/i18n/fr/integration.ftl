# Integration: open with, user commands, paste (Stage 2).
palette-open-with = Ouvrir avec…
palette-paste = Coller
open-with-placeholder = Ouvrir avec…
open-with-loading = Recherche des applications…
open-with-default = { $name } (par défaut)
open-with-choose-other = Choisir une autre application…
open-with-choose-title = Choisir une application
open-with-failed = Impossible d’ouvrir avec { $name } : { $reason }
command-missing = { $program } introuvable
command-failed = Impossible de lancer { $name } : { $reason }
command-exited = { $name } s’est terminé avec une erreur : { $status }
paste-nothing = Rien à ouvrir dans le presse-papiers
paste-failed = Impossible de coller l’image : { $reason }
drag-failed = Impossible de faire glisser le fichier : { $reason }
about-storage = Paramètres : { $settings } · Cache : { $cache }
about-storage-portable = Mode portable : réglages et cache dans { $dir }

## Scripting and recipes

palette-copy-view-command = Copier la vue comme commande shell
palette-copy-view-python = Copier la vue en Python
toast-copied-view-command = Commande copiée
script-copy-no-image = Aucune image à décrire
recipe-confirm-title = Exécuter la recette « { $name } » pour la première fois ? Elle lance : { $command }
recipe-confirm-run = Exécuter
recipe-no-file = La recette nécessite un fichier image
recipe-running = { $name } en cours…
recipe-done = { $name } : { $shapes ->
    [one] { $shapes } forme ajoutée
   *[other] { $shapes } formes ajoutées
}
recipe-failed = { $name } a échoué : { $reason }
recipe-timeout = arrêté après { $seconds } s
recipe-no-output = { $name } n’a pas écrit { $file }
