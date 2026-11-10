# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

palette-export-anonymised = Exporter une copie anonymisée…
palette-export-patient-disc = Exporter un dossier de disque patient…
palette-mask-burned-in-text = Masquer le texte incrusté
anon-title = Exporter une copie anonymisée
anon-disc-title = Exporter un dossier de disque patient
anon-images = Images
anon-images-current = Cette image
anon-images-series = { $count ->
    [one] Cette série ({ $count } fichier)
   *[other] Cette série ({ $count } fichiers)
}
anon-images-study = { $count ->
    [one] Cette étude ({ $count } fichier)
   *[other] Cette étude ({ $count } fichiers)
}
anon-images-reading = Lecture du dossier…
anon-anonymise = Anonymiser
anon-dates = Dates et heures
anon-dates-remove = Supprimer
anon-dates-keep = Conserver
anon-dates-shift = Décaler
anon-patient = Caractéristiques du patient
anon-device = Identité de l’appareil
anon-institution = Identité de l’établissement
anon-descriptions = Descriptions
anon-descriptions-remove = Supprimer
anon-descriptions-clean = Nettoyer
anon-structured = Contenu des comptes rendus
anon-uids = UID
anon-uids-replace = Remplacer
anon-uids-keep = Conserver
anon-overlays = Overlays et courbes
anon-keep = Conserver
anon-remove = Supprimer
anon-on = Activé
anon-off = Désactivé
anon-burned-in = Texte incrusté
anon-masks = { $count ->
    [one] { $count } zone masquée
   *[other] { $count } zones masquées
}
anon-masks-none = Aucun masque
anon-folder = Dossier
anon-folder-choose = Choisir…
anon-export = Exporter
anon-risk-present = Ces images indiquent contenir du texte patient : masquez-le avant l’export.
anon-risk-possible = Les images de ce type montrent souvent du texte patient : vérifiez avant l’export.
anon-progress = Export de { $done } sur { $total }…
anon-stopping = Arrêt…
anon-mask-title = Masquer le texte incrusté
anon-mask-hint = Faites glisser sur le texte pour le masquer. Les zones masquées deviennent noires dans chaque image.
anon-mask-suggest = Suggérer
anon-mask-remove = Supprimer
anon-mask-clear = Effacer
anon-mask-done = Terminé
anon-mask-area = Zone { $n }, { $width } × { $height } pixels
anon-mask-suggested = { $count ->
    [0] Aucun texte trouvé près des bords
    [one] { $count } zone de texte suggérée
   *[other] { $count } zones de texte suggérées
}
anon-done = { $count ->
    [one] { $count } fichier anonymisé écrit dans { $folder }
   *[other] { $count } fichiers anonymisés écrits dans { $folder }
}
anon-disc-done = { $count ->
    [one] Dossier de disque patient avec { $count } image écrit dans { $folder }
   *[other] Dossier de disque patient avec { $count } images écrit dans { $folder }
}
anon-skipped-burned-in = { $count ->
    [one] { $count } fichier avec texte incrusté et sans masque a été ignoré
   *[other] { $count } fichiers avec texte incrusté et sans masque ont été ignorés
}
anon-skipped-failed = { $count ->
    [one] { $count } fichier n’a pas pu être exporté
   *[other] { $count } fichiers n’ont pas pu être exportés
}
anon-stopped = Export arrêté
anon-failed = Impossible d’exporter : { $reason }
anon-not-empty = Le dossier contient déjà un disque patient
