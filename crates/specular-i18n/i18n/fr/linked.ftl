# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Comparer des séries (vues liées)
# machine-drafted, needs native review
palette-compare-layout = Disposition de vues suivante
# machine-drafted, needs native review
palette-next-viewport = Vue suivante
# machine-drafted, needs native review
palette-prev-viewport = Vue précédente
# machine-drafted, needs native review
palette-reference-lines = Afficher ou masquer les lignes de référence
# machine-drafted, needs native review
palette-sync-window = Synchro de fenêtre suivante
# machine-drafted, needs native review
palette-sync-zoom = Zoom et panoramique ensemble selon la taille
# machine-drafted, needs native review
palette-link-series = Liaison de série suivante
# machine-drafted, needs native review
palette-align-series = Aligner ici les autres séries
# machine-drafted, needs native review
palette-compare-series-help = Afficher plusieurs séries côte à côte, défilant ensemble.
# machine-drafted, needs native review
palette-compare-layout-help = Afficher plus ou moins de vues.
# machine-drafted, needs native review
palette-next-viewport-help = Faire de la vue suivante la vue active.
# machine-drafted, needs native review
palette-prev-viewport-help = Faire de la vue précédente la vue active.
# machine-drafted, needs native review
palette-reference-lines-help = Tracer où la coupe active traverse les autres vues.
# machine-drafted, needs native review
palette-sync-window-help = Choisir quelles vues partagent un changement de fenêtre/niveau : désactivée, même modalité ou toutes.
# machine-drafted, needs native review
palette-sync-zoom-help = Afficher toutes les vues à la même taille et les déplacer ensemble.
# machine-drafted, needs native review
palette-link-series-help = Choisir comment cette série suit les autres.
# machine-drafted, needs native review
palette-align-series-help = Aligner une série antérieure sur celle-ci aux coupes affichées.
# machine-drafted, needs native review
disabled-not-comparing = Comparez d’abord des séries

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Disposition : { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Lignes de référence
# machine-drafted, needs native review
linked-zoom = Zoom synchronisé
# machine-drafted, needs native review
linked-window-sync = Fenêtre synchronisée : { $mode ->
    [off] désactivée
    [modality] même modalité
   *[all] toutes
}
# machine-drafted, needs native review
linked-link = Liaison : { $mode ->
    [frame] référentiel
    [position] position du patient
   *[off] désactivée
}
# machine-drafted, needs native review
linked-align = Aligner ici

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Vue { $n } sur { $count }, active
   *[no] Vue { $n } sur { $count }
}
# machine-drafted, needs native review
linked-empty = Faites glisser une série ici
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Coupe { $n } sur { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = Dans un intervalle : coupe la plus proche à { $distance } mm
# machine-drafted, needs native review
linked-outside = En dehors de cette série ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Non liée
# machine-drafted, needs native review
linked-aligned = Alignée à la main ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = Curseur 3D en { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } image
       *[other] { $count } images
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Une seule série ici : faites-en glisser une autre depuis le navigateur d’études
   *[other] Comparaison de { $count } séries
}
# machine-drafted, needs native review
linked-no-geometry = Cette série n’a pas de positions de coupe : elle ne peut pas être liée
# machine-drafted, needs native review
linked-active = Vue { $n } : { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Rien à aligner : les autres séries partagent ce référentiel
    [one] 1 série alignée sur la série active, pour cette session
   *[other] { $count } séries alignées sur la série active, pour cette session
}
