# machine-drafted, needs native review
# Specular — Doublons et photos similaires (catégories de la galerie). Français.

similar-duplicates = Doublons
similar-similar = Photos similaires
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } groupe
   *[other] { $groups } groupes
}, { $size } à libérer
similar-menu-scanning = { $name } — recherche ({ PERCENT($percent) })
similar-menu-none = { $name } — aucun trouvé
similar-summary = { $groups ->
    [one] { $groups } groupe à examiner
   *[other] { $groups } groupes à examiner
}, { $files ->
    [one] { $files } fichier
   *[other] { $files } fichiers
} à écarter, { $size } à libérer
similar-progress = { $resolved } sur { $total } traités
similar-skipped = { $count } ignorés
similar-scanning = Recherche de doublons et de photos similaires… { $done } sur { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Vérification de la netteté… { $done } sur { $total }
# machine-drafted, needs native review
# La liste pendant la première analyse (sa progression est dans l’en-tête au-dessus).
similar-waiting = Les groupes apparaîtront ici une fois l’analyse terminée.
# machine-drafted, needs native review
similar-stopped = Analyse arrêtée : rien n’a été regroupé
similar-stopped-sharpness = Vérification de la netteté arrêtée : meilleurs choix d’après les vignettes
similar-stopped-empty = L’analyse a été arrêtée. Ce qu’elle a mesuré est conservé : Relancer l’analyse la poursuit à partir de là.
similar-scan-again = Relancer l’analyse
similar-empty-duplicates = Aucun doublon dans ce dossier.
similar-empty-similar = Aucune photo similaire dans ce dossier.
similar-all-done = Tous les groupes ont été examinés.
similar-sensitivity = Sensibilité
similar-sensitivity-strict = Stricte
similar-sensitivity-loose = Large
similar-accept-all = Accepter toutes les suggestions
similar-show-skipped = Afficher les groupes ignorés
similar-discard-with = Écarter : { $action }
similar-action-trash = Mettre à la corbeille
similar-action-move = Déplacer vers un dossier…
similar-action-move-to = Déplacer vers « { $folder } »
similar-action-reject = Marquer comme rejeté uniquement
similar-choose-folder = Choisir le dossier des fichiers écartés…
similar-confirm = Accepter les suggestions de { $groups ->
    [one] { $groups } groupe
   *[other] { $groups } groupes
} ? { $files ->
    [one] { $files } fichier
   *[other] { $files } fichiers
} ({ $size }) : { $action }.
similar-confirm-yes = { $files ->
    [one] Accepter et écarter { $files } fichier
   *[other] Accepter et écarter { $files } fichiers
}
similar-cancel = Annuler
similar-group-title = Groupe { $index } sur { $total }
similar-group-photos = { $count ->
    [one] { $count } photo
   *[other] { $count } photos
}
similar-to-free = { $size } à libérer
similar-keep = Garder
similar-discard = Écarter
similar-undecided = Indécis
similar-accept-next = Accepter et suivant
similar-not-similar = Pas similaire
similar-skip = Ignorer
similar-compare = Comparer
similar-zoom = { PERCENT(100) }
similar-back = Retour
similar-done = Terminé
similar-suggested-keep = Suggestion : garder
similar-suggested-discard = Suggestion : écarter
similar-toast-accepted = { $count ->
    [one] { $count } groupe accepté
   *[other] { $count } groupes acceptés
}
similar-toast-rejected = { $count ->
    [one] { $count } fichier marqué comme rejeté
   *[other] { $count } fichiers marqués comme rejetés
}
similar-toast-protected = Les photos notées, marquées, étiquetées, avec mots-clés ou retouchées sont toujours gardées.
# machine-drafted, needs native review
similar-zoom-already = Cette photo est déjà affichée à { PERCENT(100) } ou plus
similar-a11y-member = { $name } : { $state }, { $reason }
similar-hint-list = ↑↓ choisir · Entrée examiner · A tout accepter · Tab changer · Échap fermer
similar-reason-rated = Notée
similar-reason-flagged = Marquée comme retenue
similar-reason-labelled = A une étiquette de couleur
similar-reason-tagged = A des mots-clés
similar-reason-edited = A des retouches
similar-reason-original = L’original
similar-reason-raw-original = Original RAW
similar-reason-highest-resolution = Meilleure résolution
similar-reason-deeper-colour = Plus grande profondeur de couleur
similar-reason-least-compressed = Moins compressée
similar-reason-has-metadata = Garde ses métadonnées
similar-reason-oldest = Copie la plus ancienne
similar-reason-sharpest = La plus nette
similar-reason-steadiest = Le moins de flou de bougé
similar-reason-best-exposure = Mieux exposée
similar-reason-least-noise = Le moins de bruit
similar-reason-also-good = Bonne aussi
similar-reason-same-file = Fichier identique
similar-reason-derivative = Copie dérivée
similar-reason-rendering-of-raw = Rendu du RAW
similar-reason-smaller-copy = Copie plus petite
similar-reason-shallower-colour = Moindre profondeur de couleur
similar-reason-more-compressed = Plus compressée
similar-reason-no-metadata = Métadonnées supprimées
similar-reason-newer-copy = Copie plus récente
similar-reason-blurrier = Plus floue
similar-reason-motion-blur = Flou de bougé
similar-reason-overexposed = Surexposée
similar-reason-underexposed = Sous-exposée
similar-reason-noisier = Plus de bruit
similar-reason-lower-resolution = Résolution plus faible
