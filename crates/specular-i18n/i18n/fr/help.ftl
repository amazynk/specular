# machine-drafted, needs native review
## Le panneau du manuel, les rapports de plantage, le paquet de diagnostic, les mises à jour et À propos. Français.
## Style : court, simple, sans remplissage ; une idée par phrase.

## Palette

palette-help-manual = Aide
palette-help-manual-help = Ouvrir le manuel à la page de ce qui est à l’écran.
palette-save-diagnostics = Enregistrer un paquet de diagnostic…
palette-save-diagnostics-help = Enregistrer les rapports de plantage, le journal et les détails du système dans un fichier zip. Rien n’est envoyé.
# Entrée de la palette qui ouvre une section du manuel ; { $title } est une page ou « Page › Section ».
palette-help-topic = Aide : { $title }
palette-help-topic-help = Ouvrir cette section du manuel.
# Sous une commande indisponible dans la palette.
palette-disabled-help = Indisponible pour l’instant. Appuyez sur Entrée pour savoir pourquoi.

## Le panneau d’aide

help-title = Aide
help-search-placeholder = Rechercher dans le manuel
help-contents = Sommaire
help-back = Précédent
help-forward = Suivant
help-no-results = Aucun résultat pour « { $query } ».
help-results = { $count ->
    [one] { $count } section trouvée
    [many] { $count } sections trouvées
   *[other] { $count } sections trouvées
}
help-section-start = Prise en main
help-section-modes = Affichage et modes
help-section-tasks = Comment…
help-section-reference = Référence
help-keys-unbound = Non défini
# Une commande citée dans le manuel sans touche ; elle se lance depuis la palette.
help-key-unbound = { $name } (palette)
help-keys-command = Commande
help-keys-keys = Touches
help-commands = Toutes les commandes sont dans la palette de commandes ({ $key }). Tapez une partie du nom.
help-commands-no-key = Toutes les commandes sont dans la palette de commandes. Tapez une partie du nom.
help-link = Aller à { $title }
help-open = Aide
# Nom du panneau pour les lecteurs d’écran ; { $page } est la page affichée.
help-label = Aide : { $page }
help-disabled-why = Pourquoi ?

## Rapport de plantage proposé au démarrage suivant

crash-title = { -app-name } s’est fermé de façon inattendue
crash-body = Un rapport a été enregistré sur cet ordinateur. Rien n’a été envoyé.
crash-view = Voir le rapport
crash-hide = Masquer le rapport
crash-copy = Copier
crash-copied = Rapport copié
crash-save-bundle = Enregistrer un paquet de diagnostic…
crash-dismiss = Ignorer

## Paquet de diagnostic

diag-title = Enregistrer un paquet de diagnostic
diag-intro = Un fichier zip à joindre à un rapport de bogue. Il est enregistré sur cet ordinateur. Rien n’est envoyé.
diag-contents = Contenu
diag-include-names = Inclure les noms de fichiers
diag-include-names-help = Désactivé : les noms sont remplacés par file-1.jpg, dir-1, etc.
diag-privacy = Votre dossier personnel apparaît comme ~ et votre nom d’utilisateur comme <user>. Les mots de passe, les adresses de serveur et les images ne sont jamais inclus.
diag-where = Signaler un problème : { $issues } · { $email }
diag-save = Enregistrer…
diag-cancel = Annuler
diag-saved = Paquet de diagnostic enregistré : { $path }
diag-failed = Impossible d’enregistrer le paquet de diagnostic : { $reason }
diag-entry-summary = Version, système, GPU et pilote
diag-entry-crash = Dernier rapport de plantage
diag-entry-crashes = Rapports de plantage précédents ({ $count })
diag-entry-log = { $count } dernières lignes du journal
diag-entry-settings = Paramètres, sans mots de passe ni adresses
diag-entry-gpu = Détails du GPU et de l’affichage
diag-entry-readme = Ce qu’est ce paquet
diag-size = { $size }
# Le README.txt à l’intérieur du paquet.
diag-readme =
    Paquet de diagnostic de { -app-name }.

    Il contient ce qui aide à trouver un problème : version et détails du système, rapports de plantage, fin du journal et paramètres.

    Votre dossier personnel apparaît comme ~ et votre nom d’utilisateur comme <user>. Sauf si vous avez choisi de les inclure, les noms de fichiers et de dossiers sont remplacés par file-1, dir-1, etc. Les mots de passe et les adresses de serveur sont supprimés.

    Rien n’a été envoyé. Vous décidez de partager ou non ce fichier.

## Mises à jour (Paramètres)

settings-section-updates = Mises à jour
settings-updates-check = Rechercher les mises à jour
settings-updates-check-help = Une fois par jour, demander si une nouvelle version existe. La requête ne contient que le numéro de version : aucun identifiant, aucun cookie. Désactivé : { -app-name } ne se connecte jamais pour les mises à jour.
settings-updates-channel = Canal
settings-updates-channel-help = Stable : versions publiées uniquement. Bêta : aussi les versions de test.
settings-updates-channel-stable = Stable
settings-updates-channel-beta = Bêta
settings-updates-check-now = Vérifier maintenant
settings-updates-check-now-help = Vérifier une fois maintenant.
update-available = La version { $version } est disponible
update-notes = Notes de version
update-download = Télécharger
update-none = { -app-name } est à jour.
update-checking = Recherche de mises à jour…
update-failed = Impossible de rechercher les mises à jour : { $reason }
update-last-checked = Dernière vérification : { $when }

## À propos

about-build = Build { $commit } · { $profile } · { $target }
about-build-no-commit = Build { $profile } · { $target }
about-help = Aide
about-diagnostics = Enregistrer un paquet de diagnostic…
# machine-drafted, needs native review
about-renderer = Infos sur le rendu…
about-licences-count = { $count ->
    [one] { $count } composant tiers
    [many] { $count } composants tiers
   *[other] { $count } composants tiers
}
about-licence-group = { $licence } ({ $count })
about-contact = Problèmes : { $issues } · Assistance : { $email }
