# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Attendre la touche suivante
settings-keys-sequence-timeout-help = Durée pendant laquelle une séquence comme G G attend sa touche suivante. Une touche qui commence aussi une séquence plus longue s’exécute après cette pause.
settings-keys-bindings = Raccourcis clavier
settings-keys-bindings-help = Sélectionnez une action et appuyez sur les touches à lui associer. Chaque mode a ses propres touches ; le mode tri et les images médicales reprennent celles de la visionneuse. Pour associer Échap, appuyez brièvement ; le maintenir annule. Les raccourcis sont enregistrés dans keymap.toml.
palette-customise-keys = Personnaliser les touches…
keys-mode-viewer = Visionneuse
keys-mode-cull = Mode tri
keys-mode-medical = Images médicales
keys-modes = Mode
keys-presets = Préréglage
keys-preset-default = Par défaut
keys-preset-vim = Vim
keys-preset-culling = Tri à la Lightroom
keys-preset-photo-mechanic = À la Photo Mechanic
action-pan-left = Déplacer vers la gauche
action-pan-right = Déplacer vers la droite
action-pan-up = Déplacer vers le haut
action-pan-down = Déplacer vers le bas
action-pan-page-left = Déplacer d’un écran vers la gauche
action-pan-page-right = Déplacer d’un écran vers la droite
action-pan-page-up = Déplacer d’un écran vers le haut
action-pan-page-down = Déplacer d’un écran vers le bas
action-reload = Recharger l’image
keys-action-cull-pick = Retenir
keys-action-cull-reject = Rejeter
keys-action-cull-unflag = Retirer le marquage
keys-action-cull-rate = { $stars ->
    [0] Effacer la note
    [one] Noter 1 étoile
   *[other] Noter { $stars } étoiles
}
keys-action-cull-label = Étiquette de couleur : { $label }
keys-action-cull-copy-to = Copier vers le dossier { $slot }
keys-action-cull-move-to = Déplacer vers le dossier { $slot }
keys-import = Importer…
keys-export = Exporter…
keys-export-mode = Exporter { $mode }…
keys-reset-mode = Réinitialiser { $mode }
keys-reset-all = Réinitialiser toutes les touches
keys-add = Ajouter une touche pour { $action }
keys-change = Modifier { $keys }
keys-remove = Supprimer { $keys }
keys-reset-action = Réinitialiser { $action }
keys-none = Aucune touche
keys-changed = Modifié
keys-press = Appuyez sur les touches…
keys-recording-hint = Maintenez Échap pour annuler
keys-cancel-recording = Annuler
keys-no-match = Aucune action ni touche ne correspond à « { $query } ».
keys-list = Actions et leurs touches
keys-confirm-preset = Utiliser le préréglage { $preset } ? Il remplace tous vos raccourcis.
keys-confirm-reset-mode = Réinitialiser toutes les touches de { $mode } selon le préréglage ?
keys-confirm-reset-all = Réinitialiser toutes les touches de tous les modes selon le préréglage ?
keys-confirm-ok = Remplacer
keys-confirm-reset = Réinitialiser
keys-confirm-cancel = Annuler
keys-conflict-same = { $keys } exécute déjà « { $other } ».
keys-conflict-prefix = { $keys } et { $sequence } (« { $other } ») commencent pareil : le plus court ne s’exécute qu’après une pause.
keys-conflict-shadow = { $keys } exécute « { $other } » dans { $mode }.
keys-conflict-reserved = { $keys } risque de ne jamais atteindre Specular : { $reason }
keys-choice-replace = Remplacer
keys-choice-swap = Échanger
keys-choice-keep = Garder les deux
keys-choice-anyway = Associer quand même
keys-choice-cancel = Annuler
keys-reserved-switch = il change de fenêtre.
keys-reserved-close = il ferme la fenêtre.
keys-reserved-secure = le système le prend.
keys-reserved-desktop = les raccourcis Super appartiennent au bureau.
keys-reserved-workspaces = il change de terminal ou d’espace de travail.
keys-reserved-input = il change de méthode de saisie ou de disposition du clavier.
keys-reserved-macos = macOS le prend.
keys-announce-recording = Enregistrement des touches pour { $action }. Appuyez sur les touches, ou maintenez Échap pour annuler.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } exécute maintenant { $action }.
keys-announce-already = { $keys } exécute déjà { $action }.
keys-announce-cancelled = Enregistrement annulé.
keys-announce-removed = { $keys } retiré de { $action }.
keys-announce-reset-action = { $action } a de nouveau les touches du préréglage.
keys-announce-reset-mode = Les touches de { $mode } sont réinitialisées.
keys-announce-reset-all = Toutes les touches sont réinitialisées.
keys-announce-preset = Préréglage { $preset } utilisé.
keys-toast-imported = Raccourcis importés
keys-toast-exported = Raccourcis exportés
keys-toast-reloaded = keymap.toml modifié : raccourcis rechargés
keys-toast-error = Impossible de lire les raccourcis : { $error }
keys-toast-write-error = Impossible d’enregistrer keymap.toml : { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = Impossible de lire keymap.toml : { $error }
keys-problem-key = Touche inconnue « { $key } » ignorée.
keys-problem-action = « { $key } » désigne une action inconnue « { $action } » ; ignorée.
keys-problem-mode = La table [{ $mode }] n’est pas un mode ; ignorée.
keys-problem-duplicate = { $keys } est indiqué deux fois dans { $mode } ; le second est utilisé.
keys-problem-prefix = { $keys } commence { $longer } dans { $mode } et ne s’exécute donc qu’après une pause.
keys-problem-preset = Préréglage inconnu « { $preset } » ; les touches par défaut s’appliquent.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = Modifier keymap.toml
keys-problem-action-suggestion = « { $key } » désigne une action inconnue « { $action } » ; ignorée. Vouliez-vous dire « { $suggestion } » ?
keys-problem-mode-suggestion = La table [{ $mode }] n’est pas un mode ; ignorée. Vouliez-vous dire [{ $suggestion }] ?
keys-problem-preset-suggestion = Préréglage inconnu « { $preset } » ; les touches par défaut s’appliquent. Vouliez-vous dire « { $suggestion } » ?
