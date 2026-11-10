# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Conflits
    [changes] Modifications
   *[quantities] Quantités
}
# $tab: the tab's name.
checks-title = Contrôles du modèle · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Les contrôles du modèle demandent un modèle IFC.
    [reading] Lecture des informations du modèle…
    [ids-empty] Aucun fichier IDS. Appuyez sur O pour en ouvrir un.
    [ids-checking] Contrôle…
    [ids-no-failures] Aucun échec.
    [ids-hints] Enter échecs, aller · Backspace retour · O ouvrir · P propriétés · Ctrl+S BCF · Ctrl+E CSV · Tab suivant
    [unnamed] Spécification sans nom
    [clash-running] Recherche des conflits…
    [clash-empty] Appuyez sur R pour rechercher les conflits.
    [clash-none] Aucun conflit.
    [clashes-hints] R lancer · A B ensembles · M dur ou dégagement · + − distance · Enter aller · S statut · Ctrl+S BCF · Ctrl+E CSV · Tab suivant
    [changes-reading] Lecture de l’autre version…
    [changes-empty] Appuyez sur O pour choisir une autre version de ce modèle.
    [changes-none] Aucune modification.
    [changes-hints] O autre version · F filtre · Enter aller · P propriétés · Ctrl+S BCF · Ctrl+E CSV · Tab suivant
    [quantities-hints] G grouper · Enter aller · P propriétés · Ctrl+E CSV · Ctrl+S XLSX · Tab suivant
   *[not-drawn] Cet élément n’a pas de géométrie à afficher.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Classe
    [attribute] Attribut { $name }
    [classification] Classification
    [material] Matériau
   *[partof] Partie de { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] La classe est { $actual }
    [predefined] Le type prédéfini est { $actual }
    [missing] { $what } manque
    [empty] { $what } : vide
    [value] { $what } vaut { $actual } : non autorisé
    [datatype] { $what } a le type de données { $actual }
    [system] { $what } : le système est { $actual }
    [whole] { $what } : partie de { $actual }
    [prohibited] { $what } ne doit pas être présent
   *[unsupported] { $what } ne peut pas être contrôlé
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Étage : { $value }
    [type] Type : { $value }
    [material] Matériau : { $value }
    [container] Conteneur : { $value }
   *[classification] Classification : { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file } : { $ok } sur { $count } respectées

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } conformes
    [fail] ✗ { $failed } sur { $applicable } en échec
    [none] ✗ ne s’applique à rien
    [none-ok] ✓ ne s’applique à rien
    [prohibited] ✗ { $applicable } qui ne doivent pas exister
   *[prohibited-ok] ✓ aucun qui ne doive pas exister
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] tous les éléments
    [class] { $name }
   *[storey] étage { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A : { $a } · B : { $b } · { $mode ->
    [hard] dur, plus profond que { $distance }
   *[clearance] plus proche que { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] { $count } conflit
    [many] { $count } conflits
   *[other] { $count } conflits
}, nouveaux : { $new }, résolus : { $resolved } · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] dur { $depth }
   *[clearance] dégagement { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Dur
   *[clearance] Dégagement
}
checks-status = { $status ->
    [new] nouveau
    [active] actif
   *[resolved] résolu
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Conflit : { $a } × { $b }
checks-clash-done = { $count ->
    [0] Aucun conflit
    [one] { $count } conflit
    [many] { $count } conflits
   *[other] { $count } conflits
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file } : ajoutés { $added } · retirés { $removed } · modifiés { $changed }{ $filter ->
    [added] {" "}· ajoutés affichés
    [removed] {" "}· retirés affichés
    [changed] {" "}· modifiés affichés
   *[all] {""}
}
checks-change = { $kind ->
    [added] Ajouté
    [removed] Retiré
   *[geometry] géométrie
}
checks-change-fields = { $count ->
    [one] { $count } champ
    [many] { $count } champs
   *[other] { $count } champs
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name } : { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Déplacé de { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } élément
       *[other] { $count } éléments
    } · { $area } · { $volume } · { $by ->
    [element] par élément
    [class] par classe
    [storey] par étage
   *[type] par type
}
# Where quantities came from.
checks-source = { $source ->
    [file] du fichier
    [computed] calculé
   *[mixed] en partie calculé
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Éléments
    [class] Par classe
    [storey] Par étage
   *[type] Par type
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Spécification
    [guid] GlobalId
    [class] Classe
    [name] Nom
    [reason] Raison
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Genre
    [depth] Profondeur ou distance (m)
    [status] Statut
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Modification
    [field] Champ
    [old] Ancien
    [new] Nouveau
    [type] Type
    [container] Conteneur
    [material] Matériau
    [classification] Classification
    [storey] Étage
    [count] Nombre
    [length] Longueur (m)
    [area] Surface (m²)
    [volume] Volume (m³)
   *[source] Source
}

# $reason: why, in plain words.
checks-error = Impossible : { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Contrôlé : toutes les spécifications sont respectées
    [one] Contrôlé : { $failed } spécification n’est pas respectée
    [many] Contrôlé : { $failed } spécifications ne sont pas respectées
   *[other] Contrôlé : { $failed } spécifications ne sont pas respectées
}
# $file: the file written.
checks-saved = { $file } enregistré

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = tous les modèles
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Modèle : { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M modèle · Enter échecs, aller · Backspace retour · O ouvrir · P propriétés · Ctrl+S BCF · Ctrl+E CSV · Tab suivant
    [clashes] R lancer · A B ensembles · G H leurs modèles · M dur ou dégagement · + − distance · Enter aller · S statut · Ctrl+S BCF · Ctrl+E CSV · Tab suivant
    [changes] M modèle · O autre version · F filtre · Enter aller · P propriétés · Ctrl+S BCF · Ctrl+E CSV · Tab suivant
   *[quantities] M modèle · G grouper · Enter aller · P propriétés · Ctrl+E CSV · Ctrl+S XLSX · Tab suivant
}
