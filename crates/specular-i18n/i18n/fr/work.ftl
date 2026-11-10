# machine-drafted, needs native review
# Specular — Tâches en arrière-plan : l’indicateur affiché pendant qu’une tâche s’exécute en arrière-plan, et le panneau qu’il ouvre. Français.

## Le panneau

work-title = Tâches en arrière-plan
work-show = Afficher les tâches en arrière-plan
work-empty = Aucune tâche ne s’exécute en arrière-plan.

## Tâches

work-kind-library = Photothèque
work-kind-ai-search = Recherche IA
work-kind-faces = Personnes
work-kind-text = Texte dans les photos
work-kind-culling = Notes de tri
work-kind-download = Téléchargement du modèle d’IA
work-kind-similar = Photos similaires

## Étapes

work-step-finding = Recherche des photos
work-step-reading = Lecture des détails des photos
work-step-signatures = Comparaison des photos
work-step-grouping = Regroupement des photos similaires
work-step-text-index = Création de l’index de recherche
work-step-embedding = Indexation des photos pour la recherche IA
work-step-switching = Réindexation pour la nouvelle taille de modèle
work-step-faces = Recherche des visages
work-step-ocr = Lecture du texte dans les photos
work-step-scoring = Évaluation de la netteté et de l’exposition
work-step-downloading = Téléchargement
work-step-focus = Vérification de la netteté
work-step-item = { $step } · { $item }

## Progression

work-count-photos = { $done } sur { $total } { $count ->
    [one] photo
   *[other] photos
}
work-count = { $done } sur { $total }
work-count-left = { $count } · encore { $left }
work-so-far-photos = { $done } { $count ->
    [one] photo
   *[other] photos
} pour l’instant
work-so-far = { $done } pour l’instant

## Temps restant

work-eta-estimating = Estimation…
work-eta-seconds = Il reste environ { $n ->
    [one] { $n } seconde
   *[other] { $n } secondes
}
work-eta-minutes = Il reste environ { $n ->
    [one] { $n } minute
   *[other] { $n } minutes
}
work-eta-hours = Il reste environ { $h ->
    [one] { $h } heure
   *[other] { $h } heures
}
work-eta-hours-minutes = Il reste environ { $h ->
    [one] { $h } heure
   *[other] { $h } heures
} et { $m ->
    [one] { $m } minute
   *[other] { $m } minutes
}

## Pourquoi une tâche attend

work-paused-browsing = En pause pendant que vous parcourez vos photos
work-paused-battery = En pause sur batterie
work-paused-low-power = En pause en mode économie d’énergie
work-paused-heat = En pause tant que l’ordinateur est chaud
work-paused-memory = En pause tant que la mémoire est faible
work-paused-user = En pause

## La suite

work-then = Ensuite : { $steps }

## Boutons

work-pause = Pause
work-resume = Reprendre
work-settings = Paramètres…

## L’indicateur

work-chip = { $job } · { $done } sur { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Tâches en arrière-plan : { $status }

## Annonces

work-started = { $job } : démarré en arrière-plan
work-finished = { $job } : terminé
