# Comptes rendus structurés DICOM, tracés, dose de radiothérapie et tomographie ophtalmique. machine-drafted, needs native review

## Arbre de contenu du compte rendu structuré (le navigateur de balises en mode compte rendu)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Élément
sr-col-concept = Concept
sr-col-type = Type
sr-col-relationship = Relation
sr-col-value = Valeur
sr-show-image = Afficher l’image
sr-jump-missing = L’image référencée ne se trouve pas dans ce dossier
sr-jump-frame = Aucune image de ce référentiel ne contient ce point

## Graphiques (histogrammes dose-volume, tracés ECG)
plot-caliper-time = { $ms } ms · { $rate } /min
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Faites glisser sur un graphique pour mesurer
plot-adjust-hint = + et − changent le gain, [ et ] la vitesse

## Tracé ECG
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] { $count } autre groupe de voies n’est pas affiché
    [many] { $count } autres groupes de voies ne sont pas affichés
   *[other] { $count } autres groupes de voies ne sont pas affichés
}

## Dose de radiothérapie sur la MPR (isodoses, carte de couleurs, légende, valeur lue)
dose-value = Dose { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Dose masquée (Shift+U l’affiche)
dose-on-rx = Dose : { $dose } { $unit } prescrite · max { $max } { $unit }
dose-on-max = Dose : pourcentage du maximum, { $max } { $unit }
dose-colour = Dose en { $colormap } à { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K : l’histogramme dose-volume d’une structure
    [many] Shift+K : les histogrammes dose-volume de { $count } structures
   *[other] Shift+K : les histogrammes dose-volume de { $count } structures
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Dose en courbes isodoses
dose-mode-colour = Dose en carte de couleurs

## Histogrammes dose-volume (la feuille de graphique)
dvh-title = Histogrammes dose-volume
dvh-col-roi = Structure
dvh-col-volume = Volume
dvh-axis-dose = Dose ({ $unit })
dvh-axis-volume = Volume (%)
dvh-prescription = Prescription { $dose } Gy · cumulatif
dvh-max = Dose maximale { $dose } { $unit } · cumulatif
dvh-none = Aucun jeu de structures de ce référentiel sur lequel calculer l’histogramme de dose

## Tomographie ophtalmique (le curseur des B-scans ; l’image de repérage dans un coin)
oct-bscan = B-scan
oct-depth = { $depth } µm de profondeur
