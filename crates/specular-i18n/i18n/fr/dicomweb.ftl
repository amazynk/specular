# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Français.

palette-dicom-servers = Serveurs DICOMweb…

dicomweb-title = Serveurs DICOMweb
dicomweb-servers-intro = Les études d’un serveur ne sont recherchés et ouverts qu’à votre demande. Rien n’est envoyé avant.
dicomweb-servers = Serveurs
dicomweb-servers-empty = Aucun serveur pour l’instant. Ajoutez-en un avec son adresse DICOMweb.
dicomweb-session-only = Les mots de passe et jetons ne sont conservés que jusqu’à la fermeture de Specular.
dicomweb-add = Ajouter…
dicomweb-edit = Modifier…
dicomweb-remove = Supprimer
dicomweb-close = Fermer
dicomweb-browse = Parcourir
dicomweb-back = Retour
dicomweb-save = Enregistrer
dicomweb-search = Rechercher
dicomweb-stop = Arrêter
dicomweb-removed = { $server } supprimé
# machine-drafted, needs native review
dicomweb-remove-ask = Retirer « { $server } » des serveurs enregistrés ?
dicomweb-remove-detail = Rien n’est perdu sur le disque. Son mot de passe ou jeton pour cette session est oublié.
# Serveurs joints sans chiffrement (http://).
dicomweb-not-encrypted = Non chiffré

# Page d’ajout ou de modification d’un serveur.
dicomweb-add-title = Ajouter un serveur
dicomweb-edit-title = Réglages du serveur
dicomweb-field-name = Nom
dicomweb-hint-name = Facultatif
dicomweb-field-url = Adresse
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Connexion
dicomweb-auth-none = Sans connexion
dicomweb-auth-basic = Nom d’utilisateur et mot de passe
dicomweb-auth-bearer = Jeton d’accès
dicomweb-field-user = Nom d’utilisateur
dicomweb-field-password = Mot de passe
dicomweb-field-token = Jeton d’accès
dicomweb-hint-secret-kept = Laisser vide pour conserver la valeur actuelle
dicomweb-secret-session = Le mot de passe ou le jeton n’est conservé que jusqu’à la fermeture de Specular, jamais sur le disque.
dicomweb-secret-needed = Saisissez à nouveau le mot de passe ou le jeton : il n’est conservé que jusqu’à la fermeture de Specular.
dicomweb-http-warning = Cette adresse n’est pas chiffrée : le mot de passe ou le jeton serait envoyé en clair.
dicomweb-url-empty = Saisissez l’adresse DICOMweb du serveur.
dicomweb-url-scheme = L’adresse doit commencer par https:// ou http://.
dicomweb-url-host = L’adresse ne contient aucun nom de serveur.
dicomweb-url-userinfo = Saisissez le nom d’utilisateur et le mot de passe dans les champs de connexion, pas dans l’adresse.
dicomweb-save-failed = La liste des serveurs n’a pas pu être enregistrée : { $reason }

# Page de recherche.
dicomweb-search-title = Rechercher des études
dicomweb-field-patient-name = Nom du patient
dicomweb-field-patient-id = ID patient
dicomweb-field-date-from = Du
dicomweb-field-date-to = Au
dicomweb-hint-date = AAAAMMJJ
dicomweb-field-modality = Modalité
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Numéro d’accès
dicomweb-show-patient = Afficher les données patient
dicomweb-switch-on = Activé
dicomweb-switch-off = Désactivé
dicomweb-patient-shown = Données patient affichées
dicomweb-patient-hidden = Données patient masquées
dicomweb-studies = Études
dicomweb-search-hint = Remplissez l’un des champs et lancez la recherche.
dicomweb-no-studies = Aucune étude ne correspond.
dicomweb-searching = Recherche…
dicomweb-stopped = Arrêté
dicomweb-load-more = Charger plus
dicomweb-study-untitled = Étude sans description
dicomweb-series-untitled = Série sans description
dicomweb-study-counts = { $series ->
    [one] Une série
   *[other] { $series } séries
}, { $images ->
    [one] une image
   *[other] { $images } images
}
dicomweb-series-images = { $count ->
    [one] Une image
   *[other] { $count } images
}
dicomweb-patient-id = ID { $id }
dicomweb-accession = N° d’accès { $number }
dicomweb-found = { $count ->
    [one] Une étude trouvée
   *[other] { $count } études trouvées
}
dicomweb-series-found = { $count ->
    [one] Une série
   *[other] { $count } séries
}

# Erreurs de communication avec un serveur.
dicomweb-error-offline = { $server } est injoignable. Vérifiez le réseau et l’adresse.
dicomweb-error-auth = Le serveur a refusé le nom d’utilisateur, le mot de passe ou le jeton.
dicomweb-error-not-found = Il n’y a aucun service DICOMweb à cette adresse.
dicomweb-error-other = Le serveur n’a pas pu répondre : { $reason }

# Série chargée en continu dans la visionneuse.
dicomweb-loading = Chargement de { $series }…
dicomweb-loading-progress = Chargement de { $series } : { $done } sur { $total }
dicomweb-loaded = { $series } : { $total ->
    [one] une image chargée
   *[other] les { $total } images chargées
}
dicomweb-loaded-failed = { $failed } images sur { $total } n’ont pas pu être chargées.
dicomweb-offline-cached = { $server } est injoignable : affichage des images enregistrées précédemment.
dicomweb-cache-failed = Le dossier de téléchargement n’a pas pu être ouvert : { $reason }
