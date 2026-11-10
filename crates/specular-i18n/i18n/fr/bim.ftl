# machine-drafted, needs native review
## Revue de modèles 3D : marche et vol, plans de coupe et boîte de coupe, vues enregistrées,
## sujets BCF (mode 3D). Les touches citées sont celles par défaut.

cheatsheet-review = Revue

palette-mesh-walk = Marcher
palette-mesh-fly = Voler
palette-mesh-views = Vues enregistrées
palette-mesh-save-view = Enregistrer la vue
palette-mesh-section-plane = Ajouter un plan de coupe
palette-mesh-section-box = Boîte de coupe
palette-mesh-section-flip = Inverser le plan de coupe
palette-mesh-section-clear = Retirer toutes les coupes
palette-mesh-measure-cut = Mesurer sur la coupe
palette-mesh-bcf-topics = Sujets BCF
# machine-drafted, needs native review
palette-mesh-model-checks = Contrôles du modèle
# machine-drafted, needs native review
palette-mesh-models = Modèles de la scène
# machine-drafted, needs native review
palette-mesh-add-model = Ajouter un modèle…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Ajouter les modèles d’un dossier…

# Marche et vol : notifications et lignes d’affichage, selon $what. $eye : hauteur de l’œil
# au-dessus du sol, avec son unité (walk-line seulement).
mesh-nav = { $what ->
    [walk-toast] Marche : W A S D pour avancer, glisser pour regarder, Esc pour arrêter
    [fly-toast] Vol : W A S D pour avancer, Q E pour descendre et monter, Esc pour arrêter
    [orbit] Orbite
    [walk-line] Marche · œil à { $eye } du sol
    [walk-keys] W A S D avancer · glisser regarder · Shift plus vite · Esc arrêter
    [preparing] Murs pas encore prêts : vous les traversez
    [fly-keys] W A S D avancer · Q E descendre, monter · glisser regarder · Shift plus vite · Esc arrêter
   *[fly-line] Vol
}

# Textes courts des coupes, des mesures et des vues enregistrées, selon $what.
bim-note = { $what ->
    [plane-added] Plan de coupe ajouté : faites glisser sa poignée pour le déplacer
    [box-on] Boîte de coupe activée : faites glisser la poignée d’une face pour la déplacer
    [box-off] Boîte de coupe désactivée
    [cleared] Coupes retirées
    [measure-on] Cliquez deux points sur la coupe
    [measure-needs-cut] Ajoutez d’abord un plan de coupe ou la boîte
    [measure-off-cut] Cliquez sur la coupe
    [measure-hint] Mesure : cliquez deux points sur la coupe
    [ask-view-name] Nom de la vue
    [ask-rename] Nouveau nom
    [views-empty] Aucune vue enregistrée. Appuyez sur N pour enregistrer celle-ci.
    [views-hints] Enter aller · N enregistrer · F2 renommer · Delete retirer · P enregistrer en PNG
    [delete-detail] Seule la vue enregistrée est retirée. Le modèle n’est pas modifié.
    [delete-ok] Retirer
    [not-over-model] Choisissez un autre fichier : celui-ci remplacerait le modèle.
   *[view-name] Vue
}

# Textes courts BCF (BIM Collaboration Format), selon $what.
bcf-note = { $what ->
    [title-none] Sujets BCF
    [new-file] nouveau
    [empty] Aucun sujet. Appuyez sur O pour ouvrir un fichier BCF ou sur N pour ajouter un sujet.
    [hints] Enter aller · ← → points de vue · N sujet · C commentaire · S statut · E titre · O ouvrir · Ctrl+S exporter
    [ask-title] Titre du sujet
    [ask-comment] Commentaire
    [untitled] Sans titre
    [topic-added] Sujet ajouté avec cette vue
   *[comment-added] Commentaire ajouté avec cette vue
}

# $planes : nombre de plans de coupe.
bim-sections-line = { $planes ->
    [0] Aucun plan de coupe
    [one] { $planes } plan de coupe
   *[other] { $planes } plans de coupe
}
# $size : taille de la boîte de coupe, avec son unité.
bim-box-line = boîte { $size }
# $count : le nombre maximal de plans.
bim-planes-full = Au plus { $count } plans de coupe
# Une poignée à faire glisser, pour les lecteurs d’écran. $n : son numéro.
bim-handle = { $kind ->
    [box] Face { $n } de la boîte de coupe
   *[plane] Plan de coupe { $n }
}
# $length : la longueur avec son unité.
bim-measure-line = Longueur sur la coupe : { $length }
bim-views-title = { $count ->
    [one] { $count } vue enregistrée
   *[other] { $count } vues enregistrées
}
# $name : nom d’une vue.
bim-view-saved = « { $name } » enregistrée
bim-view-delete-title = Retirer la vue « { $name } » ?
# $file : un fichier écrit.
bim-saved-file = { $file } enregistré
# $reason : la cause de l’échec.
bim-failed = { $what ->
    [open] Impossible d’ouvrir le fichier BCF : { $reason }
    [views] Vues gardées pour cette session seulement : { $reason }
   *[save] Impossible d’enregistrer : { $reason }
}
# $count : sujets ; $file : nom du fichier de l’archive.
bcf-title = { $count ->
    [one] { $count } sujet
   *[other] { $count } sujets
} · { $file }
bcf-comments = { $count ->
    [one] { $count } commentaire
   *[other] { $count } commentaires
}
# $at : le point de vue affiché ; $count : les points de vue du sujet.
bcf-viewpoint-of = Point de vue { $at } sur { $count }
bcf-opened = { $count ->
    [one] { $count } sujet ouvert
   *[other] { $count } sujets ouverts
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modèles : 1
   *[other] Modèles : { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Ce fichier n’est pas un modèle IFC.
    [not-ifc-open] Ouvrez d’abord un modèle IFC. Seuls les modèles IFC peuvent être combinés.
    [this-model] Ce modèle
    [hints] A ajouter un modèle · Shift+A ajouter un dossier · Space afficher ou masquer · C couleur par modèle · Enter aller · Delete retirer
    [origin-ok] Placer à l’origine
    [first-stays] Le modèle ouvert reste. Ouvrez un autre fichier pour le changer.
    [remove-detail] Il quitte la scène. Le fichier sur le disque est conservé.
   *[remove-ok] Retirer
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 élément
   *[other] { $elements } éléments
} · { $shown ->
    [hidden] masqué
   *[shown] affiché
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] modèle ouvert
    [map-crs] par coordonnées cartographiques ({ $crs })
    [map] par coordonnées cartographiques
    [site] par latitude et longitude du site (approximatif)
    [project] par coordonnées de projet partagées
   *[origin] à l’origine (non aligné)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Les modèles utilisent des systèmes de coordonnées différents : { $first } et { $this }.
    [only-first] Le modèle ouvert est géoréférencé, celui-ci ne l’est pas.
    [only-this] Ce modèle est géoréférencé, le modèle ouvert ne l’est pas.
   *[broken] La conversion cartographique de ce modèle est inutilisable (son échelle est nulle).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Déplacé de { $distance }, tourné de { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Lecture d’un modèle…
   *[other] Lecture de { $count } modèles…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } ne peut pas être aligné. Le placer à l’origine ?
# machine-drafted, needs native review
fed-no-files = Aucun fichier IFC dans { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } n’a pas été ajouté : { $reason }
# machine-drafted, needs native review
fed-added = { $file } ajouté. Modèles dans la scène : { $count }.
# machine-drafted, needs native review
fed-remove-title = Retirer { $file } de la scène ?
# machine-drafted, needs native review
fed-removed = { $file } retiré de la scène.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] masqué
   *[shown] affiché
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Couleur par modèle activée
   *[off] Couleur par modèle désactivée
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI et vues enregistrées
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 annotation
   *[other] { $count } annotations
}
# machine-drafted, needs native review
pmi-view = Vue enregistrée
# machine-drafted, needs native review
pmi-view-all = affiche toutes les annotations
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] affiche 1 annotation
   *[other] affiche { $count } annotations
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Vue « { $name } » affichée. A affiche tout.
# machine-drafted, needs native review
pmi-has-faces = faces mises en évidence
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Aucune PMI dans ce modèle.
    [hints] ↑ ↓ choisir · Enter aller · A tout · D tel que dessiné · Shift+A masquer
    [hidden] Annotations masquées
    [shown] Annotations affichées
    [as-drawn] Annotations telles que le fichier les dessine
    [as-labels] Annotations en étiquettes
   *[all-shown] Toutes les annotations affichées
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Cote
    [tolerance] Tolérance géométrique
    [datum] Référence spécifiée
    [target] Référence partielle
   *[graphic] Annotation dessinée
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Distance
    [angular] Angle
    [diameter] Diamètre
    [radius] Rayon
    [length] Longueur
    [thickness] Épaisseur
   *[other] Cote
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Rectitude
    [flatness] Planéité
    [circularity] Circularité
    [cylindricity] Cylindricité
    [line-profile] Forme d’une ligne
    [surface-profile] Forme d’une surface
    [angularity] Inclinaison
    [perpendicularity] Perpendicularité
    [parallelism] Parallélisme
    [position] Localisation
    [concentricity] Concentricité
    [coaxiality] Coaxialité
    [symmetry] Symétrie
    [circular-runout] Battement circulaire
   *[total-runout] Battement total
}
