# machine-drafted, needs native review
# Photographer polish pass: Develop regions, crop rows, import choices, keyword rename and rules.

palette-develop-next-region = Zone suivante
palette-develop-next-region-help = Avancer le clavier : image, panneau, film fixe.
palette-develop-prev-region = Zone précédente
palette-develop-prev-region-help = Reculer le clavier : film fixe, panneau, image.
develop-region-picture = Image
develop-region-panel = Panneau

develop-crop-aspect-row = Format
develop-crop-aspect-option-free = Libre
develop-crop-aspect-option-original = D’origine
develop-crop-aspect-option-custom = Personnalisé
develop-crop-type-ratio = Saisir un format…
develop-crop-swap = Permuter portrait et paysage
develop-crop-turn-left = Pivoter à gauche
develop-crop-turn-right = Pivoter à droite
develop-crop-flip-horizontal = Retourner de gauche à droite
develop-crop-flip-vertical = Retourner de haut en bas
develop-crop-ratio-title = Format du recadrage
develop-crop-ratio-field = Largeur:hauteur
develop-crop-ratio-ok = Valider
develop-crop-ratio-bad = Saisissez un format comme 5:4, ou un seul nombre comme 1.85.

import-template = Modèle de métadonnées
import-style = Style de Développement
import-choice-none = Aucun
import-choice-said = { $label } : { $value }
import-keywords = Mots-clés
import-keywords-hint = Par exemple Famille; Lieux > Paris
import-style-missing = Le style { $name } n’existe plus. Les photos ont été importées sans lui.

keywords-rename = Renommer…
keywords-move = Déplacer vers…
keywords-rename-of = Nouveau nom pour { $keyword }
keywords-move-of = Placer { $keyword } sous (vide : tout en haut)
keywords-move-into-itself = { $keyword } ne peut pas être placé sous lui-même.
keywords-renaming = Renommage de { $from } en { $to }…
keywords-merging = Fusion de { $from } dans { $to }…
keywords-renamed = { $from } devient { $to } sur { $count ->
    [one] { $count } photo
   *[other] { $count } photos
}
keywords-rename-row = Mot-clé { $from } vers { $to }
keywords-rename-finding = Recherche des photos…
keywords-rename-row-detail = { $done } sur { $total } photos

develop-split-moved = Séparateur à { PERCENT($percent) }

develop-rules-placeholder-edit = Choisissez une règle pour la modifier ou la supprimer
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = Fabricant : { $value }
develop-rule-model = Modèle : { $value }
develop-rule-lens = Objectif : { $value }
develop-rule-iso = ISO : { $value }
develop-rule-type = Type de fichier : { $value }
develop-rule-style = Style : { $value }
develop-rule-any = tous
develop-rule-type-any = tout fichier
develop-rule-delete = Supprimer la règle…
develop-rule-field-title = Modifier « { $rule } »
develop-rule-field-maker = Fabricant (une partie du nom suffit ; vide : tous)
develop-rule-field-model = Modèle (une partie du nom suffit ; vide : tous)
develop-rule-field-lens = Objectif (une partie du nom suffit ; vide : tous)
develop-rule-field-iso = ISO, comme 100-800, 1600- ou -400 (vide : tous)
develop-rule-field-ok = Valider
develop-rule-iso-bad = Saisissez une plage ISO comme 100-800.
develop-rule-style-placeholder = Choisissez le style de la règle
