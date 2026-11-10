# machine-drafted, needs native review
# Specular — Develop : historique, instantanés, styles, préréglages, versions, copier-coller, correspondance. Français.

palette-develop-copy = Copier le réglage
palette-develop-copy-help = Copier le réglage de la photo, sauf le recadrage et l’objectif, pour le coller sur d’autres photos.
palette-develop-copy-some = Copier certains modules…
palette-develop-copy-some-help = Choisir les modules du réglage à copier.
palette-develop-paste = Coller le réglage
palette-develop-paste-help = Coller le réglage copié sur la photo, ou sur les photos sélectionnées dans la galerie.
palette-develop-paste-some = Coller certains modules…
palette-develop-paste-some-help = Choisir les modules copiés à coller, en ajout ou en remplacement.
palette-develop-paste-picks = Coller le réglage sur les retenues
palette-develop-paste-picks-help = Coller le réglage copié sur chaque photo retenue.
palette-develop-reference = Utiliser comme référence
palette-develop-reference-help = D’autres photos pourront reprendre l’exposition et la balance des blancs de celle-ci.
palette-develop-match = Faire correspondre à la référence
palette-develop-match-help = Régler l’exposition et la balance des blancs de la photo, de la sélection ou des retenues sur la référence.
palette-develop-sync = Synchroniser sur les retenues
palette-develop-sync-help = Mettre le réglage de cette photo, sauf le recadrage et l’objectif, sur les photos retenues.
palette-develop-sync-some = Synchroniser certains modules…
palette-develop-sync-some-help = Choisir les modules de cette photo à mettre sur les photos retenues.
palette-develop-next-capture = Style des prochaines prises…
palette-develop-next-capture-help = Le style donné à chaque nouveau fichier qui arrive dans le dossier.
palette-develop-auto-rules = Règles automatiques…
palette-develop-auto-rules-help = Les règles qui appliquent un style à la première ouverture d’une photo dans Develop.
palette-develop-import-style = Importer un style…
palette-develop-import-style-help = Ajouter un style depuis un fichier.
palette-develop-snapshot = Prendre un instantané
palette-develop-snapshot-help = Nommer le réglage tel qu’il est, pour le comparer ou y revenir plus tard.
palette-develop-compress = Compacter l’historique
palette-develop-compress-help = Retirer les étapes remplacées par d’autres. Le réglage reste le même.
palette-develop-new-version = Nouvelle version
palette-develop-new-version-help = Un nouveau réglage de la même photo, avec son propre historique. Le fichier n’est pas copié.
palette-develop-new-style = Nouveau style…
palette-develop-new-style-help = Enregistrer des modules choisis du réglage comme style.

develop-styles = Styles
develop-snapshots = Historique et instantanés
develop-versions = Versions
develop-style-new = Nouveau style d’après ce réglage…
develop-style-import = Importer un style…
develop-rules = Règles automatiques…
develop-snapshot-take = Prendre un instantané
develop-history-compress = Compacter l’historique
develop-snapshot-comparing = Comparaison : { $name }
develop-snapshot-compare-off = Comparaison arrêtée
develop-version-number = Version { $number }
develop-version-open = { $name } (ouverte)
develop-version-new = Nouvelle version
develop-version-duplicate = Dupliquer cette version

develop-history-paste = Coller
develop-history-sync = Synchroniser
develop-history-match = Correspondance à la référence
develop-history-snapshot = Instantané : { $name }
develop-history-previous = Comme la précédente

develop-rename = Renommer…
develop-delete = Supprimer
develop-save = Enregistrer
develop-name = Nom
develop-style-apply = Appliquer
develop-style-apply-replace = Appliquer en remplaçant ces modules
develop-style-auto = Appliquer aux nouvelles photos de cet appareil
develop-style-export = Exporter…
develop-snapshot-restore = Revenir à cet instantané
develop-version-open-it = Ouvrir
develop-preset-save = Enregistrer comme préréglage…
develop-preset-save-module = Enregistrer { $module } comme préréglage…
develop-preset-delete = Supprimer « { $name } »…

develop-sheet-modules = Modules
develop-sheet-included = Inclus
develop-sheet-left-out = Laissé de côté
develop-sheet-more = { $count } de plus
develop-copy-title = Copier des modules
develop-copy-ok = Copier
develop-paste-title = Coller des modules
develop-paste-ok = Coller
develop-paste-mode = Modules déjà dans le réglage
develop-paste-append = Ajouter
develop-paste-replace = Remplacer
develop-style-new-title = Nouveau style
develop-style-save = Enregistrer le style
develop-style-default-name = Mon style { $number }
develop-style-rename-title = Renommer le style
develop-preset-save-title = Préréglage { $module }
develop-preset-default-name = Mon préréglage
develop-snapshot-take-title = Prendre un instantané
develop-snapshot-default-name = Instantané { $number }
develop-snapshot-rename-title = Renommer l’instantané
develop-version-new-title = Nouvelle version
develop-version-duplicate-title = Dupliquer la version
develop-version-create = Créer
develop-version-rename-title = Renommer la version

develop-style-delete-ask = Supprimer le style « { $name } » ?
develop-style-delete-detail = Les photos où il a été appliqué gardent leur réglage. Le fichier du style est supprimé.
develop-preset-delete-ask = Supprimer le préréglage « { $name } » ?
develop-preset-delete-detail = Les réglages où il a été appliqué restent tels quels.
develop-snapshot-delete-ask = Supprimer l’instantané « { $name } » ?
develop-snapshot-delete-detail = Le réglage et son historique restent tels quels.
develop-version-delete-ask = Supprimer « { $name } » ?
develop-version-delete-detail = Le réglage et l’historique de cette version disparaissent. La photo et ses autres versions restent sur le disque.
develop-rule-delete-ask = Supprimer la règle « { $rule } » ?
develop-rule-delete-detail = Le style reste, et les photos déjà ouvertes gardent leur réglage.

develop-rule-text = { $style } : { $when }
develop-rule-every-photo = toutes les photos
develop-rule-iso-min = ISO { $iso } et plus
develop-rule-iso-max = ISO jusqu’à { $iso }
develop-rule-iso-range = ISO { $low }–{ $high }
develop-rule-raw = fichiers raw
develop-rule-other = pas raw
develop-rule-any-camera = tout appareil
develop-next-placeholder = Style de chaque nouveau fichier
develop-next-off-row = Aucun
develop-next-previous-row = Comme la photo précédente

develop-copied = { $count ->
    [one] 1 module copié depuis { $name }
   *[other] { $count } modules copiés depuis { $name }
}
develop-pasted = { $count ->
    [one] Collé sur 1 photo
   *[other] Collé sur { $count } photos
}
develop-nothing-to-copy = Aucune photo à copier.
develop-nothing-copied = Rien n’est copié. Copiez d’abord un réglage (Ctrl+Alt+C).
develop-no-photos = Aucune photo à modifier.
develop-no-picks = Aucune photo retenue.
develop-reference-set = Référence : { $name }
develop-no-reference = Pas encore de référence. Ouvrez la photo de référence et choisissez Utiliser comme référence (Ctrl+Alt+R).
develop-reference-unreadable = La photo de référence n’a pas pu être lue.
develop-matching = { $count ->
    [one] Correspondance de 1 photo…
   *[other] Correspondance de { $count } photos…
}
develop-matched = { $count ->
    [one] 1 photo mise en correspondance
   *[other] { $count } photos mises en correspondance
}
develop-matched-some = { $count } mises en correspondance. { $failed } n’ont pas pu être lues ou mesurées.
develop-style-saved = Style enregistré : { $name }
develop-style-applied = Style appliqué : { $name }
develop-style-imported = Style importé : { $name }
develop-style-not-a-style = Ce fichier n’est pas un style.
develop-style-exported = Style exporté : { $name }
develop-style-export-failed = Le style n’a pas pu être écrit à cet endroit.
develop-style-import-dialog = Importer un style
develop-style-export-dialog = Exporter le style
develop-style-file-kind = Style
develop-preset-saved = Préréglage enregistré : { $name }
develop-preset-applied = Préréglage appliqué : { $name }
develop-snapshot-taken = Instantané pris : { $name }
develop-history-compressed = { $count ->
    [0] Rien à compacter
    [one] 1 étape retirée
   *[other] { $count } étapes retirées
}
develop-version-created = Version créée : { $name }
develop-version-opened = Version ouverte : { $name }
develop-version-deleted = Version supprimée : { $name }
develop-rule-added = { $name } s’applique aux nouvelles photos de { $camera }.
develop-rule-applied = Règle automatique : { $name }
develop-rules-none = Aucune règle automatique. Ajoutez-en une depuis le menu d’un style.
develop-next-off = Les nouveaux fichiers ne reçoivent aucun style.
develop-next-previous = Les nouveaux fichiers reçoivent le réglage de la photo précédente.
develop-next-style = Les nouveaux fichiers reçoivent le style { $name }.
develop-next-applied = { $count ->
    [one] Style des prochaines prises sur 1 nouveau fichier
   *[other] Style des prochaines prises sur { $count } nouveaux fichiers
}

develop-style-specular = Specular contrasté
develop-style-soft = Doux
develop-style-vivid = Vif
develop-style-mono = Noir et blanc
develop-preset-plus-one = Plus clair (+1 IL)
develop-preset-minus-one = Plus sombre (−1 IL)
develop-preset-low-contrast = Contraste faible
develop-preset-high-contrast = Contraste fort
develop-preset-open-shadows = Ombres débouchées
develop-preset-crisp = Net
develop-preset-muted = Atténué
develop-preset-high-iso = ISO élevés
develop-preset-screen = Pour écrans

gallery-badge-versions = { $count } versions
# machine-drafted, needs native review
develop-batch-row = Enregistrement des retouches
# machine-drafted, needs native review
develop-batch-row-detail = { $done } sur { $total } photos
# machine-drafted, needs native review
develop-batch-saving = Enregistrement
# machine-drafted, needs native review
develop-batch-stopping = Arrêt en cours
# machine-drafted, needs native review
develop-batch-stopped = Arrêté. { $done } photos sur { $total } enregistrées ; les autres sont inchangées.
