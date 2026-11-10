# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = Afficher les balises DICOM
palette-next-series = Série suivante
palette-prev-series = Série précédente
disabled-not-dicom = L’image n’est pas un fichier DICOM
disabled-no-other-series = Ce dossier ne contient pas d’autre série
dicom-tags-title = Balises DICOM
dicom-tags-title-file = Balises DICOM: { $file }
dicom-tags-filter = Filtrer les balises
dicom-tags-show-patient = Afficher les données patient
dicom-tags-col-tag = Balise
dicom-tags-col-keyword = Mot-clé
dicom-tags-col-vr = VR
dicom-tags-col-length = Longueur
dicom-tags-col-value = Valeur
dicom-tags-hidden = Masqué
dicom-tags-hidden-count = { $count ->
    [one] { $count } valeur patient masquée
   *[other] { $count } valeurs patient masquées
}
dicom-tags-no-match = Aucune balise ne correspond
dicom-tags-reading = Lecture des balises…
dicom-tags-error = Impossible de lire les balises: { $reason }
dicom-tags-items = { $count ->
    [one] { $count } élément
   *[other] { $count } éléments
}
dicom-tags-fragments = { $count ->
    [one] { $count } fragment
   *[other] { $count } fragments
}
dicom-tags-item = Élément { $index }
dicom-tags-bytes = { $count ->
    [one] { $count } octet
   *[other] { $count } octets
}
dicom-tags-empty = (vide)
dicom-tags-omitted = { $count ->
    [one] { $count } autre élément non listé
   *[other] { $count } autres éléments non listés
}
dicom-series-group = Série
dicom-series-slice = Coupe
dicom-series-slice-value = { $index } sur { $count }
dicom-series-frames = { $frames ->
        [one] { $frames } trame
       *[other] { $frames } trames
    }
dicom-series-location = Position de coupe
dicom-series-mm = { $mm } mm
dicom-series-spacing = Espacement des coupes
dicom-series-which = Série
dicom-series-which-value = { $index } sur { $count }
dicom-series-order = Ordre
dicom-series-order-position = Par position
dicom-series-order-instance = Par numéro d’instance
dicom-series-order-time = Par heure d’acquisition
dicom-series-order-name = Par nom de fichier
dicom-series-gaps = Coupes manquantes
dicom-series-gaps-value = { $count ->
    [one] { $count } coupe manquante
   *[other] { $count } coupes manquantes
}
dicom-series-gaps-where = Après la coupe { $slices }
dicom-series-irregular = Espacement irrégulier
dicom-series-tilt = Inclinaison du statif
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = Doublons ignorés
dicom-series-toast = { $series ->
    [one] Série – coupes : { $count }
   *[other] Série { $index } sur { $series } – coupes : { $count }
}
dicom-series-capped = Seuls les { $count } premiers fichiers du dossier ont été lus

studies-title = Études
studies-title-medium = Études : { $medium }
studies-show-patient = Afficher les données du patient
studies-patient-hidden = Masqué
studies-patient-hidden-note = Les noms et dates de naissance des patients sont masqués
studies-none = Aucune étude trouvée
studies-reading = Lecture du disque…
studies-reading-count = Lecture du disque : { $done } sur { $total ->
        [one] { $total } fichier
       *[other] { $total } fichiers
    }
studies-opening = Ouverture de la série…
studies-error = Impossible de lire le disque : { $reason }
studies-missing = { $count ->
    [one] { $count } fichier listé sur le disque est manquant
   *[other] { $count } fichiers listés sur le disque sont manquants
}
studies-capped = Seuls les { $count } premiers fichiers ont été lus
# machine-drafted, needs native review
studies-stopped = Lecture arrêtée après { $done } sur { $total ->
        [one] { $total } fichier
       *[other] { $total } fichiers
    } : la liste est incomplète
studies-stopped-early = Lecture arrêtée : la liste est incomplète
studies-cancel-description = Arrête la lecture du disque. Ce qui a déjà été lu reste listé.
studies-patient-n = Patient { $index }
studies-patient-unknown = Patient inconnu
studies-born = Né(e) le { $date }
studies-no-description = Aucune description
studies-series = Série { $number } : { $description }
studies-series-number = Série { $number }
studies-images = { $count ->
    [one] { $count } image
   *[other] { $count } images
}
studies-images-missing = { $count } manquantes
studies-others = { $count ->
    [one] { $count } autre objet
   *[other] { $count } autres objets
}
studies-no-images = Cette série n’a aucune image sur le disque
studies-medium-inserted = Un disque contenant des études médicales a été inséré
studies-medium-show = Afficher les études
palette-studies = Afficher les études de ce disque
