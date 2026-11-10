# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Modifier les métadonnées
palette-edit-metadata-help = Modifier le titre, la légende, le créateur, les droits et les lieux des photos sélectionnées.
metaedit-menu = Modifier les métadonnées…
metaedit-title = Métadonnées
metaedit-close = Fermer les métadonnées
metaedit-photos = { $count ->
    [one] { $count } photo
   *[other] { $count } photos
}
metaedit-none-selected = Sélectionnez des photos pour modifier leurs métadonnées.
metaedit-reading = Lecture des métadonnées…
metaedit-mixed = Mixte : saisir pour appliquer à toutes
metaedit-clearing = Effacé sur toutes les photos. Esc les conserve.
metaedit-keywords-others = certaines en ont plus
metaedit-saved = { $field } enregistré.
metaedit-cleared = { $field } effacé.
metaedit-write-failed = { $count ->
    [one] Métadonnées non enregistrées pour { $count } photo : { $error }
   *[other] Métadonnées non enregistrées pour { $count } photos : { $error }
}
metaedit-writes-sidecars = Enregistré dans des fichiers annexes XMP.
metaedit-writes-files = Enregistré dans les fichiers JPEG, TIFF et DNG ; les autres fichiers reçoivent des fichiers annexes XMP.

metaedit-group-templates = Modèles
metaedit-group-description = Descriptif
metaedit-group-creator = Créateur
metaedit-group-rights = Droits
metaedit-group-shown = Lieu représenté
metaedit-group-created = Lieu de prise de vue
metaedit-group-workflow = Flux de travail
metaedit-templates-none = Aucun modèle pour l’instant. Un modèle remplit les champs que vous utilisez souvent.

metaedit-field-title = Titre
metaedit-field-headline = Titre de l’article
metaedit-field-description = Légende
metaedit-field-keywords = Mots-clés (séparés par ;)
metaedit-field-persons-shown = Personnes représentées (séparées par ;)
metaedit-field-event = Évènement
metaedit-field-creator = Créateur (séparés par ;)
metaedit-field-creator-job-title = Fonction du créateur
metaedit-field-contact-address = Adresse
metaedit-field-contact-city = Ville
metaedit-field-contact-region = État ou région
metaedit-field-contact-postal-code = Code postal
metaedit-field-contact-country = Pays
metaedit-field-contact-phone = Téléphone
metaedit-field-contact-email = E-mail
metaedit-field-contact-url = Site web
metaedit-field-copyright = Mention de copyright
metaedit-field-credit = Ligne de crédit
metaedit-field-source = Provenance
metaedit-field-usage-terms = Conditions d’utilisation
metaedit-field-shown-sublocation = Lieu précis
metaedit-field-shown-city = Ville
metaedit-field-shown-state = État ou région
metaedit-field-shown-country = Pays
metaedit-field-shown-country-code = Code pays
metaedit-field-created-sublocation = Lieu précis
metaedit-field-created-city = Ville
metaedit-field-created-state = État ou région
metaedit-field-created-country = Pays
metaedit-field-created-country-code = Code pays
metaedit-field-instructions = Consignes
metaedit-field-job-id = Identifiant de la tâche

metaedit-template-new = Nouveau modèle
metaedit-template-edit = Modifier
metaedit-template-remove = Retirer
metaedit-template-done = Terminé
metaedit-template-name = Nom du modèle
metaedit-template-default-name = Modèle { $n }
metaedit-template-title = Modèle
metaedit-template-hint = Les champs vides ne sont pas modifiés.
metaedit-template-replace-keywords = Remplacer les mots-clés des photos
metaedit-template-applied = { $count ->
    [one] « { $name } » appliqué à { $count } photo.
   *[other] « { $name } » appliqué à { $count } photos.
}
metaedit-template-remove-ask = Retirer le modèle « { $name } » ?
metaedit-template-remove-keep = Les photos gardent leurs métadonnées.
metaedit-template-remove-yes = Retirer
metaedit-template-removed = Modèle « { $name } » retiré.

app-info-edit-metadata = Modifier les métadonnées…

settings-gallery-metadata-into-files = Écrire les métadonnées dans les fichiers image
settings-gallery-metadata-into-files-help = L’éditeur de métadonnées écrit aussi dans les fichiers JPEG, TIFF et DNG, sans rien y changer d’autre. Désactivé : fichiers annexes XMP uniquement.
