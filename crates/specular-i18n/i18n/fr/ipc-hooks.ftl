# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Contrôle à distance et événements
settings-integration-ipc-help = D’autres programmes sur cet ordinateur, lancés sous votre compte, peuvent piloter { -app-name } et suivre ce qui s’y passe via un socket privé (specular ipc). Rien n’écoute sur le réseau.

## The hooks panel under "Remote control and events".
hooks-panel-title = Crochets
hooks-panel-intro = Programmes que settings.toml lance quand quelque chose se produit. Chacun demande une fois avant sa première exécution.
hooks-panel-none = Aucun crochet. Ajoutez-les en [[hooks]] dans settings.toml.
hooks-panel-keywords = crochets hooks événements automatisation scripts programmes
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Quand { $events }
hooks-panel-untrusted = Demande avant sa première exécution
hooks-panel-allowed = Autorisé
hooks-panel-remove = Retirer…
hooks-panel-unused = Inutilisé

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Lancer « { $name } » ({ $program }) quand { $events } ?
hook-consent-detail = Il lance ce programme avec les noms des photos : { $args }
hook-consent-trust = N’autorisez que des programmes de confiance. Il redemande si le crochet change.
hook-consent-allow = Autoriser
hook-consent-not-now = Pas maintenant
hook-or = { " " }ou{ " " }
hook-event-opened = une photo est ouverte
hook-event-marked = des photos sont marquées
hook-event-keywords = des mots-clés changent
hook-event-exported = des photos sont exportées
hook-event-imported = des photos sont importées
hook-event-captured = un appareil envoie une photo
hook-event-develop-saved = une retouche Développer est enregistrée
hook-event-any = quelque chose se produit

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Retirer le crochet « { $name } » ?
hook-remove-detail = Il est retiré de settings.toml. Le programme et vos fichiers restent sur le disque.
hook-remove-ok = Retirer
hook-removed = Crochet « { $name } » retiré.
hook-settings-unwritable = Impossible de modifier settings.toml : { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = Le crochet « { $name } » n’a pas fonctionné : { $reason }
hook-failed-exit = il s’est terminé avec le code { $code }
hook-failed-timeout = il a été arrêté après { $seconds } s
hook-failed-start = il n’a pas pu démarrer ({ $reason })
hook-dropped = Le crochet « { $name } » est occupé : { $count ->
    [one] { $count } événement a été ignoré.
   *[other] { $count } événements ont été ignorés.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] { $count } fichier qu’il remplacerait existe : rien n’a été exporté
   *[other] { $count } fichiers qu’il remplacerait existent : rien n’a été exporté
}
export-recipes-remote-skipped = { $file } existe
