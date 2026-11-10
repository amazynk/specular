# machine-drafted, needs native review
# Specular — Mode tri (DESIGN.md §6.3).

palette-cull = Entrer dans le mode tri ou le quitter
cheatsheet-cull = Mode tri
settings-section-cull = Mode tri
settings-cull-auto-enter = Trier les dossiers RAW
settings-cull-auto-enter-help = Ouvrir un dossier contenant surtout des fichiers RAW directement en mode tri.
settings-cull-auto-advance = Image suivante après un marquage
settings-cull-auto-advance-help = Passer à la suite après une sélection, un rejet ou une note. Les étiquettes et l’annulation du marquage restent sur l’image.
settings-cull-show = Afficher pour les paires RAW+JPEG
settings-cull-show-help = Un RAW et un JPEG portant le même nom sont triés comme une seule image, et les marquages s’appliquent aux deux. Le JPEG s’ouvre plus vite.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Empiler les rafales
settings-cull-bursts-help = Les photos prises en succession rapide par un même appareil s’affichent en une pile. S ouvre ou ferme une pile, B garde l’image affichée et rejette le reste de sa pile.
settings-cull-burst-gap = Écart de rafale
settings-cull-burst-gap-help = La pause la plus longue entre deux photos d’une rafale.
settings-cull-watch = Afficher les nouveaux fichiers
settings-cull-watch-help = Les fichiers qui apparaissent dans le dossier pendant le tri (appareil connecté, importation) sont ajoutés à la fin une fois entièrement écrits.
settings-cull-follow-newest = Suivre le fichier le plus récent
settings-cull-follow-newest-help = Aller à chaque nouveau fichier dès son arrivée. T bascule ce réglage pendant le tri.
settings-cull-slot-1 = Dossier 1
settings-cull-slot-1-help = Alt+1 copie l’image (avec son RAW ou JPEG et ses fichiers annexes) dans ce dossier, Alt+Maj+1 l’y déplace.
settings-cull-slot-1-unset = Non défini
settings-cull-slot-2 = Dossier 2
settings-cull-slot-2-help = Alt+2 copie l’image (avec son RAW ou JPEG et ses fichiers annexes) dans ce dossier, Alt+Maj+2 l’y déplace.
settings-cull-slot-2-unset = Non défini
settings-cull-slot-3 = Dossier 3
settings-cull-slot-3-help = Alt+3 copie l’image (avec son RAW ou JPEG et ses fichiers annexes) dans ce dossier, Alt+Maj+3 l’y déplace.
settings-cull-slot-3-unset = Non défini
settings-cull-collisions = Quand un nom est déjà pris
settings-cull-collisions-help = Les fichiers des dossiers ne sont jamais remplacés.
settings-cull-collisions-suffix = Ajouter un numéro
settings-cull-collisions-skip = Ignorer l’image
# machine-drafted, needs native review
# Mode tri choisi sans dossier ni images de galerie ouverts.
cull-nothing = Rien à trier — ouvrez d’abord un dossier de photos
cull-slot-unset = Aucun dossier { $slot } défini — choisissez-en un dans Paramètres → Mode tri
cull-slot-missing = Le dossier { $slot } n’est pas disponible : { $reason }
cull-copied = Copié dans { $folder }
cull-moved = Déplacé dans { $folder }
cull-slot-renamed = Enregistré sous { $name } dans { $folder }
cull-slot-skipped = { $folder } contient déjà { $name } — ignoré
cull-slot-same = L’image est déjà dans { $folder }
cull-slot-failed = { $op ->
    [move] Impossible de déplacer { $file }
   *[copy] Impossible de copier { $file }
}: { $reason }
cull-copy-kept = { $count ->
    [one] Une copie modifiée depuis a été conservée
   *[other] { $count } copies modifiées depuis ont été conservées
}
cull-undo-failed = Impossible d’annuler pour { $file } : { $reason }
cull-burst-open = Rafale { $index } sur { $count }
cull-burst-closed = Rafale de { $count }
cull-burst-toggle = Ouvrir ou fermer la rafale
cull-burst-best = 1 gardée, { $count } rejetées
cull-burst-none = Ne fait pas partie d’une rafale
cull-follow-on = Suivi des nouveaux fichiers
cull-follow-off = Nouveaux fichiers non suivis
cull-new-files = { $count ->
    [one] 1 nouveau fichier
   *[other] { $count } nouveaux fichiers
}
cull-key-copy = Copier dans le dossier 1–3
cull-key-move = Déplacer dans le dossier 1–3
cull-key-burst = Ouvrir ou fermer une rafale
cull-key-best = Garder celle-ci, rejeter le reste de la rafale
cull-key-follow = Suivre les nouveaux fichiers
cull-key-subframes = Afficher ou masquer le tableau des poses unitaires
cull-key-sort = Trier par la colonne suivante
cull-key-sort-reverse = Inverser l’ordre
cull-key-metric = Tracer la mesure suivante
cull-key-sigma-down = Rejet auto : baisser k
cull-key-sigma-up = Rejet auto : augmenter k
cull-key-auto-reject = Rejeter les poses suggérées
cull-key-move-rejects = Déplacer les poses rejetées dans un sous-dossier
cull-key-same-stretch = Même étirement pour toutes les poses

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Ouvrir la rafale
cull-tip-burst-close = Fermer la rafale
cull-tip-exit = Quitter le mode tri
cull-tip-label = Étiquette : { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Poses unitaires
cull-astro-title = { $count ->
    [one] { $count } pose unitaire
   *[other] { $count } poses unitaires
}
cull-astro-pending = mesure de { $count }
cull-astro-col-name = Pose
cull-astro-col-filter = Filtre
cull-astro-col-exposure = Expo.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Exc.
cull-astro-col-stars = Étoiles
cull-astro-col-bg = Fond de ciel
cull-astro-col-snr = SNR
cull-astro-col-noise = Bruit
cull-astro-same-stretch = Même étirement
cull-astro-own-stretch = Étirement propre
cull-astro-suggest-high = { $metric } au-dessus de { $value } (médiane + { $k }σ) : { $count ->
    [one] { $count } pose
   *[other] { $count } poses
}
cull-astro-suggest-low = { $metric } en dessous de { $value } (médiane − { $k }σ) : { $count ->
    [one] { $count } pose
   *[other] { $count } poses
}
cull-astro-suggest-none = Aucune pose au-delà de { $k }σ
cull-astro-suggest-few = Le rejet auto demande 3 poses mesurées
cull-astro-suggested = suggérée pour le rejet
cull-astro-k = k { $k }
cull-astro-reject = Rejeter { $count }
cull-astro-move = Déplacer les rejetées…
cull-astro-chart = { $metric } sur { $count ->
        [one] { $count } pose
       *[other] { $count } poses
    }, médiane { $median }.
cull-astro-sort-by = Trier par { $column }
cull-astro-ascending = croissant
cull-astro-descending = décroissant
cull-astro-sorted = Trié par { $column }
cull-astro-reversed = Ordre inversé
cull-astro-metric = Graphique : { $metric }
cull-astro-k-set = Rejet auto à { $k }σ
cull-astro-none = Aucune pose unitaire ici
cull-astro-same-on = Même étirement pour toutes les poses
cull-astro-same-off = Chaque pose étirée séparément
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = excentricité { $value }
cull-astro-line-stars = { $count ->
    [one] { $count } étoile
   *[other] { $count } étoiles
}
cull-astro-measuring = mesure des étoiles…
cull-astro-reject-confirm = { $count ->
    [one] Rejeter la pose suggérée ?
   *[other] Rejeter les { $count } poses suggérées ?
}
cull-astro-reject-detail = { $rule }. Annuler rétablit les marques.
cull-astro-reject-ok = Rejeter
cull-astro-nothing-to-reject = Aucune pose à rejeter
cull-astro-rejected = { $count ->
    [one] { $count } pose rejetée
   *[other] { $count } poses rejetées
}
cull-move-rejects-confirm = { $count ->
    [one] Déplacer la pose rejetée dans le dossier « { $folder } » ?
   *[other] Déplacer les { $count } poses rejetées dans le dossier « { $folder } » ?
}
cull-move-rejects-detail = Les fichiers sont déplacés, pas supprimés. Annuler les remet en place.
cull-move-rejects-ok = Déplacer
cull-move-rejects-none = Aucune pose rejetée à déplacer
cull-moved-rejects = { $count ->
    [one] { $count } pose déplacée dans « { $folder } »
   *[other] { $count } poses déplacées dans « { $folder } »
}
