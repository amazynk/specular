# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Français. machine-drafted, needs native review

palette-omero-servers = Serveurs OMERO…

omero-title = Serveurs OMERO
omero-servers-intro = Les projets, criblages et images d’un serveur ne sont listés que lorsque vous le parcourez. Rien n’est envoyé avant.
omero-servers = Serveurs
omero-servers-empty = Aucun serveur pour l’instant. Ajoutez-en un avec son adresse OMERO.web.
omero-session-only = Les mots de passe ne sont conservés que jusqu’à la fermeture de Specular.
omero-add = Ajouter…
omero-edit = Modifier…
omero-remove = Supprimer
omero-close = Fermer
omero-browse = Parcourir
omero-back = Retour
omero-save = Enregistrer
omero-stop = Arrêter
omero-open = Ouvrir
# $server a server's name.
omero-removed = { $server } supprimé
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Non chiffré
# A server browsed without signing in.
omero-public = Public
# $user the user name a server is signed in with.
omero-signed-in-as = En tant que { $user }

# The page that adds or edits a server.
omero-add-title = Ajouter un serveur
omero-edit-title = Réglages du serveur
omero-field-name = Nom
omero-hint-name = Facultatif
omero-field-url = Adresse
omero-hint-url = https://omero.example.org
omero-field-user = Nom d’utilisateur
omero-hint-user = Vide pour les données publiques
omero-field-password = Mot de passe
omero-hint-password-kept = Laisser vide pour garder celui déjà donné
omero-password-session = Le mot de passe n’est conservé que jusqu’à la fermeture de Specular, jamais sur le disque.
omero-password-needed = Saisissez le mot de passe : il n’est conservé que jusqu’à la fermeture de Specular.
omero-http-warning = Cette adresse n’est pas chiffrée : le mot de passe serait envoyé en clair.
omero-url-invalid = Saisissez l’adresse OMERO.web, commençant par https:// ou http://.
# $reason why the list could not be saved.
omero-save-failed = La liste des serveurs n’a pas pu être enregistrée : { $reason }

# The browse page.
omero-browse-title = Parcourir
omero-top = Projets et criblages
omero-datasets = Jeux de données
omero-plates = Plaques
omero-wells = Puits
omero-fields = Champs
omero-images = Images
omero-kind-project = Projet
omero-kind-dataset = Jeu de données
omero-kind-image = Image
omero-kind-screen = Criblage
omero-kind-plate = Plaque
omero-kind-well = Puits
# $well a well's row and column, as B7.
omero-well = Puits { $well }
omero-well-fields = { $count ->
    [one] Un champ
   *[other] { $count } champs
}
omero-children = { $count ->
    [one] Un élément
   *[other] { $count } éléments
}
omero-load-more = Charger plus
omero-loading = Chargement…
omero-empty = Rien ici
omero-stopped = Arrêté
omero-found = { $count ->
    [one] Un élément
   *[other] { $count } éléments
}
# $reason why the image could not be opened.
omero-open-failed = L’image n’a pas pu être ouverte : { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Au moins un canal reste affiché
# $server a server's name.
omero-session-ended = La session sur { $server } a pris fin. Saisissez le mot de passe pour continuer.

# Errors.
# $server a server's name.
omero-error-offline = { $server } est injoignable.
omero-error-sign-in = Ce serveur demande une connexion.
omero-error-expired = La session a pris fin.
omero-error-login = Le nom d’utilisateur ou le mot de passe a été refusé.
omero-error-forbidden = Votre compte n’a peut-être pas accès à ceci.
omero-error-not-found = Introuvable sur le serveur (il a peut-être été supprimé).
# $code an HTTP status code.
omero-error-status = Le serveur a répondu avec l’erreur { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = La réponse du serveur n’a pas pu être lue : { $reason }
